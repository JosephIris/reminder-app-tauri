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
static APPBAR_REGISTERED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
const APPBAR_CALLBACK: u32 = WM_USER + 1;

/// Register a window as an appbar docked at the bottom of the screen.
/// Returns the adjusted work area rect that the bar should occupy.
#[cfg(windows)]
pub fn register_appbar(hwnd: isize, bar_height: i32) -> Result<(i32, i32, i32, i32), String> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};

    let hwnd = HWND(hwnd as *mut _);

    // Get work area (screen minus existing appbars like taskbar)
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) };
    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    let success = unsafe { GetMonitorInfoW(monitor, &mut monitor_info) };
    if !success.as_bool() {
        return Err("Failed to get monitor info".to_string());
    }

    let work_area = monitor_info.rcWork;
    let monitor_area = monitor_info.rcMonitor;

    println!("Monitor area: left={}, top={}, right={}, bottom={}",
             monitor_area.left, monitor_area.top, monitor_area.right, monitor_area.bottom);
    println!("Work area: left={}, top={}, right={}, bottom={}",
             work_area.left, work_area.top, work_area.right, work_area.bottom);

    // Use monitor_area for left/right (full screen width) and work_area for bottom (above taskbar)
    // This ensures the bar spans the full screen width starting at x=0
    let mut abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: APPBAR_CALLBACK,
        uEdge: ABE_BOTTOM,
        rc: RECT {
            left: monitor_area.left,  // Use monitor area left (should be 0 for primary)
            top: work_area.bottom - bar_height,
            right: monitor_area.right,  // Use monitor area right (full width)
            bottom: work_area.bottom,
        },
        lParam: LPARAM(0),
    };

    // Register the appbar
    let result = unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
    if result == 0 {
        return Err("Failed to register appbar".to_string());
    }

    APPBAR_REGISTERED.store(true, Ordering::SeqCst);

    // Query the position to see what space is available
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };

    println!("After QUERYPOS: {:?}", abd.rc);

    // Set the final position
    abd.rc.top = abd.rc.bottom - bar_height;
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };

    println!("After SETPOS: {:?}", abd.rc);

    Ok((abd.rc.left, abd.rc.top, abd.rc.right - abd.rc.left, abd.rc.bottom - abd.rc.top))
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

/// Get the work area (screen minus taskbar and other appbars) for the primary monitor.
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

    // Return position and size - for primary monitor, left should typically be 0
    // but we return the actual values for multi-monitor support
    Ok((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top))
}

/// Get the primary monitor's full screen bounds (ignoring work area).
#[cfg(windows)]
pub fn get_primary_monitor_bounds() -> Result<(i32, i32, i32, i32), String> {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    // Primary monitor always starts at 0,0
    Ok((0, 0, width, height))
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

#[cfg(not(windows))]
pub fn get_primary_monitor_bounds() -> Result<(i32, i32, i32, i32), String> {
    Err("Not supported on this platform".to_string())
}
