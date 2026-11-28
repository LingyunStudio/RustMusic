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
        // 窗口仍在托盘隐藏态（刚被前一个事件短暂唤醒）时，有新关键事件需顺延挂起倒计时
        if wake {
            let hidden = app
                .get_webview_window("main")
                .map(|w| !w.is_visible().unwrap_or(true))
                .unwrap_or(false);
            if hidden {
                schedule_resuspend(app);
            }
        }
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
        let default_changed = !now_default.is_empty() && now_default != last_default;
        if default_changed {
            last_default = now_default.clone();
        }
        // 用户固定了设备时每轮都检查其是否仍在线（拔掉非默认 USB DAC 时系统默认
        // 设备不变，不能依赖 default_changed 才触发回退）
        let pinned_missing = match eng.device_preference() {
            Some(pref) => {
                if eng.is_known_device(&pref) {
                    continue;
                }
                eprintln!("[engine] 固定设备「{pref}」已不存在，跟随系统默认");
                true
            }
            None => false,
        };
        if !default_changed && !pinned_missing {
            continue;
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
        // 先于 is_active() 采样重建/切歌标志，再于判定处二次采样，消除 TOCTOU 误判自然播完
        let busy_before = eng.rebuilding.load(Ordering::Acquire)
            || eng.switching.load(Ordering::Acquire);
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
        let played_pos = eng.played_pos_ms();
        crate::taskbar_lyric::auto_tick(played_pos, fallback);

        diag += 1;
        if cfg!(debug_assertions) && diag % 40 == 0 {
            eprintln!(
                "[monitor] active={} paused={} stopped={} pos={} played={} dur={}",
                active,
                paused,
                eng.stopped.load(Ordering::Relaxed),
                eng.pos_ms.load(Ordering::Relaxed),
                played_pos,
                eng.dur_ms.load(Ordering::Relaxed),
            );
        }

        if active && !paused {
            if last_pos_emit.elapsed() >= std::time::Duration::from_millis(250) {
                last_pos_emit = std::time::Instant::now();
                let payload = serde_json::json!({
                    "pos": eng.played_pos_ms(),
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
        let rebuilding = eng.rebuilding.load(Ordering::Acquire);
        let switching = eng.switching.load(Ordering::Acquire);
        if was_active
            && !active
            && !paused
            && !eng.stopped.load(Ordering::Relaxed)
            && !busy_before
            && !rebuilding
            && !switching
        {
            // 预载下一曲（本库曲目）：引擎直接起播，挂起期间也无缝连续，
            // 前端通过 player://state 事件同步（start() 内部发出）。
            // 预载缺失/起播失败 → 原有编排路径：唤醒前端做切歌决策
            if let Some(info) = eng.take_preload() {
                let preload_id = info.id;
                match eng.play_file(info) {
                    Ok(()) => {
                        // 补记播放统计：预载续播不经 play_track 命令，此前漏写 stats 表
                        if let Some(tid) = preload_id {
                            let st = app.state::<AppState>();
                            let conn = st.db.lock();
                            crate::db::record_play(&conn, tid);
                        }
                        // 清空上一首残留的任务栏歌词行，避免托盘挂起期间继续滚旧歌歌词
                        crate::taskbar_lyric::set_lines(vec![]);
                    }
                    Err(e) => {
                        eprintln!("[monitor] 预载续播失败: {e} → 回落前端编排");
                        emit_to_frontend(&app, "player://ended", serde_json::json!({}), true);
                    }
                }
            } else {
                // 队列/循环逻辑在前端：挂起中也要唤醒投递，否则托盘播放不连续
                emit_to_frontend(&app, "player://ended", serde_json::json!({}), true);
            }
        }
        was_active = active || rebuilding || switching;
    }
}
