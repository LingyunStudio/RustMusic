//! 在线音源缓存与边下边播：缓存键设计、下载编排（进度事件/流式供数/LRU 清理）。
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use lofty::prelude::*;
use tauri::{AppHandle, Emitter, Manager};

use super::{Engine, TrackInfo};

impl Engine {
    // ---------- 在线音源 ----------

    /// 稳定的缓存键：网易云/QQ 的 CDN 直链每次请求都带新的签名参数，
    /// 按 URL 哈希命名会导致同一首歌每次播放都重新下载；
    /// 因此用歌曲 ID + 实际音质做键（同一首歌同一音质只缓存一份）。
    /// 返回 (缓存键, 扩展名)。ext 用于从 URL 提前确定文件扩展名。
    fn cache_key_for(&self, url: &str, info: &TrackInfo) -> (String, String) {
        let ext = url
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('.')
            .next()
            .map(|e| e.to_lowercase())
            .filter(|e| {
                matches!(
                    e.as_str(),
                    "mp3" | "flac" | "wav" | "ogg" | "oga" | "m4a" | "aac" | "mp4" | "m4b"
                        | "m4s"
                )
            })
            .unwrap_or_else(|| "bin".into());
        // 实际音质（FLAC 按无损档），落到缓存键里：换音质后不命中旧缓存
        let q = info
            .quality
            .as_deref()
            .map(|q| {
                if q.eq_ignore_ascii_case("flac") {
                    "flac".to_string()
                } else {
                    q.replace([' ', 'k'], "")
                }
            })
            .unwrap_or_else(|| "d".into());
        let key = match (info.kind.as_str(), info.nid, info.qid.as_deref()) {
            ("netease", Some(nid), _) => format!("net-{nid}-{q}"),
            ("qq", _, Some(qid)) => format!("qq-{qid}-{q}"),
            ("kugou", _, Some(kgid)) => format!("kug-{kgid}-{q}"),
            // B 站：qid 承载 "bvid-cid"，同一分P同一音质只缓存一份
            ("bilibili", _, Some(qid)) => format!("bili-{qid}-{q}"),
            // Navidrome：qid 为歌曲 id（流地址里的 salt/token 每次都变，
            // 不能按 URL 哈希，否则永远不命中缓存）
            ("navidrome", _, Some(qid)) => format!("nd-{qid}-{q}"),
            // 自定义在线音源没有稳定 ID，仍按 URL 哈希
            _ => {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                url.hash(&mut h);
                format!("url-{h:016x}-{q}", h = h.finish())
            }
        };
        (key, ext)
    }

    fn cache_path_for(&self, key: &str, ext: &str) -> PathBuf {
        self.downloads_dir.join(format!("{key}.{ext}"))
    }

