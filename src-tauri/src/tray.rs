//! 系统托盘：菜单构建 + 统一的媒体控制转发（托盘/SMTC → 前端）。
//! WebView 挂起中：先唤醒再延迟转发，控制恢复可用；窗口仍隐藏时稍后重新挂起。
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::webview_suspend::{resume_main_webview, schedule_resuspend, show_main};

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
pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let pp = MenuItem::with_id(app, "pp", "播放 / 暂停", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "上一首", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &sep1, &pp, &prev, &next, &sep2, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("missing app icon").clone())
        .tooltip("RustMusic")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "show" => show_main(app),
            "pp" => media_control(app, "toggle", None),
            "prev" => media_control(app, "prev", None),
            "next" => media_control(app, "next", None),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
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
