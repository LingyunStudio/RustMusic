use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{Folder, Playlist, SourceItem, TrackMeta};

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS folders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT UNIQUE NOT NULL
);
CREATE TABLE IF NOT EXISTS tracks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  artist TEXT NOT NULL DEFAULT '',
  album TEXT NOT NULL DEFAULT '',
  album_artist TEXT NOT NULL DEFAULT '',
  track_no INTEGER NOT NULL DEFAULT 0,
  disc INTEGER NOT NULL DEFAULT 0,
  year INTEGER NOT NULL DEFAULT 0,
  duration REAL NOT NULL DEFAULT 0,
  format TEXT NOT NULL DEFAULT '',
  bitrate INTEGER NOT NULL DEFAULT 0,
  sample_rate INTEGER NOT NULL DEFAULT 0,
  bit_depth INTEGER NOT NULL DEFAULT 0,
  cover TEXT NOT NULL DEFAULT '',
  lrc_path TEXT NOT NULL DEFAULT '',
  size INTEGER NOT NULL DEFAULT 0,
  mtime INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL DEFAULT 0,
  missing INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlists (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL DEFAULT 0,
  remote_kind TEXT NOT NULL DEFAULT '',
  remote_pid TEXT NOT NULL DEFAULT '',
  origin_name TEXT NOT NULL DEFAULT '',
  sort_pos INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlist_tracks (
  playlist_id INTEGER NOT NULL,
  track_id INTEGER NOT NULL,
  position INTEGER NOT NULL DEFAULT 0,
  kind TEXT NOT NULL DEFAULT 'local',
  online_id TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (playlist_id, kind, online_id, track_id)
);
CREATE TABLE IF NOT EXISTS sources (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS stats (
  track_id INTEGER PRIMARY KEY,
  play_count INTEGER NOT NULL DEFAULT 0,
  last_played INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS liked (
  track_id INTEGER PRIMARY KEY,
  liked_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS online_tracks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  rid TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  artist TEXT NOT NULL DEFAULT '',
  album TEXT NOT NULL DEFAULT '',
  cover TEXT NOT NULL DEFAULT '',
  duration_ms INTEGER NOT NULL DEFAULT 0,
  media_mid TEXT NOT NULL DEFAULT '',
  vip INTEGER NOT NULL DEFAULT 0,
  downloaded INTEGER NOT NULL DEFAULT 0,
  last_played INTEGER NOT NULL DEFAULT 0,
  play_count INTEGER NOT NULL DEFAULT 0,
  UNIQUE(kind, rid)
);
CREATE TABLE IF NOT EXISTS liked_online (
  rowid INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  rid TEXT NOT NULL,
  liked_at INTEGER NOT NULL DEFAULT 0,
  UNIQUE(kind, rid)
);
-- “资料库/我喜欢”的手动排序（列表 key + 行 key → 序号）；
-- 播放列表不用它（playlist_tracks 自带 position 列）
CREATE TABLE IF NOT EXISTS manual_order (
  list TEXT NOT NULL,
  row_key TEXT NOT NULL,
  pos INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (list, row_key)
);
"#;

// playlist_tracks 的 kind/online_id 列为渐进迁移（旧库自动补列）
pub fn migrate(conn: &Connection) {
    // 逐条补列：列已存在是预期情况（“duplicate column”），必须跳过继续；
    // 若放进一个 execute_batch，第一条失败会中止整批，后续列永远补不上
    // （线上曾因此停留在旧 schema：playlist_entries/recent_online_list
    //  查询 last_played/missing 静默失败，歌单条目与最近播放全空）
    add_column_if_missing(
        conn,
        "online_tracks",
        "downloaded",
        "ALTER TABLE online_tracks ADD COLUMN downloaded INTEGER NOT NULL DEFAULT 0",
    );
    add_column_if_missing(
        conn,
        "tracks",
        "missing",
        "ALTER TABLE tracks ADD COLUMN missing INTEGER NOT NULL DEFAULT 0",
    );
    add_column_if_missing(
        conn,
        "online_tracks",
        "last_played",
        "ALTER TABLE online_tracks ADD COLUMN last_played INTEGER NOT NULL DEFAULT 0",
    );
    add_column_if_missing(
        conn,
        "online_tracks",
        "play_count",
        "ALTER TABLE online_tracks ADD COLUMN play_count INTEGER NOT NULL DEFAULT 0",
    );
    add_column_if_missing(
        conn,
        "liked",
        "liked_at",
        "ALTER TABLE liked ADD COLUMN liked_at INTEGER NOT NULL DEFAULT 0",
    );
    // playlists 的远程歌单标识（netease/qq 的歌单 id）：重复导入时按它合并进
    // 已有列表，而不是再建一个同名列表
    add_column_if_missing(
        conn,
        "playlists",
        "remote_kind",
        "ALTER TABLE playlists ADD COLUMN remote_kind TEXT NOT NULL DEFAULT ''",
    );
    add_column_if_missing(
        conn,
        "playlists",
        "remote_pid",
        "ALTER TABLE playlists ADD COLUMN remote_pid TEXT NOT NULL DEFAULT ''",
    );
    // 播放列表的原始导入名：改名后仍能知道它来自哪个远程歌单（重导入合并、
    // 界面提示用）
    add_column_if_missing(
        conn,
        "playlists",
        "origin_name",
        "ALTER TABLE playlists ADD COLUMN origin_name TEXT NOT NULL DEFAULT ''",
    );
    // 播放列表手动排序位（侧边栏长按拖动调序）：旧行回填为 id，保持原顺序
    add_column_if_missing(
        conn,
        "playlists",
        "sort_pos",
        "ALTER TABLE playlists ADD COLUMN sort_pos INTEGER NOT NULL DEFAULT 0",
    );
    let _ = conn.execute("UPDATE playlists SET sort_pos = id WHERE sort_pos = 0", []);
    // playlist_tracks 旧主键 (playlist_id, track_id) 会吞掉同列表的多个在线条目
    // （track_id 恒为 0），检测旧结构并重建为 (playlist_id, kind, online_id, track_id)
    let old_pk: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='playlist_tracks'",
            [],
            |r| r.get(0),
        )
        .ok();
    let needs_rebuild = old_pk
        .as_deref()
        .map(|sql| sql.contains("PRIMARY KEY (playlist_id, track_id)"))
        .unwrap_or(false);
    if needs_rebuild {
        let _ = conn.execute_batch(
            "BEGIN;
             CREATE TABLE playlist_tracks_new (
               playlist_id INTEGER NOT NULL,
               track_id INTEGER NOT NULL,
               position INTEGER NOT NULL DEFAULT 0,
               kind TEXT NOT NULL DEFAULT 'local',
               online_id TEXT NOT NULL DEFAULT '',
               PRIMARY KEY (playlist_id, kind, online_id, track_id)
             );
             INSERT OR IGNORE INTO playlist_tracks_new
               (playlist_id, track_id, position, kind, online_id)
             SELECT playlist_id, track_id, position,
               COALESCE(NULLIF(kind, ''), 'local'), COALESCE(online_id, '')
             FROM playlist_tracks;
             DROP TABLE playlist_tracks;
             ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks;
             COMMIT;",
        );
    }
}

/// ALTER TABLE ... ADD COLUMN，列已存在时静默跳过（幂等迁移）
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, alter: &str) {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
            params![table, column],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !exists {
        if let Err(e) = conn.execute_batch(alter) {
            eprintln!("[db] 迁移补列失败 {table}.{column}: {e}");
        }
    }
}

pub fn init(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    migrate(&conn);
    Ok(conn)
}

// ---------- settings ----------

pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value],
    );
}

