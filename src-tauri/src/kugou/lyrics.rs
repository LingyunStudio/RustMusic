//! KRC 逐字歌词解密与增强 LRC 转换。

use super::*;

// ---------- 歌词（KRC 逐字 → 增强 LRC；回落 LRC） ----------

/// 解密 KRC：base64 → 去掉 4 字节文件头 → 16 字节循环 XOR → zlib 解压
fn decrypt_krc(content_b64: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content_b64.trim())
        .map_err(|e| format!("KRC base64 解码失败: {e}"))?;
    if bytes.len() <= 4 {
        return Err("KRC 内容为空".into());
    }
    const KEY: [u8; 16] = [
        64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105,
    ];
    let body = &bytes[4..];
    let xored: Vec<u8> = body
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    let mut out = String::new();
    ZlibDecoder::new(xored.as_slice())
        .read_to_string(&mut out)
        .map_err(|e| format!("KRC 解压失败: {e}"))?;
    Ok(out)
}

/// 歌词：krcs 检索 → 优先下载 KRC 逐字（解密转增强 LRC），失败回落
/// LRC（content 为 base64）。无歌词返回 Ok(None)
pub fn lyric(hash: &str) -> Result<Option<String>, String> {
    let search_url = format!(
        "https://krcs.kugou.com/search?ver=1&man=yes&client=mobi&keyword=&duration=&hash={}",
        form_encode(hash)
    );
    let text = get_text(&search_url, "https://krcs.kugou.com/", UA)?;
    let v = parse_json(&text, "酷狗歌词检索")?;
    let cand = v
        .pointer("/candidates/0")
        .cloned()
        .ok_or_else(|| "no candidates".to_string());
    let (id, accesskey) = match cand {
        Ok(c) => (
            c.get("id")
                .and_then(|x| {
                    x.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| x.as_i64().map(|n| n.to_string()))
                })
                .unwrap_or_default(),
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

    // KRC 逐字：解密/解析失败回落 LRC（回落原因落 stderr，便于诊断"没有逐词"类反馈）
    let krc_url = format!(
        "https://krcs.kugou.com/download?ver=1&client=mobi&fmt=krc&charset=utf8&id={id}&accesskey={accesskey}"
    );
    match (|| -> Result<String, String> {
        let text = get_text(&krc_url, "https://krcs.kugou.com/", UA)?;
        let v = parse_json(&text, "酷狗 KRC 下载")?;
        let content = v
            .get("content")
            .and_then(|c| c.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "KRC content 为空".to_string())?;
        let krc = decrypt_krc(content)?;
        let trimmed = krc.trim_start_matches('\u{feff}').trim().to_string();
        crate::lyrics::krc_to_enhanced_lrc(&trimmed)
            .ok_or_else(|| "KRC 转增强 LRC 失败".to_string())
    })() {
        Ok(enhanced) => return Ok(Some(enhanced)),
        Err(e) => eprintln!("[kugou] {id} KRC 通道失败，回落 LRC: {e}"),
    }

    // LRC 行级
    let dl_url = format!(
        "https://krcs.kugou.com/download?ver=1&client=pc&fmt=lrc&charset=utf8&id={id}&accesskey={accesskey}"
    );
    let text = get_text(&dl_url, "https://krcs.kugou.com/", UA)?;
    let v = parse_json(&text, "酷狗歌词下载")?;
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
