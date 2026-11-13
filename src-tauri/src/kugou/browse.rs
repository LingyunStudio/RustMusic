//! 排行榜、歌单广场与歌单详情（匿名）。

use super::*;

// ---------- 排行榜（匿名） ----------

/// 官方榜单列表（TOP500 / 飙升榜 / 热歌榜等，老明文接口免签名）
pub fn toplists() -> Result<Vec<KgToplist>, String> {
    let v = get_json(
        "https://mobiles.kugou.com/api/v3/rank/list?format=json",
        "https://mobiles.kugou.com/",
        UA,
        crate::i18n::pick("酷狗榜单列表", "KuGou charts list"),
    )?;
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/info").and_then(|x| x.as_array()) {
        for t in list {
            let id = t.get("rankid").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = t
                .get("rankname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id == 0 || name.is_empty() {
                continue;
            }
            let pic = cover_sized(
                t.get("imgurl")
                    .or_else(|| t.get("img_9"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                240,
            );
            out.push(KgToplist { id, name, pic });
        }
    }
    Ok(out)
}

/// 榜单曲目（网关 kmr 接口，匿名可调）。内部翻页拉全：榜单实际 100~500 首
/// （TOP500 等），单页 30 只能给个零头；每页 90，封顶 20 页防异常死循环
pub fn toplist_tracks(rankid: i64, page: i64) -> Result<Vec<KgSong>, String> {
    let pagesize = 90i64;
    let body = serde_json::json!({
        "show_portrait_mv": 1,
        "show_type_total": 1,
        "filter_original_remarks": 1,
        "area_code": 1,
        "pagesize": pagesize,
        "rank_cid": 0,
        "type": 1,
        "page": page.max(1),
        "rank_id": rankid,
    });
    let v = gateway_ex(
        "/openapi/kmr/v2/rank/audio",
        "",
        &[],
        Some(&body),
        &[("kg-tid", "369")],
    )
    .map_err(|e| format!("{}: {e}", crate::i18n::pick("获取榜单歌曲失败", "Failed to fetch chart tracks")))?;
    let list = v
        .pointer("/data/songlist")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut songs: Vec<KgSong> = list.iter().filter_map(song_from_kmr).collect();
    enrich_vip(&mut songs);
    Ok(songs)
}

/// kmr 榜单条目 → KgSong（audio_info 里带各档 hash）
fn song_from_kmr(t: &serde_json::Value) -> Option<KgSong> {
    let name = t
        .get("songname")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let hash = t
        .pointer("/audio_info/hash_128")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    if hash.is_empty() {
        return None;
    }
    let info = |k: &str| -> String {
        t.pointer(&format!("/audio_info/{k}"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
    };
    let album_id = t.get("album_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let cover = cover_sized(
        t.pointer("/album_info/sizable_cover")
            .or_else(|| t.pointer("/trans_param/union_cover"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        240,
    );
    Some(KgSong {
        id: hash,
        name,
        singer: t
            .get("author_name")
            .or_else(|| t.pointer("/authors/0/author_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        album: t
            .pointer("/album_info/album_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        // duration_128 单位是毫秒（实测 403000 → 6:43），勿再换算；
        // 搜索接口的 duration 才是秒（见 search_fallback 的 *1000）
        duration_ms: t
            .pointer("/audio_info/duration_128")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0) as u64,
        cover,
        vip: false,
        album_audio_id: t
            .get("album_audio_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        album_id,
        hq_hash: info("hash_320"),
        sq_hash: info("hash_flac"),
        super_hash: info("hash_super"),
    })
}

// ---------- 歌单广场 / 歌单详情（匿名） ----------

/// 歌单推荐页（specialrec 服务）：返回 (分类下歌单, 是否还有更多)
pub(super) fn special_recommend(category_id: i64, page: i64) -> Result<Vec<KgPublicPlaylist>, String> {
    let clienttime = now_secs().to_string();
    // key = MD5(appid + 盐 + clientver + clienttime)
    let key = md5_hex(&format!("{APPID}{SALT_ANDROID}{CLIENTVER}{clienttime}"));
    let body = serde_json::json!({
        "appid": APPID.parse::<i64>().unwrap_or(1005),
        "mid": mid_of(),
        "clientver": CLIENTVER.parse::<i64>().unwrap_or(20489),
        "platform": "android",
        "clienttime": clienttime,
        "userid": 0,
        "module_id": 1,
        "page": page.max(1),
        "pagesize": 30,
        "key": key,
        "special_recommend": {
            "withtag": 1,
            "withsong": 1,
            "sort": 1,
            "ugc": 1,
            "is_selected": 0,
            "withrecommend": 1,
            "area_code": 1,
            "categoryid": category_id,
        },
        "req_multi": 1,
        "retrun_min": 5,
        "return_special_falg": 1,
    });
    let v = gateway(
        "/v2/special_recommend",
        "specialrec.service.kugou.com",
        &[],
        Some(&body),
    )
    .map_err(|e| {
        crate::i18n::pick(
            format!("获取歌单推荐失败: {e}"),
            format!("Failed to fetch recommended playlists: {e}"),
        )
    })?;
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/special_list").and_then(|x| x.as_array()) {
        for p in list {
            let id = p
                .get("global_collection_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = p
                .get("specialname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() || name.is_empty() {
                continue;
            }
            out.push(KgPublicPlaylist {
                id,
                name,
                cover: cover_sized(
                    p.get("imgurl")
                        .or_else(|| p.get("flexible_cover"))
                        .or_else(|| p.get("pic"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    480,
                ),
                play_count: p.get("play_count").and_then(|v| v.as_i64()).unwrap_or(0),
                creator: p
                    .get("nickname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                songs: vec![],
                // 服务端声称的曲目总数（诊断翻页完整性/详情页展示用）
                total_count: p.get("count").and_then(|v| v.as_i64()).unwrap_or(0),
            });
        }
    }
    Ok(out)
}

/// 歌单详情（global_collection_id 拉全曲目，每页 90 条翻页）
pub fn playlist_tracks(gcid: &str) -> Result<KgPublicPlaylist, String> {
    let mut name = String::new();
    let mut cover = String::new();
    let mut creator = String::new();
    let mut play_count = 0i64;
    let mut songs = Vec::new();
    let pagesize = 90i64;
    for page in 1..=20i64 {
        let extra = vec![
            ("area_code".into(), "1".into()),
            ("begin_idx".into(), ((page - 1) * pagesize).to_string()),
            ("plat".into(), "1".into()),
            ("type".into(), "1".into()),
            ("mode".into(), "1".into()),
            ("personal_switch".into(), "1".into()),
            (
                "extend_fields".into(),
                "abtags,hot_cmt,popularization".into(),
            ),
            ("pagesize".into(), pagesize.to_string()),
            ("global_collection_id".into(), gcid.to_string()),
        ];
        let v = gateway("/pubsongs/v2/get_other_list_file_nofilt", "", &extra, None)
            .map_err(|e| format!("{}: {e}", crate::i18n::pick("获取歌单详情失败", "Failed to fetch playlist details")))?;
        if page == 1 {
            if let Some(info) = v.pointer("/data/list_info") {
                name = info
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                cover = cover_sized(info.get("pic").and_then(|x| x.as_str()).unwrap_or(""), 480);
                creator = info
                    .get("list_create_username")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            play_count = 0; // 该接口不返回播放量，置 0（UI 不展示）
        }
        let list = v
            .pointer("/data/songs")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let got = list.len() as i64;
        for t in &list {
            if let Some(s) = song_from_playlist(t) {
                songs.push(s);
            }
        }
        if got < pagesize {
            break;
        }
    }
    if name.is_empty() && songs.is_empty() {
        return Err(crate::i18n::pick("歌单不存在或为空", "Playlist does not exist or is empty").into());
    }
    enrich_vip(&mut songs);
    Ok(KgPublicPlaylist {
        id: gcid.to_string(),
        name,
        cover,
        play_count,
        creator,
        songs,
        total_count: 0,
    })
}

/// 批量权益查询（media.store privilege_lite，匿名可调）：返回按入参顺序的
/// (hash→pay_type, album_id, album_audio_id) 映射。pay_type==3 即 VIP/付费。
fn privilege_map(
    items: &[(String, u64)],
) -> Result<std::collections::HashMap<String, (bool, u64, u64)>, String> {
    let mut out = std::collections::HashMap::new();
    for batch in items.chunks(30) {
        let body = serde_json::json!({
            "appid": APPID.parse::<i64>().unwrap_or(1005),
            "area_code": 1,
            "behavior": "play",
            "clientver": CLIENTVER.parse::<i64>().unwrap_or(20489),
            "need_hash_offset": 1,
            "relate": 1,
            "support_verify": 1,
            "resource": batch
                .iter()
                .map(|(h, aid)| serde_json::json!({
                    "type": "audio", "page_id": 0,
                    "hash": h.to_uppercase(),
                    "album_id": aid,
                }))
                .collect::<Vec<_>>(),
            "qualities": ["128", "320", "flac", "high", "super"],
        });
        let v = gateway(
            "/v2/get_res_privilege/lite",
            "media.store.kugou.com",
            &[],
            Some(&body),
        )
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("权益查询失败", "Privilege query failed")))?;
        // 响应结构：{data: [ {...,hash, pay_type, album_audio_id}, ... ]}
        let list = v
            .pointer("/data")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        for it in list {
            let hash = it
                .get("hash")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_lowercase();
            if hash.is_empty() {
                continue;
            }
            let pay_type = it.get("pay_type").and_then(|x| x.as_i64()).unwrap_or(0);
            let album_id = it
                .get("album_id")
                .and_then(|x| {
                    x.as_str()
                        .map(|s| s.parse::<u64>().unwrap_or(0))
                        .or_else(|| x.as_u64())
                })
                .unwrap_or(0);
            let album_audio_id = it
                .get("album_audio_id")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            out.insert(hash, (pay_type == 3, album_id, album_audio_id));
        }
    }
    Ok(out)
}

/// 用权益数据回填歌单曲目的 VIP 标志与专辑 ID（歌单通道不带付费信息）
fn enrich_vip(songs: &mut [KgSong]) {
    let items: Vec<(String, u64)> = songs
        .iter()
        .map(|s| (s.id.to_lowercase(), s.album_id))
        .collect();
    let result = privilege_map(&items);
    if let Err(e) = &result {
        eprintln!("[kugou] enrich_vip 失败: {e}");
    }
    if let Ok(map) = result {
        for s in songs.iter_mut() {
            if let Some((vip, album_id, album_audio_id)) = map.get(&s.id.to_lowercase()) {
                s.vip = *vip;
                if *album_id > 0 {
                    s.album_id = *album_id;
                }
                if *album_audio_id > 0 {
                    s.album_audio_id = *album_audio_id;
                }
            }
        }
    }
}

/// 歌单条目 → KgSong
fn song_from_playlist(t: &serde_json::Value) -> Option<KgSong> {
    let hash = t.get("hash").and_then(|v| v.as_str()).unwrap_or("");
    let name = t
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if hash.is_empty() || name.is_empty() {
        return None;
    }
    let singer = t
        .pointer("/singerinfo")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("author_name")
                        .or_else(|| a.get("name"))
                        .and_then(|n| n.as_str())
                })
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default();
    // 歌单通道的 name 常是 "歌手 - 歌名" 全格式，剥掉歌手前缀避免 UI 重复
    let name = if !singer.is_empty() && name.starts_with(&format!("{singer} - ")) {
        name[singer.len() + 3..].trim().to_string()
    } else {
        name
    };
    let mut hq_hash = String::new();
    let mut sq_hash = String::new();
    let mut super_hash = String::new();
    if let Some(goods) = t.get("relate_goods").and_then(|g| g.as_array()) {
        for g in goods {
            let q = g
                .get("quality")
                .or_else(|| g.get("level"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let h = g
                .get("hash")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if h.is_empty() {
                continue;
            }
            match q {
                "320" if hq_hash.is_empty() => hq_hash = h,
                "flac" if sq_hash.is_empty() => sq_hash = h,
                "high" | "super" if super_hash.is_empty() => super_hash = h,
                _ => {}
            }
        }
    }
    // 酷狗歌单返回的 audio_id 为内部资产 ID，非取链所需的 album_audio_id；
    // 真实的 album_audio_id 存放在 mixsongid 或 add_mixsongid（若均缺失则置 0，由 enrich_vip 权益接口回填）
    let album_audio_id = t
        .get("mixsongid")
        .or_else(|| t.get("add_mixsongid"))
        .or_else(|| t.get("album_audio_id"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
        })
        .unwrap_or(0);
    let album_id = t
        .get("album_id")
        .or_else(|| t.pointer("/albuminfo/id"))
        .or_else(|| t.pointer("/albuminfo/album_id"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
        })
        .unwrap_or(0);
    Some(KgSong {
        id: hash.to_string(),
        name,
        singer,
        album: t
            .pointer("/albuminfo/name")
            .or_else(|| t.pointer("/albuminfo/album_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        duration_ms: t
            .get("timelen")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0) as u64,
        cover: cover_sized(t.get("cover").and_then(|v| v.as_str()).unwrap_or(""), 240),
        // 歌单通道的 privilege 对全部歌曲恒为 10，无法判定 VIP，置 false
        //（播放失败时错误信息已按权益明确分类）
        vip: false,
        album_audio_id,
        album_id,
        hq_hash,
        sq_hash,
        super_hash,
    })
}

/// 随机歌单（随便听听）：推荐分类下随机抽一个歌单并拉取曲目，
/// 空歌单/拉取失败自动换一个重试
pub fn random_playlist() -> Result<KgPublicPlaylist, String> {
    use rand::seq::SliceRandom;
    let specials = special_recommend(0, 1)?;
    if specials.is_empty() {
        return Err(crate::i18n::pick("歌单广场暂无数据", "Playlist plaza is currently empty").into());
    }
    let mut last_err = crate::i18n::pick("未能抽中可用歌单", "No usable playlist found").to_string();
    for _ in 0..4 {
        let pick = specials.choose(&mut rand::thread_rng()).unwrap();
        match playlist_tracks(&pick.id) {
            Ok(mut pl) => {
                if pl.name.is_empty() {
                    pl.name = pick.name.clone();
                }
                if pl.cover.is_empty() {
                    pl.cover = pick.cover.clone();
                }
                if pl.creator.is_empty() {
                    pl.creator = pick.creator.clone();
                }
                if !pl.songs.is_empty() {
                    return Ok(pl);
                }
                last_err = crate::i18n::pick("抽中的歌单暂无曲目", "Selected playlist has no tracks").into();
            }
            Err(e) => last_err = e,
        }
    }
    Err(crate::i18n::pick(
        format!("随机歌单获取失败：{last_err}"),
        format!("Failed to load random playlist: {last_err}"),
    ))
}
