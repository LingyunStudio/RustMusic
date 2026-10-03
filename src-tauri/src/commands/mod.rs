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
        .map_err(|e| format!("后台任务失败：{e}"))?
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

/// 音质回退提示：设置的档位未被账号权益满足、实际下发更低档时告知用户
/// （静默降级曾让"无损"设置实际播放 128k 而用户无从得知）
fn notify_quality_fallback(
    app: &AppHandle,
    source: &str,
    quality: &str,
    actual_label: &str,
    reason: &str,
) {
    if quality == "standard" {
        return;
    }
    let wanted = match quality {
        "lossless" => "无损",
        "high" => "HQ",
        _ => return,
    };
    if actual_label == wanted {
        return;
    }
    let _ = app.emit(
        "player://quality-fallback",
        serde_json::json!({ "message": format!("{source}：当前以「{actual_label}」音质播放（{reason}）") }),
    );
}
