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

use crate::AppState;

/// 全部命令共用：短暂锁住 AppState 只为克隆 Arc<Engine>，真正的引擎操作
/// 都发生在锁外（Engine 内部以 RwLock/原子量管理状态，见 engine.rs）。
pub(crate) fn engine_clone(state: &State<AppState>) -> std::sync::Arc<crate::engine::Engine> {
    state.engine.lock().clone()
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
