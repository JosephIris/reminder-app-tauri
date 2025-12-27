mod storage;
mod reminder;
mod appbar;

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
use reminder::Reminder;

// Counter for notification window positioning
static NOTIFICATION_COUNT: AtomicU32 = AtomicU32::new(0);

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
                ' ' => result.push_str("%20"),
                '\'' => result.push_str("%27"),
                _ => {
                    for b in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
        result
    }
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
fn get_completed_reminders(state: tauri::State<AppState>) -> Result<Vec<Reminder>, String> {
    let storage = state.lock_storage();
    Ok(storage.get_completed_reminders())
}

#[tauri::command]
fn add_reminder(
    state: tauri::State<AppState>,
    message: String,
    due_time: String,
    recurrence: String,
) -> Result<i64, String> {
    let mut storage = state.lock_storage();
    let reminder = Reminder::new(message, due_time, recurrence);
    storage.add_reminder(reminder)
}

#[tauri::command]
fn update_reminder(
    state: tauri::State<AppState>,
    id: i64,
    message: String,
    due_time: String,
    recurrence: String,
) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.update_reminder(id, message, due_time, recurrence)
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
fn snooze_reminder(state: tauri::State<AppState>, id: i64, minutes: i64) -> Result<(), String> {
    let mut storage = state.lock_storage();
    storage.snooze_reminder(id, minutes)
}

#[tauri::command]
fn refresh_from_cloud(state: tauri::State<AppState>) -> Result<bool, String> {
    let mut storage = state.lock_storage();
    storage.refresh_from_cloud()
}

#[tauri::command]
fn get_oauth_status(state: tauri::State<AppState>) -> Result<(bool, bool), String> {
    let storage = state.lock_storage();
    Ok(storage.get_oauth_status())
}

#[tauri::command]
fn save_oauth_credentials(
    state: tauri::State<AppState>,
    client_id: String,
    client_secret: String,
) -> Result<(), String> {
    let storage = state.lock_storage();
    let credentials = OAuthCredentials {
        client_id,
        client_secret,
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
fn start_oauth_flow(
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

    // Do the entire OAuth flow (this blocks until user completes OAuth in browser)
    // Using ureq instead of reqwest means no tokio runtime issues
    let result = storage::complete_oauth_flow_blocking(&app_data_path);

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

    // Calculate position (stack from right)
    let count = NOTIFICATION_COUNT.fetch_add(1, Ordering::SeqCst);
    let x = (screen_size.width as f64 / scale_factor) as u32 - popup_width - gap - (count * (popup_width + gap));
    let y = (screen_size.height as f64 / scale_factor) as u32 - popup_height - taskbar_height - gap;

    // Create unique window label
    let label = format!("notification_{}", reminder_id);

    // Check if window already exists
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }

    // Build the URL with query parameters
    let url = format!(
        "/notification.html?id={}&message={}&due_time={}",
        reminder_id,
        urlencoding::encode(&message),
        urlencoding::encode(&due_time)
    );

    // Create the notification window
    let _window = WebviewWindowBuilder::new(
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
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
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

    // Window dimensions (40% bigger than original 400x56, plus room for hint text)
    let width = 560u32;
    let height = 100u32;

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

#[tauri::command]
async fn show_reminder_bar(app: tauri::AppHandle) -> Result<(), String> {
    let label = "reminder-bar";

    // If bar already exists, just show it
    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Get work area from Windows API (this gives us the area excluding taskbar)
    let (work_x, work_y, work_width, work_height) = appbar::get_work_area()
        .unwrap_or((0, 0, 1920, 1080));

    println!("Work area: x={}, y={}, w={}, h={}", work_x, work_y, work_width, work_height);

    // Bar dimensions - increased to accommodate focused task with glow effects
    let bar_height = 68;

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
                    // Position the window to fill the appbar reserved space
                    // appbar returns logical pixels, so use Logical positioning
                    let _ = window.set_position(tauri::Position::Logical(
                        tauri::LogicalPosition::new(appbar_x as f64, appbar_y as f64)
                    ));
                    let _ = window.set_size(tauri::Size::Logical(
                        tauri::LogicalSize::new(appbar_w as f64, appbar_h as f64)
                    ));
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let storage = Storage::new().expect("Failed to initialize storage");

    tauri::Builder::default()
        // .plugin(tauri_plugin_updater::Builder::new().build())  // Disabled temporarily
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
            let show_i = MenuItem::with_id(app, "show", "Show Reminders (Ctrl+Shift+L)", true, None::<&str>)?;
            let quick_i = MenuItem::with_id(app, "quick", "Quick Add (Ctrl+Shift+R)", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quick_i, &quit_i])?;

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

            println!("Global shortcuts: Ctrl+Alt+R (quick add), Ctrl+Alt+L (show list)");

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_pending_reminders,
            get_completed_reminders,
            add_reminder,
            update_reminder,
            delete_reminder,
            complete_reminder,
            uncomplete_reminder,
            snooze_reminder,
            refresh_from_cloud,
            show_notification_window,
            close_notification_window,
            show_reminder_bar,
            hide_reminder_bar,
            show_quick_add,
            unregister_shortcuts,
            register_shortcuts,
            get_oauth_status,
            save_oauth_credentials,
            get_oauth_credentials,
            get_oauth_url,
            start_oauth_flow,
            disconnect_drive,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
