//! 搜索（匿名可用）。

use super::*;

// ---------- 搜索（匿名可用） ----------

/// 从搜索条目解析 QqSong（新旧通道字段兼容：
/// 新=mid/name/singer/album.mid、旧=songmid/songname/albummid）
fn song_from_search_json(s: &serde_json::Value) -> Option<QqSong> {
    let id = s
        .get("mid")
        .or_else(|| s.get("songmid"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if id.is_empty() {
        return None;
    }
    let singer = s
        .get("singer")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default();
    Some(QqSong {
        id: id.to_string(),
        name: s
            .get("name")
            .or_else(|| s.get("songname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        singer,
        album: s
            .pointer("/album/name")
            .or_else(|| s.get("albumname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        album_mid: s
            .pointer("/album/mid")
            .or_else(|| s.get("albummid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        media_mid: s
            .get("media_mid")
            .or_else(|| s.pointer("/file/media_mid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        duration_ms: s.get("interval").and_then(|v| v.as_i64()).unwrap_or(0) as u64 * 1000,
        vip: s
            .pointer("/pay/pay_play")
            .or_else(|| s.pointer("/pay/payplay"))
            .and_then(|v| v.as_i64())
            .map(|v| v != 0)
            .unwrap_or(false),
    })
}

pub fn search(keyword: &str, limit: i64, page: i64) -> Result<Vec<QqSong>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(vec![]);
    }
    // 主通道：soso 明文接口 search_for_qq_cp（匿名 GET 可用）。
    // 同族 client_search_cp 已 500，签名版 SearchCgiService 对部分网络
    // 返回 500003（拒绝匿名请求），故以明文 soso 为主。
    let url = format!(
        "https://c.y.qq.com/soso/fcgi-bin/search_for_qq_cp?w={}&format=json&n={}&p={}&g_tk=5381",
        form_encode(keyword),
        limit,
        page
    );
    let mut last_err: String = match plain_get(&url, "https://y.qq.com/") {
        Err(e) => e,
        Ok(text) => {
            let resp: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return Err(format!("QQ 音乐搜索解析失败: {e}")),
            };
            if resp.get("code").and_then(|c| c.as_i64()) == Some(0) {
                if let Some(list) = resp.pointer("/data/song/list").and_then(|v| v.as_array()) {
                    let out: Vec<QqSong> = list.iter().filter_map(song_from_search_json).collect();
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            }
            format!(
                "soso 通道无数据（code {}）",
                resp.get("code").and_then(|c| c.as_i64()).unwrap_or(-1)
            )
        }
    };

    // 备用通道：musicu 的 SearchCgiService（网页版现行搜索，部分网络可用）
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.adsense.SearchCgiService",
            "method": "DoSearchForQmicMusic",
            "param": {
                "search_type": 0,
                "query": keyword,
                "page_num": page,
                "num_per_page": limit
            }
        }
    });
    if let Ok(resp) = musicu_signed(&payload, None) {
        let code = resp
            .pointer("/req_1/code")
            .and_then(|c| c.as_i64())
            .unwrap_or(-1);
        if code == 0 {
            if let Some(list) = resp
                .pointer("/req_1/data/body/song/list")
                .and_then(|v| v.as_array())
            {
                let out: Vec<QqSong> = list.iter().filter_map(song_from_search_json).collect();
                if !out.is_empty() {
                    return Ok(out);
                }
            }
            last_err = format!("{last_err}，签名通道无歌曲数据");
        } else {
            last_err = format!("{last_err}，签名通道 code {code}");
        }
    } else {
        last_err = format!("{last_err}，签名通道请求失败");
    }
    Err(format!("QQ 音乐搜索失败: {last_err}"))
}
