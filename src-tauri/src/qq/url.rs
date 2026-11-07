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
/// 缓存 GetCdnDispatch 返回的可用 CDN 节点列表
static CDN_SIPS: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

fn fallback_cdn_sip(musicid: &str, musickey: &str) -> String {
    if let Some(s) = CDN_SIPS.read().unwrap().first().cloned() {
        return s;
    }
    let payload = serde_json::json!({
        "comm": {"ct": "11", "cv": "13020508", "v": "13020508", "tmeAppID": "qqmusic", "uin": musicid},
        "req_1": {
            "module": "music.audioCdnDispatch.cdnDispatch",
            "method": "GetCdnDispatch",
            "param": {"guid": guid(), "uid": musicid, "use_new_domain": 1, "use_ipv6": 0}
        }
    });
    let cookie = credential_cookie(musicid, musickey);
    if let Ok(v) = musicu_signed(&payload, Some(&cookie)) {
        if let Some(arr) = v.pointer("/req_1/data/sip").and_then(|a| a.as_array()) {
            let mut list: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            // 优先 https 节点
            list.sort_by_key(|s| if s.starts_with("https") { 0 } else { 1 });
            if !list.is_empty() {
                let first = list[0].clone();
                *CDN_SIPS.write().unwrap() = list;
                return first;
            }
        }
    }
    "https://ws.stream.qqmusic.qq.com/".to_string()
}

