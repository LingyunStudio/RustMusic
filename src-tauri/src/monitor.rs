//! 后台监控循环：播放进度事件推送 / SMTC 同步 / 自然播完检测 / 输出设备热切换监听。
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::webview_suspend::{resume_main_webview, schedule_resuspend};

/// 发送需要前端处理的事件。挂起中可丢的事件（如进度帧）直接跳过；
/// 必须处理的事件（如“播完自动切歌”）先唤醒 WebView 再延迟投递。
fn emit_to_frontend(app: &AppHandle, event: &str, payload: serde_json::Value, wake: bool) {
    let suspended = app
        .state::<AppState>()
        .webview_suspended
        .load(Ordering::SeqCst);
    if !suspended {
        let _ = app.emit(event, payload);
        return;
    }
    if !wake {
        return;
    }
    resume_main_webview(app, true);
    let app2 = app.clone();
    let event = event.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let _ = app2.emit(&event, payload);
        schedule_resuspend(&app2);
    });
}
/// 监听系统默认输出设备变化（耳机插入/拔出、切换默认设备）：
/// 用户未固定设备时自动重建输出流跟到新默认设备，并通知前端刷新设置页。
/// cpal/Windows 无设备变更回调，用轮询实现（2s 间隔，仅查名字开销可忽略）。
#[allow(deprecated)] // 同 engine::build_output：name() 是设备偏好的持久化键
pub(crate) fn device_watcher(app: AppHandle) {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let eng = app.state::<AppState>().engine.clone();
    let mut last_default: String = rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let now_default: String = rodio::cpal::default_host()
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();
        if now_default == last_default || now_default.is_empty() {
            continue;
        }
        last_default = now_default.clone();
        // 用户固定了设备（且该设备仍存在）时不打扰；跟随系统则自动切换
        if let Some(pref) = eng.device_preference() {
            let still_there = rodio::cpal::default_host()
                .output_devices()
                .map(|mut ds| ds.any(|d| d.name().ok().as_deref() == Some(pref.as_str())))
                .unwrap_or(false);
            if still_there {
                continue;
            }
            eprintln!("[engine] 固定设备「{pref}」已不存在，跟随系统默认");
        }
        eprintln!("[engine] 默认输出设备变更 → 切到 {now_default}");
        if eng.switch_output_device(None).is_ok() {
            let _ = app.emit(
                "device://changed",
                serde_json::json!({ "current": eng.current_device_name() }),
            );
        }
    }
}
pub(crate) fn monitor(app: AppHandle) {
    let eng = app.state::<AppState>().engine.clone();
    let mut was_active = false;
    let mut last_pos_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut last_smtc = std::time::Instant::now() - std::time::Duration::from_secs(1);

    let mut diag = 0u32;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(120));
        let active = eng.is_active();
        let paused = eng.user_paused.load(Ordering::Relaxed);

        diag += 1;
        if cfg!(debug_assertions) && diag % 40 == 0 {
            eprintln!(
                "[monitor] active={} paused={} stopped={} pos={} dur={}",
                active,
                paused,
                eng.stopped.load(Ordering::Relaxed),
                eng.pos_ms.load(Ordering::Relaxed),
                eng.dur_ms.load(Ordering::Relaxed),
            );
        }

        if active && !paused {
            if last_pos_emit.elapsed() >= std::time::Duration::from_millis(250) {
                last_pos_emit = std::time::Instant::now();
                let payload = serde_json::json!({
                    "pos": eng.pos_ms.load(Ordering::Relaxed),
                    "dur": eng.dur_ms.load(Ordering::Relaxed),
                });
                // WebView 挂起时进度帧可丢（恢复后由 resync 补发），不值得唤醒
                emit_to_frontend(&app, "player://pos", payload, false);
            }
            // 同步系统媒体浮窗（SMTC）进度，每秒刷新一次
            if last_smtc.elapsed() >= std::time::Duration::from_secs(1) {
                last_smtc = std::time::Instant::now();
                eng.notify_smtc_pos();
            }
        }

        // 一首曲目自然播完（非暂停、非手动停止；FLAC 重建 / 换曲瞬间 sink 短暂为空，跳过）
        let rebuilding = eng.rebuilding.load(Ordering::Relaxed);
        let switching = eng.switching.load(Ordering::Relaxed);
        if was_active
            && !active
            && !paused
            && !eng.stopped.load(Ordering::Relaxed)
            && !rebuilding
            && !switching
        {
            // 队列/循环逻辑在前端：挂起中也要唤醒投递，否则托盘播放不连续
            emit_to_frontend(&app, "player://ended", serde_json::json!({}), true);
        }
        was_active = active || rebuilding || switching;
    }
}
