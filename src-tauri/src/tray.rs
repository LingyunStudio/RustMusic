//! 系统托盘：菜单构建 + 统一的媒体控制转发（托盘/SMTC → 前端）。
//! WebView 挂起中：先唤醒再延迟转发，控制恢复可用；窗口仍隐藏时稍后重新挂起。
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::AppState;
use crate::webview_suspend::{resume_main_webview, schedule_resuspend, show_main};

static DLYRICS_ITEM: std::sync::Mutex<Option<MenuItem<tauri::Wry>>> = std::sync::Mutex::new(None);

/// 统一的媒体控制转发（托盘/SMTC → 前端）。
/// WebView 挂起中：先唤醒再延迟转发，控制恢复可用；窗口仍隐藏时稍后重新挂起。
pub(crate) fn media_control(app: &AppHandle, action: &str, value: Option<f64>) {
    if action == "show" {
        show_main(app);
        return;
    }
    let suspended = app
        .state::<AppState>()
        .webview_suspended
        .load(Ordering::SeqCst);
    if !suspended {
        let _ = app.emit(
            "media://control",
            serde_json::json!({ "action": action, "value": value }),
        );
        return;
    }
    resume_main_webview(app, true);
    let app2 = app.clone();
    let action = action.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let _ = app2.emit(
            "media://control",
            serde_json::json!({ "action": action, "value": value }),
        );
        schedule_resuspend(&app2);
    });
}

/// 更新托盘菜单中桌面歌词控制项的文本（开启 / 关闭 / 解锁）
pub(crate) fn update_dlyrics_tray(app: &AppHandle) {
    let is_open = app.get_webview_window("desktop-lyrics").is_some();
    let is_locked = app
        .state::<AppState>()
        .dlyrics_locked
        .load(Ordering::SeqCst);

    let text = if !is_open {
        "开启桌面歌词"
    } else if is_locked {
        "解锁桌面歌词"
    } else {
        "关闭桌面歌词"
    };

    if let Some(item) = DLYRICS_ITEM.lock().unwrap().as_ref() {
        let _ = item.set_text(text);
    }
}

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let dlyrics = MenuItem::with_id(app, "dlyrics", "开启桌面歌词", true, None::<&str>)?;
    *DLYRICS_ITEM.lock().unwrap() = Some(dlyrics.clone());
    let sep1 = PredefinedMenuItem::separator(app)?;
    let pp = MenuItem::with_id(app, "pp", "播放 / 暂停", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "上一首", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &dlyrics, &sep1, &pp, &prev, &next, &sep2, &quit])?;

    // 监听桌面歌词锁定与关闭事件，实时更新托盘菜单文字
    let app_lock = app.clone();
    app.listen("dlyrics://lock", move |ev| {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(ev.payload()) {
            let locked = payload.get("locked").and_then(|v| v.as_bool()).unwrap_or(false);
            app_lock
                .state::<AppState>()
                .dlyrics_locked
                .store(locked, Ordering::SeqCst);
            update_dlyrics_tray(&app_lock);
        }
    });

    let app_closed = app.clone();
    app.listen("dlyrics://closed", move |_| {
        app_closed
            .state::<AppState>()
            .dlyrics_locked
            .store(false, Ordering::SeqCst);
        update_dlyrics_tray(&app_closed);
    });

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("missing app icon").clone())
        .tooltip("RustMusic")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "show" => show_main(app),
            "dlyrics" => media_control(app, "dlyrics_toggle", None),
            "pp" => media_control(app, "toggle", None),
            "prev" => media_control(app, "prev", None),
            "next" => media_control(app, "next", None),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            update_dlyrics_tray(tray.app_handle());
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
