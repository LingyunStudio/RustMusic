//! 本地歌词读取（同目录 .lrc 优先，其次内嵌标签）

use lofty::prelude::*;
use tauri::State;

use crate::db;
use crate::lyrics;
use crate::models::LyricsPayload;
use crate::AppState;

/// 读取文本文件：UTF-8（剥 BOM）优先，非法 UTF-8 时按 GB18030 解码
/// （GBK 的超集；Windows 记事本时代的中文 .lrc / 外部工具导出的 m3u
/// 大量是 GBK——read_to_string 遇到会直接报错，内容静默消失）。
/// .lrc 歌词与 m3u 歌单导入共用
pub(crate) fn read_text_file_decoded(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let bytes = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]) {
        Some(rest) => rest.to_vec(),
        None => bytes,
    };
    if let Ok(text) = String::from_utf8(bytes.clone()) {
        return Some(text);
    }
    let (decoded, _, had_errors) = encoding_rs::GB18030.decode(&bytes);
    if had_errors {
        return None;
    }
    Some(decoded.into_owned())
}

fn read_lrc_text(path: &std::path::Path) -> Option<String> {
    read_text_file_decoded(path)
}

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
        if let Some(text) = read_lrc_text(std::path::Path::new(&lrc_path)) {
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
