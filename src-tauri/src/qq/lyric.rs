//! 歌词（LRC/QRC，匿名可用）。

use super::url::*;
use super::*;

// ---------- 歌词（匿名可用，返回 base64 编码的 LRC） ----------

/// 歌词 + 官方翻译（都是 base64 的 LRC；trans 可能为空）
pub struct LyricTrans {
    pub lrc: String,
    pub trans: Option<String>,
}

fn decode_b64_lrc(v: &serde_json::Value) -> Option<String> {
    let b64 = v.as_str().filter(|s| !s.is_empty())?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let s = String::from_utf8_lossy(&bytes).into_owned();
    let s = s.trim_start_matches('\u{feff}').trim().to_string();
    (!s.is_empty()).then_some(s)
}

pub fn lyric(songmid: &str) -> Result<Option<LyricTrans>, String> {
    let url = format!(
        "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&g_tk=5381&format=json",
        form_encode(songmid)
    );
    let text = plain_get(&url, "https://y.qq.com/")?;
    let resp: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("歌词解析失败: {e}"))?;
    let lrc = resp.get("lyric").and_then(|v| decode_b64_lrc(v));
    // 响应里的 trans 字段是官方翻译（同为 base64 LRC）——此前被直接丢弃
    let trans = resp.get("trans").and_then(|v| decode_b64_lrc(v));
    Ok(lrc.map(|lrc| LyricTrans { lrc, trans }))
}

/// 逐字歌词（QRC）：musicu 的 PlayLyricInfo 模块，需登录凭证。
/// lyric 字段为 hex 编码的 QQ 魔改 Triple-DES + zlib + XML 数据，
/// 解密提取 LyricContent 后转换为增强 LRC；无逐字数据返回 None 回落行级。
/// 返回 (增强 LRC 逐字歌词, 官方翻译 LRC)
pub fn lyric_qrc(
    songmid: &str,
    musicid: &str,
    musickey: &str,
) -> Result<Option<(String, Option<String>)>, String> {
    let payload = serde_json::json!({
        "comm": {"uin": musicid, "format": "json", "ct": 19, "cv": 0},
        "req_1": {
            "module": "music.musichallSong.PlayLyricInfo",
            "method": "GetPlayLyricInfo",
            "param": {
                "songMid": songmid,
                "qrc": 1,            // 1 = 请求逐字（QRC），0 = 行级
                "qrc_tts": 0,
                "romalrc": 0,
                "trans": 1
            }
        }
    });
    let cookie = credential_cookie(musicid, musickey);
    let resp = musicu_signed(&payload, Some(&cookie))?;
    let code = resp
        .pointer("/req_1/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(0);
    if code != 0 {
        return Ok(None);
    }
    // qrc=1 → lyric 为逐字（加密）；qrc=0 → 行级 LRC（无逐字可回落）
    let is_qrc = resp
        .pointer("/req_1/data/qrc")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        == 1;
    if !is_qrc {
        return Ok(None);
    }
    let encoded = resp
        .pointer("/req_1/data/lyric")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    // 解密失败（格式变化等）时返回 None，调用方回落行级 LRC
    let xml = match crate::qrc::decrypt(encoded) {
        Ok(x) => x,
        Err(_) => return Ok(None),
    };
    let Some(content) = crate::qrc::extract_lyric_content(&xml) else {
        return Ok(None);
    };
    // XML 属性里的换行是转义文本：还原为真实行
    let content = content.replace("\\n", "\n");
    let enhanced = crate::qrc::to_enhanced_lrc(&content);
    if enhanced.is_empty() {
        return Ok(None);
    }
    // 响应同步带回的官方翻译（base64 LRC；trans=1 时与逐字数据并列）
    let trans = resp
        .pointer("/req_1/data/trans")
        .and_then(|v| decode_b64_lrc(v));
    Ok(Some((enhanced, trans)))
}
