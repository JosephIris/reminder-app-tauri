//! Windows AppBar support for docking the reminder bar above the taskbar.
//! This makes the bar reserve screen space so other windows avoid it.

#[cfg(windows)]
use windows::{
    Win32::Foundation::{HWND, LPARAM, RECT},
    Win32::UI::Shell::{
        SHAppBarMessage, ABM_NEW, ABM_REMOVE, ABM_QUERYPOS, ABM_SETPOS,
        ABE_BOTTOM, APPBARDATA,
    },
    Win32::UI::WindowsAndMessaging::WM_USER,
};

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::Mutex;

#[cfg(windows)]
static APPBAR_REGISTERED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static APPBAR_MUTEX: Mutex<()> = Mutex::new(());

#[cfg(windows)]
const APPBAR_CALLBACK: u32 = WM_USER + 1;

/// Write debug info to a log file in the user's temp directory
/// Log file is at: %TEMP%\reminder-app-debug.log
#[cfg(windows)]
fn log_debug(msg: &str) {
    use std::io::Write;
    if let Some(temp_dir) = std::env::var_os("TEMP") {
        let log_path = std::path::Path::new(&temp_dir).join("reminder-app-debug.log");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, msg);
        }
    }
}

/// Get the debug log file path
#[cfg(windows)]
pub fn get_log_path() -> Option<std::path::PathBuf> {
    std::env::var_os("TEMP").map(|temp_dir| {
        std::path::Path::new(&temp_dir).join("reminder-app-debug.log")
    })
}

#[cfg(not(windows))]
pub fn get_log_path() -> Option<std::path::PathBuf> {
    None
}

/// Register a window as an appbar docked at the bottom of the screen.
/// bar_height is in logical pixels (will be converted to physical for Windows API).
/// Returns the adjusted work area rect in logical pixels for Tauri.
#[cfg(windows)]
pub fn register_appbar(hwnd: isize, bar_height: i32) -> Result<(i32, i32, i32, i32), String> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Shell::ABM_GETTASKBARPOS;

    const DEFAULT_DPI: u32 = 96;  // Standard Windows DPI (100% scaling)

    // Acquire mutex to prevent concurrent registration attempts
    let _guard = APPBAR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // Use compare_exchange to atomically check and set the flag
    // This ensures only ONE thread can proceed even if they arrive at the same instant
    let was_registered = APPBAR_REGISTERED.compare_exchange(
        false,  // expected: not registered
        true,   // set to: registered
        Ordering::SeqCst,
        Ordering::SeqCst
    );

    if was_registered.is_err() {
        log_debug("=== register_appbar called but already registered (atomic check), skipping ===");
        return Err("AppBar already registered".to_string());
    }

    log_debug("=== register_appbar called (won atomic race) ===");

    let hwnd = HWND(hwnd as *mut _);

    // Get DPI scale for this specific window (more accurate than system DPI)
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = dpi as f64 / DEFAULT_DPI as f64;

    // Convert logical bar height to physical pixels for Windows API
    let physical_bar_height = (bar_height as f64 * scale).round() as i32;

    log_debug(&format!("DPI: {}, scale: {:.3}, logical bar height: {}, physical: {}",
             dpi, scale, bar_height, physical_bar_height));

    // Get monitor info for the monitor containing the taskbar (primary monitor)
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) };
    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    let success = unsafe { GetMonitorInfoW(monitor, &mut monitor_info) };
    if !success.as_bool() {
        log_debug("ERROR: Failed to get monitor info");
        return Err("Failed to get monitor info".to_string());
    }

    // Use rcWork (work area) instead of rcMonitor (full monitor area)
    // rcWork excludes the taskbar, so we can position our bar correctly
    let work_area = monitor_info.rcWork;
    let monitor_area = monitor_info.rcMonitor;

    log_debug(&format!("Monitor area: left={}, top={}, right={}, bottom={}",
             monitor_area.left, monitor_area.top, monitor_area.right, monitor_area.bottom));
    log_debug(&format!("Work area: left={}, top={}, right={}, bottom={}",
             work_area.left, work_area.top, work_area.right, work_area.bottom));

    // Query the taskbar position directly for vertical positioning
    let mut taskbar_abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    unsafe { SHAppBarMessage(ABM_GETTASKBARPOS, &mut taskbar_abd) };

    log_debug(&format!("Taskbar rect: left={}, top={}, right={}, bottom={}, edge={:?}",
             taskbar_abd.rc.left, taskbar_abd.rc.top, taskbar_abd.rc.right, taskbar_abd.rc.bottom, taskbar_abd.uEdge));

    // Calculate where our bar should go - directly above the taskbar
    // The taskbar's top edge is where our bar's bottom should be
    let bar_bottom = taskbar_abd.rc.top;
    let bar_top = bar_bottom - physical_bar_height;

    // IMPORTANT: Use work_area.left/right for horizontal bounds, NOT monitor_area
    // On multi-monitor setups, the work area correctly reflects the usable space
    // on this specific monitor, while monitor_area might have incorrect bounds
    // due to virtual screen coordinate issues
    let bar_left = work_area.left;
    let bar_right = work_area.right;

    log_debug(&format!("Using horizontal bounds from work_area: left={}, right={}", bar_left, bar_right));

    let mut abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: APPBAR_CALLBACK,
        uEdge: ABE_BOTTOM,
        rc: RECT {
            left: bar_left,
            top: bar_top,
            right: bar_right,
            bottom: bar_bottom,
        },
        lParam: LPARAM(0),
    };

    log_debug(&format!("Requesting appbar rect: left={}, top={}, right={}, bottom={}",
             abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom));

    // Register the appbar
    let result = unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
    if result == 0 {
        log_debug("ERROR: Failed to register appbar");
        // Reset flag since registration failed
        APPBAR_REGISTERED.store(false, Ordering::SeqCst);
        return Err("Failed to register appbar".to_string());
    }
    // Note: APPBAR_REGISTERED was already set to true atomically at function start

    // Query the position to see what space is available
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };

    log_debug(&format!("After QUERYPOS: left={}, top={}, right={}, bottom={}",
             abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom));

    // Set the final position - ensure we request exactly the height we need
    abd.rc.top = abd.rc.bottom - physical_bar_height;
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };

    log_debug(&format!("After SETPOS: left={}, top={}, right={}, bottom={}",
             abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom));

    // Convert back to logical pixels for Tauri using precise rounding
    let logical_x = (abd.rc.left as f64 / scale).round() as i32;
    let logical_y = (abd.rc.top as f64 / scale).round() as i32;
    let logical_w = ((abd.rc.right - abd.rc.left) as f64 / scale).round() as i32;
    let logical_h = ((abd.rc.bottom - abd.rc.top) as f64 / scale).round() as i32;

    log_debug(&format!("Returning logical rect: x={}, y={}, w={}, h={}", logical_x, logical_y, logical_w, logical_h));

    Ok((logical_x, logical_y, logical_w, logical_h))
}

