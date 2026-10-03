//! 设置：播放音质 / 关闭行为 / 自动更新 / 缓存管理

use serde_json::json;
use tauri::State;

use crate::db;
use crate::models::SettingsPayload;
use crate::AppState;

use super::engine_clone;
#[tauri::command]
pub async fn set_play_quality(state: State<'_, AppState>, quality: String) -> Result<(), String> {
    if !matches!(quality.as_str(), "standard" | "high" | "lossless") {
        return Err("无效的音质选项".into());
    }
    let conn = state.db.lock();
    db::set_setting(&conn, "quality", &quality);
    Ok(())
}

/// 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用
#[tauri::command]
pub async fn set_close_action(state: State<'_, AppState>, action: String) -> Result<(), String> {
    if !matches!(action.as_str(), "tray" | "exit") {
        return Err("无效的关闭行为".into());
    }
    let conn = state.db.lock();
    db::set_setting(&conn, "close_action", &action);
    Ok(())
}
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let conn = state.db.lock();
    let volume: f32 = db::get_setting(&conn, "volume")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.8);
    let speed: f32 = db::get_setting(&conn, "speed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let eq_gains: Vec<f32> = db::get_setting(&conn, "eq_gains")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| vec![0.0; 10]);
    let eq_enabled = db::get_setting(&conn, "eq_enabled")
        .map(|s| s == "true")
        .unwrap_or(false);
    let quality = db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string());
    let cache_limit: u64 = db::get_setting(&conn, "cache_limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2 * 1024 * 1024 * 1024);
    let close_action = db::get_setting(&conn, "close_action").unwrap_or_else(|| "tray".to_string());
    let auto_update = db::get_setting(&conn, "auto_update")
        .map(|s| s != "false")
        .unwrap_or(true);
    let wasapi_exclusive = db::get_setting(&conn, "wasapi_exclusive")
        .map(|s| s == "true")
        .unwrap_or(false);
    Ok(SettingsPayload {
        volume,
        speed,
        eq_gains,
        eq_enabled,
        quality,
        cache_limit,
        close_action,
        auto_update,
        wasapi_exclusive,
    })
}

/// 设置是否在启动时自动检查更新
#[tauri::command]
pub async fn set_auto_update(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "auto_update", if enabled { "true" } else { "false" });
    Ok(())
}
#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<u32, String> {
    Ok(engine_clone(&state).clear_cache())
}

/// 当前缓存占用（bytes）与文件数
#[tauri::command]
pub async fn cache_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (bytes, files) = engine_clone(&state).cache_usage();
    Ok(json!({ "bytes": bytes, "files": files }))
}

/// 设置缓存上限（bytes，0 = 不限制）；超限立即 LRU 清理
#[tauri::command]
pub async fn set_cache_limit(state: State<'_, AppState>, bytes: u64) -> Result<(), String> {
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "cache_limit", &bytes.to_string());
    }
    engine_clone(&state).set_cache_limit(bytes);
    Ok(())
}
