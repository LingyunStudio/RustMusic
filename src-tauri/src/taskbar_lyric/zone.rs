//! 副屏任务栏时钟区左缘测量（UIA，后台线程 + SWR 缓存）。

use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, FindWindowW, IsWindow};
use windows::core::w;
use super::render::scale;
use super::*;

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
pub(super) unsafe fn secondary_zone_left(tray: HWND, tray_r: &RECT) -> Option<i32> {
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
