use crate::models::LyricLine;

pub struct Parsed {
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

/// 解析 LRC 文本：支持多时间标签、[offset:...] 元数据、剥离逐字 <mm:ss.xx> 标签
pub fn parse(text: &str) -> Parsed {
    let mut out: Vec<LyricLine> = Vec::new();
    let mut offset_ms: i64 = 0;

    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times: Vec<u64> = Vec::new();

        loop {
            if !rest.starts_with('[') {
                break;
            }
            let Some(close) = rest.find(']') else { break };
            let tag = &rest[1..close];
            if let Some(t) = parse_time_tag(tag) {
                times.push(t);
                rest = rest[close + 1..].trim_start();
                continue;
            }
            if let Some(v) = tag.strip_prefix("offset:") {
                offset_ms = v.trim().parse::<i64>().unwrap_or(0);
                rest = rest[close + 1..].trim_start();
                continue;
            }
            // 其他元数据标签（[ti:] [ar:] 等）：跳过标签本身
            rest = rest[close + 1..].trim_start();
            break;
        }

        let content = strip_word_tags(rest).trim().to_string();
        if times.is_empty() {
            // 无时间标签的内容行（纯文本歌词），元数据行内容通常带冒号被保留——仍展示无妨
            if !content.is_empty() {
                out.push(LyricLine { time_ms: None, text: content });
            }
        } else {
            let adj = |t: u64| -> u64 {
                let v = t as i64 + offset_ms;
                v.max(0) as u64
            };
            for t in times {
                out.push(LyricLine { time_ms: Some(adj(t)), text: content.clone() });
            }
        }
    }

    out.sort_by_key(|l| l.time_ms.unwrap_or(u64::MAX));
    let synced = out.iter().any(|l| l.time_ms.is_some());
    Parsed { synced, lines: out }
}

/// [mm:ss] / [mm:ss.xx] / [mm:ss.xxx] → 毫秒
fn parse_time_tag(tag: &str) -> Option<u64> {
    let (mm, rest) = tag.split_once(':')?;
    let minutes: i64 = mm.trim().parse().ok()?;
    if minutes < 0 {
        return None;
    }
    let secs: f64 = rest.trim().replace(',', ".").parse().ok()?;
    if !(0.0..60.0).contains(&secs) {
        return None;
    }
    Some((minutes as f64 * 60_000.0 + secs * 1000.0) as u64)
}

fn strip_word_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}
