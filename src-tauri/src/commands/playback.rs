//! 播放控制：本地曲目播放 / 播放状态 / 音量、倍速、均衡器 / 输出设备

use serde_json::json;
use tauri::State;

use crate::db;
use crate::engine::TrackInfo;
use crate::AppState;

use super::engine_clone;
// ---------- 输出设备 ----------

/// 枚举输出设备 + 当前生效的设备名
#[tauri::command]
pub async fn list_output_devices(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let engine = engine_clone(&state);
    let devices = engine.list_output_devices();
    Ok(json!({
        "devices": devices,
        "current": engine.current_device_name(),
        "preference": engine.device_preference(),
    }))
}

/// 切换输出设备；name 为空 = 跟随系统默认（并持久化偏好）
#[tauri::command]
pub async fn set_output_device(
    state: State<'_, AppState>,
    name: Option<String>,
) -> Result<(), String> {
    let engine = engine_clone(&state);
    let pref = name.as_deref().filter(|s| !s.is_empty());
    engine.switch_output_device(pref)?;
    let conn = state.db.lock();
    db::set_setting(&conn, "output_device", pref.unwrap_or(""));
    Ok(())
}
// ---------- 播放控制 ----------

#[tauri::command]
pub async fn play_track(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let meta = {
        let conn = state.db.lock();
        db::get_track(&conn, id).ok_or("曲目不存在")?
    };
    let local_quality = if meta.bit_depth >= 16 && meta.sample_rate >= 44100 {
        format!(
            "{}kHz/{}bit",
            meta.sample_rate / 1000,
            meta.bit_depth.max(16)
        )
    } else if meta.bitrate > 0 {
        format!("{}kbps", meta.bitrate / 1000)
    } else {
        String::new()
    };
    let info = TrackInfo {
        id: Some(meta.id),
        kind: "track".into(),
        path: meta.path,
        title: meta.title,
        artist: meta.artist,
        album: meta.album,
        cover: meta.cover,
        duration_ms: (meta.duration * 1000.0) as u64,
        nid: None,
        qid: None,
        kgid: None,
        quality: (!local_quality.is_empty()).then_some(local_quality),
    };
    let r = engine_clone(&state).play_file(info);
    // 开播成功才计入播放次数/最近播放：文件损坏等播放失败不计
    if r.is_ok() {
        let conn = state.db.lock();
        db::record_play(&conn, id);
    }
    r
}
/// WASAPI 独占模式开关（切换后下一首生效）
#[tauri::command]
pub async fn set_wasapi_exclusive(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let eng = engine_clone(&state);
    eng.set_exclusive_enabled(enabled);
    // 关闭独占时立即停止会话、把设备还给系统混音器，并从当前进度切回共享续播
    if !enabled {
        if let Err(e) = eng.stop_exclusive_resume_shared() {
            eprintln!("[engine] 关闭独占切回共享失败: {e}");
        }
    }
    let conn = state.db.lock();
    db::set_setting(
        &conn,
        "wasapi_exclusive",
        if enabled { "true" } else { "false" },
    );
    Ok(())
}
#[tauri::command]
pub async fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).toggle();
    Ok(())
}

/// 当前播放状态快照（含进度）。WebView 挂起恢复窗口期的事件推送可能丢失，
/// 前端恢复后主动拉取本命令做权威同步，不依赖任何固定延迟的补发。
#[tauri::command]
pub async fn get_play_state(
    state: State<'_, AppState>,
) -> Result<Option<crate::engine::PlayStateSnapshot>, String> {
    Ok(engine_clone(&state).snapshot())
}

#[tauri::command]
pub async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).pause();
    Ok(())
}

#[tauri::command]
pub async fn resume(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).resume();
    Ok(())
}

#[tauri::command]
pub async fn stop(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).stop();
    Ok(())
}

#[tauri::command]
pub async fn seek(state: State<'_, AppState>, ms: u64) -> Result<(), String> {
    engine_clone(&state).seek(ms)
}

#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, v: f32) -> Result<(), String> {
    engine_clone(&state).set_volume(v);
    let conn = state.db.lock();
    db::set_setting(&conn, "volume", &format!("{}", v));
    Ok(())
}

#[tauri::command]
pub async fn set_speed(state: State<'_, AppState>, v: f32) -> Result<(), String> {
    engine_clone(&state).set_speed(v);
    let conn = state.db.lock();
    db::set_setting(&conn, "speed", &format!("{}", v));
    Ok(())
}

#[tauri::command]
pub async fn set_eq(
    state: State<'_, AppState>,
    gains: Vec<f32>,
    enabled: bool,
) -> Result<(), String> {
    if gains.len() != 10 {
        return Err("均衡器需要 10 个频段的增益".into());
    }
    let mut arr = [0f32; 10];
    arr.copy_from_slice(&gains);
    engine_clone(&state).eq.set(arr, enabled);
    let conn = state.db.lock();
    db::set_setting(&conn, "eq_gains", &serde_json::to_string(&gains).unwrap());
    db::set_setting(&conn, "eq_enabled", &enabled.to_string());
    Ok(())
}
