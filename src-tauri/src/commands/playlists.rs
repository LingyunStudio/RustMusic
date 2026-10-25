//! 播放列表：增删改 / 排序 / 条目管理（本地 + 在线条目）

use tauri::State;

use crate::db;
use crate::models::Playlist;
use crate::AppState;
// ---------- 播放列表 ----------

/// 在线歌单导入的归一化条目（各平台曲目 → 此结构 → 统一入库）
pub(crate) struct OnlineTrackRow {
    pub rid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration_ms: i64,
    /// QQ 存 media_mid；酷狗存 album_audio_id 字符串（恢复播放时带全取链参数）
    pub media_mid: String,
    pub vip: bool,
}

/// 在线歌单导入的公共入库流程：按远程 id 查找/创建本地歌单 → 单事务内
/// 批量 upsert 在线条目并追加（已存在的去重跳过），返回 (歌单 id, 本次新增条数)。
/// 事务 + position 起始值只查一次：逐条 `MAX(position)+1` 无法走索引，
/// 千首歌单导入曾是 O(n²) 的独立自动提交
pub(crate) fn import_online_tracks(
    conn: &rusqlite::Connection,
    kind: &str,
    remote_pid: &str,
    name: &str,
    tracks: &[OnlineTrackRow],
) -> Result<(i64, i64), String> {
    let pid = match db::find_playlist_by_remote(conn, kind, remote_pid, name) {
        Some(id) => id,
        None => db::create_playlist(conn, name)?,
    };
    db::set_playlist_remote(conn, pid, kind, remote_pid);
    // 记录原始导入名：改名后重导入仍能认出（配合 remote id 兜底）
    db::set_playlist_origin(conn, pid, name);
    if tracks.is_empty() {
        return Ok((pid, 0));
    }
    match conn.unchecked_transaction() {
        Ok(tx) => {
            for t in tracks {
                db::upsert_online_track(
                    &tx,
                    kind,
                    &t.rid,
                    &t.title,
                    &t.artist,
                    &t.album,
                    &t.cover,
                    t.duration_ms,
                    &t.media_mid,
                    t.vip,
                );
            }
            let mut pos: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(position), 0) FROM playlist_tracks WHERE playlist_id = ?1",
                    rusqlite::params![pid],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let mut added = 0i64;
            for t in tracks {
                pos += 1;
                let inserted = tx
                    .execute(
                        "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position, kind, online_id)
                         VALUES(?1, 0, ?2, ?3, ?4)",
                        rusqlite::params![pid, pos, kind, t.rid],
                    )
                    .map(|n| n > 0)
                    .unwrap_or(false);
                if inserted {
                    added += 1;
                }
            }
            tx.commit().map_err(|e| e.to_string())?;
            Ok((pid, added))
        }
        Err(e) => {
            eprintln!("[playlists] 导入事务开启失败，回退逐条写入: {e}");
            let mut added = 0i64;
            for t in tracks {
                db::upsert_online_track(
                    conn,
                    kind,
                    &t.rid,
                    &t.title,
                    &t.artist,
                    &t.album,
                    &t.cover,
                    t.duration_ms,
                    &t.media_mid,
                    t.vip,
                );
                if db::add_online_to_playlist(conn, pid, kind, &t.rid)? {
                    added += 1;
                }
            }
            Ok((pid, added))
        }
    }
}

#[tauri::command]
pub async fn list_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    let conn = state.db.lock();
    Ok(db::list_playlists(&conn))
}

#[tauri::command]
pub async fn create_playlist(state: State<'_, AppState>, name: String) -> Result<i64, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("播放列表名称不能为空".into());
    }
    let conn = state.db.lock();
    db::create_playlist(&conn, &name)
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    db::delete_playlist(&conn, id);
    Ok(())
}

#[tauri::command]
pub async fn delete_playlists(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    db::delete_playlists(&conn, &ids);
    Ok(())
}

#[tauri::command]
pub async fn rename_playlist(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::rename_playlist(&conn, id, &name);
    Ok(())
}

/// 播放列表手动排序（侧边栏长按拖动）：按 id 序列重写 sort_pos
#[tauri::command]
pub async fn reorder_playlists(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    db::reorder_playlists(&conn, &ids);
    Ok(())
}

#[tauri::command]
pub async fn add_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::add_to_playlist(&conn, playlist_id, track_id)
}

