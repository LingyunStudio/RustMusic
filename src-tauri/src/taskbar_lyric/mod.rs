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

use parking_lot::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::MonitorFromWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowExW, FindWindowW,
    GetAncestor, GetMessageW, GetWindowRect, IsWindow, KillTimer, PostMessageW, RegisterClassExW,
    SetParent, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    CS_HREDRAW, CS_VREDRAW, GA_PARENT, HWND_MESSAGE, HWND_TOP, MSG,
    SET_WINDOW_POS_FLAGS, SHOW_WINDOW_CMD, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE,
    SW_SHOWNOACTIVATE, ULW_ALPHA, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WM_TIMER, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{w, Interface, PCWSTR};


mod displays;
mod render;
mod zone;
pub use displays::{DisplayInfo, list_displays};
use displays::monitor_device_of;

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

/// 前端推送的同步歌词行
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarLine {
    pub time_ms: u64,
    pub text: String,
}

static SPAWNED: AtomicBool = AtomicBool::new(false);

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
        render::init();

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

    let colorref = render::resolve_color(color.as_deref());
    let taskbars = match_taskbars(&display);
    prune_windows(&taskbars);
    for tray in taskbars {
        let Some(hwnd) = ensure_lyric_window(tray) else {
            continue;
        };
        render::render_line(hwnd, tray, &text, colorref);
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

unsafe fn find_tray_notify(tray: HWND) -> Option<HWND> {
    FindWindowExW(Some(tray), None, w!("TrayNotifyWnd"), None)
        .ok()
        .filter(|t| IsWindow(Some(*t)).as_bool())
}
