//! 任务栏歌词技术探针（Windows 专属工具，其它平台编译为空）。
//!
//! 与正式模块（src/taskbar_lyric.rs）同款的 DirectWrite + Direct2D 渲染路径，
//! 用于在不启动完整应用的情况下验证/调试任务栏效果：
//! - `taskbar_lyric_probe dump`：转储任务栏层级与歌词窗口实际位置
//! - `taskbar_lyric_probe secondary`：嵌入副屏任务栏（图标区右侧锚定）
//! - 无参数：嵌入主屏任务栏（托盘区左缘锚定），显示 20 秒后退出
#![cfg(windows)]

use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Brush, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_TEXT_ANTIALIAS_MODE_DEFAULT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFactory2, IDWriteRenderingParams,
    IDWriteTextFormat, IDWriteTextLayout, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_GRID_FIT_MODE_DEFAULT,
    DWRITE_PIXEL_GEOMETRY_FLAT, DWRITE_RENDERING_MODE_NATURAL, DWRITE_TEXT_METRICS,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW,
    MonitorFromWindow, ReleaseDC, ScreenToClient, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, HMONITOR,
    MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowExW, FindWindowW, GetAncestor,
    GetMessageW, GetWindowRect, IsWindow, IsWindowVisible, KillTimer, RegisterClassExW, SetParent,
    SetTimer,
    SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW,
    GA_PARENT, HWND_TOP, MSG, SET_WINDOW_POS_FLAGS, SHOW_WINDOW_CMD, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_DESTROY, WM_TIMER, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{w, Interface, PCWSTR};
use windows_numerics::Vector2;

const CLASS_NAME: PCWSTR = w!("RustMusic_TaskbarLyricProbe");
const TIMER_ANCHOR: usize = 1;
const SAMPLE: &str = "♪ RustMusic 任务栏歌词预览 · Hello Taskbar 123 · 更行更远还生歌";

/// 诊断时间戳基准（进程启动）
static T0: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

static PROBE_HWND: AtomicIsize = AtomicIsize::new(0);

fn main() {
    let _ = T0.set(Instant::now());
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "dump") {
        dump();
        return;
    }
    let secondary = args.iter().any(|a| a == "secondary");
    show_sample(secondary);
}

/// 转储全部任务栏层级与歌词窗口位置
fn dump() {
    unsafe {
        println!("== 任务栏窗口 ==");
        for tray in enum_taskbars() {
            let mut r = RECT::default();
            let _ = GetWindowRect(tray, &mut r);
            let hmon = MonitorFromWindow(tray, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFOEXW::default();
            mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            let dev = if GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO)
                .as_bool()
            {
                let end = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(32);
                String::from_utf16_lossy(&mi.szDevice[..end])
            } else {
                "?".into()
            };
            let cls = class_name(tray);
            println!(
                "[{cls}] hwnd={tray:?} rect=({},{})-({},{}) monitor={dev}",
                r.left, r.top, r.right, r.bottom
            );
            let mut prev: Option<HWND> = None;
            loop {
                let child = match FindWindowExW(Some(tray), prev, PCWSTR::null(), None) {
                    Ok(h) => h,
                    Err(_) => break,
                };
                let mut cr = RECT::default();
                let _ = GetWindowRect(child, &mut cr);
                println!(
                    "    [{}] hwnd={child:?} rect=({},{})-({},{})",
                    class_name(child),
                    cr.left,
                    cr.top,
                    cr.right,
                    cr.bottom
                );
                prev = Some(child);
            }
        }
        println!("\n== 歌词窗口（RustMusic_TaskbarLyric*） ==");
        for tray in enum_taskbars() {
            let mut prev: Option<HWND> = None;
            loop {
                let child = match FindWindowExW(Some(tray), prev, PCWSTR::null(), None) {
                    Ok(h) => h,
                    Err(_) => break,
                };
                let cn = class_name(child);
                if cn.starts_with("RustMusic_TaskbarLyric") {
                    let mut r = RECT::default();
                    let _ = GetWindowRect(child, &mut r);
                    let vis = IsWindowVisible(child).as_bool();
                    let mut cloaked: u32 = 0;
                    let _ = windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
                        child,
                        windows::Win32::Graphics::Dwm::DWMWA_CLOAKED,
                        &mut cloaked as *mut u32 as *mut _,
                        std::mem::size_of::<u32>() as u32,
                    );
                    let mut owner_pid: u32 = 0;
                    let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                        child,
                        Some(&mut owner_pid),
                    );
                    println!(
                        "tray={tray:?} [{cn}] visible={vis} cloaked={cloaked} pid={owner_pid} (dump pid={}) screen_rect=({},{})-({},{}) size={}x{}",
                        std::process::id(),
                        r.left,
                        r.top,
                        r.right,
                        r.bottom,
                        r.right - r.left,
                        r.bottom - r.top
                    );
                }
                prev = Some(child);
            }
        }
    }
}

