//! 搜索与音质 hash 增强（mobiles 主搜索 + songsearch 元数据增强）。

use super::*;

// ---------- 搜索 ----------

/// 搜索（每页 30 条）。主通道为老 mobilecdn 明文接口——**songsearch_v2
/// 匿名请求会把 VIP 曲目整体过滤掉**（实测 pay_type==3 的歌不出现在结果
/// 里），必须以它为主才能展示 VIP 曲目；songsearch_v2 仅按 hash 做元数据
/// 增强（高音质文件 hash、更全的封面），失败不影响搜索结果。
pub fn search(keyword: &str, page: i64) -> Result<Vec<KgSong>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(vec![]);
    }
    let mut songs = search_fallback(keyword, page)?;
    enrich_from_songsearch(&mut songs, keyword, page);
    Ok(songs)
}

/// 存档行（最近播放/收藏/歌单）只落库了 128 hash：按"标题+歌手"反查补齐
/// 各档，已有值保持不变（engine/commands 的 HQ/无损取链前置步骤）。
/// artist 为空时仅用标题——同名翻唱可能导致匹配不到原曲，调用方应尽量传入
pub fn enrich_hashes(
    hash: &str,
    title: &str,
    artist: &str,
    hq_hash: &str,
    sq_hash: &str,
    super_hash: &str,
) -> (String, String, String) {
    // 三档全齐才无需反查：此前 hq/sq 齐但 super 为空就早退，
    // Hi-Res 的 super hash 永远补不上——音质选无损/Hi-Res 永远只拿到 flac
    if !sq_hash.is_empty() && !hq_hash.is_empty() && !super_hash.is_empty() {
        return (
            hq_hash.to_string(),
            sq_hash.to_string(),
            super_hash.to_string(),
        );
    }
    let keyword = if artist.trim().is_empty() {
        title.to_string()
    } else {
        format!("{title} {artist}")
    };
    match quality_hashes_by_search(hash, &keyword) {
        Some((h, s, sup)) => (
            if hq_hash.is_empty() {
                h
            } else {
                hq_hash.to_string()
            },
            if sq_hash.is_empty() {
                s
            } else {
                sq_hash.to_string()
            },
            if super_hash.is_empty() {
                sup
            } else {
                super_hash.to_string()
            },
        ),
        None => (
            hq_hash.to_string(),
            sq_hash.to_string(),
            super_hash.to_string(),
        ),
    }
}

/// 按 128 hash 反查各档质量 hash：最近播放/收藏/歌单等存档行只落库了
/// 128 hash，播放/下载时补齐，音质设置（HQ/无损）才能生效。
/// keyword 必须含歌手名（只按标题搜会命中同名翻唱、匹配不到原曲行）。
/// 主通道用 v3 搜索（结果自带 320hash/sqhash，且与存档 hash 同源），
/// 按 FileHash 精确匹配；未命中再试 song_search_v2。
pub fn quality_hashes_by_search(hash: &str, keyword: &str) -> Option<(String, String, String)> {
    if hash.is_empty() || keyword.trim().is_empty() {
        return None;
    }
    // 结果近静态：内存 TTL 缓存。此前同一首歌每次点播/下载都重新搜索
    // 1-2 次（串行追加在点播关键路径上），白白多 1-2 个 RTT。
    // 分级 TTL：三档齐（含 Hi-Res 标识）30 分钟；super 为空只 5 分钟——
    // 空 super 往往是 v2 瞬时失败/风控，长缓存会把“永远拿不到 Hi-Res”钉死
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<
                String,
                (std::time::Instant, Option<(String, String, String)>),
            >,
        >,
    > = std::sync::OnceLock::new();
    const TTL_FULL: std::time::Duration = std::time::Duration::from_secs(30 * 60);
    const TTL_NO_HIRES: std::time::Duration = std::time::Duration::from_secs(5 * 60);
    const CAP: usize = 256;
    let slot = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = slot.lock() {
        if let Some((at, hit)) = guard.get(&hash.to_lowercase()) {
            let ttl = match hit {
                Some((_, _, sup)) if !sup.is_empty() => TTL_FULL,
                _ => TTL_NO_HIRES,
            };
            if at.elapsed() < ttl {
                return hit.clone();
            }
        }
    }
    let result = quality_hashes_by_search_uncached(hash, keyword);
    if let Ok(mut guard) = slot.lock() {
        while guard.len() >= CAP {
            if let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone())
            {
                guard.remove(&oldest);
            } else {
                break;
            }
        }
        guard.insert(
            hash.to_lowercase(),
            (std::time::Instant::now(), result.clone()),
        );
    }
    result
}

