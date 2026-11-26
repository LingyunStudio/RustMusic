//! 本地歌词读取、在线三源自动匹配、内部缓存与可选导出（同目录 .lrc / 内嵌音频标签）

use lofty::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LyricCandidate {
    pub source: String, // "netease" | "qq" | "kugou"
    pub rid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub score: i32,
}

fn norm_match_text(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(c))
        .collect()
}

fn score_candidate(
    target_title: &str,
    target_artist: &str,
    target_dur_ms: u64,
    cand_title: &str,
    cand_artist: &str,
    cand_dur_ms: u64,
) -> i32 {
    let mut score = 0i32;
    let tt = norm_match_text(target_title);
    let ct = norm_match_text(cand_title);
    if !tt.is_empty() && !ct.is_empty() {
        if ct == tt {
            score += 60;
        } else if ct.contains(&tt) || tt.contains(&ct) {
            score += 36;
        }
    }

    let ta = norm_match_text(target_artist);
    let ca = norm_match_text(cand_artist);
    if !ta.is_empty() && !ca.is_empty() && ta != "unknownartist" && ta != "未知艺术家" {
        if ca == ta {
            score += 35;
        } else if ca.contains(&ta) || ta.contains(&ca) {
            score += 22;
        }
    }

    if target_dur_ms > 0 && cand_dur_ms > 0 {
        let diff = target_dur_ms.abs_diff(cand_dur_ms);
        if diff <= 2_000 {
            score += 30;
        } else if diff <= 5_000 {
            score += 18;
        } else if diff <= 12_000 {
            score += 6;
        } else if diff > 30_000 {
            score -= 20;
        }
    }

    // 轻度降权伴奏/Live/DJ（除非原曲标题本身包含这些关键词）
    let low_c = cand_title.to_lowercase();
    let low_t = target_title.to_lowercase();
    for kw in ["伴奏", "instrumental", "live", "dj", "remix", "翻唱", "cover"] {
        if low_c.contains(kw) && !low_t.contains(kw) {
            score -= 18;
        }
    }
    score
}

/// 并发搜索网易云 / QQ / 酷狗三源候选并按匹配度排序
fn search_candidates_blocking(
    keyword: &str,
    target_title: &str,
    target_artist: &str,
    target_dur_ms: u64,
    music_u: Option<String>,
) -> Vec<LyricCandidate> {
    let kw = keyword.trim();
    if kw.is_empty() {
        return vec![];
    }
    let kw1 = kw.to_string();
    let kw2 = kw.to_string();
    let kw3 = kw.to_string();

    let h_net = std::thread::spawn(move || {
        crate::netease::search(&kw1, 6, 0, music_u.as_deref())
            .map(|r| r.songs)
            .unwrap_or_default()
    });
    let h_qq = std::thread::spawn(move || crate::qq::search(&kw2, 6, 1).unwrap_or_default());
    let h_kg = std::thread::spawn(move || crate::kugou::search(&kw3, 1).unwrap_or_default());

    let mut out = Vec::new();

    if let Ok(songs) = h_net.join() {
        for s in songs.into_iter().take(6) {
            let artist = s.artist_str();
            let album = s.album_name();
            let dur_ms = s.duration_ms().max(0) as u64;
            let score = score_candidate(
                target_title,
                target_artist,
                target_dur_ms,
                &s.name,
                &artist,
                dur_ms,
            );
            out.push(LyricCandidate {
                source: "netease".into(),
                rid: s.id.to_string(),
                title: s.name,
                artist,
                album,
                duration_ms: dur_ms,
                score,
            });
        }
    }
    if let Ok(songs) = h_qq.join() {
        for s in songs.into_iter().take(6) {
            let score = score_candidate(
                target_title,
                target_artist,
                target_dur_ms,
                &s.name,
                &s.singer,
                s.duration_ms,
            );
            out.push(LyricCandidate {
                source: "qq".into(),
                rid: s.id,
                title: s.name,
                artist: s.singer,
                album: s.album,
                duration_ms: s.duration_ms,
                score,
            });
        }
    }
    if let Ok(songs) = h_kg.join() {
        for s in songs.into_iter().take(6) {
            let score = score_candidate(
                target_title,
                target_artist,
                target_dur_ms,
                &s.name,
                &s.singer,
                s.duration_ms,
            );
            out.push(LyricCandidate {
                source: "kugou".into(),
                rid: s.id,
                title: s.name,
                artist: s.singer,
                album: s.album,
                duration_ms: s.duration_ms,
                score,
            });
        }
    }

    out.sort_by(|a, b| b.score.cmp(&a.score));
    out
}

