//! 酷狗音乐接口客户端（搜索 / 取播放链接 / 歌词）
//!
//! 匿名访问其公共接口：免费曲目直接返回可播直链；付费/VIP 曲目由接口
//! 明确返回“需要付费”，前端按权益不足提示（随登录状态可恢复的场景）。
//! 不包含任何绕过付费 / 版权限制的功能。

use base64::Engine as _;
use serde::Serialize;
use std::io::Read;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(12);

/// 酷狗的公开接口走 HTTP（HTTPS 对部分 CDN 域不可用）；仅取公共曲目
/// 元数据与音频直链，不含用户数据
fn get_text(url: &str, referer: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .set("Referer", referer)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("请求酷狗接口失败: {e}"))?;
    let mut buf = String::new();
    resp.into_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("酷狗响应读取失败: {e}"))?;
    Ok(buf)
}

fn form_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgSong {
    /// 歌曲 hash（酷狗的唯一曲目标识）
    pub id: String,
    pub name: String,
    pub singer: String,
    pub album: String,
    pub duration_ms: u64,
    pub cover: String,
    /// pay_type == 3（付费/VIP 曲目）：匿名不可播，界面上打 VIP 角标
    pub vip: bool,
}

/// 搜索（每页 30 条，与 QQ 曲库分页语义一致）
pub fn search(keyword: &str, page: i64) -> Result<Vec<KgSong>, String> {
    let url = format!(
        "http://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={}&page={page}&pagesize=30",
        form_encode(keyword)
    );
    let text = get_text(&url, "http://mobilecdn.kugou.com/")?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("酷狗搜索解析失败: {e}"))?;
    let code = v.get("status").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 1 {
        return Err(format!("酷狗搜索失败（status {code}）"));
    }
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/info").and_then(|x| x.as_array()) {
        for t in list {
            let hash = t
                .get("hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if hash.is_empty() {
                continue;
            }
            let name = t
                .get("songname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let cover = t
                .pointer("/trans_param/union_cover")
                .and_then(|v| v.as_str())
                .map(|s| s.replace("{size}", "240"))
                .unwrap_or_default();
            let duration = t.get("duration").and_then(|v| v.as_i64()).unwrap_or(0);
            out.push(KgSong {
                id: hash,
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
                duration_ms: (duration.max(0) as u64) * 1000,
                cover,
                vip: t.get("pay_type").and_then(|v| v.as_i64()).unwrap_or(0) == 3,
            });
        }
    }
    Ok(out)
}

/// 取播放直链（匿名）：免费曲目返回直链；付费曲目返回明确错误。
/// 返回 (url, 扩展名)
pub fn song_url(hash: &str, vip: bool) -> Result<(String, String), String> {
    let url = format!(
        "http://m.kugou.com/app/i/getSongInfo.php?cmd=playInfo&hash={}",
        form_encode(hash)
    );
    let text = get_text(&url, "http://m.kugou.com/")?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("酷狗歌曲信息解析失败: {e}"))?;
    let play_url = v
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();
    if play_url.is_empty() {
        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("");
        let privilege = v.get("privilege").and_then(|p| p.as_i64()).unwrap_or(0);
        return Err(if err.contains("付费") || privilege == 10 || vip {
            "该曲目需要酷狗 VIP 或付费购买".into()
        } else {
            "该歌曲在酷狗暂无可播放链接（可能已下架）".into()
        });
    }
    // 直链一般无扩展名，从路径里找；找不到按 mp3（匿名直链恒为有损档）
    let path = play_url.split('?').next().unwrap_or("");
    let ext = path
        .rsplit('.')
        .next()
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("mp3")
        .to_lowercase();
    Ok((play_url, ext))
}

/// 歌词：krcs 检索 → 下载 LRC（content 为 base64）。
/// 无歌词返回 Ok(None)
pub fn lyric(hash: &str) -> Result<Option<String>, String> {
    let search_url = format!(
        "http://krcs.kugou.com/search?ver=1&man=yes&client=mobi&keyword=&duration=&hash={}",
        form_encode(hash)
    );
    let text = get_text(&search_url, "http://krcs.kugou.com/")?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("酷狗歌词检索解析失败: {e}"))?;
    let cand = v
        .pointer("/candidates/0")
        .cloned()
        .ok_or_else(|| "no candidates".to_string());
    let (id, accesskey) = match cand {
        Ok(c) => (
            c.get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            c.get("accesskey")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        Err(_) => return Ok(None),
    };
    if id.is_empty() {
        return Ok(None);
    }
    let dl_url = format!(
        "http://krcs.kugou.com/download?ver=1&client=pc&fmt=lrc&charset=utf8&id={id}&accesskey={accesskey}"
    );
    let text = get_text(&dl_url, "http://krcs.kugou.com/")?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("酷狗歌词下载解析失败: {e}"))?;
    let content = v
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    if content.is_empty() {
        return Ok(None);
    }
    // content 为 base64（UTF-8 文本，可能带 BOM）；解不开时退回原文
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(content.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or(content);
    let trimmed = decoded.trim_start_matches('\u{feff}').trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // 需要网络
    fn test_search_and_lyric() {
        let songs = search("晴天", 1).unwrap();
        assert!(!songs.is_empty());
        assert!(!songs[0].id.is_empty());
        // 付费曲目报“需要 VIP/付费”，免费曲目拿到直链
        let free = songs.iter().find(|s| !s.vip).expect("有免费曲目");
        let (url, _ext) = song_url(&free.id, false).unwrap();
        assert!(url.starts_with("http"));
        let lyric = lyric(&free.id).unwrap();
        let _ = lyric; // 有无歌词均可（有词时应包含时间标签）
    }
}