fn quality_hashes_by_search_uncached(
    hash: &str,
    keyword: &str,
) -> Option<(String, String, String)> {
    // v3 主通道：320hash/sqhash 可靠，但 viphash（Hi-Res 标识）经常为空
    let mut best: Option<(String, String, String)> = None;
    if let Ok(songs) = search_fallback(keyword, 1) {
        if let Some(s) = songs.iter().find(|s| s.id.eq_ignore_ascii_case(hash)) {
            if !s.hq_hash.is_empty() || !s.sq_hash.is_empty() {
                best = Some((s.hq_hash.clone(), s.sq_hash.clone(), s.super_hash.clone()));
            }
        }
    }
    // 缺 super（Hi-Res 文件标识）就补查 songsearch_v2：SuperFileHash 是
    // Hi-Res 取链的可靠来源——此前 v3 命中即返回，super 为空不再查 v2，
    // Hi-Res 阶梯永远没有 super hash 可用（“有权益却永远拿无损”的根因）
    if best.as_ref().map_or(true, |(_, _, sup)| sup.is_empty()) {
        if let Some((h, s, sup)) = hashes_by_songsearch_v2(hash, keyword) {
            best = Some(match best {
                Some((bh, bs, _)) => (
                    if bh.is_empty() { h } else { bh },
                    if bs.is_empty() { s } else { bs },
                    sup,
                ),
                None => (h, s, sup),
            });
        }
    }
    // 第三来源：权益接口（get_res_privilege/lite 按 qualities 查询），
    // 响应条目可能带各档位 hash——容错解析（字段名随端点版本变化，
    // 递归找 {quality: super/high, hash: ...} 形态的对象）
    if best.as_ref().map_or(true, |(_, _, sup)| sup.is_empty()) {
        if let Some(sup) = hires_hash_by_privilege(hash) {
            eprintln!("[kugou] Hi-Res 标识由权益接口补齐");
            best = Some(match best {
                Some((bh, bs, _)) => (bh, bs, sup),
                None => (String::new(), String::new(), sup),
            });
        }
    }
    best
}