    /// 播放在线音源：有缓存直接播放，否则后台下载（带进度事件）完成后自动播放。
    /// 下载完成后按缓存上限做 LRU 清理。
    pub fn play_url(self: &Arc<Self>, url: String, info: TrackInfo) -> Result<(), String> {
        let url = url.trim().to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("音源地址必须以 http:// 或 https:// 开头".into());
        }
        if url.contains(".m3u8") {
            return Err("暂不支持 m3u8/HLS 流，请使用音频文件直链".into());
        }
        let (key, ext) = self.cache_key_for(&url, &info);
        let cache = self.cache_path_for(&key, &ext);
        if cache.exists() && cache.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            // 命中缓存：更新访问时间（LRU 依据），当前曲目直接播放
            let _ = filetime::set_file_mtime(
                &cache,
                filetime::FileTime::from_system_time(std::time::SystemTime::now()),
            );
            let mut info = info;
            info.path = cache.to_string_lossy().into_owned();
            if info.duration_ms == 0 {
                info.duration_ms = probe_duration(&cache);
            }
            *self.want_url.write() = None;
            return self.play_file(info);
        }
        *self.want_url.write() = Some(url.clone());
        // 同一缓存键已有下载在进行：只登记意图后返回，复用进行中的下载，
        // 避免两个线程同时写同一个 .part 缓存文件导致内容损坏
        {
            let mut dl = self.downloading.write();
            if dl.contains(&key) {
                return Ok(());
            }
            dl.insert(key.clone());
        }
        // B 站 CDN 直链必须带 Referer/UA（部分边缘节点裸请求 403）；
        // 且 playurl 每次可能返回不同 CDN 主机，不能按主机名判断，按来源标记
        let is_bili = info.kind == "bilibili";
        let engine = Arc::clone(self);
        let app = self.app.clone();

        // 边下边播：下载照常落盘 .part，预缓冲达标（或整段完成）即起播，
        // 播放端读取器越过下载前沿时阻塞等待新数据，无需等整段下完。
        let shared = crate::streaming::StreamingDownload::new(
            cache.with_extension("part"),
            cache.clone(),
        );
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // 监视线程：预缓冲达标后以流式源起播（仅当用户仍在等这首）。
        // 与下载线程的"完成后播放"通过 started 原子交换保证只有一方起播。
        {
            let engine = Arc::clone(&engine);
            let shared = Arc::clone(&shared);
            let started = Arc::clone(&started);
            let url = url.clone();
            let info = info.clone();
            let cache = cache.clone();
            std::thread::spawn(move || loop {
                let (_ready, done, failed) = shared.wait_prebuffer(512 * 1024);
                if failed || done || started.load(Ordering::Relaxed) {
                    // 整段完成 → 由下载线程负责最终播放；失败 → 其已回报错误
                    break;
                }
                let armed = engine.want_url.read().as_deref() == Some(url.as_str());
                if !armed {
                    // 用户暂已切走：继续等下载完成（重新点播可在完成前再入本分支）
                    continue;
                }
                // 预缓冲达标：嗅探容器（此时文件头必然已落盘）
                if let Ok(mut f) = std::fs::File::open(&shared.part) {
                    let mut head = [0u8; 8];
                    use std::io::Read as _;
                    if f.read(&mut head).unwrap_or(0) == 8 && head[4..8] == *b"ftyp" {
                        shared.set_mp4(true);
                    }
                } else {
                    break;
                }
                // 先登记流式关联再置 started：open_current_source 与下载线程的
                // 兜底播放判断都依赖它。起播走 play_file 完整流程——
                // 独占模式开启时同样能协商独占会话（读取越界阻塞由流读取器承担）
                let mut i = info.clone();
                i.path = cache.to_string_lossy().into_owned();
                *engine.active_streaming.write() = Some((i.path.clone(), Arc::clone(&shared)));
                started.store(true, Ordering::SeqCst);
                *engine.want_url.write() = None;
                if let Err(e) = engine.play_file(i) {
                    let _ = engine.app.emit(
                        "download://progress",
                        serde_json::json!({
                            "url": url, "done": true,
                            "error": format!("播放失败：{e}")
                        }),
                    );
                }
                break;
            });
        }

        std::thread::spawn(move || {
            let result = download_to(&app, &url, &cache, is_bili, Some(&shared));
            engine.downloading.write().remove(&key);
            match result {
                Err(e) => {
                    let _ = app.emit(
                        "download://progress",
                        serde_json::json!({ "url": url, "done": true, "error": e }),
                    );
                    // 用户仍在等这首时清除意图，便于下次点击重新发起下载
                    if engine.want_url.read().as_deref() == Some(url.as_str()) {
                        *engine.want_url.write() = None;
                    }
                }
                Ok(()) => {
                    // 流式源仍在播这首 → 不重复起播；否则（小文件整段下完 /
                    // 总长未知 / 流式起播失败 / 跳歌后重播）在此播放完整缓存
                    let part_str = cache.to_string_lossy().into_owned();
                    let streaming_active = started.load(Ordering::SeqCst)
                        && engine
                            .active_streaming
                            .read()
                            .as_ref()
                            .map_or(false, |(p, _)| *p == part_str);
                    if !streaming_active {
                        let still_wanted =
                            engine.want_url.read().as_deref() == Some(url.as_str());
                        if still_wanted {
                            let mut info = info;
                            info.path = cache.to_string_lossy().into_owned();
                            if info.duration_ms == 0 {
                                info.duration_ms = probe_duration(&cache);
                            }
                            // 播放失败必须回报：此前静默吞噬，前端停在"播放中"
                            // 却没有任何声音，用户无从得知原因
                            if let Err(e) = engine.play_file(info) {
                                let _ = app.emit(
                                    "download://progress",
                                    serde_json::json!({
                                        "url": url, "done": true,
                                        "error": format!("播放失败：{e}")
                                    }),
                                );
                            }
                        }
                    }
                    // 下载成功后按上限清理（跳过正在播放/下载中的文件）
                    engine.evict_cache();
                }
            }
        });
        Ok(())
    }

    // ---------- 缓存管理 ----------

    /// 缓存目录内所有完整缓存文件（含大小与最后访问时间），按新旧降序
    fn cache_entries(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.downloads_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) == Some("part") {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((p, meta.len(), mtime));
            }
        }
        out.sort_by(|a, b| b.2.cmp(&a.2)); // 新 -> 旧
        out
    }

    /// 当前缓存占用（字节）与文件数
    pub fn cache_usage(&self) -> (u64, u32) {
        let mut total = 0u64;
        let mut count = 0u32;
        for (_, len, _) in self.cache_entries() {
            total += len;
            count += 1;
        }
        (total, count)
    }

    /// 强制清理全部缓存；正在播放的文件跳过（Windows 上删除会失败）。
    /// 返回删除的文件数。
    pub fn clear_cache(&self) -> u32 {
        let playing = self
            .current
            .read()
            .as_ref()
            .map(|c| c.path.clone())
            .unwrap_or_default();
        let mut n = 0u32;
        for (p, _, _) in self.cache_entries() {
            if !playing.is_empty() && p == PathBuf::from(&playing) {
                continue;
            }
            if std::fs::remove_file(&p).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// 缓存上限（字节），0 = 不限制
    pub fn cache_limit(&self) -> u64 {
        self.cache_limit.load(Ordering::Relaxed)
    }

    pub fn set_cache_limit(&self, bytes: u64) {
        self.cache_limit.store(bytes, Ordering::Relaxed);
        self.evict_cache();
    }

    /// LRU 清理：超过上限时从最旧开始删，直到回到上限内。
    /// 跳过正在播放的文件和 .part 下载中间文件（后者不占上限，由下载流程自管）。
    fn evict_cache(&self) {
        let limit = self.cache_limit();
        if limit == 0 {
            return;
        }
        let playing = self
            .current
            .read()
            .as_ref()
            .map(|c| c.path.clone())
            .unwrap_or_default();
        let entries = self.cache_entries();
        let mut total = 0u64;
        for (_, len, _) in &entries {
            total += len;
        }
        if total <= limit {
            return;
        }
        for (p, len, _) in entries.iter().rev() {
            if total <= limit {
                break;
            }
            if !playing.is_empty() && *p == PathBuf::from(&playing) {
                continue;
            }
            if std::fs::remove_file(p).is_ok() {
                total = total.saturating_sub(*len);
            }
        }
    }
}
fn probe_duration(path: &Path) -> u64 {
    lofty::read_from_path(path)
        .ok()
        .map(|t| t.properties().duration().as_millis() as u64)
        .unwrap_or(0)
}
/// 下载 URL 到本地缓存文件，通过 download://progress 事件回报进度
fn download_to(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    with_bili_headers: bool,
    stream: Option<&Arc<crate::streaming::StreamingDownload>>,
) -> Result<(), String> {
    let r = download_inner(app, url, dest, with_bili_headers, stream);
    if let Some(s) = stream {
        // 保证失败时读取端也能立即终止（成功路径已在 inner 内 finish）
        if r.is_err() {
            s.finish(false, 0);
        }
    }
    r
}

fn download_inner(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    with_bili_headers: bool,
    stream: Option<&Arc<crate::streaming::StreamingDownload>>,
) -> Result<(), String> {
    let part = dest.with_extension("part");
    // 连接与读取分段超时：整体超时会在大文件（FLAC 等几十 MB）下载中途掐断连接
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .build();
    let mut req = agent.get(url);
    // B 站 CDN 直链必须带 Referer 与浏览器 UA，否则部分边缘节点裸请求 403
    if with_bili_headers {
        req = req
            .set("Referer", "https://www.bilibili.com/")
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36");
    }
    let resp = req.call().map_err(|e| format!("下载音源失败: {e}"))?;
    let total: u64 = resp
        .header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if let Some(s) = stream {
        s.set_total(total);
    }

    let title_hint = resp
        .header("content-disposition")
        .and_then(|v| {
            v.split(';')
                .rev()
                .find_map(|seg| seg.trim().strip_prefix("filename="))
                .map(|s| s.trim_matches('"').to_string())
        })
        .unwrap_or_default();

    let mut file = File::create(&part).map_err(|e| format!("创建缓存文件失败: {e}"))?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let io_result = (|| -> Result<(), String> {
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("下载数据流中断: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("写入缓存失败: {e}"))?;
            received += n as u64;
            // 边下边播：推进数据前沿，唤醒阻塞中的播放端读取器
            if let Some(s) = stream {
                s.publish(received);
            }
            if last_emit.elapsed() >= Duration::from_millis(300) {
                // WebView 挂起（托盘隐藏）时跳过进度推送：避免反复唤醒渲染进程。
                // 恢复后前端 webview://resumed 兜底复位下载条
                let suspended = app
                    .try_state::<crate::AppState>()
                    .map(|st| st.webview_suspended.load(Ordering::SeqCst))
                    .unwrap_or(false);
                if !suspended {
                    last_emit = std::time::Instant::now();
                    let pct = if total > 0 {
                        (received as f64 / total as f64 * 100.0) as u64
                    } else {
                        0
                    };
                    let _ = app.emit(
                        "download://progress",
                        serde_json::json!({ "url": url, "received": received, "total": total, "pct": pct, "done": false }),
                    );
                }
            }
        }
        Ok(())
    })();
    if let Some(s) = stream {
        // 先终态后重命名：读取器收到 EOF 后释放文件，重命名成功率更高
        s.finish(io_result.is_ok(), received);
    }
    io_result?;
    drop(file);
    // 播放端可能仍持有 .part 句柄（流式播放中）：Windows 上重命名会失败，
    // 短暂重试，仍失败则留给读取器 Drop 时收尾
    let mut renamed = false;
    for _ in 0..30 {
        match std::fs::rename(&part, dest) {
            Ok(()) => {
                renamed = true;
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    if !renamed {
        eprintln!("[engine] .part 暂无法重命名（播放中仍持有），由读取器收尾");
    }
    let _ = app.emit(
        "download://progress",
        serde_json::json!({ "url": url, "received": received, "total": if total == 0 { received } else { total }, "pct": 100, "done": true }),
    );
    if !title_hint.is_empty() {
        let st = app.state::<crate::AppState>();
        let conn = st.db.lock();
        if let Some(row) = db_lookup_source(&conn, url) {
            if row.1.is_empty() {
                crate::db::update_source_title(&conn, row.0, &title_hint);
            }
        }
    }
    Ok(())
}

fn db_lookup_source(conn: &rusqlite::Connection, url: &str) -> Option<(i64, String)> {
    conn.query_row(
        "SELECT id, title FROM sources WHERE url = ?1",
        rusqlite::params![url],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}