// ---------- folders ----------

pub fn list_folders(conn: &Connection) -> Vec<Folder> {
    let mut stmt = match conn.prepare("SELECT id, path FROM folders ORDER BY id") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |r| {
        Ok(Folder {
            id: r.get(0)?,
            path: r.get(1)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn add_folder(conn: &Connection, path: &str) -> Result<i64, String> {
    conn.execute(
        "INSERT OR IGNORE INTO folders(path) VALUES(?1)",
        params![path],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn remove_folder(conn: &Connection, id: i64) {
    let path: Option<String> = conn
        .query_row("SELECT path FROM folders WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten();
    if let Some(p) = path {
        // 软删除该文件夹下的全部曲目（missing=1）：
        // 记录保留（喜欢/最近播放仍显示），文件夹重新添加后扫描复活
        let sep = format!("{p}\\");
        let _ = conn.execute(
            "UPDATE tracks SET missing = 1
             WHERE path = ?1 OR substr(path, 1, ?2) = ?3",
            params![p, sep.len() as i64, sep],
        );
    }
    let _ = conn.execute("DELETE FROM folders WHERE id = ?1", params![id]);
}

// ---------- tracks ----------

pub struct NewTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_no: i64,
    pub disc: i64,
    pub year: i64,
    pub duration: f64,
    pub format: String,
    pub bitrate: i64,
    pub sample_rate: i64,
    pub bit_depth: i64,
    pub cover: String,
    pub lrc_path: String,
    pub size: i64,
    pub mtime: i64,
}

pub fn upsert_track(conn: &Connection, t: &NewTrack) {
    let _ = conn.execute(
        r#"INSERT INTO tracks(path, title, artist, album, album_artist, track_no, disc, year,
             duration, format, bitrate, sample_rate, bit_depth, cover, lrc_path, size, mtime, added_at)
           VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
           ON CONFLICT(path) DO UPDATE SET
             title=?2, artist=?3, album=?4, album_artist=?5, track_no=?6, disc=?7, year=?8,
             duration=?9, format=?10, bitrate=?11, sample_rate=?12, bit_depth=?13,
             cover=?14, lrc_path=?15, size=?16, mtime=?17"#,
        params![
            t.path, t.title, t.artist, t.album, t.album_artist, t.track_no, t.disc, t.year,
            t.duration, t.format, t.bitrate, t.sample_rate, t.bit_depth, t.cover, t.lrc_path,
            t.size, t.mtime, now_secs(),
        ],
    );
}

pub fn track_paths(conn: &Connection) -> Vec<(String, i64, i64)> {
    let mut stmt = match conn.prepare("SELECT path, mtime, size FROM tracks") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// 文件已不存在时软删除（missing=1）：记录保留（含喜欢/播放统计/歌单引用），
/// 资料库隐藏；文件重新加入后 upsert 自动复活
pub fn delete_missing(conn: &Connection, seen: &HashSet<String>, folder_prefixes: &[String]) {
    // 曾在监控目录下、现已不在任何目录前缀内的 → missing
    let stale: Vec<String> = track_paths(conn)
        .into_iter()
        .map(|(p, _, _)| p)
        .filter(|p| !seen.contains(p) && folder_prefixes.iter().any(|f| p.starts_with(f.as_str())))
        .collect();
    for p in &stale {
        let _ = conn.execute("UPDATE tracks SET missing = 1 WHERE path = ?1", params![p]);
    }
    // 目录重新添加/文件回归：复活对应记录
    let revived: Vec<String> = seen
        .iter()
        .filter(|p| folder_prefixes.iter().any(|f| p.starts_with(f.as_str())))
        .cloned()
        .collect();
    for p in &revived {
        let _ = conn.execute("UPDATE tracks SET missing = 0 WHERE path = ?1", params![p]);
    }
}

fn row_to_meta(r: &Row) -> rusqlite::Result<TrackMeta> {
    Ok(TrackMeta {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        album_artist: r.get(5)?,
        track_no: r.get(6)?,
        disc: r.get(7)?,
        year: r.get(8)?,
        duration: r.get(9)?,
        format: r.get(10)?,
        bitrate: r.get(11)?,
        sample_rate: r.get(12)?,
        bit_depth: r.get(13)?,
        cover: r.get(14)?,
        has_lrc: !r.get::<_, String>(15)?.is_empty(),
        size: r.get(16)?,
        mtime: r.get(17)?,
        liked: r.get::<_, i64>(18)? != 0,
        play_count: r.get(19)?,
        last_played: r.get(20)?,
        missing: r.get::<_, i64>(21)? != 0,
        liked_at: r.get(22)?,
    })
}

const TRACK_SELECT: &str = r#"
SELECT t.id, t.path, t.title, t.artist, t.album, t.album_artist, t.track_no, t.disc, t.year,
       t.duration, t.format, t.bitrate, t.sample_rate, t.bit_depth, t.cover, t.lrc_path,
       t.size, t.mtime,
       CASE WHEN l.track_id IS NULL THEN 0 ELSE 1 END,
       COALESCE(s.play_count, 0), COALESCE(s.last_played, 0), t.missing,
       COALESCE(l.liked_at, 0)
FROM tracks t
LEFT JOIN liked l ON l.track_id = t.id
LEFT JOIN stats s ON s.track_id = t.id
"#;

pub fn list_tracks(conn: &Connection) -> Vec<TrackMeta> {
    let mut stmt = match conn.prepare(&(TRACK_SELECT.to_string() + "ORDER BY t.id")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[db] 读取曲目列表失败: {e}");
            return vec![];
        }
    };
    stmt.query_map([], row_to_meta)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

pub fn get_track(conn: &Connection, id: i64) -> Option<TrackMeta> {
    conn.query_row(
        &(TRACK_SELECT.to_string() + "WHERE t.id = ?1"),
        params![id],
        row_to_meta,
    )
    .optional()
    .ok()
    .flatten()
}

pub fn get_track_path(conn: &Connection, id: i64) -> Option<String> {
    conn.query_row("SELECT path FROM tracks WHERE id = ?1", params![id], |r| {
        r.get(0)
    })
    .optional()
    .ok()
    .flatten()
}

pub fn get_lrc_path(conn: &Connection, id: i64) -> Option<String> {
    let v: Option<String> = conn
        .query_row(
            "SELECT lrc_path FROM tracks WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    v.filter(|s| !s.is_empty())
}

pub fn like_track(conn: &Connection, id: i64, on: bool) {
    if on {
        // 重复喜欢不刷新时间戳（ON CONFLICT DO NOTHING），保持首次喜欢时间
        let _ = conn.execute(
            "INSERT INTO liked(track_id, liked_at) VALUES(?1, ?2)
             ON CONFLICT(track_id) DO NOTHING",
            params![id, now_secs()],
        );
    } else {
        let _ = conn.execute("DELETE FROM liked WHERE track_id = ?1", params![id]);
    }
}

pub fn record_play(conn: &Connection, id: i64) {
    let _ = conn.execute(
        "INSERT INTO stats(track_id, play_count, last_played) VALUES(?1, 1, ?2)
         ON CONFLICT(track_id) DO UPDATE SET play_count = play_count + 1, last_played = ?2",
        params![id, now_secs()],
    );
}

// ---------- playlists ----------

pub fn list_playlists(conn: &Connection) -> Vec<Playlist> {
    let mut stmt = match conn.prepare(
        "SELECT id, name, created_at, remote_kind, remote_pid, origin_name
         FROM playlists ORDER BY sort_pos, id",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let mut out: Vec<Playlist> = stmt
        .query_map([], |r| {
            Ok(Playlist {
                id: r.get(0)?,
                name: r.get(1)?,
                track_ids: vec![],
                entries: vec![],
                cover: String::new(),
                created_at: r.get(2)?,
                remote_kind: r.get(3)?,
                remote_pid: r.get(4)?,
                origin_name: r.get(5)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    for pl in out.iter_mut() {
        for e in playlist_entries(conn, pl.id) {
            if pl.cover.is_empty() && !e.cover.is_empty() {
                pl.cover = e.cover.clone();
            }
            if e.kind == "local" {
                pl.track_ids.push(e.track_id);
            }
            pl.entries.push(crate::models::PlaylistEntryMeta {
                rowid: e.rowid,
                kind: e.kind.clone(),
                track_id: (e.kind == "local").then_some(e.track_id),
                online_id: (e.kind != "local").then_some(e.online_id.clone()),
                title: e.title.clone(),
                artist: e.artist.clone(),
                album: e.album.clone(),
                cover: e.cover.clone(),
                duration: e.duration,
                media_mid: e.media_mid.clone(),
                vip: e.vip,
                last_played: e.last_played,
                liked_at: e.liked_at,
            });
        }
    }
    out
}

pub fn create_playlist(conn: &Connection, name: &str) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO playlists(name, created_at, sort_pos)
         VALUES(?1, ?2, COALESCE((SELECT MAX(sort_pos) FROM playlists), 0) + 1)",
        params![name, now_secs()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// 按远程歌单标识找本地播放列表（重复导入合并用）：
/// 优先远程 id；旧版本导入的列表没存远程 id，退化为同名/原始名匹配
pub fn find_playlist_by_remote(
    conn: &Connection,
    kind: &str,
    remote_pid: &str,
    name: &str,
) -> Option<i64> {
    let by_remote: Option<i64> = conn
        .query_row(
            "SELECT id FROM playlists WHERE remote_kind = ?1 AND remote_pid = ?2 ORDER BY id LIMIT 1",
            params![kind, remote_pid],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    if by_remote.is_some() {
        return by_remote;
    }
    // 同名或原始名匹配（origin_name = 导入时记录的远程歌单名，
    // 列表被改名后仍能靠它认出）
    conn.query_row(
        "SELECT id FROM playlists
         WHERE remote_kind = '' AND (name = ?1 OR (origin_name != '' AND origin_name = ?1))
         ORDER BY id LIMIT 1",
        params![name],
        |r| r.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// 记录/补全播放列表的远程歌单标识（同名匹配到的旧列表在此回填）
pub fn set_playlist_remote(conn: &Connection, id: i64, kind: &str, remote_pid: &str) {
    let _ = conn.execute(
        "UPDATE playlists SET remote_kind = ?2, remote_pid = ?3 WHERE id = ?1",
        params![id, kind, remote_pid],
    );
}

/// 补全播放列表的原始导入名（已设置过的不覆盖：用户改名后原值仍在）
pub fn set_playlist_origin(conn: &Connection, id: i64, name: &str) {
    if name.is_empty() {
        return;
    }
    let _ = conn.execute(
        "UPDATE playlists SET origin_name = ?2 WHERE id = ?1 AND origin_name = ''",
        params![id, name],
    );
}

pub fn delete_playlist(conn: &Connection, id: i64) {
    let _ = conn.execute("DELETE FROM playlists WHERE id = ?1", params![id]);
    let _ = conn.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
        params![id],
    );
}

/// 重命名播放列表。远程导入的列表若还没记录原始导入名，先以当前名
///（即当初的导入名）回填 origin_name，再写入新名——改名不影响重导入合并。
pub fn rename_playlist(conn: &Connection, id: i64, name: &str) {
    let _ = conn.execute(
        "UPDATE playlists SET
           origin_name = CASE WHEN origin_name = '' AND remote_pid != '' THEN name ELSE origin_name END,
           name = ?2
         WHERE id = ?1",
        params![id, name],
    );
}

/// 播放列表手动排序：按传入的 id 序列重写 sort_pos（1 起）
pub fn reorder_playlists(conn: &Connection, ids: &[i64]) {
    for (i, id) in ids.iter().enumerate() {
        let _ = conn.execute(
            "UPDATE playlists SET sort_pos = ?1 WHERE id = ?2",
            params![(i + 1) as i64, id],
        );
    }
}

pub fn add_online_to_playlist(
    conn: &Connection,
    pid: i64,
    kind: &str,
    rid: &str,
) -> Result<bool, String> {
    Ok(add_playlist_entry(conn, pid, kind, 0, rid))
}

pub fn add_to_playlist(conn: &Connection, pid: i64, tid: i64) -> Result<(), String> {
    let pos: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![pid],
            |r| r.get(0),
        )
        .unwrap_or(1);
    conn.execute(
        "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position) VALUES(?1, ?2, ?3)",
        params![pid, tid, pos],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_from_playlist(conn: &Connection, pid: i64, tid: i64) {
    let _ = conn.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
        params![pid, tid],
    );
}

// ---------- sources ----------

pub fn list_sources(conn: &Connection) -> Vec<SourceItem> {
    let mut stmt =
        match conn.prepare("SELECT id, url, title, created_at FROM sources ORDER BY id DESC") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
    stmt.query_map([], |r| {
        Ok(SourceItem {
            id: r.get(0)?,
            url: r.get(1)?,
            title: r.get(2)?,
            created_at: r.get(3)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn add_source(conn: &Connection, url: &str, title: &str) -> Result<i64, String> {
    conn.execute(
        "INSERT OR IGNORE INTO sources(url, title, created_at) VALUES(?1, ?2, ?3)",
        params![url, title, now_secs()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_source(conn: &Connection, id: i64) {
    let _ = conn.execute("DELETE FROM sources WHERE id = ?1", params![id]);
}

pub fn get_source(conn: &Connection, id: i64) -> Option<SourceItem> {
    conn.query_row(
        "SELECT id, url, title, created_at FROM sources WHERE id = ?1",
        params![id],
        |r| {
            Ok(SourceItem {
                id: r.get(0)?,
                url: r.get(1)?,
                title: r.get(2)?,
                created_at: r.get(3)?,
            })
        },
    )
    .optional()
    .ok()
    .flatten()
}

pub fn update_source_title(conn: &Connection, id: i64, title: &str) {
    let _ = conn.execute(
        "UPDATE sources SET title = ?2 WHERE id = ?1",
        params![id, title],
    );
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------- 在线曲目（网易云 / QQ 音乐条目入库） ----------

pub fn upsert_online_track(
    conn: &Connection,
    kind: &str,
    rid: &str,
    title: &str,
    artist: &str,
    album: &str,
    cover: &str,
    duration_ms: i64,
    media_mid: &str,
    vip: bool,
) -> i64 {
    let _ = conn.execute(
        "INSERT INTO online_tracks(kind, rid, title, artist, album, cover, duration_ms, media_mid, vip)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(kind, rid) DO UPDATE SET
           title=?3, artist=?4, album=?5, cover=?6, duration_ms=?7, media_mid=?8, vip=?9",
        params![kind, rid, title, artist, album, cover, duration_ms, media_mid, vip as i64],
    );
    conn.last_insert_rowid()
}

/// 记录在线曲目播放（未入库的自动补录元数据，供“最近播放”显示）；
/// INSERT OR IGNORE：已存在（如之前收藏过）不覆盖其 vip/封面等元数据
pub fn record_play_online(
    conn: &Connection,
    kind: &str,
    rid: &str,
    title: &str,
    artist: &str,
    album: &str,
    cover: &str,
    duration_ms: i64,
    media_mid: &str,
    vip: bool,
) {
    let _ = conn.execute(
        "INSERT OR IGNORE INTO online_tracks(kind, rid, title, artist, album, cover, duration_ms, media_mid, vip)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![kind, rid, title, artist, album, cover, duration_ms, media_mid, vip as i64],
    );
    if let Err(e) = conn.execute(
        "UPDATE online_tracks SET last_played = ?3, play_count = play_count + 1 WHERE kind = ?1 AND rid = ?2",
        params![kind, rid, now_secs()],
    ) {
        eprintln!("[db] 记录在线播放失败 (kind={kind}, rid={rid}): {e}");
    }
}

/// “最近播放”的在线曲目部分（有播放记录的），按最近播放时间倒序
pub fn recent_online_list(conn: &Connection, limit: i64) -> Vec<PlaylistEntryRow> {
    let mut stmt = match conn.prepare(
        "SELECT kind, rid, title, artist, album, cover, duration_ms, media_mid, vip, last_played
         FROM online_tracks
         WHERE last_played > 0
         ORDER BY last_played DESC
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[db] 读取在线最近播放失败: {e}");
            return vec![];
        }
    };
    stmt.query_map([limit], |r| {
        Ok(PlaylistEntryRow {
            rowid: 0,
            kind: r.get(0)?,
            track_id: 0,
            online_id: r.get(1)?,
            title: r.get(2)?,
            artist: r.get(3)?,
            album: r.get(4)?,
            cover: r.get(5)?,
            duration: r.get::<_, i64>(6).unwrap_or(0) as f64 / 1000.0,
            media_mid: r.get(7)?,
            vip: r.get::<_, i64>(8)? != 0,
            last_played: r.get::<_, i64>(9).unwrap_or(0),
            liked_at: 0,
        })
    })
    .map(|rows| rows.filter_map(|x| x.ok()).collect())
    .unwrap_or_default()
}

// ---------- 播放列表条目（本地 + 在线混合） ----------

#[derive(Debug, Clone)]
pub struct PlaylistEntryRow {
    pub rowid: i64,
    pub kind: String,
    pub track_id: i64,
    pub online_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration: f64,
    pub media_mid: String,
    pub vip: bool,
    /// 最近播放时间（unix 秒；“最近播放”合并排序用，0 = 无记录）
    pub last_played: i64,
    /// 收藏时间（unix 秒；“我喜欢”合并排序用，0 = 无记录）
    pub liked_at: i64,
}

pub fn playlist_entries(conn: &Connection, pid: i64) -> Vec<PlaylistEntryRow> {
    let mut stmt = match conn
        .prepare(
            "SELECT pt.rowid, pt.kind, pt.track_id, pt.online_id,
              COALESCE(t.title, ot.title, '') AS title,
              COALESCE(t.artist, ot.artist, '') AS artist,
              COALESCE(t.album, ot.album, '') AS album,
              COALESCE(t.cover, ot.cover, '') AS cover,
              COALESCE(t.duration, ot.duration_ms / 1000.0, 0) AS duration,
              COALESCE(ot.media_mid, '') AS media_mid,
              COALESCE(ot.vip, 0) AS vip,
              COALESCE(ot.last_played, 0) AS last_played,
              COALESCE(l.liked_at, lo.liked_at, 0) AS liked_at
             FROM playlist_tracks pt
             LEFT JOIN tracks t ON pt.kind = 'local' AND t.id = pt.track_id
             LEFT JOIN online_tracks ot ON pt.kind != 'local' AND ot.kind = pt.kind AND ot.rid = pt.online_id
             LEFT JOIN liked l ON pt.kind = 'local' AND l.track_id = pt.track_id
             LEFT JOIN liked_online lo ON pt.kind != 'local' AND lo.kind = pt.kind AND lo.rid = pt.online_id
             WHERE pt.playlist_id = ?1
             ORDER BY pt.position, pt.rowid",
        )
    {
        Ok(s) => s,
        Err(e) => {
            // 读取失败时明确日志：列表“空但导入成功”这类表象的根因都在这里
            eprintln!("[db] 读取播放列表条目失败 (playlist={pid}): {e}");
            return vec![];
        }
    };
    stmt.query_map(params![pid], |r| {
        Ok(PlaylistEntryRow {
            rowid: r.get(0)?,
            kind: r.get(1)?,
            track_id: r.get(2)?,
            online_id: r.get(3)?,
            title: r.get(4)?,
            artist: r.get(5)?,
            album: r.get(6)?,
            cover: r.get(7)?,
            duration: r.get(8)?,
            media_mid: r.get(9)?,
            vip: r.get::<_, i64>(10)? != 0,
            last_played: r.get::<_, i64>(11).unwrap_or(0),
            liked_at: r.get::<_, i64>(12).unwrap_or(0),
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// 追加条目到播放列表末尾；已存在（主键冲突）时跳过。
/// 返回是否真的新增（导入合并时的“新增 N 首”计数用）
pub fn add_playlist_entry(
    conn: &Connection,
    pid: i64,
    kind: &str,
    track_id: i64,
    online_id: &str,
) -> bool {
    let pos: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![pid],
            |r| r.get(0),
        )
        .unwrap_or(1);
    conn.execute(
        "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position, kind, online_id) VALUES(?1,?2,?3,?4,?5)",
        params![pid, track_id, pos, kind, online_id],
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

pub fn remove_playlist_entry(conn: &Connection, rowid: i64) {
    let _ = conn.execute(
        "DELETE FROM playlist_tracks WHERE rowid = ?1",
        params![rowid],
    );
}

// ---------- 在线喜欢（轻量引用，不下载） ----------

pub fn like_online_track(conn: &Connection, kind: &str, rid: &str) {
    let _ = conn.execute(
        "INSERT OR IGNORE INTO liked_online(kind, rid, liked_at) VALUES(?1, ?2, ?3)",
        params![kind, rid, now_secs()],
    );
}

pub fn unlike_online_track(conn: &Connection, kind: &str, rid: &str) {
    let _ = conn.execute(
        "DELETE FROM liked_online WHERE kind = ?1 AND rid = ?2",
        params![kind, rid],
    );
}

pub fn liked_online_list(conn: &Connection) -> Vec<PlaylistEntryRow> {
    let mut stmt = match conn.prepare(
        "SELECT l.kind, l.rid, ot.title, ot.artist, ot.album, ot.cover, ot.duration_ms, ot.media_mid, ot.vip, l.liked_at
         FROM liked_online l
         LEFT JOIN online_tracks ot ON ot.kind = l.kind AND ot.rid = l.rid
         ORDER BY l.liked_at DESC, l.rowid DESC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |r| {
        Ok(PlaylistEntryRow {
            rowid: 0,
            kind: r.get(0)?,
            track_id: 0,
            online_id: r.get(1)?,
            title: r.get(2)?,
            artist: r.get(3)?,
            album: r.get(4)?,
            cover: r.get(5)?,
            duration: r.get::<_, i64>(6).unwrap_or(0) as f64 / 1000.0,
            media_mid: r.get(7)?,
            vip: r.get::<_, i64>(8)? != 0,
            last_played: 0,
            liked_at: r.get::<_, i64>(9).unwrap_or(0),
        })
    })
    .map(|rows| rows.filter_map(|x| x.ok()).collect())
    .unwrap_or_default()
}

#[allow(dead_code)]
pub fn is_liked_online(conn: &Connection, kind: &str, rid: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM liked_online WHERE kind = ?1 AND rid = ?2",
        params![kind, rid],
        |_| Ok(()),
    )
    .is_ok()
}

pub fn mark_online_downloaded(conn: &Connection, kind: &str, rid: &str) {
    let _ = conn.execute(
        "UPDATE online_tracks SET downloaded = 1 WHERE kind = ?1 AND rid = ?2",
        params![kind, rid],
    );
}

/// 取在线条目存的封面 URL
pub fn get_online_cover(conn: &Connection, kind: &str, rid: &str) -> Option<String> {
    conn.query_row(
        "SELECT cover FROM online_tracks WHERE kind = ?1 AND rid = ?2",
        params![kind, rid],
        |r| r.get(0),
    )
    .ok()
    .filter(|s: &String| !s.is_empty())
}

// ---------- 手动排序（“资料库/我喜欢”共用；播放列表走 playlist_tracks.position） ----------

/// 保存一份列表的完整手动顺序（全量覆盖，行 key 序列即顺序）。
/// list: "library" | "liked"；row_key: 本地 "track:<id>" / 在线 "netease:<rid>" / "qq:<rid>"
///（与 unavailable 键的格式一致）。未包含的 key（新入库/新收藏）自动排到已排序项之后。
pub fn save_manual_order(conn: &Connection, list: &str, keys: &[String]) {
    let _ = conn.execute("DELETE FROM manual_order WHERE list = ?1", params![list]);
    let tx = match conn.unchecked_transaction() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[db] 手动排序事务失败: {e}");
            return;
        }
    };
    for (i, k) in keys.iter().enumerate() {
        let _ = tx.execute(
            "INSERT OR REPLACE INTO manual_order(list, row_key, pos) VALUES(?1, ?2, ?3)",
            params![list, k, (i + 1) as i64],
        );
    }
    if let Err(e) = tx.commit() {
        eprintln!("[db] 保存手动排序失败: {e}");
    }
}

/// 读手动排序（row_key → 序号，1 起；无记录的 key 视为 0，排已排序项之后）
pub fn manual_order_map(conn: &Connection, list: &str) -> std::collections::HashMap<String, i64> {
    let mut out = std::collections::HashMap::new();
    let mut stmt = match conn.prepare("SELECT row_key, pos FROM manual_order WHERE list = ?1") {
        Ok(s) => s,
        Err(_) => return out,
    };
    let rows: Vec<(String, i64)> = stmt
        .query_map(params![list], |r| Ok((r.get(0)?, r.get(1)?)))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    for (k, p) in rows {
        out.insert(k, p);
    }
    out
}

/// 播放列表条目手动排序：按给定 rowid 序列重写 position（全量覆盖）。
/// 前端总是提交整份序列（含搜索时不可见的行），未提到的行保持原位。
pub fn reorder_playlist(conn: &Connection, pid: i64, rowids: &[i64]) {
    for (i, rid) in rowids.iter().enumerate() {
        let _ = conn.execute(
            "UPDATE playlist_tracks SET position = ?3 WHERE rowid = ?2 AND playlist_id = ?1",
            params![pid, rid, (i + 1) as i64],
        );
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// 线上曾出现的旧 schema：online_tracks 只有 downloaded（无 last_played/play_count），
    /// tracks 无 missing——migrate() 曾因 execute_batch 首条失败而整批中止，后续列
    /// 永远补不上，导致 playlist_entries/recent_online_list/list_tracks 静默返回空。
    /// 此测试锁定该升级路径：旧库 init() 后上述查询必须全部可用。
    #[test]
    fn legacy_schema_upgrades_and_queries_work() {
        let conn = Connection::open_in_memory().unwrap();
        // 1) 旧库 schema（含 v0.1.0 首版就有、会触发“列已存在”的 downloaded 列）
        conn.execute_batch(
            r#"
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT UNIQUE NOT NULL);
            CREATE TABLE tracks (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              path TEXT UNIQUE NOT NULL,
              title TEXT NOT NULL DEFAULT '',
              artist TEXT NOT NULL DEFAULT '',
              album TEXT NOT NULL DEFAULT '',
              album_artist TEXT NOT NULL DEFAULT '',
              track_no INTEGER NOT NULL DEFAULT 0,
              disc INTEGER NOT NULL DEFAULT 0,
              year INTEGER NOT NULL DEFAULT 0,
              duration REAL NOT NULL DEFAULT 0,
              format TEXT NOT NULL DEFAULT '',
              bitrate INTEGER NOT NULL DEFAULT 0,
              sample_rate INTEGER NOT NULL DEFAULT 0,
              bit_depth INTEGER NOT NULL DEFAULT 0,
              cover TEXT NOT NULL DEFAULT '',
              lrc_path TEXT NOT NULL DEFAULT '',
              size INTEGER NOT NULL DEFAULT 0,
              mtime INTEGER NOT NULL DEFAULT 0,
              added_at INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE playlists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE playlist_tracks (
              playlist_id INTEGER NOT NULL,
              track_id INTEGER NOT NULL,
              position INTEGER NOT NULL DEFAULT 0,
              kind TEXT NOT NULL DEFAULT 'local',
              online_id TEXT NOT NULL DEFAULT '',
              PRIMARY KEY (playlist_id, kind, online_id, track_id)
            );
            CREATE TABLE sources (id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT UNIQUE NOT NULL, title TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE stats (track_id INTEGER PRIMARY KEY, play_count INTEGER NOT NULL DEFAULT 0, last_played INTEGER NOT NULL DEFAULT 0);
            CREATE TABLE liked (track_id INTEGER PRIMARY KEY);
            CREATE TABLE online_tracks (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              kind TEXT NOT NULL,
              rid TEXT NOT NULL,
              title TEXT NOT NULL DEFAULT '',
              artist TEXT NOT NULL DEFAULT '',
              album TEXT NOT NULL DEFAULT '',
              cover TEXT NOT NULL DEFAULT '',
              duration_ms INTEGER NOT NULL DEFAULT 0,
              media_mid TEXT NOT NULL DEFAULT '',
              vip INTEGER NOT NULL DEFAULT 0,
              downloaded INTEGER NOT NULL DEFAULT 0,
              UNIQUE(kind, rid)
            );
            CREATE TABLE liked_online (
              rowid INTEGER PRIMARY KEY AUTOINCREMENT,
              kind TEXT NOT NULL,
              rid TEXT NOT NULL,
              liked_at INTEGER NOT NULL DEFAULT 0,
              UNIQUE(kind, rid)
            );
            "#,
        )
        .unwrap();
        // 2) 旧库里的既有数据：导入过歌单的在线条目 + 本地曲目
        upsert_online_track(
            &conn,
            "qq",
            "001Song",
            "歌 A",
            "歌手",
            "专辑",
            "http://c/1.jpg",
            200000,
            "M001",
            true,
        );
        add_online_to_playlist(&conn, 1, "qq", "001Song").unwrap();
        upsert_online_track(
            &conn,
            "netease",
            "101Song",
            "歌 B",
            "歌手",
            "专辑",
            "http://c/2.jpg",
            180000,
            "",
            false,
        );
        add_online_to_playlist(&conn, 1, "netease", "101Song").unwrap();

        // 3) 升级（init 的 SCHEMA 是 CREATE IF NOT EXISTS，不会动旧表；migrate 负责补列）
        conn.execute_batch(SCHEMA).unwrap();
        migrate(&conn);

        // 4) 三条曾静默失败的查询路径必须恢复
        // 4a) 歌单条目（歌单导入后“歌曲信息没被导入”的读取路径）
        let entries = playlist_entries(&conn, 1);
        assert_eq!(
            entries.len(),
            2,
            "playlist_entries 应返回 2 条，实际 {entries:?}"
        );
        assert_eq!(entries[0].title, "歌 A");
        assert_eq!(entries[1].title, "歌 B");
        assert_eq!(entries[0].vip, true);

        // 4b) 最近播放（在线）：记录后必须读得回来
        record_play_online(
            &conn,
            "qq",
            "001Song",
            "歌 A",
            "歌手",
            "专辑",
            "http://c/1.jpg",
            200000,
            "M001",
            true,
        );
        let recent = recent_online_list(&conn, 100);
        assert_eq!(
            recent.len(),
            1,
            "recent_online_list 应返回 1 条，实际 {recent:?}"
        );
        assert_eq!(recent[0].title, "歌 A");
        assert!(recent[0].last_played > 0);

        // 4c) 资料库列表（t.missing 列）
        let tracks = list_tracks(&conn);
        assert_eq!(tracks.len(), 0, "尚未入库本地曲目");

        // 5) 幂等：再次 migrate 不报错、不重复补列
        migrate(&conn);
        let entries2 = playlist_entries(&conn, 1);
        assert_eq!(entries2.len(), 2);
    }

    /// migrate 的 execute_batch 老写法回归：任何一条 ALTER 已存在即中止整批。
    /// 此测试保证新的逐列补列法即使某表已完全最新也能全部通过。
    #[test]
    fn migrate_is_idempotent_on_current_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        migrate(&conn);
        migrate(&conn);
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('online_tracks')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(cols.contains(&"last_played".to_string()));
        assert!(cols.contains(&"play_count".to_string()));
        let one: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tracks') WHERE name='missing'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(one, 1);
    }

    /// 重复导入歌单的合并语义：
    /// 1) 已有条目不重复；2) 已有条目的顺序（position）不被改写；
    /// 3) 用户手动加进列表的其他歌不被删除；
    /// 4) 远程歌单新增的歌追加到本地列表末尾；
    /// 5) 按远程 id（优先）/同名（旧数据回退）找到已有列表而不是新建。
    #[test]
    fn reimport_playlist_merges_without_duplicates_or_reorder() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        migrate(&conn);

        // 首次导入：远程歌单 3 首
        let pid = find_playlist_by_remote(&conn, "qq", "777", "我的最爱")
            .unwrap_or_else(|| create_playlist(&conn, "我的最爱").unwrap());
        set_playlist_remote(&conn, pid, "qq", "777");
        for rid in ["a", "b", "c"] {
            add_online_to_playlist(&conn, pid, "qq", rid).unwrap();
        }
        // 用户手动再往这个列表加两首（本地曲目 + 另一首在线）
        add_to_playlist(&conn, pid, 42).unwrap();
        add_online_to_playlist(&conn, pid, "netease", "n1").unwrap();
        // 用户手动调过顺序：把 c（position 序里的第 3 项）挪到最前
        let ids: Vec<i64> = playlist_entries(&conn, pid)
            .iter()
            .map(|e| e.rowid)
            .collect();
        let c_rowid = ids[2];
        let mut reordered: Vec<i64> = ids.iter().copied().filter(|r| *r != c_rowid).collect();
        reordered.insert(0, c_rowid); // c 放到最前
        reorder_playlist(&conn, pid, &reordered);

        // 远程歌单更新：少了一首 b，多了两首新歌 d/e（a、c 还在）
        let merged = find_playlist_by_remote(&conn, "qq", "777", "我的最爱").unwrap();
        assert_eq!(merged, pid, "同远程 id 应命中已有列表");
        let mut added = 0;
        for rid in ["a", "c", "d", "e"] {
            if add_online_to_playlist(&conn, pid, "qq", rid).unwrap() {
                added += 1;
            }
        }
        assert_eq!(added, 2, "只有 d/e 是新歌");

        // 最终列表：5 首原有（a c b 42 n1，c 在前）+ 2 首新歌在末尾；
        // 远程删掉的 b 仍保留（本地不追随远程删除）
        let entries = playlist_entries(&conn, pid);
        let seq: Vec<(String, String)> = entries
            .iter()
            .map(|e| (e.kind.clone(), e.online_id.clone()))
            .collect();
        assert_eq!(
            seq,
            vec![
                ("qq".into(), "c".into()),
                ("qq".into(), "a".into()),
                ("qq".into(), "b".into()),
                ("local".into(), "".into()),
                ("netease".into(), "n1".into()),
                ("qq".into(), "d".into()),
                ("qq".into(), "e".into()),
            ],
            "合并后顺序：手动序保持、手动加的歌保留、新歌在末尾，实际 {seq:?}"
        );

        // 同名回退：无远程标识的旧列表按名字匹配（旧版本导入的数据）
        let legacy = create_playlist(&conn, "我的最爱").unwrap();
        let hit = find_playlist_by_remote(&conn, "netease", "999", "我的最爱").unwrap();
        assert_eq!(hit, legacy, "无远程 id 时应同名匹配到旧列表");
        // 已带远程标识的列表不再参与同名匹配
        let miss = find_playlist_by_remote(&conn, "netease", "888", "不存在的名字");
        assert!(miss.is_none());
    }
}