/// 按来源拉取单条候选的完整歌词（优先逐字 + 翻译）
fn fetch_candidate_lyric_blocking(
    app: &AppHandle,
    source: &str,
    rid: &str,
    music_u: Option<&str>,
) -> Result<LyricsPayload, String> {
    match source {
        "netease" => {
            let id: i64 = rid
                .parse()
                .map_err(|_| crate::i18n::pick("无效的网易云歌曲 ID", "Invalid NetEase track ID").to_string())?;
            if let Ok(Some(t)) = crate::netease::lyric_yrc(id, music_u) {
                let mut p = lyrics::parse(&t.lrc);
                if p.synced {
                    if let Some(tr) = &t.trans {
                        lyrics::attach_translations(&mut p, tr);
                    }
                    let mut payload = LyricsPayload::new(true, p.lines);
                    payload.source = Some("匹配 · 网易云 YRC".into());
                    return Ok(payload);
                }
            }
            let Some(t) = crate::netease::lyric(id, music_u)? else {
                return Ok(LyricsPayload::new(false, vec![]));
            };
            let mut p = lyrics::parse(&t.lrc);
            if let Some(tr) = &t.trans {
                lyrics::attach_translations(&mut p, tr);
            }
            let mut payload = LyricsPayload::new(p.synced, p.lines);
            if p.synced {
                payload.source = Some("匹配 · 网易云 LRC".into());
            }
            Ok(payload)
        }
        "qq" => {
            let st = app.state::<AppState>();
            let has_login = super::qq::load_qq_session(&st).is_some();
            if has_login {
                if let Ok(Some((enhanced, trans))) =
                    super::qq::qq_with_auto_refresh(app, |musicid, musickey| {
                        crate::qq::lyric_qrc(rid, musicid, musickey)
                    })
                {
                    let mut p = lyrics::parse(&enhanced);
                    if p.synced {
                        if let Some(tr) = &trans {
                            lyrics::attach_translations(&mut p, tr);
                        }
                        let mut payload = LyricsPayload::new(true, p.lines);
                        payload.source = Some("匹配 · QQ QRC".into());
                        return Ok(payload);
                    }
                }
            }
            let Some(lt) = crate::qq::lyric(rid)? else {
                return Ok(LyricsPayload::new(false, vec![]));
            };
            let mut p = lyrics::parse(&lt.lrc);
            if let Some(tr) = &lt.trans {
                lyrics::attach_translations(&mut p, tr);
            }
            let mut payload = LyricsPayload::new(p.synced, p.lines);
            if p.synced {
                payload.source = Some("匹配 · QQ LRC".into());
            }
            Ok(payload)
        }
        "kugou" => {
            let Some(text) = crate::kugou::lyric(rid)? else {
                return Ok(LyricsPayload::new(false, vec![]));
            };
            if text.is_empty() {
                return Ok(LyricsPayload::new(false, vec![]));
            }
            let p = lyrics::parse(&text);
            let word_level = p.lines.iter().any(|l| l.words.is_some());
            let mut payload = LyricsPayload::new(p.synced, p.lines);
            if p.synced {
                payload.source = Some(
                    if word_level {
                        "匹配 · 酷狗 KRC"
                    } else {
                        "匹配 · 酷狗 LRC"
                    }
                    .into(),
                );
            }
            Ok(payload)
        }
        _ => Err(crate::i18n::pick("不支持的歌词来源", "Unsupported lyric source").into()),
    }
}

/// 将 LyricsPayload 转为标准 LRC 文本（供导出同目录 .lrc 或内嵌标签）
fn payload_to_lrc(payload: &LyricsPayload) -> String {
    let mut out = String::new();
    for line in &payload.lines {
        if let Some(ms) = line.time_ms {
            let total_cs = ms / 10;
            let cs = total_cs % 100;
            let total_sec = total_cs / 100;
            let sec = total_sec % 60;
            let min = total_sec / 60;
            out.push_str(&format!("[{min:02}:{sec:02}.{cs:02}]{}\n", line.text));
            if let Some(tr) = line.trans.as_deref().filter(|s| !s.trim().is_empty()) {
                out.push_str(&format!("[{min:02}:{sec:02}.{cs:02}]{tr}\n"));
            }
        } else {
            out.push_str(&line.text);
            out.push('\n');
        }
    }
    out
}

