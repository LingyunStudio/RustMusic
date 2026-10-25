//! 后台监控循环：播放进度事件推送 / SMTC 同步 / 自然播完检测 / 输出设备热切换监听。
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::webview_suspend::{resume_main_webview, schedule_resuspend};
use crate::AppState;

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
pub(crate) fn device_watcher(app: AppHandle) {
    let eng = app.state::<AppState>().engine.clone();
    // 用 WASAPI 完整 FriendlyName 比较（cpal name() 对蓝牙等设备只有「耳机」短词，
    // 不同设备的短名可能相同，会导致变化检测失灵）
    let mut last_default = crate::engine::default_device_display_name();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let now_default = crate::engine::default_device_display_name();
        if now_default == last_default || now_default.is_empty() {
            continue;
        }
        last_default = now_default.clone();
        // 用户固定了设备（且该设备仍存在）时不打扰；跟随系统则自动切换。
        // 匹配走 engine 的双通道规则（全名 + 旧版短名兜底）
        if let Some(pref) = eng.device_preference() {
            if eng.is_known_device(&pref) {
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

        // 任务栏歌词自动换行：WebView 挂起（托盘隐藏）期间前端停摆、
        // 进度事件被丢弃，由本循环按引擎位置直接驱动（行数据由前端
        // 在歌词加载/切歌时整包推送，见 taskbar_lyric::set_lines）
        let fallback = eng
            .current
            .read()
            .as_ref()
            .map(|t| format!("♪ {}", t.title.trim()));
        crate::taskbar_lyric::auto_tick(eng.pos_ms.load(Ordering::Relaxed), fallback);

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
            // 预载下一曲（本库曲目）：引擎直接起播，挂起期间也无缝连续，
            // 前端通过 player://state 事件同步（start() 内部发出）。
            // 预载缺失/起播失败 → 原有编排路径：唤醒前端做切歌决策
            if let Some(info) = eng.take_preload() {
                if let Err(e) = eng.play_file(info) {
                    eprintln!("[monitor] 预载续播失败: {e} → 回落前端编排");
                    emit_to_frontend(&app, "player://ended", serde_json::json!({}), true);
                }
            } else {
                // 队列/循环逻辑在前端：挂起中也要唤醒投递，否则托盘播放不连续
                emit_to_frontend(&app, "player://ended", serde_json::json!({}), true);
            }
        }
        was_active = active || rebuilding || switching;
    }
}
