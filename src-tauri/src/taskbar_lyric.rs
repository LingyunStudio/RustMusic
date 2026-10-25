//! 任务栏歌词（Windows 10/11）：在任务栏托盘折叠图标左侧显示当前歌词行。
//!
//! 原理：`Shell_TrayWnd`（主屏任务栏）与 `Shell_SecondaryTrayWnd`（副屏任务栏，
//! 每个副显示器一个）的窗口层级稳定存在。主屏任务栏含 `TrayNotifyWnd`
//! （托盘/折叠箭头区）；副屏没有托盘区窗口，且新版 Win11 副屏右缘有镜像的
//! 系统时钟——副屏歌词改为紧跟应用图标区（`WorkerW`）右侧。
//! 把一个鼠标穿透的 32bpp ARGB 图层窗口以 WS_POPUP + SetParent 嵌入任务栏
//!（跨进程不允许直接 CreateWindowEx(WS_CHILD)，TrafficMonitor 同款做法）。
//! 托盘图标增减会改变托盘区宽度，200ms 定时器重读位置自动跟随——
//! 任务栏自动隐藏/全屏应用覆盖时随父窗口一起消失，无需额外处理。
//! 支持选择显示器：全部（默认）/ 指定显示器（按显示器设备名匹配）。
//!
//! 文字渲染走 DirectWrite + Direct2D（软件 DC 目标画进 32bpp DIB），
//! 与系统任务栏原生文字同源：伽马校正的字形抗锯齿、精确的字体选择
//!（GDI 路径在此场景下笔画发虚，已弃用）。超宽歌词按像素宽度二分截断
//! 并追加省略号。
//!
//! 数据流与菜单栏歌词（Linux）共用前端管线：状态帧 → 行变化才 IPC
//! → 本模块 PostMessage 唤醒渲染线程。歌词窗口常驻本进程，应用退出即销毁，
//! 因此没有 Linux 文件方案的 staleness/心跳问题。
//!
//! 线程模型：全部 Win32 操作都在专属线程（消息循环线程）执行，命令线程只
//! 更新共享状态后 PostMessage 唤醒。控制枢纽是一个 HWND_MESSAGE 消息窗口，
//! 歌词窗口按需延迟创建/销毁（开启但未播放时不占用任务栏）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

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
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, DISPLAY_DEVICEW, EnumDisplayDevicesW,
    EnumDisplayMonitors, HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITORINFO, MONITORINFOEXW,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, TreeScope_Descendants,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowExW, FindWindowW,
    GetAncestor, GetMessageW, GetWindowRect, IsWindow, KillTimer, PostMessageW, RegisterClassExW,
    SetParent, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    CS_HREDRAW, CS_VREDRAW, GA_PARENT, HWND_MESSAGE, HWND_TOP, MONITORINFOF_PRIMARY, MSG,
    SET_WINDOW_POS_FLAGS, SHOW_WINDOW_CMD, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE,
    SW_SHOWNOACTIVATE, ULW_ALPHA, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WM_TIMER, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{w, Interface, PCWSTR};
use windows_numerics::Vector2;

/// 共享状态（命令线程写、渲染线程读）
struct Shared {
    /// 功能开关；关闭时销毁全部歌词窗口
    enabled: bool,
    /// 当前歌词行（空 = 前奏/间奏/无曲目 → 隐藏）
    text: String,
    /// 用户自选颜色（"#rrggbb[aa]"；None/非法 = 默认色随任务栏深浅）
    color: Option<String>,
    /// 显示器选择："all" = 全部；其它 = 显示器设备名（\\.\DISPLAYn）
    display: String,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    enabled: false,
    text: String::new(),
    color: None,
    display: String::new(),
});

/// 控制枢纽窗口（HWND_MESSAGE，渲染线程内创建）
static CTL_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// WM_APP 基址：共享状态有变更，按需创建/隐藏/渲染歌词窗口
const WM_APP_RENDER: u32 = 0x8000;
const TIMER_ANCHOR: usize = 1;

const LYRIC_CLASS: PCWSTR = w!("RustMusic_TaskbarLyric");
const CTL_CLASS: PCWSTR = w!("RustMusic_TaskbarLyricCtl");

