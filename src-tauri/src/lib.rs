mod storage;
mod reminder;
mod appbar;
mod urlencoding;
mod updater;

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    Emitter,
    WebviewUrl,
    WebviewWindowBuilder,
};
use storage::{Storage, OAuthCredentials};
use reminder::{Reminder, Urgency, ListType};
use chrono::{Datelike, Timelike};

/// Monitor Windows display changes and power events to reposition the reminder bar
/// Listens for WM_DISPLAYCHANGE (resolution/monitor changes) and WM_POWERBROADCAST (resume from sleep)
#[cfg(windows)]
fn monitor_display_changes(app_handle: tauri::AppHandle) {
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPED,
        WM_DISPLAYCHANGE, WM_POWERBROADCAST,
    };

    // Track last reposition time to debounce rapid events
    static LAST_REPOSITION: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

    // Store app handle globally for the window proc
    static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();
    let _ = APP_HANDLE.set(app_handle);

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DISPLAYCHANGE | WM_POWERBROADCAST => {
                println!("Display/power event detected (msg={}), will reposition bar", msg);

                // Debounce: only reposition if 2+ seconds since last reposition
                let should_reposition = {
                    let mut last = LAST_REPOSITION.lock().unwrap();
                    let now = Instant::now();
                    if last.map_or(true, |t| now.duration_since(t) > Duration::from_secs(2)) {
                        *last = Some(now);
                        true
                    } else {
                        false
                    }
                };

                if should_reposition {
                    if let Some(app) = APP_HANDLE.get() {
                        let app = app.clone();
                        // Delay and retry multiple times to handle monitor wake settling
                        std::thread::spawn(move || {
                            // Retry at increasing intervals: 500ms, 1.5s, 3s
                            let delays = [500, 1500, 3000];
                            for (i, delay_ms) in delays.iter().enumerate() {
                                std::thread::sleep(Duration::from_millis(*delay_ms));
                                println!("Repositioning bar attempt {} after {}ms", i + 1, delay_ms);
                                let app_clone = app.clone();
                                tauri::async_runtime::block_on(async {
                                    let _ = reposition_reminder_bar(app_clone).await;
                                });
                            }
                        });
                    }
                }

                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    // Create a hidden message-only window to receive system messages
    unsafe {
        let class_name: Vec<u16> = "ReminderAppDisplayMonitor\0".encode_utf16().collect();

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: windows::Win32::Foundation::HINSTANCE::default(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        if RegisterClassW(&wc) == 0 {
            println!("Failed to register display monitor window class");
            return;
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR::null(),
            WS_OVERLAPPED,
            0, 0, 0, 0,
            HWND::default(),
            None,
            None,
            None,
        );

        if hwnd.is_err() || hwnd.as_ref().unwrap().is_invalid() {
            println!("Failed to create display monitor window");
            return;
        }

        println!("Display change monitoring started");

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

// Counter for notification window positioning
static NOTIFICATION_COUNT: AtomicU32 = AtomicU32::new(0);

// Organization prompt state - tracks which prompt slots have been triggered today
// Format: (day_of_year, triggered_8am, triggered_1pm, triggered_6pm)
static ORGANIZE_PROMPT_STATE: Mutex<Option<(u32, bool, bool, bool)>> = Mutex::new(None);

/// Organization prompt scheduler - checks every minute and triggers at 8 AM, 1 PM, 6 PM
fn start_organize_prompt_scheduler(app_handle: tauri::AppHandle) {
    use chrono::{Local, Timelike};
    use std::thread;
    use std::time::Duration;

    thread::spawn(move || {
        loop {
            // Sleep for 1 minute between checks
            thread::sleep(Duration::from_secs(60));

            let now = Local::now();
            let hour = now.hour();
            let minute = now.minute();
            let day = now.ordinal();

            // Only trigger during the first 5 minutes of the trigger hours
            if minute >= 5 {
                continue;
            }

            // Check if this is a trigger time (8, 13, 18)
            let is_trigger_time = hour == 8 || hour == 13 || hour == 18;
            if !is_trigger_time {
                continue;
            }

            // Check if already triggered for this slot today
            let should_trigger = {
                let mut state = ORGANIZE_PROMPT_STATE.lock().unwrap();
                let current = state.get_or_insert((day, false, false, false));

                // Reset if new day
                if current.0 != day {
                    *current = (day, false, false, false);
                }

                // Determine which slot and if already triggered
                let (slot_triggered, slot_index) = match hour {
                    8 => (current.1, 1),
                    13 => (current.2, 2),
                    18 => (current.3, 3),
                    _ => continue,
                };

                if !slot_triggered {
                    // Mark as triggered
                    match slot_index {
                        1 => current.1 = true,
                        2 => current.2 = true,
                        3 => current.3 = true,
                        _ => {}
                    }
                    true
                } else {
                    false
                }
            };

            if should_trigger {
                println!("Organization prompt triggered at {:02}:{:02}", hour, minute);
                // Emit event to frontend
                if let Err(e) = app_handle.emit("organization-prompt", ()) {
                    println!("Failed to emit organization-prompt: {:?}", e);
                }
            }
        }
    });
}

pub struct AppState {
    pub storage: Mutex<Storage>,
}

impl AppState {
    /// Lock storage, recovering from poison if needed
    fn lock_storage(&self) -> std::sync::MutexGuard<'_, Storage> {
        self.storage.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[tauri::command]
fn get_pending_reminders(state: tauri::State<AppState>) -> Result<Vec<Reminder>, String> {
    let storage = state.lock_storage();
    Ok(storage.get_pending_reminders())
}

#[tauri::command]
fn get_actual_reminders(state: tauri::State<AppState>) -> Result<Vec<Reminder>, String> {
    let storage = state.lock_storage();
    Ok(storage.get_actual_reminders())
}

#[tauri::command]
fn get_backlog_reminders(state: tauri::State<AppState>) -> Result<Vec<Reminder>, String> {
    let storage = state.lock_storage();
    Ok(storage.get_backlog_reminders())
}

#[tauri::command]
fn get_completed_reminders(state: tauri::State<AppState>) -> Result<Vec<Reminder>, String> {
    let storage = state.lock_storage();
    Ok(storage.get_completed_reminders())
}

#[tauri::command]
fn get_completion_stats(state: tauri::State<AppState>) -> Result<(usize, usize), String> {
    let storage = state.lock_storage();
    Ok(storage.get_completion_stats())
}

#[tauri::command]
fn get_historical_stats(
    state: tauri::State<AppState>,
) -> Result<(Vec<(String, usize)>, Vec<usize>, Vec<usize>, usize), String> {
    let storage = state.lock_storage();
    Ok(storage.get_historical_stats())
}

#[tauri::command]
fn dismiss_organize_prompt() -> Result<(), String> {
    use chrono::{Local, Timelike};

    let now = Local::now();
    let day = now.ordinal();
    let hour = now.hour();

    let mut state = ORGANIZE_PROMPT_STATE.lock().unwrap();
    let current = state.get_or_insert((day, false, false, false));

    // Reset if new day
    if current.0 != day {
        *current = (day, false, false, false);
    }

    // Mark the current slot as dismissed based on hour
    if hour < 13 {
        current.1 = true; // 8 AM slot
    } else if hour < 18 {
        current.2 = true; // 1 PM slot
    } else {
        current.3 = true; // 6 PM slot
    }

    Ok(())
}

#[tauri::command]
fn add_reminder(
    state: tauri::State<AppState>,
    message: String,
    urgency: String,
    list_type: String,
) -> Result<i64, String> {
    let mut storage = state.lock_storage();
    let urgency_enum = match urgency.as_str() {
        "now" => Urgency::Now,
        "today" => Urgency::Today,
        "soon" => Urgency::Soon,
        _ => Urgency::Whenever,
    };
    let list_type_enum = match list_type.as_str() {
        "backlog" => ListType::Backlog,
        _ => ListType::Actual,
    };
    let reminder = Reminder::new(message, urgency_enum, list_type_enum);
    storage.add_reminder(reminder)
}

#[tauri::command]
fn update_reminder(
    state: tauri::State<AppState>,
    id: i64,
    message: String,
    urgency: String,
) -> Result<(), String> {
    let mut storage = state.lock_storage();
    let urgency_enum = match urgency.as_str() {
        "now" => Urgency::Now,
        "today" => Urgency::Today,
        "soon" => Urgency::Soon,
        _ => Urgency::Whenever,
    };
    storage.update_reminder(id, message, urgency_enum)
}

#[tauri::command]
fn move_reminder(
    state: tauri::State<AppState>,
    id: i64,
    to_list: String,
) -> Result<(), String> {
    let mut storage = state.lock_storage();
    let list_type = match to_list.as_str() {
        "backlog" => ListType::Backlog,
        _ => ListType::Actual,
    };
    storage.move_reminder(id, list_type)
}

#[tauri::command]
fn set_urgency(
    state: tauri::State<AppState>,
    id: i64,
    urgency: String,
) -> Result<(), String> {
    let mut storage = state.lock_storage();
    let urgency_enum = match urgency.as_str() {
        "now" => Urgency::Now,
        "today" => Urgency::Today,
        "soon" => Urgency::Soon,
        _ => Urgency::Whenever,
    };
    storage.set_urgency(id, urgency_enum)
}

#[tauri::command]
fn delete_reminder(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.delete_reminder(id)
}

#[tauri::command]
fn complete_reminder(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.complete_reminder(id)
}

#[tauri::command]
fn uncomplete_reminder(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.uncomplete_reminder(id)
}

#[tauri::command]
fn reorder_reminders(state: tauri::State<AppState>, ordered_ids: Vec<i64>) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.reorder_reminders(ordered_ids)
}

#[tauri::command]
async fn sync_to_cloud_background(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.sync_to_cloud()
}

#[tauri::command]
fn refresh_from_cloud(state: tauri::State<AppState>) -> Result<bool, String> {
    let mut storage = state.lock_storage();
    storage.refresh_from_cloud()
}

#[tauri::command]
fn sync_on_startup(state: tauri::State<AppState>) -> Result<bool, String> {
    let mut storage = state.lock_storage();
    // Try to sync from cloud if connected
    storage.refresh_from_cloud()
}

#[tauri::command]
fn get_oauth_status(state: tauri::State<AppState>) -> Result<(bool, bool), String> {
    let storage = state.lock_storage();
    Ok(storage.get_oauth_status())
}

/// Check if auth is working by attempting a cloud operation
/// Returns: (has_credentials, is_logged_in, auth_valid)
#[tauri::command]
fn check_auth_status(state: tauri::State<AppState>) -> Result<(bool, bool, bool), String> {
    let mut storage = state.lock_storage();
    let (has_creds, is_logged_in) = storage.get_oauth_status();

    if !has_creds || !is_logged_in {
        return Ok((has_creds, is_logged_in, false));
    }

    // Try to refresh from cloud to verify auth is working
    match storage.refresh_from_cloud() {
        Ok(_) => Ok((has_creds, is_logged_in, true)),
        Err(e) => {
            eprintln!("Auth validation failed: {}", e);
            // Auth failed - likely expired
            Ok((has_creds, is_logged_in, false))
        }
    }
}

#[tauri::command]
fn save_oauth_credentials(
    state: tauri::State<AppState>,
    client_id: String,
    client_secret: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    let storage = state.lock_storage();
    let credentials = OAuthCredentials {
        client_id,
        client_secret,
        folder_id: folder_id.unwrap_or_else(|| "1F0qYeAVU_7H73kX9uz-1ZF3i2KS_V-mk".to_string()),
    };
    storage.save_oauth_credentials(&credentials)
}

#[tauri::command]
fn get_oauth_url(state: tauri::State<AppState>) -> Result<String, String> {
    let storage = state.lock_storage();
    storage.get_oauth_url()
}

#[tauri::command]
fn get_oauth_credentials(state: tauri::State<AppState>) -> Result<(String, String), String> {
    let storage = state.lock_storage();
    match storage.get_oauth_credentials() {
        Some(creds) => Ok((creds.client_id, creds.client_secret)),
        None => Err("No credentials found".to_string()),
    }
}

#[tauri::command]
async fn start_oauth_flow(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Get the OAuth URL and app data path for the background thread
    let (url, app_data_path) = {
        let storage = state.lock_storage();
        let url = storage.get_oauth_url()?;
        let path = storage.get_app_data_path().to_path_buf();
        (url, path)
    };

    // Open browser
    open::that(&url).map_err(|e| format!("Failed to open browser: {}", e))?;

    // Run the blocking OAuth flow in a separate thread to avoid blocking the main thread
    let result = tauri::async_runtime::spawn_blocking(move || {
        storage::complete_oauth_flow_blocking(&app_data_path)
    })
    .await
    .map_err(|e| format!("OAuth task failed: {}", e))?;

    // If successful, reload the storage state
    if result.is_ok() {
        let mut storage = state.lock_storage();
        storage.reload_oauth_state()?;
        eprintln!("OAuth flow completed successfully");
    } else {
        eprintln!("OAuth flow failed: {:?}", result);
    }

    result
}

#[tauri::command]
fn disconnect_drive(state: tauri::State<AppState>) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.disconnect_drive()
}

#[tauri::command]
async fn show_notification_window(
    app: tauri::AppHandle,
    reminder_id: i64,
    message: String,
    due_time: String,
) -> Result<(), String> {
    // Create unique window label
    let label = format!("notification_{}", reminder_id);

    // Check if window already exists first (before incrementing counter)
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }

    // Get screen dimensions
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let primary = monitors.into_iter().next().ok_or("No monitor found")?;
    let screen_size = primary.size();
    let scale_factor = primary.scale_factor();

    // Notification dimensions
    let popup_width = 360u32;
    let popup_height = 80u32;
    let gap = 12u32;
    let taskbar_height = 48u32;

    // Calculate position (stack from right) - increment counter only after existence check
    let count = NOTIFICATION_COUNT.fetch_add(1, Ordering::SeqCst);
    let x = (screen_size.width as f64 / scale_factor) as u32 - popup_width - gap - (count * (popup_width + gap));
    let y = (screen_size.height as f64 / scale_factor) as u32 - popup_height - taskbar_height - gap;

    // Build the URL with query parameters
    let url = format!(
        "/notification.html?id={}&message={}&due_time={}",
        reminder_id,
        urlencoding::encode(&message),
        urlencoding::encode(&due_time)
    );

    // Create the notification window
    let window_result = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(url.into()),
    )
    .title("")
    .inner_size(popup_width as f64, popup_height as f64)
    .position(x as f64, y as f64)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .focused(true)
    .build();

    // Roll back counter if window creation failed
    if window_result.is_err() {
        NOTIFICATION_COUNT.fetch_sub(1, Ordering::SeqCst);
    }

    window_result.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn close_notification_window(app: tauri::AppHandle, reminder_id: i64) -> Result<(), String> {
    let label = format!("notification_{}", reminder_id);
    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|e| e.to_string())?;
        NOTIFICATION_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
async fn show_quick_add(app: tauri::AppHandle) -> Result<(), String> {
    let label = "quick-add";

    // If window exists, just show and focus it
    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Get primary monitor for centering
    let primary = app.primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No primary monitor found")?;

    let screen_size = primary.size();
    let screen_position = primary.position();
    let scale_factor = primary.scale_factor();

    // Window dimensions (40% bigger than original 400x56, plus room for selectors and hint text)
    let width = 560u32;
    let height = 150u32;

    // Calculate logical screen dimensions
    let screen_width = (screen_size.width as f64 / scale_factor) as i32;
    let screen_height = (screen_size.height as f64 / scale_factor) as i32;

    // Center on the primary monitor (accounting for monitor position in multi-monitor setups)
    let x = screen_position.x + (screen_width - width as i32) / 2;
    let y = screen_position.y + (screen_height - height as i32) / 2;

    // Create the quick-add window
    let window = WebviewWindowBuilder::new(
        &app,
        label,
        WebviewUrl::App("/quick-add.html".into()),
    )
    .title("")
    .inner_size(width as f64, height as f64)
    .position(x as f64, y as f64)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .focused(true)
    .build()
    .map_err(|e| e.to_string())?;

    // Explicitly set focus after creation (needed on Windows)
    window.set_focus().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn unregister_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    app.global_shortcut().unregister_all().map_err(|e| e.to_string())
}

#[tauri::command]
async fn register_shortcuts(app: tauri::AppHandle, quick_add: String, show_list: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    // Unregister existing shortcuts first
    let _ = app.global_shortcut().unregister_all();

    // Parse and register quick add shortcut
    let quick_add_shortcut: Shortcut = quick_add.parse()
        .map_err(|e| format!("Invalid quick add shortcut: {:?}", e))?;

    let show_list_shortcut: Shortcut = show_list.parse()
        .map_err(|e| format!("Invalid show list shortcut: {:?}", e))?;

    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(quick_add_shortcut, move |_app, shortcut, event| {
        if event.state == ShortcutState::Pressed {
            println!("Quick add shortcut triggered: {:?}", shortcut);
            // Show quick-add popup window
            let app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let _ = show_quick_add(app).await;
            });
        }
    }).map_err(|e| format!("Failed to register quick add: {:?}", e))?;

    let app_handle2 = app.clone();
    app.global_shortcut().on_shortcut(show_list_shortcut, move |_app, shortcut, event| {
        if event.state == ShortcutState::Pressed {
            println!("Show list shortcut triggered: {:?}", shortcut);
            if let Some(window) = app_handle2.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }).map_err(|e| format!("Failed to register show list: {:?}", e))?;

    println!("Shortcuts registered: {} (quick add), {} (show list)", quick_add, show_list);
    Ok(())
}

