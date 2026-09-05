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
  added_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlists (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlist_tracks (
  playlist_id INTEGER NOT NULL,
  track_id INTEGER NOT NULL,
  position INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (playlist_id, track_id)
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
  track_id INTEGER PRIMARY KEY
);
"#;

pub fn init(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
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
        Ok(Folder { id: r.get(0)?, path: r.get(1)? })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn add_folder(conn: &Connection, path: &str) -> Result<i64, String> {
    conn.execute("INSERT OR IGNORE INTO folders(path) VALUES(?1)", params![path])
        .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn remove_folder(conn: &Connection, id: i64) {
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

/// 删除不再存在于扫描目录内的曲目（仅清理位于 folders 前缀下的记录）
pub fn delete_missing(conn: &Connection, seen: &HashSet<String>, folder_prefixes: &[String]) {
    let stale: Vec<String> = track_paths(conn)
        .into_iter()
        .map(|(p, _, _)| p)
        .filter(|p| {
            if seen.contains(p) {
                return false;
            }
            folder_prefixes
                .iter()
                .any(|f| p.starts_with(f.as_str()))
        })
        .collect();
    for p in stale {
        let id: Option<i64> = conn
            .query_row("SELECT id FROM tracks WHERE path = ?1", params![p], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        if let Some(id) = id {
            let _ = conn.execute("DELETE FROM tracks WHERE id = ?1", params![id]);
            let _ = conn.execute("DELETE FROM playlist_tracks WHERE track_id = ?1", params![id]);
            let _ = conn.execute("DELETE FROM stats WHERE track_id = ?1", params![id]);
            let _ = conn.execute("DELETE FROM liked WHERE track_id = ?1", params![id]);
        }
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
    })
}

const TRACK_SELECT: &str = r#"
SELECT t.id, t.path, t.title, t.artist, t.album, t.album_artist, t.track_no, t.disc, t.year,
       t.duration, t.format, t.bitrate, t.sample_rate, t.bit_depth, t.cover, t.lrc_path,
       t.size, t.mtime,
       CASE WHEN l.track_id IS NULL THEN 0 ELSE 1 END,
       COALESCE(s.play_count, 0), COALESCE(s.last_played, 0)
FROM tracks t
LEFT JOIN liked l ON l.track_id = t.id
LEFT JOIN stats s ON s.track_id = t.id
"#;

pub fn list_tracks(conn: &Connection) -> Vec<TrackMeta> {
    let mut stmt = match conn.prepare(&(TRACK_SELECT.to_string() + "ORDER BY t.id")) {
        Ok(s) => s,
        Err(_) => return vec![],
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
        .query_row("SELECT lrc_path FROM tracks WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten();
    v.filter(|s| !s.is_empty())
}

pub fn like_track(conn: &Connection, id: i64, on: bool) {
    if on {
        let _ = conn.execute("INSERT OR IGNORE INTO liked(track_id) VALUES(?1)", params![id]);
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
    let mut stmt = match conn.prepare("SELECT id, name, created_at FROM playlists ORDER BY id") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let mut out: Vec<Playlist> = stmt
        .query_map([], |r| {
            Ok(Playlist {
                id: r.get(0)?,
                name: r.get(1)?,
                track_ids: vec![],
                created_at: r.get(2)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    let mut pstm = match conn.prepare(
        "SELECT playlist_id, track_id FROM playlist_tracks ORDER BY position, track_id",
    ) {
        Ok(s) => s,
        Err(_) => return out,
    };
    let pairs: Vec<(i64, i64)> = pstm
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    for (pid, tid) in pairs {
        if let Some(pl) = out.iter_mut().find(|p| p.id == pid) {
            pl.track_ids.push(tid);
        }
    }
    out
}

pub fn create_playlist(conn: &Connection, name: &str) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO playlists(name, created_at) VALUES(?1, ?2)",
        params![name, now_secs()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_playlist(conn: &Connection, id: i64) {
    let _ = conn.execute("DELETE FROM playlists WHERE id = ?1", params![id]);
    let _ = conn.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id]);
}

pub fn rename_playlist(conn: &Connection, id: i64, name: &str) {
    let _ = conn.execute("UPDATE playlists SET name = ?2 WHERE id = ?1", params![id, name]);
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
    let mut stmt = match conn.prepare("SELECT id, url, title, created_at FROM sources ORDER BY id DESC") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |r| {
        Ok(SourceItem { id: r.get(0)?, url: r.get(1)?, title: r.get(2)?, created_at: r.get(3)? })
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
    let _ = conn.execute("UPDATE sources SET title = ?2 WHERE id = ?1", params![id, title]);
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
