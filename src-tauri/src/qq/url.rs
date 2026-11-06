//! 播放链接取链（需登录 cookie）。

use super::*;

// ---------- 播放链接（需登录 cookie） ----------

pub(super) fn credential_cookie(musicid: &str, musickey: &str) -> String {
    format!(
        "uin={id}; qqmusic_uin={id}; qm_keyst={key}; qqmusic_key={key}",
        id = musicid,
        key = musickey
    )
}

/// 在 QQLogin 响应的任意层级查找同时含 musicid 与 musickey 的对象
pub(super) fn find_str_anywhere(v: &serde_json::Value, key: &str) -> Option<String> {
    if let Some(obj) = v.as_object() {
        if let Some(x) = obj.get(key) {
            if let Some(s) = x.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        for (_, child) in obj {
            if let Some(found) = find_str_anywhere(child, key) {
                return Some(found);
            }
        }
    }
    if let Some(arr) = v.as_array() {
        for child in arr {
            if let Some(found) = find_str_anywhere(child, key) {
                return Some(found);
            }
        }
    }
    None
}

pub(super) fn find_credential(v: &serde_json::Value) -> Option<(String, String)> {
    if let Some(obj) = v.as_object() {
        let get = |k: &str| -> Option<String> {
            obj.get(k).and_then(|x| {
                x.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| x.as_i64().map(|n| n.to_string()))
            })
        };
        if let (Some(id), Some(key)) = (
            get("musicid").or_else(|| get("str_musicid")),
            get("musickey"),
        ) {
            if !id.is_empty() && !key.is_empty() {
                return Some((id, key));
            }
        }
        for (_, child) in obj {
            if let Some(found) = find_credential(child) {
                return Some(found);
            }
        }
    }
    if let Some(arr) = v.as_array() {
        for child in arr {
            if let Some(found) = find_credential(child) {
                return Some(found);
            }
        }
    }
    None
}

