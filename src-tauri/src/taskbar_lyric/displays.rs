//! 显示器枚举（设置页的显示器选择列表）。

use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST, DISPLAY_DEVICEW,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::PCWSTR;
use super::*;

/// 窗口所在显示器设备名（\\.\DISPLAYn）
pub(super) unsafe fn monitor_device_of(hwnd: HWND) -> Option<String> {
    let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if hmon.is_invalid() {
        return None;
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let ok = GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO);
    if ok.as_bool() {
        Some(device_string(&info.szDevice))
    } else {
        None
    }
}

pub(super) fn device_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 一块显示器：id 供显示器选择持久化（设备名），name 给 UI 展示
#[derive(Debug, serde::Serialize)]
pub struct DisplayInfo {
    /// 设备名（\\.\DISPLAYn）：任务栏→显示器匹配与选择持久化的键
    pub id: String,
    /// 显示器友好名（如 "DELL U2723QE"），失败时回落设备名
    pub name: String,
    pub primary: bool,
    /// 该显示器当前是否有任务栏（副屏热插拔后可能短暂为 false）
    pub has_taskbar: bool,
}

/// 枚举系统显示器（任意线程可调用，纯查询无窗口操作）
pub fn list_displays() -> Vec<DisplayInfo> {
    unsafe {
        let taskbars = enum_taskbars();
        let taskbar_monitors: Vec<Option<String>> =
            taskbars.iter().map(|t| monitor_device_of(*t)).collect();
        let mut out: Vec<DisplayInfo> = Vec::new();
        let ctx = LPARAM(&mut out as *mut Vec<DisplayInfo> as isize);
        let _ = EnumDisplayMonitors(None, None, Some(enum_monitor_proc), ctx);
        // 补充友好名与 taskbar 标记
        for d in out.iter_mut() {
            d.name = monitor_friendly_name(&d.id).unwrap_or_else(|| d.id.clone());
            d.has_taskbar = taskbar_monitors
                .iter()
                .any(|m| m.as_deref() == Some(d.id.as_str()));
        }
        // 主显示器排前
        out.sort_by_key(|d| !d.primary);
        out
    }
}

unsafe extern "system" fn enum_monitor_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> windows::core::BOOL {
    let out = lparam.0 as *mut Vec<DisplayInfo>;
    if out.is_null() {
        return windows::core::BOOL(1);
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let ok = GetMonitorInfoW(hmon, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO);
    if ok.as_bool() {
        (*out).push(DisplayInfo {
            id: device_string(&info.szDevice),
            name: String::new(),
            primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
            has_taskbar: false,
        });
    }
    windows::core::BOOL(1)
}

/// 通过 EnumDisplayDevices 取显示器友好名（两步：适配器设备名 → 监视器）
fn monitor_friendly_name(device: &str) -> Option<String> {
    let wide: Vec<u16> = device.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut dd = DISPLAY_DEVICEW::default();
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = EnumDisplayDevicesW(PCWSTR(wide.as_ptr()), 0, &mut dd, 0);
        if ok.as_bool() {
            let name = device_string(&dd.DeviceString);
            if !name.is_empty() {
                return Some(name);
            }
        }
        None
    }
}