unsafe fn class_name(hwnd: HWND) -> String {
    #[link(name = "user32")]
    extern "system" {
        fn GetClassNameW(hwnd: HWND, buf: *mut u16, max: i32) -> i32;
    }
    let mut buf = [0u16; 128];
    let n = GetClassNameW(hwnd, buf.as_mut_ptr(), 128);
    if n <= 0 {
        return "?".into();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

unsafe fn enum_taskbars() -> Vec<HWND> {
    let mut v = Vec::new();
    if let Ok(t) = FindWindowW(w!("Shell_TrayWnd"), None) {
        v.push(t);
    }
    let mut prev: Option<HWND> = None;
    loop {
        let next = match FindWindowExW(None, prev, w!("Shell_SecondaryTrayWnd"), None) {
            Ok(h) => h,
            Err(_) => break,
        };
        v.push(next);
        prev = Some(next);
    }
    v
}

fn show_sample(secondary: bool) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap().into();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        let atom = RegisterClassExW(&wc);
        assert_ne!(atom, 0, "RegisterClassExW 失败");

        let (tray, tag) = if secondary {
            (
                enum_taskbars()
                    .into_iter()
                    .find(|t| class_name(*t) == "Shell_SecondaryTrayWnd")
                    .expect("未找到副屏任务栏"),
                "副屏",
            )
        } else {
            (FindWindowW(w!("Shell_TrayWnd"), None).expect("找不到 Shell_TrayWnd"), "主屏")
        };
        let mut r = RECT::default();
        GetWindowRect(tray, &mut r).unwrap();
        println!(
            "{tag}任务栏 rect=({},{})-({},{}) size={}x{} dpi={}",
            r.left,
            r.top,
            r.right,
            r.bottom,
            r.right - r.left,
            r.bottom - r.top,
            GetDpiForWindow(tray)
        );

        // 跨进程 CreateWindowEx(WS_CHILD) 不允许：先建顶层 WS_POPUP 再 SetParent
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            CLASS_NAME,
            w!("RustMusic 任务栏歌词探针"),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            Some(hinstance),
            None,
        )
        .expect("创建歌词窗口失败");
        SetParent(hwnd, Some(tray)).expect("SetParent 嵌入任务栏失败");
        PROBE_HWND.store(hwnd.0 as isize, Ordering::Release);
        SetTimer(Some(hwnd), TIMER_ANCHOR, 200, None);
        anchor(hwnd);
        println!("窗口已嵌入{tag}任务栏，20 秒后退出…");
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if Instant::now() > deadline {
                break;
            }
        }
    }
    println!("探针退出");
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER if wparam.0 == TIMER_ANCHOR => {
            anchor(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(Some(hwnd), TIMER_ANCHOR);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 重锚定 + 渲染（锚定逻辑与正式模块一致）
unsafe fn anchor(hwnd: HWND) {
    let Some(tray) = enum_taskbars()
        .into_iter()
        .find(|t| GetAncestor(hwnd, GA_PARENT) == *t)
    else {
        return;
    };
    let mut tray_r = RECT::default();
    let _ = GetWindowRect(tray, &mut tray_r);
    let taskbar_w = tray_r.right - tray_r.left;
    let taskbar_h = tray_r.bottom - tray_r.top;

    // 渲染（DirectWrite + Direct2D，与正式模块同参数）
    let Ok(dwrite) = DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED) else {
        return;
    };
    let Ok(d2d) = D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) else {
        return;
    };
    let Ok(f2) = dwrite.cast::<IDWriteFactory2>() else {
        return;
    };
    let Ok(params) = f2
        .CreateCustomRenderingParams(
            2.2,
            1.0,
            1.0,
            0.0,
            DWRITE_PIXEL_GEOMETRY_FLAT,
            DWRITE_RENDERING_MODE_NATURAL,
            DWRITE_GRID_FIT_MODE_DEFAULT,
        )
        .map(|p| p.cast::<IDWriteRenderingParams>().unwrap())
    else {
        return;
    };
    let Ok(format) = dwrite.CreateTextFormat(
        w!("Microsoft YaHei UI"),
        None,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        (((taskbar_h as f32 * 0.34).clamp(12.0, 20.0)).round()) as f32,
        w!(""),
    ) else {
        return;
    };

    let max_w = ((taskbar_w * 35 / 100).max(80)).min(560) as f32;
    let wide: Vec<u16> = SAMPLE.encode_utf16().collect();
    let Ok(layout) = dwrite.CreateTextLayout(&wide, &format, max_w, taskbar_h as f32 * 2.0) else {
        return;
    };
    let mut m = DWRITE_TEXT_METRICS::default();
    if layout.GetMetrics(&mut m).is_err() {
        return;
    }
    let pad = 4i32;
    let draw_w = (m.width.ceil() as i32).max(1);
    let w = draw_w + pad * 2;
    let h = (m.height.ceil() as i32) + 2 * scale(taskbar_h, 2);

    // 锚点：主屏=托盘区左缘；副屏=右缘预留主屏托盘区宽度（时钟区）左侧
    let (x, y) = if let Ok(Some(tn)) = FindWindowExW(Some(tray), None, w!("TrayNotifyWnd"), None)
        .map(|h| Some(h).filter(|t| IsWindow(Some(*t)).as_bool()))
    {
        let mut tn_r = RECT::default();
        let _ = GetWindowRect(tn, &mut tn_r);
        let mut pt = POINT { x: tn_r.left, y: 0 };
        let _ = ScreenToClient(tray, &mut pt);
        ((pt.x - w - scale(taskbar_h, 8)).max(scale(taskbar_h, 4)), (taskbar_h - h) / 2)
    } else {
        // 副屏：UIA 实测右侧时钟/通知区左边界（与正式模块同逻辑），失败回落估算
        let zone = measure_zone_uia(tray, &tray_r)
            .unwrap_or_else(|| tray_r.right - scale(taskbar_h, 240));
        let mut pt = POINT { x: zone, y: 0 };
        let _ = ScreenToClient(tray, &mut pt);
        (
            ((pt.x - w - scale(taskbar_h, 8)).min(taskbar_w - w - scale(taskbar_h, 4)))
                .max(scale(taskbar_h, 4)),
            (taskbar_h - h) / 2,
        )
    };

    // DIB + D2D 绘制
    let dc = CreateCompatibleDC(None);
    let mut bi = BITMAPINFO::default();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = w;
    bi.bmiHeader.biHeight = -h;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB.0;
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let Ok(bmp) = CreateDIBSection(Some(dc), &bi, DIB_RGB_COLORS, &mut bits, None, 0) else {
        return;
    };
    let old = SelectObject(dc, HGDIOBJ(bmp.0));

    let props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let Ok(rt) = d2d.CreateDCRenderTarget(&props) else {
        return;
    };
    let _ = rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_DEFAULT);
    let _ = rt.SetTextRenderingParams(Some(&params));
    let rect = RECT { left: 0, top: 0, right: w, bottom: h };
    if rt.BindDC(dc, &rect).is_err() {
        return;
    }
    let color = D2D1_COLOR_F { r: 0.1, g: 0.1, b: 0.1, a: 1.0 };
    let Ok(brush) = rt.CreateSolidColorBrush(&color, None) else {
        return;
    };
    let Ok(brush_base) = brush.cast::<ID2D1Brush>() else {
        return;
    };
    rt.BeginDraw();
    let _ = rt.Clear(None);
    rt.DrawTextLayout(
        Vector2 { X: pad as f32, Y: (h as f32 - m.height) / 2.0 },
        &layout,
        &brush_base,
        D2D1_DRAW_TEXT_OPTIONS_NONE,
    );
    if let Err(e) = rt.EndDraw(None, None) {
        eprintln!("D2D 绘制失败: {e}");
        return;
    }
    // 诊断：统计非零像素（判断 D2D 是否真的画了内容）
    {
        let px = bits as *const u32;
        let mut nonzero = 0usize;
        for i in 0..((w * h) as usize) {
            if *px.add(i) != 0 {
                nonzero += 1;
            }
        }
        eprintln!("[probe {:?}ms] DIB 非零像素 = {nonzero} / {}", T0.get().map(|t| t.elapsed()).unwrap_or_default(), (w * h) as usize);
    }

    let pos = POINT { x, y };
    let size = SIZE { cx: w, cy: h };
    let src = POINT { x: 0, y: 0 };
    let screen_dc = GetDC(Some(tray));
    let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let res = UpdateLayeredWindow(
        hwnd,
        Some(screen_dc),
        Some(&pos),
        Some(&size),
        Some(dc),
        Some(&src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
    let _ = ReleaseDC(Some(tray), screen_dc);
    if let Err(e) = res {
        eprintln!("UpdateLayeredWindow 失败: {e}");
    }
    let _ = ShowWindow(hwnd, SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0));
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOP),
        0,
        0,
        0,
        0,
        SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_SHOWWINDOW.0),
    );
    // 诊断：窗口终态
    {
        let mut vr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut vr);
        eprintln!(
            "[probe {:?}ms] after-ULW rect=({},{})-({},{}) visible={} parent={:?}",
            T0.get().map(|t| t.elapsed()).unwrap_or_default(),
            vr.left,
            vr.top,
            vr.right,
            vr.bottom,
            IsWindowVisible(hwnd).as_bool(),
            GetAncestor(hwnd, GA_PARENT)
        );
    }
    let _ = SelectObject(dc, old);
    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(dc);
}