/// 按音质请求播放直链，从所选音质逐级回退；返回 (url, ext)
///
/// `vip`：前端缓存里该曲目的 VIP 标志（来自搜索结果的 pay.pay_play）。
/// 用于失败分类——VIP 曲目拿不到链接一律按"权益不足"报错（登录/VIP 状态
/// 变化后可恢复），绝不误报无版权；非 VIP 曲目在会话有效仍拿不到链接时
/// 才判为无版权/下架。
pub fn song_url(
    songmid: &str,
    media_mid: &str,
    musicid: &str,
    musickey: &str,
    quality: &str,
    vip: bool,
) -> Result<(String, String, &'static str), String> {
    // 前缀（社区逆向共识）：M500=128k M800=320k F000=SQ无损 RS01=Hi-Res。
    // 无 media_mid 时按官方规则用 songmid 拼接。高档位被权益/曲目拒绝时
    // 自动沿阶梯回落。
    // 注意：臻品母带（RS02，.mflac2）与臻品全景声是 QMCv2 加密流，明文
    // 播放管线无法直接交付，未提供对应档位（需 ekey 解密管线后接入）。
    let mut ladder: Vec<(&str, &str, &'static str)> = match quality {
        "master" | "atmos" => vec![
            // 历史设置值兼容：按 Hi-Res 取（母带/全景声需解密支持后再开档）
            ("RS01", "flac", "Hi-Res"),
            ("F000", "flac", "无损"),
            ("M800", "mp3", "HQ"),
            ("M500", "mp3", "标准"),
        ],
        "hires" => vec![
            ("RS01", "flac", "Hi-Res"),
            ("F000", "flac", "无损"),
            ("M800", "mp3", "HQ"),
            ("M500", "mp3", "标准"),
        ],
        "lossless" => vec![
            ("F000", "flac", "无损"),
            ("M800", "mp3", "HQ"),
            ("M500", "mp3", "标准"),
        ],
        "standard" => vec![("M500", "mp3", "标准")],
        _ => vec![("M800", "mp3", "HQ"), ("M500", "mp3", "标准")],
    };
    let mut last_resp = String::new();
    // 所有尝试都返回 code==0（会话有效、接口无异常）但始终没有链接
    let mut all_ok_but_no_url = true;

    // ---------- Hi-Res（RS01）专用：一次 GetVkey 带全部文件名候选 ----------
    // Hi-Res 文件的 media_mid 与 songmid 不同，且搜索/详情等元数据通道
    // 都拿不到分档 mediaMid、sip 列表也可能为空——因此把候选文件名批量
    // 发给 GetVkey（filename 本就是数组），逐个 midurlinfo 试，再用多
    // CDN 基址探测下载（ws.stream 上没有 Hi-Res 文件，实测 404）。
    let ladder_has_hires = ladder.iter().any(|(p, _, _)| *p == "RS01");
    if ladder_has_hires {
        let detail_mm = song_detail_file(songmid, &credential_cookie(musicid, musickey))
            .ok()
            .and_then(|f| {
                f.get("media_mid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
            });
        // 候选文件基（去重）：真实 media_mid 优先（双拼/单拼），再猜 songmid
        let mut bases: Vec<String> = Vec::new();
        if let Some(mm) = &detail_mm {
            bases.push(format!("RS01{mm}{songmid}"));
            if mm != songmid {
                bases.push(format!("RS01{mm}"));
            }
        }
        bases.push(format!("RS01{songmid}{songmid}"));
        bases.push(format!("RS01{songmid}"));
        bases.dedup();

        let filenames: Vec<String> = bases.iter().map(|b| format!("{b}.flac")).collect();
        let payload = serde_json::json!({
            "comm": {"ct": 19, "cv": 1859},
            "req_1": {
                "module": "music.vkey.GetVkey",
                "method": "UrlGetVkey",
                "param": {
                    "uin": musicid,
                    "filename": filenames,
                    "guid": guid(),
                    "songmid": [songmid],
                    "songtype": [0],
                    "ctx": 0,
                }
            }
        });
        let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey)))?;
        if resp.pointer("/req_1/code").and_then(|c| c.as_i64()) == Some(104009) {
            return Err("QQ 音乐登录已过期，请重新登录".into());
        }
        let infos = resp
            .pointer("/req_1/data/midurlinfo")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for mi in &infos {
            let purl = mi.get("purl").and_then(|p| p.as_str()).unwrap_or("");
            if purl.is_empty() {
                continue;
            }
            let url = if purl.starts_with("http") {
                purl.to_string()
            } else {
                let sip = resp
                    .pointer("/req_1/data/sip")
                    .and_then(|v| v.as_array())
                    .and_then(|a| {
                        let list: Vec<&str> =
                            a.iter().filter_map(|v| v.as_str()).collect();
                        list.iter()
                            .copied()
                            .find(|s| s.starts_with("https"))
                            .or_else(|| list.iter().copied().find(|s| s.starts_with("http")))
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "https://ws.stream.qqmusic.qq.com/".to_string());
                format!("{sip}{purl}")
            };
            // CDN 基址探测：Hi-Res 不在 ws.stream 上（实测 404），逐基址试
            let mut candidates: Vec<String> = vec![url.clone()];
            for base in [
                "https://isure.stream.qqmusic.qq.com/",
                "https://dl.stream.qqmusic.qq.com/",
                "https://aqqmusic.tc.qq.com/amobile.music.tc.qq.com/",
                "https://ws.stream.qqmusic.qq.com/",
                "http://dl.stream.qqmusic.qq.com/",
                "http://ws.stream.qqmusic.qq.com/",
                "http://aqqmusic.tc.qq.com/amobile.music.tc.qq.com/",
            ] {
                let cand = format!("{base}{purl}");
                if !candidates.contains(&cand) {
                    candidates.push(cand);
                }
            }
            let mut picked: Option<String> = None;
            for cand in &candidates {
                match ureq::get(cand).timeout(std::time::Duration::from_secs(8)).call() {
                    Ok(_) => {
                        // ureq 仅对 2xx 返回 Ok
                        picked = Some(cand.clone());
                        break;
                    }
                    // 4xx/5xx：该基址无此文件，试下一个基址
                    Err(ureq::Error::Status(_, _)) => continue,
                    // 传输错误无法验证：接受当前候选，交给播放端处理
                    Err(_) => {
                        picked = Some(cand.clone());
                        break;
                    }
                }
            }
            if let Some(u) = picked {
                return Ok((u, "flac".to_string(), "Hi-Res"));
            }
            eprintln!("[qq] RS01 候选在全部 CDN 基址上均 4xx，降档");
        }
        // Hi-Res 全部候选失败：移除 RS01 继续降档到无损及以下
        all_ok_but_no_url = false;
        ladder.retain(|(p, _, _)| *p != "RS01");
    }

    let mut rungs: Vec<(String, &str, &'static str)> = Vec::new();
    for (prefix, ext, label) in ladder {
        let base = if media_mid.is_empty() {
            format!("{songmid}{songmid}")
        } else {
            format!("{media_mid}{songmid}")
        };
        rungs.push((format!("{prefix}{base}"), ext, label));
    }
    for (file_base, ext, label) in rungs {
        let payload = serde_json::json!({
            "comm": {"ct": 19, "cv": 1859},
            "req_1": {
                "module": "music.vkey.GetVkey",
                "method": "UrlGetVkey",
                "param": {
                    "uin": musicid,
                    "filename": [format!("{file_base}.{ext}")],
                    "guid": guid(),
                    "songmid": [songmid],
                    "songtype": [0],
                    "ctx": 0,
                }
            }
        });
        let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey)))?;
        if resp.pointer("/req_1/code").and_then(|c| c.as_i64()) == Some(104009) {
            return Err("QQ 音乐登录已过期，请重新登录".into());
        }
        if resp.pointer("/req_1/code").and_then(|c| c.as_i64()) != Some(0) {
            all_ok_but_no_url = false;
        }
        let purl = resp
            .pointer("/req_1/data/midurlinfo/0/purl")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !purl.is_empty() {
            let url = if purl.starts_with("http") {
                purl.to_string()
            } else {
                let sip = resp
                    .pointer("/req_1/data/sip")
                    .and_then(|v| v.as_array())
                    .and_then(|a| {
                        let list: Vec<&str> =
                            a.iter().filter_map(|v| v.as_str()).collect();
                        list.iter()
                            .copied()
                            .find(|s| s.starts_with("https"))
                            .or_else(|| list.iter().copied().find(|s| s.starts_with("http")))
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "https://ws.stream.qqmusic.qq.com/".to_string());
                format!("{sip}{purl}")
            };
            return Ok((url, ext.to_string(), label));
        }
        last_resp = serde_json::to_string(&resp).unwrap_or_default();
    }
    // 会话有效仍拿不到链接：按 VIP 标志分类——VIP 曲目是权益不足（可随
    // 登录/会员状态恢复），非 VIP 曲目才是真无版权/下架（永久）。
    Err(if all_ok_but_no_url {
        if vip {
            "该曲目需要 QQ 音乐 VIP 权益（请确认已登录且会员状态有效）".into()
        } else {
            "该歌曲在 QQ 音乐无版权或已下架".into()
        }
    } else {
        format!(
            "该歌曲暂无可播放链接（接口异常，可稍后重试）| {}",
            last_resp.chars().take(120).collect::<String>()
        )
    })
}

