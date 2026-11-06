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
    let ladder: Vec<(&str, &str, &'static str)> = match quality {
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
    // 每档可带多个文件名候选（对 RS01 尤其重要：Hi-Res 文件的 mediaMid
    // 与 songmid 常不相同，搜索通道（soso）又不返回 file.mediaMid——
    // 双拼文件名 M500/M800/F000 能被服务端模糊解析，RS01 的 CDN 则
    // 严格 404（issue：选 Hi-Res 播放失败）。因此 media_mid 缺失时
    // RS01 档同时尝试"单 mid"与"双拼"两种文件名）
    // Hi-Res 档先查匿名单曲详情：拿到真实 media_mid（与 songmid 不同！）
    // 并预判该曲有无 Hi-Res 文件（size_hires=0 直接跳过 RS01，省一次 404）
    // detail_ok=true：详情拿到了（size_hires 如实）；detail_failed=true：详情请求失败（保留猜测候选+探测兜底）
    let (hires_media_mid, detail_says_no_hires, _detail_failed) =
        if ladder.iter().any(|(p, _, _)| *p == "RS01") {
            match song_detail_file(songmid, &credential_cookie(musicid, musickey)) {
                Ok(f) => {
                    let size = f.get("size_hires").and_then(|v| v.as_u64()).unwrap_or(0);
                    if size == 0 {
                        (None, true, false)
                    } else {
                        let mm = f
                            .get("media_mid")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        (Some(mm), false, false)
                    }
                }
                Err(_) => (None, false, true),
            }
        } else {
            (None, false, false)
        };
    let mut rungs: Vec<(String, &str, &'static str)> = Vec::new();
    for (prefix, ext, label) in ladder {
        if prefix == "RS01" && detail_says_no_hires {
            // 详情确认该曲没有 Hi-Res 文件：跳过 RS01，直接降档
            continue;
        }
        let mut bases: Vec<String> = Vec::new();
        if prefix == "RS01" {
            if let Some(mm) = &hires_media_mid {
                // 真实 media_mid：双拼与单 mid 都试
                bases.push(format!("{prefix}{mm}{songmid}"));
                bases.push(format!("{prefix}{mm}"));
            } else {
                // 详情失败（探测兜底）：猜测候选
                let base_double = if media_mid.is_empty() {
                    format!("{songmid}{songmid}")
                } else {
                    format!("{media_mid}{songmid}")
                };
                if media_mid.is_empty() {
                    bases.push(format!("{prefix}{songmid}"));
                }
                bases.push(format!("{prefix}{base_double}"));
            }
        } else {
            let base = if media_mid.is_empty() {
                format!("{songmid}{songmid}")
            } else {
                format!("{media_mid}{songmid}")
            };
            bases.push(format!("{prefix}{base}"));
        }
        for b in bases {
            rungs.push((b, ext, label));
        }
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
        // 104009 = 会话无效（未登录 / musickey 已过期）。
        // 换音质也无法挽回，立即中止并让前端引导重新登录。
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
                // 优先 https 流媒体前缀（接口可能同时返回 http/https 两个前缀），
                // 明文前缀仅作无 https 时的回落；基址回落同样走 https
                let sip = resp
                    .pointer("/req_1/data/sip")
                    .and_then(|v| v.as_array())
                    .and_then(|a| {
                        let list: Vec<&str> =
                            a.iter().filter_map(|v| v.as_str()).collect();
                        list.iter()
                            .copied()
                            .find(|s| s.starts_with("https"))
                            .or_else(|| {
                                list.iter().copied().find(|s| s.starts_with("http"))
                            })
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "https://ws.stream.qqmusic.qq.com/".to_string());
                format!("{}{}", sip, purl)
            };
            // RS01 档 purl 必须实测可下载才接受：Hi-Res 文件的 mediaMid 与
            // songmid 常不相同，搜索通道又拿不到 file.mediaMid，请求文件名
            // 只能猜测——服务端可能模糊匹配出一个不存在的文件（purl 非空
            // 但下载 404），若照单全收，播放会失败而不是降档（issue：选
            // Hi-Res 播放失败）。4xx 视为本档无文件，沿阶梯继续回落。
            if label == "Hi-Res" {
                let probe = ureq::get(&url)
                    .timeout(std::time::Duration::from_secs(8))
                    .call();
                if let Err(ureq::Error::Status(code, _)) = &probe {
                    if *code >= 400 {
                        eprintln!("[qq] RS01 purl 探测 HTTP {code}，该曲无可用 Hi-Res 文件，降档");
                        all_ok_but_no_url = false;
                        last_resp = serde_json::to_string(&resp).unwrap_or_default();
                        continue;
                    }
                }
            }
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