/// Unregister the appbar when done.
#[cfg(windows)]
pub fn unregister_appbar(hwnd: isize) {
    if !APPBAR_REGISTERED.load(Ordering::SeqCst) {
        return;
    }

    let hwnd = HWND(hwnd as *mut _);
    let mut abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        ..Default::default()
    };

    unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
    APPBAR_REGISTERED.store(false, Ordering::SeqCst);
}

/// Get the DPI scale factor for the primary monitor
#[cfg(windows)]
fn get_dpi_scale() -> f64 {
    use windows::Win32::UI::HiDpi::GetDpiForSystem;

    const DEFAULT_DPI: u32 = 96;  // Standard Windows DPI (100% scaling)

    let dpi = unsafe { GetDpiForSystem() };
    dpi as f64 / DEFAULT_DPI as f64
}

/// Get the work area (screen minus taskbar and other appbars) for the primary monitor.
/// Returns values in logical pixels (DPI-scaled for Tauri).
#[cfg(windows)]
pub fn get_work_area() -> Result<(i32, i32, i32, i32), String> {
    use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};

    let mut rect = RECT::default();
    let success = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };

    if success.is_err() {
        return Err("Failed to get work area".to_string());
    }

    // Convert from physical pixels to logical pixels for Tauri
    let scale = get_dpi_scale();
    let x = (rect.left as f64 / scale) as i32;
    let y = (rect.top as f64 / scale) as i32;
    let width = ((rect.right - rect.left) as f64 / scale) as i32;
    let height = ((rect.bottom - rect.top) as f64 / scale) as i32;

    println!("DPI scale: {}, physical rect: {:?}, logical: ({}, {}, {}, {})",
             scale, rect, x, y, width, height);

    Ok((x, y, width, height))
}

// Non-Windows stubs
#[cfg(not(windows))]
pub fn register_appbar(_hwnd: isize, _bar_height: i32) -> Result<(i32, i32, i32, i32), String> {
    Err("AppBar not supported on this platform".to_string())
}

#[cfg(not(windows))]
pub fn unregister_appbar(_hwnd: isize) {}

#[cfg(not(windows))]
pub fn get_work_area() -> Result<(i32, i32, i32, i32), String> {
    Err("Not supported on this platform".to_string())
}