fn netease_cookie_opt(state: &State<'_, AppState>) -> Option<String> {
    let conn = state.db.lock();
    db::get_setting(&conn, "netease_cookie").filter(|s| !s.is_empty())
}

// ---------- 歌词命令 ----------

#[tauri::command]
pub async fn get_lyrics(
    app: AppHandle,
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<LyricsPayload, String> {
    let (path, lrc, title, artist, dur_sec, manual_cached, auto_cached, music_u) = {
        let conn = state.db.lock();
        let (path, lrc, title, artist, dur_sec) = db::get_track_lyric_info(&conn, track_id)
            .ok_or_else(|| crate::i18n::pick("曲目不存在", "Track not found"))?;
        let tid = track_id.to_string();
        let manual = db::get_lyrics_cache(&conn, "local_manual", &tid);
        let auto = db::get_lyrics_cache(&conn, "local_auto", &tid);
        let music_u = db::get_setting(&conn, "netease_cookie").filter(|s| !s.is_empty());
        (path, lrc, title, artist, dur_sec, manual, auto, music_u)
    };

    // 1. 用户手动匹配/指定的歌词优先
    if let Some(json) = manual_cached {
        if let Ok(p) = serde_json::from_str::<LyricsPayload>(&json) {
            if !p.lines.is_empty() {
                return Ok(p);
            }
        }
    }

    // 2. 已登记的同目录 .lrc
    if let Some(lrc_path) = lrc {
        if let Some(text) = read_lrc_text(std::path::Path::new(&lrc_path)) {
            let p = lyrics::parse(&text);
            if !p.lines.is_empty() {
                return Ok(LyricsPayload::new(p.synced, p.lines));
            }
        }
    }

    // 3. 实时检查同目录同名 .lrc（用户后补 .lrc 时兜底读取并回填 tracks.lrc_path）
    let sidecar = std::path::Path::new(&path).with_extension("lrc");
    if sidecar.exists() {
        if let Some(text) = read_lrc_text(&sidecar) {
            let p = lyrics::parse(&text);
            if !p.lines.is_empty() {
                let conn = state.db.lock();
                let _ = conn.execute(
                    "UPDATE tracks SET lrc_path = ?1 WHERE id = ?2",
                    rusqlite::params![sidecar.to_string_lossy().as_ref(), track_id],
                );
                return Ok(LyricsPayload::new(p.synced, p.lines));
            }
        }
    }

    // 4. 音频内嵌标签歌词
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

    // 5. 软件内部自动匹配缓存（离线秒开、零污染用户音乐目录和原始音频）
    if let Some(json) = auto_cached {
        if let Ok(p) = serde_json::from_str::<LyricsPayload>(&json) {
            if !p.lines.is_empty() {
                return Ok(p);
            }
        }
    }

    // 6. 本地无任何歌词 → 自动在网易云 / QQ / 酷狗三源匹配最佳同步歌词并缓存
    let clean_title = title.trim().to_string();
    if clean_title.is_empty() {
        return Ok(LyricsPayload::new(false, vec![]));
    }
    let clean_artist = if artist == "未知艺术家" || artist.eq_ignore_ascii_case("Unknown Artist") {
        String::new()
    } else {
        artist.trim().to_string()
    };
    let keyword = if clean_artist.is_empty() {
        clean_title.clone()
    } else {
        format!("{clean_title} {clean_artist}")
    };
    let dur_ms = (dur_sec.max(0.0) * 1000.0) as u64;
    let task_app = app.clone();

    let matched = super::blocking(move || -> Result<Option<LyricsPayload>, String> {
        let cands = search_candidates_blocking(
            &keyword,
            &clean_title,
            &clean_artist,
            dur_ms,
            music_u.clone(),
        );
        // 至少要有一定匹配分才自动采用（避免毫不相干的胡乱匹配）
        for c in cands.into_iter().filter(|c| c.score >= 30).take(4) {
            if let Ok(p) =
                fetch_candidate_lyric_blocking(&task_app, &c.source, &c.rid, music_u.as_deref())
            {
                if p.synced && !p.lines.is_empty() {
                    return Ok(Some(p));
                }
            }
        }
        Ok(None)
    })
    .await
    .ok()
    .flatten();

    if let Some(payload) = matched {
        if let Ok(json) = serde_json::to_string(&payload) {
            let conn = state.db.lock();
            db::set_lyrics_cache(&conn, "local_auto", &track_id.to_string(), &json);
        }
        return Ok(payload);
    }

    Ok(LyricsPayload::new(false, vec![]))
}

#[tauri::command]
pub async fn search_local_lyric_candidates(
    state: State<'_, AppState>,
    keyword: String,
    title: String,
    artist: String,
    duration_ms: u64,
) -> Result<Vec<LyricCandidate>, String> {
    let music_u = netease_cookie_opt(&state);
    super::blocking(move || {
        Ok(search_candidates_blocking(&keyword, &title, &artist, duration_ms, music_u))
    })
    .await
}

#[tauri::command]
pub async fn apply_local_lyric_candidate(
    app: AppHandle,
    state: State<'_, AppState>,
    track_id: i64,
    source: String,
    rid: String,
) -> Result<LyricsPayload, String> {
    let music_u = netease_cookie_opt(&state);
    let task_app = app.clone();
    let payload = super::blocking(move || {
        fetch_candidate_lyric_blocking(&task_app, &source, &rid, music_u.as_deref())
    })
    .await?;
    if payload.lines.is_empty() {
        return Err(crate::i18n::pick("该候选暂无歌词", "No lyrics found for this candidate").into());
    }
    if let Ok(json) = serde_json::to_string(&payload) {
        let conn = state.db.lock();
        db::set_lyrics_cache(&conn, "local_manual", &track_id.to_string(), &json);
    }
    Ok(payload)
}

#[tauri::command]
pub async fn clear_local_lyric_match(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    let tid = track_id.to_string();
    db::delete_lyrics_cache(&conn, "local_manual", &tid);
    db::delete_lyrics_cache(&conn, "local_auto", &tid);
    Ok(())
}

#[tauri::command]
pub async fn export_local_lyric(
    state: State<'_, AppState>,
    track_id: i64,
    mode: String,
    payload: LyricsPayload,
) -> Result<String, String> {
    if payload.lines.is_empty() {
        return Err(crate::i18n::pick("当前没有可导出的歌词", "No lyrics to export").into());
    }
    let path = {
        let conn = state.db.lock();
        db::get_track_path(&conn, track_id)
            .ok_or_else(|| crate::i18n::pick("曲目不存在", "Track not found"))?
    };
    let lrc_text = payload_to_lrc(&payload);

    match mode.as_str() {
        "sidecar" => {
            let dest = std::path::Path::new(&path).with_extension("lrc");
            std::fs::write(&dest, lrc_text.as_bytes()).map_err(|e| {
                crate::i18n::pick(
                    format!("写入 .lrc 失败: {e}"),
                    format!("Failed to write .lrc: {e}"),
                )
            })?;
            let dest_str = dest.to_string_lossy().into_owned();
            let conn = state.db.lock();
            let _ = conn.execute(
                "UPDATE tracks SET lrc_path = ?1 WHERE id = ?2",
                rusqlite::params![dest_str, track_id],
            );
            Ok(dest_str)
        }
        "embed" => {
            let ext = std::path::Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "wav" | "dsf" | "dff" | "aiff" | "aif") {
                return Err(crate::i18n::pick(
                    format!(".{ext} 格式不支持标准内嵌歌词，建议保存为同目录 .lrc"),
                    format!(".{ext} does not support embedded lyrics; save as sidecar .lrc instead"),
                ));
            }
            let path_clone = path.clone();
            super::blocking(move || -> Result<(), String> {
                let mut tagged = lofty::read_from_path(&path_clone).map_err(|e| e.to_string())?;
                let tag_type = tagged.primary_tag_type();
                let tag = match tagged.primary_tag_mut() {
                    Some(t) => t,
                    None => {
                        tagged.insert_tag(lofty::tag::Tag::new(tag_type));
                        tagged
                            .primary_tag_mut()
                            .ok_or_else(|| crate::i18n::pick("无法创建音频标签", "Failed to create audio tag").to_string())?
                    }
                };
                tag.insert_text(lofty::tag::ItemKey::Lyrics, lrc_text);
                let mut file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path_clone)
                    .map_err(|e| e.to_string())?;
                tagged
                    .save_to(&mut file, lofty::config::WriteOptions::default())
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
            .await?;
            Ok(path)
        }
        _ => Err(crate::i18n::pick("未知的导出方式", "Unknown export mode").into()),
    }
}
