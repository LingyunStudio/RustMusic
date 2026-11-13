//! 设置：播放音质 / 关闭行为 / 自动更新 / 缓存管理

use serde_json::json;
use tauri::State;

use crate::db;
use crate::models::SettingsPayload;
use crate::AppState;

use super::engine_clone;
#[tauri::command]
pub async fn set_play_quality(
    state: State<'_, AppState>,
    kind: String,
    quality: String,
) -> Result<(), String> {
    let key = match kind.as_str() {

        "qq" => {
            // QQ 音乐官方 App 完整档位：
            // standard(128k) / high(HQ 320k) / lossless(SQ 无损) / hires(Hi-Res 24bit)
            // atmos_2(臻品音质 2.0 SVIP) / atmos(臻品全景声 SVIP) / master(臻品母带 24bit/192kHz SVIP)
            if !matches!(
                quality.as_str(),
                "standard" | "high" | "lossless" | "hires" | "atmos_2" | "atmos" | "master"
            ) {
                return Err(crate::i18n::pick("无效的音质选项", "Invalid audio quality option").into());
            }
            "quality_qq".to_string()
        }
        "netease" => {
            // 网易云官方 App 完整档位（weapi level）：
            // standard(128k) / higher(192k) / high(极高 320k) / lossless(无损) / hires(Hi-Res)
            // jyeffect(高清环绕声 VIP) / sky(沉浸环绕声 SVIP) / jymaster(超清母带 SVIP)
            if !matches!(
                quality.as_str(),
                "standard"
                    | "higher"
                    | "high"
                    | "lossless"
                    | "hires"
                    | "jyeffect"
                    | "sky"
                    | "jymaster"
            ) {
                return Err(crate::i18n::pick("无效的音质选项", "Invalid audio quality option").into());
            }
            "quality_netease".to_string()
        }
        "kugou" => {
            // 酷狗音乐官方 App 完整档位：
            // standard(128k) / high(HQ 320k) / lossless(SQ 无损) / hires(Hi-Res)
            // viper_clear(蝰蛇超清 SVIP) / viper_atmos(蝰蛇全景声 SVIP) / viper_tape(蝰蛇母带 SVIP)
            if !matches!(
                quality.as_str(),
                "standard"
                    | "high"
                    | "lossless"
                    | "hires"
                    | "viper_clear"
                    | "viper_atmos"
                    | "viper_tape"
                    | "master"
            ) {
                return Err(crate::i18n::pick("无效的音质选项", "Invalid audio quality option").into());
            }
            "quality_kugou".to_string()
        }
        _ => return Err(crate::i18n::pick("无效的音源", "Invalid audio source").into()),
    };
    let conn = state.db.lock();
    db::set_setting(&conn, &key, &quality);
    Ok(())
}

/// 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用
#[tauri::command]
pub async fn set_close_action(state: State<'_, AppState>, action: String) -> Result<(), String> {
    if !matches!(action.as_str(), "tray" | "exit") {
        return Err(crate::i18n::pick("无效的关闭行为", "Invalid close window action").into());
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
    // 三源分别的音质（issue 跟进：各源权益/档位不同，分开设置）；
    // 未单独设置过时回落到旧的单一 quality，老用户设置无缝迁移
    let quality_netease = super::source_quality(&conn, "netease");
    let quality_qq = super::source_quality(&conn, "qq");
    let quality_kugou = super::source_quality(&conn, "kugou");
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
    let language = db::get_setting(&conn, "language")
        .map(|s| crate::i18n::normalize_pref(&s).to_string())
        .unwrap_or_else(|| "auto".to_string());
    Ok(SettingsPayload {
        volume,
        speed,
        eq_gains,
        eq_enabled,
        quality,
        quality_netease,
        quality_qq,
        quality_kugou,
        cache_limit,
        close_action,
        auto_update,
        wasapi_exclusive,
        language,
    })
}

/// 设置界面语言（"auto" | "zh" | "en"），并即时同步托盘与桌面歌词窗口标题
#[tauri::command]
pub async fn set_language(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    lang: String,
    system_hint: Option<String>,
) -> Result<String, String> {
    use tauri::{Emitter, Manager};
    let normalized = crate::i18n::normalize_pref(&lang).to_string();
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "language", &normalized);
    }
    if let Some(hint) = system_hint.as_deref() {
        crate::i18n::set_system_hint(Some(hint));
    }
    crate::i18n::set_preference(&normalized);
    crate::tray::refresh_tray_labels(&app);
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.set_title(crate::i18n::pick("桌面歌词", "Desktop Lyrics"));
    }
    let _ = app.emit(
        "app://language-changed",
        serde_json::json!({ "language": normalized }),
    );
    Ok(normalized)
}

/// 同步系统语言提示（macOS/Linux GUI 启动无 LANG 时由前端传入 navigator.language）
#[tauri::command]
pub async fn sync_system_lang(app: tauri::AppHandle, hint: String) -> Result<(), String> {
    use tauri::Manager;
    crate::i18n::set_system_hint(Some(&hint));
    if crate::i18n::get_preference() == "auto" {
        crate::tray::refresh_tray_labels(&app);
        if let Some(w) = app.get_webview_window("desktop-lyrics") {
            let _ = w.set_title(crate::i18n::pick("桌面歌词", "Desktop Lyrics"));
        }
    }
    Ok(())
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