fn scale(taskbar_h: i32, v: i32) -> i32 {
    (v as f32 * taskbar_h as f32 / 48.0).round() as i32
}

/// UIA 实测副屏任务栏右侧时钟/通知区左边界（屏幕坐标）——与正式模块
/// secondary_zone_left 同逻辑（探针独立编译，无法复用 bin 内模块）
unsafe fn measure_zone_uia(tray: HWND, tray_r: &RECT) -> Option<i32> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
    let automation: IUIAutomation =
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
    let root = automation.ElementFromHandle(tray).ok()?;
    let cond = automation.CreateTrueCondition().ok()?;
    let all = root.FindAll(TreeScope_Descendants, &cond).ok()?;
    let taskbar_w = tray_r.right - tray_r.left;
    let band = scale(tray_r.bottom - tray_r.top, 420);
    let mut zone_left: Option<i32> = None;
    let n = all.Length().ok()?;
    for i in 0..n {
        let Ok(e) = all.GetElement(i) else { continue };
        let Ok(r) = e.CurrentBoundingRectangle() else { continue };
        let (l, t, rgt, b) = (r.left, r.top, r.right, r.bottom);
        if rgt <= l || b <= t {
            continue;
        }
        if l < tray_r.left || rgt > tray_r.right || t < tray_r.top || b > tray_r.bottom {
            continue;
        }
        if rgt - l >= taskbar_w * 2 / 5 {
            continue;
        }
        if l < tray_r.right - band {
            continue;
        }
        println!("    UIA 元素 left={l} right={rgt} top={t}");
        zone_left = Some(zone_left.map_or(l, |z| z.min(l)));
    }
    println!("UIA 时钟区左边界 = {zone_left:?}");
    zone_left
}