/// 权益接口反查 Hi-Res（super/high 档）hash：单个曲目查询，
/// 递归扫描响应里 “quality ∈ {super, high} 且带非空 hash” 的对象
fn hires_hash_by_privilege(hash: &str) -> Option<String> {
    let body = serde_json::json!({
        "appid": APPID.parse::<i64>().unwrap_or(1005),
        "area_code": 1,
        "behavior": "play",
        "clientver": CLIENTVER.parse::<i64>().unwrap_or(20489),
        "need_hash_offset": 1,
        "relate": 1,
        "support_verify": 1,
        "resource": [{
            "type": "audio", "page_id": 0,
            "hash": hash.to_uppercase(),
            "album_id": 0,
        }],
        "qualities": ["128", "320", "flac", "high", "super"],
    });
    let v = gateway(
        "/v2/get_res_privilege/lite",
        "media.store.kugou.com",
        &[],
        Some(&body),
    )
    .map_err(|e| format!("权益查询失败: {e}"))
    .ok()?;
    fn scan(node: &serde_json::Value, out: &mut Option<String>) {
        if out.is_some() {
            return;
        }
        match node {
            serde_json::Value::Object(m) => {
                let q = m
                    .get("quality")
                    .or_else(|| m.get("name"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if (q == "super" || q == "high") && !q.is_empty() {
                    if let Some(h) = m.get("hash").and_then(|x| x.as_str()) {
                        if !h.is_empty() {
                            *out = Some(h.to_lowercase());
                            return;
                        }
                    }
                }
                for v in m.values() {
                    scan(v, out);
                    if out.is_some() {
                        return;
                    }
                }
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    scan(v, out);
                    if out.is_some() {
                        return;
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = None;
    scan(&v, &mut out);
    if out.is_none() {
        eprintln!(
            "[kugou] 权益接口未解析出 Hi-Res hash（resp 前 200 字：{}）",
            serde_json::to_string(&v)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        );
    }
    out
}

/// songsearch_v2 按 128 hash 精确反查各档 hash（SuperFileHash 是 Hi-Res 的可靠来源）
fn hashes_by_songsearch_v2(hash: &str, keyword: &str) -> Option<(String, String, String)> {
    let url = format!(
        "https://songsearch.kugou.com/song_search_v2?keyword={}&page=1&pagesize=30",
        form_encode(keyword)
    );
    let v = match get_json(&url, "https://www.kugou.com/", UA, "酷狗搜索增强") {
        Ok(v) => v,
        Err(e) => {
            // 此处失败被静默吞掉会让 Hi-Res 标识永远补不上——必须留痕
            eprintln!("[kugou] songsearch_v2 请求失败（kw={keyword}）：{e}");
            return None;
        }
    };
    let list = match v.pointer("/data/lists").and_then(|x| x.as_array()) {
        Some(l) => l,
        None => {
            eprintln!("[kugou] songsearch_v2 响应无 data/lists（kw={keyword}）");
            return None;
        }
    };
    let t = match list.iter().find(|t| {
        t.get("FileHash")
            .and_then(|h| h.as_str())
            .map(|h| h.eq_ignore_ascii_case(hash))
            .unwrap_or(false)
    }) {
        Some(t) => t,
        None => {
            eprintln!(
                "[kugou] songsearch_v2 结果未命中 FileHash（kw={keyword}，{} 条）",
                list.len()
            );
            return None;
        }
    };
    let g = |k: &str| {
        t.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_lowercase()
    };
    Some((g("HQFileHash"), g("SQFileHash"), g("SuperFileHash")))
}

/// 用 songsearch_v2 的结果按 hash 补充多音质 hash 与缺失封面
fn enrich_from_songsearch(songs: &mut [KgSong], keyword: &str, page: i64) {
    let url = format!(
        "https://songsearch.kugou.com/song_search_v2?keyword={}&page={page}&pagesize=30",
        form_encode(keyword)
    );
    let Ok(v) = get_json(&url, "https://www.kugou.com/", UA, "酷狗搜索增强") else {
        return;
    };
    let Some(list) = v.pointer("/data/lists").and_then(|x| x.as_array()) else {
        return;
    };
    let by_hash: std::collections::HashMap<String, &serde_json::Value> = list
        .iter()
        .filter_map(|t| {
            t.get("FileHash")
                .and_then(|h| h.as_str())
                .map(|h| (h.to_lowercase(), t))
        })
        .collect();
    for s in songs.iter_mut() {
        let Some(t) = by_hash.get(&s.id.to_lowercase()) else {
            continue;
        };
        let get_str = |k: &str| -> String {
            t.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_lowercase()
        };
        if s.hq_hash.is_empty() {
            s.hq_hash = get_str("HQFileHash");
        }
        if s.sq_hash.is_empty() {
            s.sq_hash = get_str("SQFileHash");
        }
        if s.super_hash.is_empty() {
            s.super_hash = get_str("SuperFileHash");
        }
        if s.cover.is_empty() {
            s.cover = cover_sized(t.get("Image").and_then(|x| x.as_str()).unwrap_or(""), 240);
        }
    }
}

/// 搜索：mobiles v3 接口（https；无多音质 hash，songsearch 仅做元数据增强）
fn search_fallback(keyword: &str, page: i64) -> Result<Vec<KgSong>, String> {
    let url = format!(
        "https://mobiles.kugou.com/api/v3/search/song?format=json&keyword={}&page={page}&pagesize=30",
        form_encode(keyword)
    );
    let v = get_json(&url, "https://mobiles.kugou.com/", UA, "酷狗搜索")?;
    if v.get("status").and_then(|s| s.as_i64()) != Some(1) {
        return Err("酷狗搜索失败".into());
    }
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/info").and_then(|x| x.as_array()) {
        for t in list {
            let hash = t.get("hash").and_then(|v| v.as_str()).unwrap_or("");
            let name = t
                .get("songname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if hash.is_empty() || name.is_empty() {
                continue;
            }
            out.push(KgSong {
                id: hash.to_string(),
                name,
                singer: t
                    .get("singername")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                album: t
                    .get("album_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                duration_ms: t
                    .get("duration")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    .max(0) as u64
                    * 1000,
                cover: cover_sized(
                    t.pointer("/trans_param/union_cover")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    240,
                ),
                vip: t.get("pay_type").and_then(|v| v.as_i64()).unwrap_or(0) == 3,
                album_audio_id: json_num(t.get("album_audio_id")),
                album_id: json_num(t.get("album_id")),
                // v3 搜索自带各档 hash（320hash/sqhash/viphash），登录后按音质取链必需
                hq_hash: t
                    .get("320hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase(),
                sq_hash: t
                    .get("sqhash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase(),
                super_hash: t
                    .get("viphash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase(),
            });
        }
    }
    Ok(out)
}