/// 按音质请求播放直链，从所选音质逐级回退；返回 (url, ext, 档位标签)
///
/// 官方 App 音质档位（从高到低）：
/// - `master`：臻品母带（AI00，24Bit/192kHz FLAC，SVIP 专享）
/// - `atmos`：臻品全景声（Q001，5.1 空间音频 FLAC，SVIP 专享）
/// - `atmos_2`：臻品音质 2.0（Q000，24Bit 银河音效 FLAC，SVIP 专享）
/// - `hires`：Hi-Res 无损（RS01 / 24Bit 高解析度 FLAC，VIP/SVIP）
/// - `lossless`：SQ 无损品质（F000，16Bit/44.1kHz FLAC，VIP/SVIP）
/// - `high`：HQ 高品质（M800，320kbps MP3，VIP/SVIP）
/// - `standard`：标准品质（M500，128kbps MP3，免费）
pub fn song_url(
    songmid: &str,
    media_mid: &str,
    musicid: &str,
    musickey: &str,
    quality: &str,
    vip: bool,
) -> Result<(String, String, &'static str), String> {
    let cookie = credential_cookie(musicid, musickey);

    // 1. 通过 UniformRuleCtrl.CgiGetTrackInfo 获取曲目真实 file 元数据与各档位文件大小。
    //    注意：QQ 的 UrlGetVkey 只负责按账号权限签发 vkey，不会检查 CDN 上是否存在该音质文件！
    //    若不先核对 file.size_* / file.size_new[i] 是否 > 0，直接对不存在的档位签发 purl 会导致
    //    播放/下载时遭遇 CDN HTTP 404（此前 RS01 在无独立 Hi-Res 文件的曲目上必 404 的根因）。
    let track_payload = serde_json::json!({
        "comm": {
            "ct": "11",
            "cv": "13020508",
            "v": "13020508",
            "tmeAppID": "qqmusic",
            "uin": musicid,
        },
        "req_1": {
            "module": "music.trackInfo.UniformRuleCtrl",
            "method": "CgiGetTrackInfo",
            "param": {"types": [1], "ids": [0], "mids": [songmid]}
        }
    });
    let track_info = musicu_signed(&track_payload, Some(&cookie))
        .ok()
        .and_then(|v| v.pointer("/req_1/data/tracks/0").cloned());
    let file_obj = track_info
        .as_ref()
        .and_then(|t| t.get("file"))
        .cloned()
        .unwrap_or_default();
    let interval_secs = track_info
        .as_ref()
        .and_then(|t| t.get("interval"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let detail_mm = file_obj
        .get("media_mid")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(media_mid);
    let base = if detail_mm.is_empty() {
        format!("{songmid}{songmid}")
    } else {
        format!("{detail_mm}{songmid}")
    };

    let size_128 = file_obj.get("size_128mp3").and_then(|v| v.as_u64()).unwrap_or(0);
    let size_320 = file_obj.get("size_320mp3").and_then(|v| v.as_u64()).unwrap_or(0);
    let size_flac = file_obj.get("size_flac").and_then(|v| v.as_u64()).unwrap_or(0);
    let size_hires = file_obj.get("size_hires").and_then(|v| v.as_u64()).unwrap_or(0);
    let size_new: Vec<u64> = file_obj
        .get("size_new")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0)).collect())
        .unwrap_or_default();
    let size_master = size_new.first().copied().unwrap_or(0);   // size_new[0]: 臻品母带 AI00
    let size_atmos2 = size_new.get(1).copied().unwrap_or(0);    // size_new[1]: 臻品音质 2.0 Q000
    let size_atmos51 = size_new.get(2).copied().unwrap_or(0);   // size_new[2]: 臻品全景声 5.1 Q001
    let has_file_meta = size_128 > 0 || size_320 > 0 || size_flac > 0 || size_master > 0;

    // 部分新曲/周杰伦等曲目不单独拆分 RS01，而是直接把 24Bit/48kHz Hi-Res FLAC 放在 F000
    // （如《晴天》size_flac=55.4MB，码率 1645kbps，size_hires=0）。当用户选 Hi-Res 及以上且
    // F000 本身已是 >1250kbps 的 24Bit 高解析度母本时，F000 标签记为 "Hi-Res"。
    let f000_is_hires = size_hires == 0
        && size_flac > 35_000_000
        && (interval_secs == 0 || (size_flac * 8) / interval_secs > 1_250_000);
    let flac_label_for_high_tier: &'static str = if f000_is_hires { "Hi-Res" } else { "无损" };

    // (前缀, 扩展名, 档位标签, 该档位已知文件大小)
    let raw_ladder: Vec<(&str, &str, &'static str, u64)> = match quality {
        "master" => vec![
            ("AI00", "flac", "臻品母带", size_master),
            ("Q000", "flac", "臻品音质2.0", size_atmos2),
            ("RS01", "flac", "Hi-Res", size_hires),
            ("F000", "flac", flac_label_for_high_tier, size_flac),
            ("M800", "mp3", "HQ", size_320),
            ("M500", "mp3", "标准", size_128),
        ],
        "atmos" => vec![
            ("Q001", "flac", "臻品全景声", size_atmos51),
            ("Q000", "flac", "臻品音质2.0", size_atmos2),
            ("RS01", "flac", "Hi-Res", size_hires),
            ("F000", "flac", flac_label_for_high_tier, size_flac),
            ("M800", "mp3", "HQ", size_320),
            ("M500", "mp3", "标准", size_128),
        ],
        "atmos_2" => vec![
            ("Q000", "flac", "臻品音质2.0", size_atmos2),
            ("RS01", "flac", "Hi-Res", size_hires),
            ("F000", "flac", flac_label_for_high_tier, size_flac),
            ("M800", "mp3", "HQ", size_320),
            ("M500", "mp3", "标准", size_128),
        ],
        "hires" => vec![
            ("RS01", "flac", "Hi-Res", size_hires),
            ("F000", "flac", flac_label_for_high_tier, size_flac),
            ("M800", "mp3", "HQ", size_320),
            ("M500", "mp3", "标准", size_128),
        ],
        "lossless" => vec![
            ("F000", "flac", "无损", size_flac),
            ("M800", "mp3", "HQ", size_320),
            ("M500", "mp3", "标准", size_128),
        ],
        "standard" => vec![("M500", "mp3", "标准", size_128)],
        _ => vec![
            ("M800", "mp3", "HQ", size_320),
            ("M500", "mp3", "标准", size_128),
        ],
    };

    // 过滤掉明确不存在（size == 0）的高阶档位，避免 UrlGetVkey 签出 404 空链；
    // M500 始终保留兜底以防部分老歌元数据不全
    let mut rungs: Vec<(String, &'static str, &'static str)> = Vec::new();
    for (prefix, ext, label, sz) in raw_ladder {
        if has_file_meta && sz == 0 && prefix != "M500" {
            continue;
        }
        rungs.push((format!("{prefix}{base}.{ext}"), ext, label));
        if prefix == "RS01" && !detail_mm.is_empty() && detail_mm != songmid {
            rungs.push((format!("RS01{detail_mm}.{ext}"), ext, label));
        }
    }
    if rungs.is_empty() {
        rungs.push((format!("M500{base}.mp3"), "mp3", "标准"));
    }

    // 2. 一次批量 UrlGetVkey 请求全部候选档位（comm 携带 authst/qqmusic_key 以启用 SVIP 鉴权）
    let filenames: Vec<String> = rungs.iter().map(|(f, _, _)| f.clone()).collect();
    let songmids: Vec<&str> = rungs.iter().map(|_| songmid).collect();
    let songtypes: Vec<i32> = rungs.iter().map(|_| 0).collect();
    let payload = serde_json::json!({
        "comm": {
            "ct": 19,
            "cv": 1859,
            "uin": musicid,
            "authst": musickey,
            "qqmusic_key": musickey,
        },
        "req_1": {
            "module": "music.vkey.GetVkey",
            "method": "UrlGetVkey",
            "param": {
                "uin": musicid,
                "filename": filenames,
                "guid": guid(),
                "songmid": songmids,
                "songtype": songtypes,
                "ctx": 0,
            }
        }
    });
    let resp = musicu_signed(&payload, Some(&cookie))?;
    let code = resp.pointer("/req_1/code").and_then(|c| c.as_i64());
    if code == Some(104009) {
        return Err("QQ 音乐登录已过期，请重新登录".into());
    }
    let all_ok_but_no_url = code == Some(0);
    let sip = resp
        .pointer("/req_1/data/sip")
        .and_then(|v| v.as_array())
        .and_then(|a| {
            let list: Vec<&str> = a.iter().filter_map(|v| v.as_str()).filter(|s| !s.is_empty()).collect();
            list.iter()
                .copied()
                .find(|s| s.starts_with("https"))
                .or_else(|| list.first().copied())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| fallback_cdn_sip(musicid, musickey));

    if let Some(infos) = resp.pointer("/req_1/data/midurlinfo").and_then(|v| v.as_array()) {
        for ((_, ext, label), mi) in rungs.iter().zip(infos.iter()) {
            let purl = mi.get("purl").and_then(|p| p.as_str()).unwrap_or("");
            if purl.is_empty() {
                continue;
            }
            let url = if purl.starts_with("http") {
                purl.to_string()
            } else {
                format!("{sip}{purl}")
            };
            // 若 CgiGetTrackInfo 未返回文件大小元数据，对高阶档位做一次轻量 Range 探活防 404
            if !has_file_meta && *ext == "flac" {
                if let Err(ureq::Error::Status(status, _)) = ureq::get(&url)
                    .set("Range", "bytes=0-15")
                    .timeout(std::time::Duration::from_secs(5))
                    .call()
                {
                    if status >= 400 {
                        continue;
                    }
                }
            }
            return Ok((url, (*ext).to_string(), label));
        }
    }

    let last_resp = serde_json::to_string(&resp).unwrap_or_default();
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
