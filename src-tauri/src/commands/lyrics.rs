//! 本地歌词读取（同目录 .lrc 优先，其次内嵌标签）

use lofty::prelude::*;
use tauri::State;

use crate::db;
use crate::lyrics;
use crate::models::LyricsPayload;
use crate::AppState;
// ---------- 歌词 ----------

#[tauri::command]
pub async fn get_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<LyricsPayload, String> {
    let (path, lrc) = {
        let conn = state.db.lock();
        let path = db::get_track_path(&conn, track_id).ok_or("曲目不存在")?;
        let lrc = db::get_lrc_path(&conn, track_id);
        (path, lrc)
    };

    if let Some(lrc_path) = lrc {
        if let Ok(text) = std::fs::read_to_string(&lrc_path) {
            let p = lyrics::parse(&text);
            if !p.lines.is_empty() {
                return Ok(LyricsPayload::new(p.synced, p.lines));
            }
        }
    }

    // 内嵌歌词
    if let Ok(tagged) = lofty::read_from_path(&path) {
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(text) = tag.get_string(&lofty::tag::ItemKey::Lyrics) {
                let p = lyrics::parse(text);
                if !p.lines.is_empty() {
                    return Ok(LyricsPayload::new(p.synced, p.lines));
                }
            }
        }
    }
    Ok(LyricsPayload::new(false, vec![]))
}
