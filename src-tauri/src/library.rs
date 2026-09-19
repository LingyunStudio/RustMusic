use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use lofty::prelude::*;

use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

use crate::db::{self, NewTrack};
use crate::AppState;

pub const AUDIO_EXTS: [&str; 9] = ["mp3", "flac", "wav", "ogg", "oga", "m4a", "aac", "mp4", "m4b"];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// 规范化路径（去掉 Windows \\?\ 前缀），统一用反斜杠
pub fn norm_path(p: &Path) -> String {
    let s = fs::canonicalize(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .into_owned();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

pub fn spawn_scan(app: &AppHandle) {
    let a = app.clone();
    std::thread::spawn(move || run_scan(a));
}

pub fn run_scan(app: AppHandle) {
    let st = app.state::<AppState>();
    let folders = {
        let conn = st.db.lock();
        db::list_folders(&conn)
    };
    let prefixes: Vec<String> = folders.iter().map(|f| f.path.clone()).collect();

    let emit = |done: usize, total: usize, active: bool| {
        let payload = serde_json::json!({ "done": done, "total": total, "active": active });
        // 快照留给挂起恢复后的补发（webview://resumed → get_scan_state）
        if let Some(st) = app.try_state::<AppState>() {
            *st.scan_last.lock() = payload.clone();
        }
        let _ = app.emit("scan://progress", payload);
    };

    if folders.is_empty() {
        emit(0, 0, false);
        return;
    }

    // 收集音频文件
    let mut files: Vec<PathBuf> = Vec::new();
    for f in &folders {
        for entry in WalkDir::new(&f.path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && is_audio(entry.path()) {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    let total = files.len();
    emit(0, total, true);

    // 现有库记录
    let existing: HashMap<String, (i64, i64)> = {
        let conn = st.db.lock();
        db::track_paths(&conn)
            .into_iter()
            .map(|(p, mt, sz)| (p, (mt, sz)))
            .collect()
    };

    let mut seen: HashSet<String> = HashSet::with_capacity(files.len());
    let mut to_parse: Vec<PathBuf> = Vec::new();
    for p in &files {
        let np = norm_path(p);
        seen.insert(np.clone());
        let (mtime, size) = file_stat(p);
        match existing.get(&np) {
            Some((emt, esz)) if *emt == mtime && *esz == size => {}
            _ => to_parse.push(p.clone()),
        }
    }

    // 并行解析 + 分批入库
    let mut done = total - to_parse.len();
    emit(done, total, true);
    for chunk in to_parse.chunks(24) {
        let app_data = st.app_data.clone();
        let parsed: Vec<Option<NewTrack>> = chunk
            .par_iter()
            .map(|p| parse_track(p, &app_data))
            .collect();
        {
            let conn = st.db.lock();
            for t in parsed.into_iter().flatten() {
                db::upsert_track(&conn, &t);
            }
        }
        done += chunk.len();
        emit(done, total, true);
    }

    // 清理已删除文件
    {
        let conn = st.db.lock();
        db::delete_missing(&conn, &seen, &prefixes);
    }
    emit(total, total, false);
}

fn file_stat(p: &Path) -> (i64, i64) {
    match fs::metadata(p) {
        Ok(m) => (
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            m.len() as i64,
        ),
        Err(_) => (0, 0),
    }
}

pub fn parse_track(path: &Path, app_data: &Path) -> Option<NewTrack> {
    let np = norm_path(path);
    let (mtime, size) = file_stat(path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let format = match ext.as_str() {
        "mp3" => "MP3",
        "flac" => "FLAC",
        "wav" => "WAV",
        "ogg" | "oga" => "OGG",
        "m4a" | "mp4" | "m4b" => "M4A",
        "aac" => "AAC",
        _ => "AUDIO",
    }
    .to_string();

    let (mut title, mut artist, mut album, mut album_artist) =
        (String::new(), String::new(), String::new(), String::new());
    let (mut track_no, mut disc, mut year) = (0i64, 0i64, 0i64);
    let (mut duration, mut bitrate, mut sample_rate, mut bit_depth) = (0f64, 0i64, 0i64, 0i64);
    let mut cover = String::new();

    let mut lrc_path = String::new();
    let lrc_candidate = path.with_extension("lrc");
    if lrc_candidate.exists() {
        lrc_path = lrc_candidate.to_string_lossy().into_owned();
    }

    let tagged = lofty::read_from_path(path).ok();
    if let Some(tagged) = &tagged {
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
        if let Some(tag) = tag {
            title = tag.title().map(|s| s.to_string()).unwrap_or_default();
            artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
            album = tag.album().map(|s| s.to_string()).unwrap_or_default();
            album_artist = tag
                .get_string(&lofty::tag::ItemKey::AlbumArtist)
                .unwrap_or("")
                .to_string();
            track_no = tag.track().map(|v| v as i64).unwrap_or(0);
            disc = tag.disk().map(|v| v as i64).unwrap_or(0);
            year = tag
                .get_string(&lofty::tag::ItemKey::Year)
                .and_then(|s| s.chars().take(4).collect::<String>().parse().ok())
                .unwrap_or(0);

            let pic = tag
                .pictures()
                .iter()
                .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
                .or_else(|| tag.pictures().first());
            if let Some(pic) = pic {
                cover = save_cover(app_data, &np, pic.data(), pic.mime_type()).unwrap_or_default();
            }
        }
        let props = tagged.properties();
        duration = props.duration().as_secs_f64();
        bitrate = props.audio_bitrate().map(|v| v as i64).unwrap_or(0);
        sample_rate = props.sample_rate().map(|v| v as i64).unwrap_or(0);
        bit_depth = props.bit_depth().map(|v| v as i64).unwrap_or(0);
    }

    // 无标签时回退到文件名解析
    if title.is_empty() {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("未知曲目");
        if let Some((a, t)) = stem.split_once(" - ") {
            if artist.is_empty() {
                artist = a.trim().to_string();
            }
            title = t.trim().to_string();
        } else {
            title = stem.to_string();
        }
    }

    Some(NewTrack {
        path: np,
        title,
        artist,
        album,
        album_artist,
        track_no,
        disc,
        year,
        duration,
        format,
        bitrate,
        sample_rate,
        bit_depth,
        cover,
        lrc_path,
        size,
        mtime,
    })
}

fn save_cover(
    app_data: &Path,
    track_path: &str,
    data: &[u8],
    mime: Option<&lofty::picture::MimeType>,
) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    track_path.hash(&mut h);
    let name = format!("{:016x}", h.finish());
    let dir = app_data.join("covers");
    let _ = fs::create_dir_all(&dir);

    // 优先解码并压缩为统一尺寸的 JPEG
    if let Ok(img) = image::load_from_memory(data) {
        let img = if img.width() > 600 || img.height() > 600 {
            img.thumbnail(600, 600)
        } else {
            img
        };
        let dest = dir.join(format!("{name}.jpg"));
        if img
            .to_rgb8()
            .save_with_format(&dest, image::ImageFormat::Jpeg)
            .is_ok()
        {
            return Some(dest.to_string_lossy().into_owned());
        }
    }

    // 解码失败则按原始格式保存
    let ext = match mime {
        Some(lofty::picture::MimeType::Png) => "png",
        Some(lofty::picture::MimeType::Jpeg) => "jpg",
        Some(lofty::picture::MimeType::Bmp) => "bmp",
        Some(lofty::picture::MimeType::Gif) => "gif",
        Some(lofty::picture::MimeType::Tiff) => "tiff",
        _ => "img",
    };
    let dest = dir.join(format!("{name}.{ext}"));
    fs::write(&dest, data).ok()?;
    Some(dest.to_string_lossy().into_owned())
}