/// 已嵌入的歌词窗口（tray hwnd → lyric hwnd），渲染线程私有
static LYRIC_WINDOWS: Mutex<Vec<(isize, isize)>> = Mutex::new(Vec::new());

/// 同步歌词行（毫秒, 文本）：前端在歌词加载/切歌时整包推送。
/// WebView 挂起期间前端停摆（托盘挂起回收渲染内存），行推进改由
/// monitor 线程按引擎播放位置驱动（auto_tick），不依赖前端逐行推送
static AUTO_LINES: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());

/// 自动推进的上一行文本（去重，避免每 120ms 重复触发渲染）
static LAST_AUTO_TEXT: Mutex<String> = Mutex::new(String::new());

/// 前端推送的同步歌词行
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarLine {
    pub time_ms: u64,
    pub text: String,
}

static SPAWNED: AtomicBool = AtomicBool::new(false);

/// DWrite/D2D 渲染上下文（渲染线程私有；COM 指针不跨线程使用）
struct RenderCtx {
    dwrite: IDWriteFactory,
    d2d: ID2D1Factory,
    params: IDWriteRenderingParams,
    rt: Option<ID2D1DCRenderTarget>,
    format: Option<(IDWriteTextFormat, i32)>,
    brush: Option<ID2D1SolidColorBrush>,
    brush_color: u32,
}

// COM 指针仅在渲染线程触碰
unsafe impl Send for RenderCtx {}

static CTX: Mutex<Option<RenderCtx>> = Mutex::new(None);

/// 启动渲染线程（进程生命周期一次；setup 阶段调用，重复调用为无操作）
pub fn spawn() {
    if SPAWNED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("taskbar-lyric".into())
        .spawn(run)
        .expect("taskbar-lyric 线程启动失败");
}

/// 命令线程入口：更新开关并唤醒渲染线程
pub fn set_enabled(on: bool) {
    SHARED.lock().enabled = on;
    wake_render();
}

/// 命令线程入口：更新当前行并唤醒渲染线程
pub fn push_line(text: String, color: Option<String>) {
    {
        let mut s = SHARED.lock();
        s.text = text;
        s.color = color;
    }
    wake_render();
}

/// 命令线程入口：更新显示器选择（"all" 或显示器设备名）
pub fn set_display(display: String) {
    SHARED.lock().display = display;
    wake_render();
}

/// 前端整包推送同步歌词行（歌词加载/切歌时一次；空 = 无同步歌词，
/// 用 ♪ 标题回落）。行推进由 monitor 线程 auto_tick 驱动
pub fn set_lines(lines: Vec<(u64, String)>) {
    *AUTO_LINES.lock() = lines;
}

/// 前端改色（设置页取色时）；当前行文本由 auto_tick / 既有推送保持
pub fn set_color(color: Option<String>) {
    SHARED.lock().color = color;
    wake_render();
}

/// monitor 线程每帧调用：按引擎播放位置推进当前行。
/// fallback = 无同步歌词时的显示文本（♪ 标题，调用方从引擎当前曲目取）
pub fn auto_tick(pos_ms: u64, fallback: Option<String>) {
    let text = {
        let lines = AUTO_LINES.lock();
        if lines.is_empty() {
            match fallback {
                Some(t) if !t.is_empty() => t,
                _ => String::new(),
            }
        } else {
            // 当前行二分：最后一个 timeMs <= pos（前奏/间奏为空 → 隐藏）
            let mut lo = 0usize;
            let mut hi = lines.len();
            let mut idx: Option<usize> = None;
            while lo < hi {
                let mid = (lo + hi) / 2;
                if lines[mid].0 <= pos_ms {
                    idx = Some(mid);
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            match idx {
                Some(i) => lines[i].1.clone(),
                None => String::new(),
            }
        }
    };
    let mut s = SHARED.lock();
    if !s.enabled || s.text == text {
        return;
    }
    s.text = text;
    drop(s);
    wake_render();
}

fn wake_render() {
    let h = CTL_HWND.load(Ordering::Acquire);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_APP_RENDER, WPARAM(0), LPARAM(0));
        }
    }
}

