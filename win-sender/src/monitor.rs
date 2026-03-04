use std::mem;

use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};

/// Information about a connected display monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Zero-based index (matches GStreamer monitor-index).
    pub index: usize,
    /// Device name (e.g. `\\.\DISPLAY1`).
    pub name: String,
    /// Whether this is the primary monitor.
    pub primary: bool,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Left edge in virtual screen coordinates.
    pub x: i32,
    /// Top edge in virtual screen coordinates.
    pub y: i32,
}

/// Enumerate all connected display monitors.
pub fn list_monitors() -> Vec<MonitorInfo> {
    let mut handles: Vec<HMONITOR> = Vec::new();

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_callback),
            LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
        );
    }

    let mut monitors = Vec::new();
    for (i, &hmon) in handles.iter().enumerate() {
        let mut info: MONITORINFOEXW = unsafe { mem::zeroed() };
        info.monitorInfo.cbSize = mem::size_of::<MONITORINFOEXW>() as u32;

        let ok = unsafe { GetMonitorInfoW(hmon, &mut info as *mut _ as *mut _) };
        if !ok.as_bool() {
            continue;
        }

        let rc = info.monitorInfo.rcMonitor;
        let name = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())],
        );

        monitors.push(MonitorInfo {
            index: i,
            name,
            primary: (info.monitorInfo.dwFlags & 1) != 0, // MONITORINFOF_PRIMARY
            width: (rc.right - rc.left) as u32,
            height: (rc.bottom - rc.top) as u32,
            x: rc.left,
            y: rc.top,
        });
    }

    monitors
}

/// Print a summary table of all monitors.
pub fn print_monitors(monitors: &[MonitorInfo]) {
    println!("[Sender] Detected {} monitor(s):", monitors.len());
    for m in monitors {
        println!(
            "  [{}] {} {}x{} at ({},{}){}",
            m.index,
            m.name,
            m.width,
            m.height,
            m.x,
            m.y,
            if m.primary { " (primary)" } else { "" },
        );
    }
}

unsafe extern "system" fn enum_callback(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let handles = &mut *(lparam.0 as *mut Vec<HMONITOR>);
    handles.push(hmon);
    TRUE
}