/// 批量添加本地曲目到歌单（多选场景）：单事务，position 起始值只查一次
#[tauri::command]
pub async fn add_tracks_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<usize, String> {
    if track_ids.is_empty() {
        return Ok(0);
    }
    let conn = state.db.lock();
    let mut pos: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM playlist_tracks WHERE playlist_id = ?1",
            rusqlite::params![playlist_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mut added = 0usize;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for tid in &track_ids {
        pos += 1;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position) VALUES(?1, ?2, ?3)",
                rusqlite::params![playlist_id, tid, pos],
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        if inserted {
            added += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(added)
}

// ---------- m3u 导入 / 导出 ----------

/// 导出歌单为 .m3u8（UTF-8）。本地曲目写绝对路径；在线条目写
/// `# rustmusic:kind:rid` 注释行（配合 EXTINF 元数据，重导入可还原）
#[tauri::command]
pub async fn export_playlist_m3u(
    state: State<'_, AppState>,
    playlist_id: i64,
    path: String,
) -> Result<usize, String> {
    let entries = {
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare(
                "SELECT pt.kind, COALESCE(t.path, ''), COALESCE(t.title, ot.title, ''),
                        COALESCE(t.artist, ot.artist, ''), COALESCE(t.duration, ot.duration_ms / 1000.0, 0),
                        pt.online_id
                 FROM playlist_tracks pt
                 LEFT JOIN tracks t ON pt.kind = 'local' AND t.id = pt.track_id
                 LEFT JOIN online_tracks ot ON pt.kind != 'local' AND ot.kind = pt.kind AND ot.rid = pt.online_id
                 WHERE pt.playlist_id = ?1
                 ORDER BY pt.position, pt.rowid",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, String, String, String, f64, String)> = stmt
            .query_map(rusqlite::params![playlist_id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        rows
    };
    if entries.is_empty() {
        return Err("歌单是空的，没有可导出的内容".into());
    }
    let mut out = String::from("#EXTM3U\n");
    let mut n = 0usize;
    for (kind, path, title, artist, duration, online_id) in &entries {
        let meta = if artist.is_empty() {
            title.clone()
        } else {
            format!("{artist} - {title}")
        };
        out.push_str(&format!("#EXTINF:{:.0},{meta}\n", duration.max(0.0)));
        if kind == "local" {
            out.push_str(path);
            out.push('\n');
        } else {
            // 在线条目无法用路径表达：注释行承载还原信息
            out.push_str(&format!("# rustmusic:{kind}:{online_id}\n"));
        }
        n += 1;
    }
    std::fs::write(&path, out).map_err(|e| format!("写入文件失败：{e}"))?;
    Ok(n)
}

/// 从 .m3u/.m3u8 导入歌单。本地路径行：命中资料库按既有曲目引用；
/// 磁盘上存在但未入库的音频文件先入库再引用；在线注释行（# rustmusic:kind:rid）
/// 在库里有对应元数据时还原。返回 (歌单 id, 实际导入条数)
#[tauri::command]
pub async fn import_playlist_m3u(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> Result<serde_json::Value, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("播放列表名称不能为空".into());
    }
    let text = super::lyrics::read_text_file_decoded(std::path::Path::new(&path))
        .ok_or_else(|| "读取文件失败（既不是 UTF-8 也不是 GB18030 编码）".to_string())?;
    let mut local_paths: Vec<String> = Vec::new();
    let mut online_refs: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            // 在线条目还原注释
            if let Some(rest) = line.strip_prefix("# rustmusic:") {
                if let Some((kind, rid)) = rest.split_once(':') {
                    if ["netease", "qq", "kugou", "bilibili", "navidrome"].contains(&kind) {
                        online_refs.push((kind.to_string(), rid.to_string()));
                    }
                }
            }
            continue;
        }
        local_paths.push(line.to_string());
    }
    if local_paths.is_empty() && online_refs.is_empty() {
        return Err("文件里没有可导入的条目".into());
    }

    // 本地行解析入库（文件级元数据解析是阻塞 IO，走专用线程池）。
    // 读取带编码回落（UTF-8 BOM / GB18030）：外部工具导出的中文 m3u 常是 GBK
    let paths_for_parse = local_paths.clone();
    let app_data = state.app_data.clone();
    let parsed: Vec<Option<crate::db::NewTrack>> = super::blocking(move || {
        Ok(paths_for_parse
            .iter()
            .map(|p| {
                let pb = std::path::PathBuf::from(p);
                if pb.exists() && pb.is_file() && crate::library::is_audio(&pb) {
                    crate::library::parse_track(&pb, &app_data)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>())
    })
    .await
    .map_err(|e| e.to_string())?;

    let conn = state.db.lock();
    let pid = db::create_playlist(&conn, &name)?;
    let mut pos: i64 = 0;
    let mut added = 0usize;
    let mut missing_files = 0usize;
    // 已入库曲目按路径直接引用；未入库但解析成功的先行入库。
    // 解析结果与 local_paths 按下标严格对齐——此前用迭代器且只在
    // “库中查不到”分支推进，已入库/未入库路径交错时取错元数据、加错歌
    let mut parsed = parsed;
    for (i, p) in local_paths.iter().enumerate() {
        let norm = crate::library::norm_path(std::path::Path::new(p));
        let tid: Option<i64> = match conn.query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            rusqlite::params![norm],
            |r| r.get(0),
        ) {
            Ok(id) => Some(id),
            Err(_) => {
                let t = std::mem::take(&mut parsed[i]);
                match t {
                    Some(t) => {
                        db::upsert_track(&conn, &t);
                        conn.query_row(
                            "SELECT id FROM tracks WHERE path = ?1",
                            rusqlite::params![t.path],
                            |r| r.get(0),
                        )
                        .ok()
                    }
                    None => None,
                }
            }
        };
        pos += 1;
        match tid {
            Some(tid) => {
                // 计数只算真正新增的行（INSERT OR IGNORE 命中已有行时为 0）
                let n = conn
                    .execute(
                        "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position) VALUES(?1, ?2, ?3)",
                        rusqlite::params![pid, tid, pos],
                    )
                    .unwrap_or(0);
                if n > 0 {
                    added += 1;
                }
            }
            None => {
                missing_files += 1;
            }
        }
    }
    // 在线条目：库里有元数据的还原引用（计数同样只算真正新增）
    for (kind, rid) in &online_refs {
        let known: bool = conn
            .query_row(
                "SELECT 1 FROM online_tracks WHERE kind = ?1 AND rid = ?2",
                rusqlite::params![kind, rid],
                |_| Ok(()),
            )
            .is_ok();
        if known {
            pos += 1;
            let n = conn
                .execute(
                    "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position, kind, online_id)
                     VALUES(?1, 0, ?2, ?3, ?4)",
                    rusqlite::params![pid, pos, kind, rid],
                )
                .unwrap_or(0);
            if n > 0 {
                added += 1;
            }
        } else {
            missing_files += 1;
        }
    }
    if added == 0 {
        let _ = db::delete_playlist(&conn, pid);
        return Err("文件里的条目都无法解析（本地文件不存在且无在线元数据）".into());
    }
    Ok(serde_json::json!({ "playlistId": pid, "added": added, "skipped": missing_files }))
}

#[tauri::command]
pub async fn remove_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::remove_from_playlist(&conn, playlist_id, track_id);
    Ok(())
}
#[tauri::command]
pub async fn add_online_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    kind: String,
    rid: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    cover: Option<String>,
    duration_ms: Option<i64>,
    media_mid: Option<String>,
    vip: Option<bool>,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::upsert_online_track(
        &conn,
        &kind,
        &rid,
        &title,
        &artist.unwrap_or_default(),
        &album.unwrap_or_default(),
        &cover.unwrap_or_default(),
        duration_ms.unwrap_or(0),
        &media_mid.unwrap_or_default(),
        vip.unwrap_or(false),
    );
    db::add_online_to_playlist(&conn, playlist_id, &kind, &rid)?;
    Ok(())
}

#[tauri::command]
pub async fn remove_playlist_entry(state: State<'_, AppState>, rowid: i64) -> Result<(), String> {
    let conn = state.db.lock();
    db::remove_playlist_entry(&conn, rowid);
    Ok(())
}

#[tauri::command]
pub async fn remove_playlist_entries(
    state: State<'_, AppState>,
    rowids: Vec<i64>,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::remove_playlist_entries(&conn, &rowids);
    Ok(())
}

/// 手动排序持久化：“资料库 / 我喜欢”整份顺序（全量覆盖）。
/// list: "library" | "liked"；keys 为行标识序列：
/// 本地 "track:<id>"、网易云 "netease:<rid>"、QQ "qq:<rid>"
#[tauri::command]
pub async fn save_manual_order(
    state: State<'_, AppState>,
    list: String,
    keys: Vec<String>,
) -> Result<(), String> {
    if !matches!(list.as_str(), "library" | "liked") {
        return Err("未知排序列表".into());
    }
    let conn = state.db.lock();
    db::save_manual_order(&conn, &list, &keys);
    Ok(())
}

/// 读取“资料库 / 我喜欢”的手动排序（row key → 序号；无记录的行序号为 0）
#[tauri::command]
pub async fn get_manual_order(
    state: State<'_, AppState>,
    list: String,
) -> Result<std::collections::HashMap<String, i64>, String> {
    if !matches!(list.as_str(), "library" | "liked") {
        return Err("未知排序列表".into());
    }
    let conn = state.db.lock();
    Ok(db::manual_order_map(&conn, &list))
}

/// 播放列表条目手动排序：按 rowid 序列重写 position（全量覆盖）
#[tauri::command]
pub async fn reorder_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    rowids: Vec<i64>,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::reorder_playlist(&conn, playlist_id, &rowids);
    Ok(())
}
