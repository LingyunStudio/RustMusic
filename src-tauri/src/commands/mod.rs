//! 命令层：按业务域拆分为子模块，对外仍以 `commands::命令名` 统一暴露
//! （pub use 再导出，main.rs 的 generate_handler! 注册路径保持不变）。
mod app;
mod bilibili;
mod download;
mod kugou;
mod library;
mod lyrics;
mod navidrome;
mod netease;
mod playback;
mod playlists;
mod qq;
mod settings;
mod sources;
mod updater;

pub use app::*;
pub use bilibili::*;
pub use download::*;
pub use kugou::*;
pub use library::*;
pub use lyrics::*;
pub use navidrome::*;
pub use netease::*;
pub use playback::*;
pub use playlists::*;
pub use qq::*;
pub use settings::*;
pub use sources::*;
pub use updater::*;

use tauri::{AppHandle, Emitter, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::AppState;

/// 全部命令共用：克隆引擎句柄。Engine 内部以细粒度 RwLock/原子量管理状态，
/// 命令层不持有任何全局锁（见 engine.rs）
pub(crate) fn engine_clone(state: &State<AppState>) -> std::sync::Arc<crate::engine::Engine> {
    state.engine.clone()
}

/// 在线曲目"最近播放"入库（起播成功后调用）：rid 按 kind 从 TrackInfo 推导
/// （netease=nid、kugou=kgid、其余=qid）。media_mid/vip 单独传——
/// TrackInfo 未携带（仅 QQ 存 media_mid、酷狗存 album_audio_id）。
pub(crate) fn record_online_play(
    state: &State<AppState>,
    info: &TrackInfo,
    media_mid: &str,
    vip: bool,
) {
    let rid = match info.kind.as_str() {
        "netease" => info.nid.map(|n| n.to_string()).unwrap_or_default(),
        "kugou" => info.kgid.clone().unwrap_or_default(),
        _ => info.qid.clone().unwrap_or_default(),
    };
    if rid.is_empty() {
        return;
    }
    let conn = state.db.lock();
    db::record_play_online(
        &conn,
        &info.kind,
        &rid,
        &info.title,
        &info.artist,
        &info.album,
        &info.cover,
        info.duration_ms as i64,
        media_mid,
        vip,
    );
}

/// 同步阻塞任务（网络请求 / 凭据管理器 / 大文件 IO）统一放到阻塞线程池执行：
/// async 命令跑在 tokio 运行时上，直接调用同步网络接口会占死 worker 线程，
/// 并发请求一多整个运行时都会卡顿。返回值已抹平 JoinError。
pub(crate) async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| {
            crate::i18n::pick(
                format!("后台任务失败：{e}"),
                format!("Background task failed: {e}"),
            )
        })?
}
/// 音质标签（在线曲目播放栏徽标）：FLAC → 无损；≥320kbps → HQ；其余 → 标准
fn quality_tag(ext: &str, br_kbps: i64) -> String {
    if ext.eq_ignore_ascii_case("flac") {
        "无损".to_string()
    } else if br_kbps >= 320 {
        "HQ".to_string()
    } else {
        "标准".to_string()
    }
}

/// 三源分别读取播放音质（quality_{kind}），未单独设置过回落旧单一设置
pub(crate) fn source_quality(conn: &rusqlite::Connection, kind: &str) -> String {
    let q = db::get_setting(conn, &format!("quality_{kind}"))
        .or_else(|| db::get_setting(conn, "quality"))
        .unwrap_or_else(|| "high".to_string());
    match (kind, q.as_str()) {
        // 存量兼容：旧版酷狗 master 归一为官方 App 命名的 viper_tape（蝰蛇母带）
        ("kugou", "master") => "viper_tape".to_string(),
        _ => q,
    }
}

pub(crate) fn resolve_quality(_kind: &str, quality: &str) -> String {
    quality.to_string()
}

fn quality_key_rank(quality: &str) -> i32 {
    match quality {
        "jymaster" | "master" | "viper_tape" => 80,
        "sky" | "atmos" | "viper_atmos" => 75,
        "jyeffect" | "atmos_2" | "viper_clear" => 70,
        "hires" => 50,
        "lossless" => 40,
        "high" | "exhigh" => 30,
        "higher" => 20,
        "standard" => 10,
        _ => 0,
    }
}

fn quality_label_rank(label: &str) -> i32 {
    match label {
        "超清母带" | "臻品母带" | "蝰蛇母带" | "超品母带" => 80,
        "沉浸环绕声" | "臻品全景声" | "蝰蛇全景声" => 75,
        "高清环绕声" | "臻品音质 2.0" | "臻品音质2.0" | "蝰蛇超清" => 70,
        "Hi-Res" => 50,
        "无损" => 40,
        "HQ" | "极高" => 30,
        "较高" => 20,
        "标准" => 10,
        _ => 0,
    }
}

/// 音质回退提示：设置的档位未被账号权益或曲目音源满足、实际下发更低档时告知用户
/// （静默降级曾让"无损"设置实际播放 128k 而用户无从得知）
fn notify_quality_fallback(
    app: &AppHandle,
    source: &str,
    quality: &str,
    actual_label: &str,
    reason: &str,
) {
    let wanted_rank = quality_key_rank(quality);
    let actual_rank = quality_label_rank(actual_label);
    // 酷狗高阶母带：酷狗 24bit/96kHz 真正母带级音频流由"蝰蛇超清"承载，用户选"蝰蛇母带"播放"蝰蛇超清"属正常同级匹配，非降级
    if quality == "viper_tape" && actual_label == "蝰蛇超清" {
        return;
    }
    if wanted_rank <= 10 || actual_rank == 0 || actual_rank >= wanted_rank {
        return;
    }
fn quality_display_label(label: &str) -> &str {
    if !crate::i18n::is_en() {
        return label;
    }
    match label {
        "超清母带" | "臻品母带" | "蝰蛇母带" | "超品母带" => "Master",
        "沉浸环绕声" | "臻品全景声" | "蝰蛇全景声" => "Dolby / Spatial",
        "高清环绕声" => "HD Surround",
        "臻品音质 2.0" | "臻品音质2.0" => "Premium 2.0",
        "蝰蛇超清" => "Viper HD",
        "Hi-Res" => "Hi-Res",
        "无损" => "Lossless",
        "HQ" => "HQ",
        "极高" => "Very High",
        "较高" => "Higher",
        "标准" => "Standard",
        other => other,
    }
}

fn source_display_label(source: &str) -> &str {
    if !crate::i18n::is_en() {
        return source;
    }
    match source {
        "网易云" | "网易云音乐" => "NetEase Cloud",
        "QQ 音乐" | "QQ音乐" => "QQ Music",
        "酷狗" | "酷狗音乐" => "KuGou",
        other => other,
    }
}

    let source_disp = source_display_label(source);
    let label_disp = quality_display_label(actual_label);
    let _ = app.emit(
        "player://quality-fallback",
        serde_json::json!({
            "message": crate::i18n::pick(
                format!("{source_disp}：当前以「{label_disp}」音质播放（{reason}）"),
                format!("{source_disp}: playing in \"{label_disp}\" quality ({reason})"),
            )
        }),
    );
}