fn run() {
    unsafe {
        // 渲染上下文先于消息循环就绪（失败则功能静默失效，仅留日志）
        match init_ctx() {
            Ok(ctx) => *CTX.lock() = Some(ctx),
            Err(e) => eprintln!("[taskbar-lyric] 渲染上下文初始化失败: {e}"),
        }

        let hinstance = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(e) => {
                eprintln!("[taskbar-lyric] GetModuleHandleW 失败: {e}");
                return;
            }
        };
        // 控制枢纽窗口：仅收消息，永不显示
        let wc_ctl = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(ctl_wnd_proc),
            hInstance: hinstance,
            lpszClassName: CTL_CLASS,
            ..Default::default()
        };
        if RegisterClassExW(&wc_ctl) == 0 {
            eprintln!("[taskbar-lyric] 注册控制窗口类失败");
            return;
        }
        let ctl = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CTL_CLASS,
            w!("RustMusic 任务栏歌词控制"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[taskbar-lyric] 创建控制窗口失败: {e}");
                return;
            }
        };
        CTL_HWND.store(ctl.0 as isize, Ordering::Release);
        // 锚定定时器：跟随托盘位置变化 / explorer 重启重建 / 显示器热插拔
        let _ = SetTimer(Some(ctl), TIMER_ANCHOR, 200, None);

        // 歌词窗口类（实际嵌入任务栏的显示窗口）
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(lyric_wnd_proc),
            hInstance: hinstance,
            lpszClassName: LYRIC_CLASS,
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            eprintln!("[taskbar-lyric] 注册歌词窗口类失败");
            return;
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// DWrite/D2D 工厂与默认渲染参数（伽马 2.2 + 对比度增强：
/// 与 ClearType 的视觉粗细对齐，避免 GDI 灰度路径的"发虚"观感）
unsafe fn init_ctx() -> windows::core::Result<RenderCtx> {
    let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
    let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
    let f2: IDWriteFactory2 = dwrite.cast()?;
    let params = f2
        .CreateCustomRenderingParams(
            2.2,
            1.0,
            1.0,
            0.0,
            DWRITE_PIXEL_GEOMETRY_FLAT,
            DWRITE_RENDERING_MODE_NATURAL,
            DWRITE_GRID_FIT_MODE_DEFAULT,
        )?
        .cast::<IDWriteRenderingParams>()?;
    Ok(RenderCtx {
        dwrite,
        d2d,
        params,
        rt: None,
        format: None,
        brush: None,
        brush_color: 0,
    })
}

