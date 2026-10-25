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
    // 前缀：M500=128k M800=320k F000=flac；无 media_mid 时按官方规则用 songmid 拼接
    let ladder: Vec<(&str, &str, &'static str)> = match quality {
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
    for (prefix, ext, label) in ladder {
        let file_base = if media_mid.is_empty() {
            format!("{songmid}{songmid}")
        } else {
            format!("{media_mid}{songmid}")
        };
        let payload = serde_json::json!({
            "comm": {"ct": 19, "cv": 1859},
            "req_1": {
                "module": "music.vkey.GetVkey",
                "method": "UrlGetVkey",
                "param": {
                    "uin": musicid,
                    "filename": [format!("{prefix}{file_base}.{ext}")],
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
