//! 在线音源（URL 直链 / B 站视频）管理与播放

use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::models::SourceItem;
use crate::AppState;

use super::bilibili::spawn_bili_lyric_warmup;
use super::engine_clone;
// ---------- 在线音源 ----------

#[tauri::command]
pub async fn list_sources(state: State<'_, AppState>) -> Result<Vec<SourceItem>, String> {
    let conn = state.db.lock();
    Ok(db::list_sources(&conn))
}

#[tauri::command]
pub async fn add_source(
    state: State<'_, AppState>,
    url: String,
    title: Option<String>,
) -> Result<i64, String> {
    let url = url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("音源地址必须以 http:// 或 https:// 开头".into());
    }
    if url.contains(".m3u8") {
        return Err("暂不支持 m3u8/HLS，请使用音频文件直链".into());
    }
    let conn = state.db.lock();
    if let Some(item) = db::list_sources(&conn).into_iter().find(|s| s.url == url) {
        return Ok(item.id);
    }
    db::add_source(&conn, &url, title.as_deref().unwrap_or(""))
}

#[tauri::command]
pub async fn delete_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    db::delete_source(&conn, id);
    Ok(())
}
#[tauri::command]
pub async fn play_source(app: AppHandle, id: i64) -> Result<(), String> {
    let task_app = app.clone();
    super::blocking(move || play_source_task(&task_app, id)).await
}

/// 同步任务体：B 站源解析音频直链是阻塞网络调用
fn play_source_task(app: &AppHandle, id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    // B 站视频源（bili:// 存储链接 / 原始视频链接）：解析音频直链后走在线缓存播放
    let bili_input = {
        let conn = state.db.lock();
        db::get_source(&conn, id)
            .map(|item| item.url)
            .filter(|u| crate::bilibili::looks_like_bili(u))
    };
    if let Some(url) = bili_input {
        if let Some((audio_url, info)) = crate::bilibili::resolve(&url)? {
            // 字幕与音频并行预取：开播时歌词缓存已就绪
            if let Some(rid) = info.qid.clone() {
                spawn_bili_lyric_warmup(&state, rid);
            }
            // 记录最近播放（rid 与我喜欢/播放列表条目一致：BVxxx-cid）
            {
                let conn = state.db.lock();
                db::record_play_online(
                    &conn,
                    "bilibili",
                    info.qid.as_deref().unwrap_or(url.as_str()),
                    &info.title,
                    &info.artist,
                    &info.album,
                    &info.cover,
                    info.duration_ms as i64,
                    "",
                    false,
                );
            }
            return engine_clone(&state).play_url(audio_url, info);
        }
    }
    let item = {
        let conn = state.db.lock();
        db::get_source(&conn, id).ok_or("音源不存在")?
    };
    let info = TrackInfo {
        id: None,
        kind: "url".into(),
        path: String::new(),
        title: if item.title.is_empty() {
            item.url
                .split('/')
                .next_back()
                .unwrap_or("在线音源")
                .to_string()
        } else {
            item.title.clone()
        },
        artist: "在线音源".into(),
        album: String::new(),
        cover: String::new(),
        duration_ms: 0,
        nid: None,
        qid: None,
        kgid: None,
        quality: None,
    };
    engine_clone(&state).play_url(item.url, info)
}