unsafe extern "system" fn ctl_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_RENDER => {
            apply_state();
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == TIMER_ANCHOR => {
            // 定时心跳与命令路径共用：顺带兜底显示器热插拔、
            // explorer 重启、新任务栏出现等静默变化
            apply_state();
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(Some(hwnd), TIMER_ANCHOR);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn lyric_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 按共享状态创建/隐藏/渲染全部歌词窗口（渲染线程内执行）。
/// WM_TIMER 与 WM_APP_RENDER 共用，幂等无变更时开销极小
unsafe fn apply_state() {
    let (enabled, text, color, display) = {
        let s = SHARED.lock();
        (
            s.enabled,
            s.text.clone(),
            s.color.clone(),
            s.display.clone(),
        )
    };

    if !enabled {
        destroy_all();
        return;
    }
    if text.is_empty() {
        // 前奏/间奏/无曲目：隐藏（窗口保留，切回歌词无需重建）
        hide_all();
        return;
    }

    let colorref = resolve_color(color.as_deref());
    let taskbars = match_taskbars(&display);
    prune_windows(&taskbars);
    for tray in taskbars {
        let Some(hwnd) = ensure_lyric_window(tray) else {
            continue;
        };
        render_line(hwnd, tray, &text, colorref);
    }
}

fn hide_all() {
    for (_, lyric) in LYRIC_WINDOWS.lock().iter() {
        unsafe {
            let _ = ShowWindow(HWND(*lyric as *mut _), SW_HIDE);
        }
    }
}

/// 枚举全部任务栏窗口（主屏 Shell_TrayWnd + 副屏 Shell_SecondaryTrayWnd）
unsafe fn enum_taskbars() -> Vec<HWND> {
    let mut v = Vec::new();
    if let Ok(t) = FindWindowW(w!("Shell_TrayWnd"), None) {
        if IsWindow(Some(t)).as_bool() {
            v.push(t);
        }
    }
    let mut prev = None;
    loop {
        let next = match FindWindowExW(None, prev, w!("Shell_SecondaryTrayWnd"), None) {
            Ok(h) => h,
            Err(_) => break,
        };
        if IsWindow(Some(next)).as_bool() {
            v.push(next);
        }
        prev = Some(next);
    }
    v
}

/// 显示器选择 → 参与显示的任务栏列表。"all" = 全部；
/// 指定设备名时按任务栏所在显示器匹配，匹配不到（显示器已拔）回落空
unsafe fn match_taskbars(display: &str) -> Vec<HWND> {
    let all = enum_taskbars();
    if display.is_empty() || display == "all" {
        return all;
    }
    all.into_iter()
        .filter(|t| monitor_device_of(*t).as_deref() == Some(display))
        .collect()
}

/// 窗口所在显示器设备名（\\.\DISPLAYn）
unsafe fn monitor_device_of(hwnd: HWND) -> Option<String> {
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

fn device_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 清理不再参与显示/已失效的歌词窗口
unsafe fn prune_windows(current: &[HWND]) {
    let mut map = LYRIC_WINDOWS.lock();
    let cur: Vec<isize> = current.iter().map(|h| h.0 as isize).collect();
    map.retain(|(tray, lyric)| {
        if cur.contains(tray) && IsWindow(Some(HWND(*lyric as *mut _))).as_bool() {
            true
        } else {
            let _ = DestroyWindow(HWND(*lyric as *mut _));
            false
        }
    });
}

unsafe fn destroy_all() {
    for (_, lyric) in LYRIC_WINDOWS.lock().drain(..) {
        let hwnd = HWND(lyric as *mut _);
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// 延迟创建歌词窗口并嵌入对应任务栏；父子关系失效（explorer 重启）时重嵌
unsafe fn ensure_lyric_window(tray: HWND) -> Option<HWND> {
    {
        let map = LYRIC_WINDOWS.lock();
        if let Some((_, lyric)) = map.iter().find(|(t, _)| *t == tray.0 as isize) {
            let h = HWND(*lyric as *mut _);
            if IsWindow(Some(h)).as_bool() {
                if GetAncestor(h, GA_PARENT) != tray {
                    let _ = SetParent(h, Some(tray));
                }
                return Some(h);
            }
        }
    }
    let hinstance = GetModuleHandleW(None).ok()?.into();
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
        LYRIC_CLASS,
        w!("RustMusic 任务栏歌词"),
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
    .ok()?;
    if SetParent(hwnd, Some(tray)).is_err() {
        eprintln!("[taskbar-lyric] SetParent 嵌入任务栏失败");
        let _ = DestroyWindow(hwnd);
        return None;
    }
    LYRIC_WINDOWS
        .lock()
        .push((tray.0 as isize, hwnd.0 as isize));
    Some(hwnd)
}

/// 渲染一行歌词并更新图层窗口位置/大小。
/// 主屏：锚定 TrayNotifyWnd（托盘区）左侧；副屏（无托盘区窗口）：
/// 新版 Win11 副屏右缘是镜像的系统时钟，改为紧跟应用图标区（WorkerW）右侧。
/// 竖排任务栏：停靠侧对侧、底部对齐
unsafe fn render_line(hwnd: HWND, tray: HWND, text: &str, color: COLORREF) {
    let mut guard = CTX.lock();
    static WARNED: AtomicBool = AtomicBool::new(false);
    let Some(ctx) = guard.as_mut() else {
        if !WARNED.swap(true, Ordering::Relaxed) {
            eprintln!("[taskbar-lyric] 渲染上下文未就绪，歌词不可用");
        }
        return;
    };

    let mut tray_r = RECT::default();
    let _ = GetWindowRect(tray, &mut tray_r);
    let taskbar_w = tray_r.right - tray_r.left;
    let taskbar_h = tray_r.bottom - tray_r.top;
    if taskbar_w <= 0 || taskbar_h <= 0 {
        return;
    }
    let font_px = ((taskbar_h as f32 * 0.34).clamp(12.0, 20.0)).round() as i32;
    let Some(format) = ensure_format(ctx, font_px) else {
        return;
    };

    // 测量与截断（DirectWrite 排版，超宽二分 + 省略号）
    let wide: Vec<u16> = text.encode_utf16().collect();
    let max_w = ((taskbar_w * 35 / 100).max(80)).min(560) as f32;
    let max_h = taskbar_h as f32 * 2.0;
    let Some((layout, text_w, line_h)) = fit_text(ctx, &format, &wide, max_w, max_h) else {
        return;
    };
    let pad = 4i32;
    let draw_w = (text_w.ceil() as i32).max(1);
    let w = draw_w + pad * 2;
    let h = (line_h.ceil() as i32) + 2 * scale(taskbar_h, 2);
    if w <= pad * 2 || h <= 0 {
        return;
    }

    // 锚点（子窗口坐标 = 任务栏客户区坐标）
    let (x, y) = anchor_point(tray, &tray_r, taskbar_w, taskbar_h, w, h);

    let Some(buf) = FrameBuf::new(w, h) else {
        return;
    };
    let Some(rt) = ensure_rt(ctx, buf.dc, w, h) else {
        return;
    };
    let Some(brush) = ensure_brush(ctx, &rt, color) else {
        return;
    };
    rt.BeginDraw();
    // BeginDraw 后背景为透明黑（Clear 冗余但显式表达意图）
    let _ = rt.Clear(None);
    let origin = Vector2 {
        X: pad as f32,
        Y: (h as f32 - line_h) / 2.0,
    };
    let Ok(brush_base) = brush.cast::<ID2D1Brush>() else {
        return;
    };
    rt.DrawTextLayout(origin, &layout, &brush_base, D2D1_DRAW_TEXT_OPTIONS_NONE);
    if let Err(e) = rt.EndDraw(None, None) {
        eprintln!("[taskbar-lyric] D2D 绘制失败: {e}");
        return;
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
        Some(buf.dc),
        Some(&src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
    let _ = ReleaseDC(Some(tray), screen_dc);
    if let Err(e) = res {
        eprintln!("[taskbar-lyric] UpdateLayeredWindow 失败: {e}");
        return;
    }
    let _ = ShowWindow(hwnd, SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0));
    // 保持位于任务栏其它子窗口（XAML 岛）之上；开销可忽略，每次渲染顺手断言
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOP),
        0,
        0,
        0,
        0,
        SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_NOMOVE.0 | SWP_NOSIZE.0),
    );
}

/// 锚点计算：横排主屏 = 托盘区左缘 - 宽 - 间距；横排副屏 = 图标区右缘 + 间距；
/// 竖排 = 停靠侧对侧、底部对齐
unsafe fn anchor_point(
    tray: HWND,
    tray_r: &RECT,
    taskbar_w: i32,
    taskbar_h: i32,
    w: i32,
    h: i32,
) -> (i32, i32) {
    let gap = scale(taskbar_h, 8);
    let inset = scale(taskbar_h, 4);
    if taskbar_w < taskbar_h {
        // 竖排：停靠侧对侧、底部对齐
        let docked_left = tray_r.left <= 0;
        let x = if docked_left { taskbar_w - w - gap } else { gap };
        return (x.max(inset), taskbar_h - h - gap);
    }
    if let Some(tn) = find_tray_notify(tray) {
        // 主屏（有托盘区窗口）：托盘区左缘
        let mut tn_r = RECT::default();
        let _ = GetWindowRect(tn, &mut tn_r);
        let mut pt = POINT { x: tn_r.left, y: 0 };
        let _ = ScreenToClient(tray, &mut pt);
        return ((pt.x - w - gap).max(inset), (taskbar_h - h) / 2);
    }
    // 副屏：右缘是镜像的系统时钟（XAML 绘制，无锚定窗口；图标区宿主
    // WorkerW 的矩形不随 XAML 布局更新，不可作为锚点）。
    // 用 UIA 实测时钟元素左缘，歌词紧贴其左侧（仅留常规间距）；
    // 测量在后台线程进行（实测一次可达数秒，绝不能阻塞渲染线程），
    // 未落地前按固定预留回落（贴近实测结果，落位几乎无跳动）
    let zone_screen_x = secondary_zone_left(tray, tray_r)
        .unwrap_or_else(|| tray_r.right - scale(taskbar_h, 96));
    let mut pt = POINT { x: zone_screen_x, y: 0 };
    let _ = ScreenToClient(tray, &mut pt);
    let x = (pt.x - w - gap).min(taskbar_w - w - inset);
    (x.max(inset), (taskbar_h - h) / 2)
}

/// 副屏时钟区测量缓存（后台线程写入，渲染线程只读）
struct ZoneCache {
    tray: isize,
    at: Instant,
    zone_left: i32,
}

static ZONE_CACHE: Mutex<Option<ZoneCache>> = Mutex::new(None);

static ZONE_WORKER_BUSY: AtomicBool = AtomicBool::new(false);

/// 副屏任务栏右侧"时钟/通知区"的左边界（屏幕坐标）。
/// UIA 遍历任务栏元素树取右带内最靠左的真实元素——实测一次可达数秒，
/// 因此测量在后台线程执行（60s 缓存 + 单飞），渲染线程绝不等待：
/// 未就绪时返回 None，调用方用主屏托盘宽度/固定值回落，就绪后
/// 200ms 定时器自动重锚定到精确位置。
/// 取"右带内最靠左元素"意味着图标拥挤时最后的应用图标也会计入，
/// 歌词会被推到图标左侧，避免遮挡。
unsafe fn secondary_zone_left(tray: HWND, tray_r: &RECT) -> Option<i32> {
    const TTL: Duration = Duration::from_secs(60);
    let key = tray.0 as isize;
    let cached = ZONE_CACHE
        .lock()
        .as_ref()
        .filter(|c| c.tray == key)
        .map(|c| (c.zone_left, c.at.elapsed() < TTL));
    match cached {
        Some((zone, fresh)) => {
            if !fresh {
                request_zone_measure(tray, tray_r);
            }
            // 过期值仍继续使用（SWR）：位置保持连续，后台刷新后就位
            Some(zone)
        }
        None => {
            // 从未测过：先走回落估算，测量落地后 200ms 定时器重锚定
            request_zone_measure(tray, tray_r);
            None
        }
    }
}

/// 请求后台测量（单飞；UIA 遍历可达数秒，绝不能阻塞渲染线程）
fn request_zone_measure(tray: HWND, tray_r: &RECT) {
    if ZONE_WORKER_BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let tray_ptr = tray.0 as isize;
    let rect = RECT {
        left: tray_r.left,
        top: tray_r.top,
        right: tray_r.right,
        bottom: tray_r.bottom,
    };
    let spawned = std::thread::Builder::new()
        .name("taskbar-lyric-zone".into())
        .spawn(move || unsafe { measure_zone_worker(HWND(tray_ptr as *mut _), rect) });
    if spawned.is_err() {
        ZONE_WORKER_BUSY.store(false, Ordering::SeqCst);
    }
}

/// 后台测量线程：UIA 遍历任务栏树（可耗时数秒），结果写入缓存并唤醒渲染
unsafe fn measure_zone_worker(tray: HWND, tray_r: RECT) {
    let t0 = Instant::now();
    let zone = measure_zone_uia(tray, &tray_r);
    let elapsed = t0.elapsed();
    if let Some(zone) = zone {
        *ZONE_CACHE.lock() = Some(ZoneCache {
            tray: tray.0 as isize,
            at: Instant::now(),
            zone_left: zone,
        });
    }
    ZONE_WORKER_BUSY.store(false, Ordering::SeqCst);
    eprintln!(
        "[taskbar-lyric] 副屏时钟区测量 {:?} → {:?}",
        elapsed,
        zone.map(|z| z.to_string()).unwrap_or_else(|| "失败".into())
    );
    wake_render();
}

/// UIA 遍历任务栏元素树，取位于右侧 420px（按任务栏高度缩放）带内、
/// 非通栏（宽 < 40% 任务栏）、非自身歌词窗口的元素中最靠左者
unsafe fn measure_zone_uia(tray: HWND, tray_r: &RECT) -> Option<i32> {
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    let automation: IUIAutomation =
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
    let root = automation.ElementFromHandle(tray).ok()?;
    let cond = automation.CreateTrueCondition().ok()?;
    let all = root.FindAll(TreeScope_Descendants, &cond).ok()?;

    let taskbar_w = tray_r.right - tray_r.left;
    let band = scale(tray_r.bottom - tray_r.top, 420);
    let ours: Vec<isize> = LYRIC_WINDOWS.lock().iter().map(|(_, l)| *l).collect();
    let mut zone_left: Option<i32> = None;
    let n = all.Length().ok()?;
    for i in 0..n {
        let Ok(e) = all.GetElement(i) else { continue };
        let Ok(r) = e.CurrentBoundingRectangle() else { continue };
        let (l, t, rgt, b) = (r.left, r.top, r.right, r.bottom);
        if rgt <= l || b <= t {
            continue;
        }
        // 跳过自身歌词窗口（避免测量到自己的位置形成反馈回路）
        if e.CurrentNativeWindowHandle()
            .map(|h| ours.contains(&(h.0 as isize)))
            .unwrap_or(false)
        {
            continue;
        }
        // 完全位于任务栏条带内
        if l < tray_r.left || rgt > tray_r.right || t < tray_r.top || b > tray_r.bottom {
            continue;
        }
        // 排除通栏容器（任务栏框架/XAML 岛横跨整条）
        if rgt - l >= taskbar_w * 2 / 5 {
            continue;
        }
        // 仅取右侧带内的元素
        if l < tray_r.right - band {
            continue;
        }
        zone_left = Some(zone_left.map_or(l, |z| z.min(l)));
    }
    zone_left
}

/// 主屏托盘区（TrayNotifyWnd）宽度：副屏时钟预留区的参考值
#[allow(dead_code)]
unsafe fn primary_tray_width() -> Option<i32> {
    let tray = FindWindowW(w!("Shell_TrayWnd"), None)
        .ok()
        .filter(|t| IsWindow(Some(*t)).as_bool())?;
    let tn = find_tray_notify(tray)?;
    let mut r = RECT::default();
    let _ = GetWindowRect(tn, &mut r);
    let width = r.right - r.left;
    (width > 0).then_some(width)
}

unsafe fn find_tray_notify(tray: HWND) -> Option<HWND> {
    FindWindowExW(Some(tray), None, w!("TrayNotifyWnd"), None)
        .ok()
        .filter(|t| IsWindow(Some(*t)).as_bool())
}

fn scale(taskbar_h: i32, v: i32) -> i32 {
    (v as f32 * taskbar_h as f32 / 48.0).round() as i32
}

/// 字体格式缓存：任务栏高度（近似 DPI）变化时重建
unsafe fn ensure_format(ctx: &mut RenderCtx, font_px: i32) -> Option<IDWriteTextFormat> {
    if let Some((f, px)) = &ctx.format {
        if *px == font_px {
            return Some(f.clone());
        }
    }
    let f = ctx
        .dwrite
        .CreateTextFormat(
            w!("Microsoft YaHei UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_px as f32,
            w!(""),
        )
        .ok()?;
    ctx.format = Some((f.clone(), font_px));
    Some(f)
}

/// DC 渲染目标缓存：每帧重新 BindDC（DIB 尺寸随文本变化）
unsafe fn ensure_rt(ctx: &mut RenderCtx, dc: HDC, w: i32, h: i32) -> Option<ID2D1DCRenderTarget> {
    if ctx.rt.is_none() {
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0, // DIP == 物理像素
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        match ctx.d2d.CreateDCRenderTarget(&props) {
            Ok(rt) => {
                let _ = rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_DEFAULT);
                let _ = rt.SetTextRenderingParams(Some(&ctx.params));
                ctx.rt = Some(rt);
            }
            Err(e) => {
                eprintln!("[taskbar-lyric] 创建 D2D 渲染目标失败: {e}");
                return None;
            }
        }
    }
    let rt = ctx.rt.as_ref()?;
    let rect = RECT { left: 0, top: 0, right: w, bottom: h };
    if rt.BindDC(dc, &rect).is_err() {
        return None;
    }
    Some(rt.clone())
}

/// 画刷缓存：颜色变化时重建
unsafe fn ensure_brush(
    ctx: &mut RenderCtx,
    rt: &ID2D1DCRenderTarget,
    color: COLORREF,
) -> Option<ID2D1SolidColorBrush> {
    if ctx.brush.is_none() || ctx.brush_color != color.0 {
        let c = D2D1_COLOR_F {
            r: (color.0 & 0xFF) as f32 / 255.0,
            g: ((color.0 >> 8) & 0xFF) as f32 / 255.0,
            b: ((color.0 >> 16) & 0xFF) as f32 / 255.0,
            a: 1.0,
        };
        match rt.CreateSolidColorBrush(&c, None) {
            Ok(b) => {
                ctx.brush = Some(b.clone());
                ctx.brush_color = color.0;
                return Some(b);
            }
            Err(e) => {
                eprintln!("[taskbar-lyric] 创建画刷失败: {e}");
                return None;
            }
        }
    }
    ctx.brush.clone()
}

/// 排版一帧文本：自然宽度内直接用；超宽二分最长前缀 + "…"。
/// 返回（待绘制布局, 实际宽度 DIP, 行高 DIP）
unsafe fn fit_text(
    ctx: &RenderCtx,
    format: &IDWriteTextFormat,
    text: &[u16],
    max_w: f32,
    max_h: f32,
) -> Option<(IDWriteTextLayout, f32, f32)> {
    let layout = ctx.dwrite.CreateTextLayout(text, format, max_w, max_h).ok()?;
    let mut m = DWRITE_TEXT_METRICS::default();
    layout.GetMetrics(&mut m).ok()?;
    if m.width <= max_w {
        return Some((layout, m.width, m.height));
    }
    let ell: Vec<u16> = "…".encode_utf16().collect();
    let mut lo = 0usize;
    let mut hi = text.len();
    let mut best: Option<(IDWriteTextLayout, f32, f32)> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let mut cand = text[..mid].to_vec();
        cand.extend_from_slice(&ell);
        if let Ok(l) = ctx.dwrite.CreateTextLayout(&cand, format, max_w, max_h) {
            let mut cm = DWRITE_TEXT_METRICS::default();
            if l.GetMetrics(&mut cm).is_ok() && cm.width <= max_w {
                best = Some((l, cm.width, cm.height));
                lo = mid + 1;
                continue;
            }
        }
        if mid == 0 {
            break;
        }
        hi = mid - 1;
    }
    best.or(Some((layout, max_w, m.height)))
}

/// 解析 "#rgb" / "#rrggbb" / "#rrggbbaa"；空/非法 → 默认色（随任务栏深浅）
fn resolve_color(color: Option<&str>) -> COLORREF {
    let Some(c) = color.map(str::trim).filter(|c| !c.is_empty()) else {
        return default_color();
    };
    let hex = c.strip_prefix('#').unwrap_or(c);
    let ok = !hex.is_empty() && hex.len() <= 8 && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if !ok {
        return default_color();
    }
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    COLORREF(match hex.len() {
        3 => {
            let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
            (r * 17) | ((g * 17) << 8) | ((b * 17) << 16)
        }
        6 => ((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF),
        // 带透明度的 #rrggbbaa：忽略 alpha（任务栏文字应不透明），取前 6 位
        _ => {
            let v = v >> 8;
            ((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF)
        }
    })
}

/// 默认文字色：读系统主题（任务栏跟随系统深浅色），浅色 → 深字，深色 → 白字
fn default_color() -> COLORREF {
    let mut light: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let light_theme = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut light as *mut u32 as *mut _),
            Some(&mut size),
        )
        .is_ok()
    };
    COLORREF(if light_theme && light != 0 { 0x000000 } else { 0xFFFFFF })
}

/// 渲染缓冲：内存 DC + 32bpp top-down DIB，drop 时释放全部 GDI 资源。
/// 像素由 DirectWrite/D2D 以预乘 ARGB 写入，与 ULW 的合成语义一致
struct FrameBuf {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
}

impl FrameBuf {
    unsafe fn new(w: i32, h: i32) -> Option<Self> {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB.0;
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = match CreateDIBSection(Some(dc), &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[taskbar-lyric] CreateDIBSection 失败: {e}");
                let _ = DeleteDC(dc);
                return None;
            }
        };
        let old = SelectObject(dc, HGDIOBJ(bmp.0));
        Some(Self { dc, bmp, old })
    }
}

impl Drop for FrameBuf {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bmp.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

// ---------- 显示器列表（供设置界面选择） ----------

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