// Guard to prevent concurrent bar creation
static BAR_CREATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
async fn show_reminder_bar(app: tauri::AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    let label = "reminder-bar";

    // If bar already exists, just show it
    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Atomic guard to prevent concurrent creation attempts
    if BAR_CREATING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        println!("show_reminder_bar: already creating, skipping duplicate call");
        return Ok(());
    }

    // Ensure we reset the flag when done (success or failure)
    struct ResetGuard;
    impl Drop for ResetGuard {
        fn drop(&mut self) {
            BAR_CREATING.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _reset_guard = ResetGuard;

    // Get work area from Windows API (this gives us the area excluding taskbar)
    let (work_x, work_y, work_width, work_height) = appbar::get_work_area()
        .unwrap_or((0, 0, 1920, 1080));

    println!("Work area: x={}, y={}, w={}, h={}", work_x, work_y, work_width, work_height);

    // Bar dimensions - needs to fit: card (44px) + wrapper padding (4px) + vertical padding (12px) = 60px
    let bar_height = 60;

    // Use 98% of work area width, centered
    let bar_width = (work_width as f64 * 0.98) as i32;
    let x = work_x + (work_width - bar_width) / 2;
    let y = work_y + work_height - bar_height;

    println!("Bar position: ({}, {}), size: {}x{}", x, y, bar_width, bar_height);

    // Create the reminder bar window - initially at calculated position
    let window = WebviewWindowBuilder::new(
        &app,
        label,
        WebviewUrl::App("/reminder-bar.html".into()),
    )
    .title("Reminders")
    .inner_size(bar_width as f64, bar_height as f64)
    .position(x as f64, y as f64)
    .resizable(false)
    .decorations(false)
    .always_on_top(false)  // Will be managed by appbar
    .skip_taskbar(true)
    .transparent(true)
    .focused(false)
    .maximizable(false)
    .minimizable(false)
    .build()
    .map_err(|e| e.to_string())?;

    // Register as an AppBar on Windows - this reserves screen space so other windows don't overlap
    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            let hwnd_val = hwnd.0 as isize;

            // Register appbar with full work area width to reserve the space
            match appbar::register_appbar(hwnd_val, bar_height) {
                Ok((appbar_x, appbar_y, appbar_w, appbar_h)) => {
                    println!("AppBar registered at: ({}, {}), size: {}x{}", appbar_x, appbar_y, appbar_w, appbar_h);

                    // Use Windows API directly to set exact window position/size
                    // This bypasses Tauri's size handling which may add padding
                    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER, SWP_NOACTIVATE, HWND_TOP};
                    use windows::Win32::Foundation::HWND;

                    let hwnd_win = HWND(hwnd_val as *mut _);
                    let result = unsafe {
                        SetWindowPos(
                            hwnd_win,
                            HWND_TOP,
                            appbar_x,
                            appbar_y,
                            appbar_w,
                            appbar_h,
                            SWP_NOZORDER | SWP_NOACTIVATE
                        )
                    };
                    println!("SetWindowPos result: {:?}", result);

                    // Verify actual position after setting
                    if let Ok(actual_pos) = window.outer_position() {
                        println!("Actual window position after set: {:?}", actual_pos);
                    }
                    if let Ok(actual_size) = window.outer_size() {
                        println!("Actual window size after set: {:?}", actual_size);
                    }
                }
                Err(e) => {
                    println!("Failed to register appbar: {}, falling back to always-on-top", e);
                    let _ = window.set_always_on_top(true);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
async fn hide_reminder_bar(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("reminder-bar") {
        // Unregister appbar before hiding on Windows
        #[cfg(windows)]
        {
            if let Ok(hwnd) = window.hwnd() {
                appbar::unregister_appbar(hwnd.0 as isize);
            }
        }
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn reset_bar_position(app: tauri::AppHandle) -> Result<(), String> {
    // If bar exists, close it and recreate it to reset position
    let had_bar = if let Some(window) = app.get_webview_window("reminder-bar") {
        #[cfg(windows)]
        {
            if let Ok(hwnd) = window.hwnd() {
                appbar::unregister_appbar(hwnd.0 as isize);
            }
        }
        window.close().map_err(|e| e.to_string())?;
        true
    } else {
        false
    };

    if had_bar {
        // Wait for window to be fully destroyed (poll until it's gone)
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if app.get_webview_window("reminder-bar").is_none() {
                break;
            }
        }

        // Recreate the bar
        show_reminder_bar(app).await?;
    }
    Ok(())
}

#[tauri::command]
async fn reposition_reminder_bar(app: tauri::AppHandle) -> Result<(), String> {
    let window = match app.get_webview_window("reminder-bar") {
        Some(w) => w,
        None => return Ok(()), // Bar not visible, nothing to do
    };

    // Only reposition on Windows where we use AppBar
    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            let hwnd_val = hwnd.0 as isize;
            let bar_height = 60;

            // Unregister existing appbar
            appbar::unregister_appbar(hwnd_val);

            // Re-register with current monitor dimensions
            match appbar::register_appbar(hwnd_val, bar_height) {
                Ok((appbar_x, appbar_y, appbar_w, appbar_h)) => {
                    // Sanity check: skip if values look wrong (negative Y or tiny width)
                    // This can happen when monitor is still waking up
                    if appbar_y < 0 || appbar_w < 800 || appbar_h <= 0 {
                        println!("AppBar got invalid values: ({}, {}), size: {}x{} - skipping",
                            appbar_x, appbar_y, appbar_w, appbar_h);
                        return Ok(());
                    }

                    println!("AppBar repositioned to: ({}, {}), size: {}x{}", appbar_x, appbar_y, appbar_w, appbar_h);

                    // Use Windows API directly to set exact window position/size
                    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER, SWP_NOACTIVATE, HWND_TOP};
                    use windows::Win32::Foundation::HWND;

                    let hwnd_win = HWND(hwnd_val as *mut _);
                    let _ = unsafe {
                        SetWindowPos(
                            hwnd_win,
                            HWND_TOP,
                            appbar_x,
                            appbar_y,
                            appbar_w,
                            appbar_h,
                            SWP_NOZORDER | SWP_NOACTIVATE
                        )
                    };
                }
                Err(e) => {
                    println!("Failed to reposition appbar: {}, falling back to always-on-top", e);
                    let _ = window.set_always_on_top(true);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
async fn check_for_update() -> Result<Option<updater::UpdateInfo>, String> {
    // Run the blocking network request in a separate thread
    tauri::async_runtime::spawn_blocking(|| {
        updater::check_for_update()
    })
    .await
    .map_err(|e| format!("Update check task failed: {}", e))?
}

#[tauri::command]
async fn install_update(download_url: String) -> Result<(), String> {
    // Run the blocking download/install in a separate thread to avoid freezing UI
    tauri::async_runtime::spawn_blocking(move || {
        updater::install_update(&download_url)
    })
    .await
    .map_err(|e| format!("Update task failed: {}", e))?
}

#[tauri::command]
fn get_debug_log_path() -> Option<String> {
    appbar::get_log_path().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn open_debug_log() -> Result<(), String> {
    if let Some(path) = appbar::get_log_path() {
        open::that(&path).map_err(|e| format!("Failed to open log file: {}", e))?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set Per-Monitor DPI awareness before any windows are created
    // This ensures coordinates are consistent on high-DPI displays like Surface
    #[cfg(windows)]
    {
        use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    let storage = Storage::new().expect("Failed to initialize storage");

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--startup"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            storage: Mutex::new(storage),
        })
        .setup(|app| {
            // Create tray menu
            let show_i = MenuItem::with_id(app, "show", "Show Reminders (Ctrl+Alt+L)", true, None::<&str>)?;
            let quick_i = MenuItem::with_id(app, "quick", "Quick Add (Ctrl+Alt+R)", true, None::<&str>)?;
            let reset_bar_i = MenuItem::with_id(app, "reset_bar", "Reset Bar Position (Ctrl+Alt+B)", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quick_i, &reset_bar_i, &quit_i])?;

            // Build tray icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Reminder App")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quick" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = show_quick_add(app).await;
                            });
                        }
                        "reset_bar" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = reset_bar_position(app).await;
                            });
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Register global shortcuts
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

            // Ctrl+Alt+R - Quick Add (show window and focus input)
            let quick_add_shortcut: Shortcut = "Ctrl+Alt+R".parse().unwrap();

            // Ctrl+Alt+L - Show List (show window without focusing input)
            let show_list_shortcut: Shortcut = "Ctrl+Alt+L".parse().unwrap();

            // Ctrl+Alt+B - Reset Bar Position
            let reset_bar_shortcut: Shortcut = "Ctrl+Alt+B".parse().unwrap();

            let app_handle = app.handle().clone();
            match app.global_shortcut().on_shortcut(quick_add_shortcut, move |_app, shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    println!("Quick add shortcut triggered: {:?}", shortcut);
                    // Show quick-add popup window
                    let app = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = show_quick_add(app).await;
                    });
                }
            }) {
                Ok(_) => println!("Ctrl+Alt+R registered successfully"),
                Err(e) => println!("Failed to register Ctrl+Alt+R: {:?}", e),
            }

            let app_handle2 = app.handle().clone();
            match app.global_shortcut().on_shortcut(show_list_shortcut, move |_app, shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    println!("Show list shortcut triggered: {:?}", shortcut);
                    if let Some(window) = app_handle2.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }) {
                Ok(_) => println!("Ctrl+Alt+L registered successfully"),
                Err(e) => println!("Failed to register Ctrl+Alt+L: {:?}", e),
            }

            let app_handle3 = app.handle().clone();
            match app.global_shortcut().on_shortcut(reset_bar_shortcut, move |_app, shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    println!("Reset bar shortcut triggered: {:?}", shortcut);
                    let app = app_handle3.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = reset_bar_position(app).await;
                    });
                }
            }) {
                Ok(_) => println!("Ctrl+Alt+B registered successfully"),
                Err(e) => println!("Failed to register Ctrl+Alt+B: {:?}", e),
            }

            println!("Global shortcuts: Ctrl+Alt+R (quick add), Ctrl+Alt+L (show list), Ctrl+Alt+B (reset bar)");

            // Handle launch arguments
            let args: Vec<String> = std::env::args().collect();
            let has_show = args.contains(&"--show".to_string());
            let has_quick = args.contains(&"--quick-add".to_string());
            let has_startup = args.contains(&"--startup".to_string());

            if let Some(window) = app.get_webview_window("main") {
                if has_quick {
                    // Show window and focus input for quick add
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("focus-input", ());
                } else if has_show || !has_startup {
                    // Show window normally (unless --startup flag)
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                // If --startup, window stays hidden (minimized to tray)
            }

            // Set up display change monitoring (Windows)
            #[cfg(windows)]
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    monitor_display_changes(app_handle);
                });
            }

            // Start organization prompt scheduler
            {
                let app_handle = app.handle().clone();
                start_organize_prompt_scheduler(app_handle);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_pending_reminders,
            get_actual_reminders,
            get_backlog_reminders,
            get_completed_reminders,
            get_completion_stats,
            get_historical_stats,
            dismiss_organize_prompt,
            add_reminder,
            update_reminder,
            move_reminder,
            set_urgency,
            delete_reminder,
            complete_reminder,
            uncomplete_reminder,
            reorder_reminders,
            sync_to_cloud_background,
            refresh_from_cloud,
            sync_on_startup,
            show_notification_window,
            close_notification_window,
            show_reminder_bar,
            hide_reminder_bar,
            reposition_reminder_bar,
            reset_bar_position,
            show_quick_add,
            unregister_shortcuts,
            register_shortcuts,
            get_oauth_status,
            check_auth_status,
            save_oauth_credentials,
            get_oauth_credentials,
            get_oauth_url,
            start_oauth_flow,
            disconnect_drive,
            check_for_update,
            install_update,
            get_debug_log_path,
            open_debug_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