/// 单曲详情（匿名可用）：返回 file.media_mid 与 size_hires 等，
/// 用于 Hi-Res 档的文件名解析与有无判断（soso 搜索通道不返回 file 对象）
pub fn song_detail_file(songmid: &str, cookie: &str) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://c.y.qq.com/v8/fcg-bin/fcg_play_single_song.fcg?songmid={songmid}&format=json"
    );
    let resp = crate::net::with_retry(|| {
        let mut r = ureq::get(&url)
            .set("Referer", "https://y.qq.com/")
            .set("User-Agent", crate::qq::UA)
            .timeout(std::time::Duration::from_secs(8));
        if !cookie.is_empty() {
            r = r.set("Cookie", cookie);
        }
        r.call()
    })?;
    let text = resp.into_string().map_err(|e| format!("歌曲详情读取失败: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("歌曲详情解析失败: {e}"))?;
    let data = match v.get("data").and_then(|d| d.as_array()) {
        Some(a) => a.first().cloned().unwrap_or_default(),
        None => v.get("data").cloned().unwrap_or_default(),
    };
    Ok(data.get("file").cloned().unwrap_or_default())
}

/// 审计探针：dump Hi-Res vkey 响应的关键字段 + 逐个 sip 试下载
pub fn hires_dump(songmid: &str, musicid: &str, musickey: &str) {
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.vkey.GetVkey",
            "method": "UrlGetVkey",
            "param": {
                "uin": musicid,
                "filename": [format!("RS01{songmid}{songmid}.flac")],
                "guid": guid(),
                "songmid": [songmid],
                "songtype": [0],
                "ctx": 0,
            }
        }
    });
    let resp = match musicu_signed(&payload, Some(&credential_cookie(musicid, musickey))) {
        Ok(v) => v,
        Err(e) => {
            println!("  [dump] 签名请求失败: {e}");
            return;
        }
    };

    println!(
        "  [dump] req_1.code={:?}",
        resp.pointer("/req_1/code").cloned().unwrap_or_default()
    );
    let mi = resp.pointer("/req_1/data/midurlinfo/0").cloned().unwrap_or_default();
    println!(
        "  [dump] midurlinfo[0].filename={:?} purl={:?}",
        mi.get("filename").and_then(|f| f.as_str()),
        mi.get("purl").and_then(|p| p.as_str()).map(|s| &s[..s.len().min(70)])
    );
    let sips: Vec<String> = resp
        .pointer("/req_1/data/sip")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    println!("  [dump] sip 列表: {sips:?}");
    let purl = mi.get("purl").and_then(|p| p.as_str()).unwrap_or("");
    if purl.is_empty() {
        return;
    }
    for sip in &sips {
        if sip.is_empty() {
            continue;
        }
        let url = format!("{sip}{purl}");
        let st = match ureq::get(&url).timeout(std::time::Duration::from_secs(8)).call() {
            Ok(r) => format!("HTTP {} len={}", r.status(), r.header("content-length").unwrap_or("?")),
            Err(ureq::Error::Status(c, _)) => format!("HTTP {c}"),
            Err(e) => format!("传输: {e}"),
        };
        println!("  [dump] sip={sip} → {st}");
    }
}

/// 审计探针：以任意文件名请求 GetVkey，返回拼好的 purl（不探测下载）
#[cfg(test)]
pub fn hires_url_probe(filename: &str, musicid: &str, musickey: &str) -> Option<String> {
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.vkey.GetVkey",
            "method": "UrlGetVkey",
            "param": {
                "uin": musicid,
                "filename": [format!("{filename}.flac")],
                "guid": guid(),
                "songtype": [0],
                "ctx": 0,
            }
        }
    });
    let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey))).ok()?;
    if resp.pointer("/req_1/code").and_then(|c| c.as_i64()) != Some(0) {
        return None;
    }
    let purl = resp
        .pointer("/req_1/data/midurlinfo/0/purl")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if purl.is_empty() {
        return None;
    }
    if purl.starts_with("http") {
        return Some(purl.to_string());
    }
    let sip = resp
        .pointer("/req_1/data/sip")
        .and_then(|v| v.as_array())
        .and_then(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .find(|s| s.starts_with("https"))
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "https://ws.stream.qqmusic.qq.com/".to_string());
    Some(format!("{sip}{purl}"))
}
