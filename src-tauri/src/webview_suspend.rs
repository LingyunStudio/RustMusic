//! 主窗口 WebView 的挂起/恢复（WebView2 TrySuspend）：托盘隐藏时冻结渲染进程回收内存。
//! 音频由后端 rodio 播放，不依赖 WebView，挂起不影响播放。
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

pub(crate) fn show_main(app: &AppHandle) {
    let was_suspended = app
        .state::<AppState>()
        .webview_suspended
        .load(Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        if !was_suspended {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    if was_suspended {
        // 先恢复 WebView（Resume + SetIsVisible(true)），再延迟显示窗口：
        // 恢复后的前几帧合成器/毛玻璃背景/封面图需要重新采样渲染，
        // 直接 show 会看到 1~2 帧过渡画面（闪屏）。等渲染稳定后再显示，
        // 过渡帧发生在窗口不可见期间，用户看不到。
        resume_main_webview(app, true);
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            if let Some(w) = app2.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        });
    }
}
/// 挂起主窗口 WebView（WebView2 TrySuspend）：托盘隐藏时冻结并释放渲染进程内存。
/// 音频由后端 rodio 播放，不依赖 WebView，挂起不影响播放。
/// 桌面歌词开着时不挂起：悬浮窗的歌词/暂停状态依赖主 WebView 的推送。
pub(crate) fn suspend_main_webview(app: &AppHandle) {
    if app.get_webview_window("desktop-lyrics").is_some() {
        return;
    }
    let st = app.state::<AppState>();
    if st.webview_suspended.swap(true, Ordering::SeqCst) {
        return; // 已挂起
    }
    drop(st);
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if let Err(e) = win.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
        use windows::core::Interface as _;
        let controller = webview.controller();
        unsafe {
            let Ok(core) = controller.CoreWebView2() else {
                eprintln!("[webview] CoreWebView2() 失败");
                return;
            };
            let Ok(cwv3) = core.cast::<ICoreWebView2_3>() else {
                eprintln!("[webview] cast ICoreWebView2_3 失败");
                return;
            };
            // IsVisible=false 是 TrySuspend 的前置条件
            let _ = controller.SetIsVisible(false);
            let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(|hr, ok| {
                eprintln!("[webview] TrySuspend 完成 hr={hr:?} ok={ok:?}");
                if ok {
                    // 冻结后 Windows 仍惰性裁剪工作集（实测半分钟才落到底），
                    // 主动换出物理页让内存即刻回落；进程已冻结，操作安全
                    std::thread::spawn(|| {
                        std::thread::sleep(Duration::from_millis(800));
                        trim_webview_working_sets();
                    });
                }
                Ok(())
            }));
            if let Err(e) = cwv3.TrySuspend(&handler) {
                eprintln!("[webview] 挂起失败: {e}");
            }
        }
    }) {
        eprintln!("[webview] with_webview 失败: {e}");
    }
}
/// 把本应用派生的全部 WebView2 子进程（浏览器/GPU/渲染/实用工具）的工作集
/// 换出到待命列表。TrySuspend 冻结进程后 Windows 要几十秒才惰性裁剪完物理页，
/// EmptyWorkingSet 立即完成这一步；恢复时页面从待命列表软错误换回，代价可忽略
fn trim_webview_working_sets() {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
    };

    let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut webview_procs: Vec<(u32, u32)> = Vec::new(); // (pid, ppid)
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(&entry.szExeFile);
                let name = name.trim_end_matches('\0');
                if name.eq_ignore_ascii_case("msedgewebview2.exe") {
                    webview_procs.push((entry.th32ProcessID, entry.th32ParentProcessID));
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }

    // 归属判定：浏览器进程父 PID = 本应用，渲染/GPU/工具进程父 PID = 浏览器进程
    let my_pid = std::process::id();
    let mut targets: Vec<u32> = webview_procs
        .iter()
        .filter(|(_, ppid)| *ppid == my_pid)
        .map(|(pid, _)| *pid)
        .collect();
    loop {
        let before = targets.len();
        for (pid, ppid) in &webview_procs {
            if !targets.contains(pid) && targets.contains(ppid) {
                targets.push(*pid);
            }
        }
        if targets.len() == before {
            break;
        }
    }

    let rights = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA;
    let mut trimmed = 0;
    for pid in targets {
        if let Ok(h) = unsafe { OpenProcess(rights, false, pid) } {
            unsafe {
                if EmptyWorkingSet(h).is_ok() {
                    trimmed += 1;
                }
                let _ = CloseHandle(h);
            }
        }
    }
    eprintln!("[webview] 工作集裁剪完成，进程数 {trimmed}");
}
/// 恢复主窗口 WebView。notify=true 时补发播放状态/进度并通知前端
/// 刷新数据（挂起期间发往前端的事件都会被丢弃）。
pub(crate) fn resume_main_webview(app: &AppHandle, notify: bool) {
    {
        let st = app.state::<AppState>();
        if !st.webview_suspended.swap(false, Ordering::SeqCst) && !notify {
            return;
        }
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.with_webview(|webview| {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
            use windows::core::Interface as _;
            let controller = webview.controller();
            unsafe {
                let Ok(core) = controller.CoreWebView2() else {
                    return;
                };
                let Ok(cwv3) = core.cast::<ICoreWebView2_3>() else {
                    return;
                };
                let _ = cwv3.Resume();
                let _ = controller.SetIsVisible(true);
            }
        });
    }
    if notify {
        let app2 = app.clone();
        std::thread::spawn(move || {
            // 等 WebView 完全恢复后再补发，事件才不会被丢
            std::thread::sleep(Duration::from_millis(250));
            {
                let st = app2.state::<AppState>();
                st.engine.resync_ui();
            }
            let _ = app2.emit("webview://resumed", serde_json::json!({}));
        });
    }
}
/// 窗口仍隐藏时延迟重新挂起（短暂唤醒处理完托盘操作/换曲后回收内存）
pub(crate) fn schedule_resuspend(app: &AppHandle) {
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(8));
        let hidden = app2
            .get_webview_window("main")
            .map(|w| !w.is_visible().unwrap_or(false))
            .unwrap_or(false);
        if hidden {
            suspend_main_webview(&app2);
        }
    });
}
