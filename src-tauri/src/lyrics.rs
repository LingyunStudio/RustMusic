use crate::models::{LyricLine, Word};

pub struct Parsed {
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

/// 解析 LRC 文本：支持多时间标签、[offset:...] 元数据、增强 LRC 的
/// 逐字标签 <mm:ss.xx>（yrc 转换后同样落成该格式）
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

        let (content, words) = parse_word_tags(rest);
        let content = content.trim().to_string();
        if times.is_empty() {
            // 无时间标签的内容行（纯文本歌词），元数据行内容通常带冒号被保留——仍展示无妨
            if !content.is_empty() {
                out.push(LyricLine {
                    time_ms: None,
                    text: content,
                    words: None,
                });
            }
        } else {
            let adj = |t: u64| -> u64 {
                let v = t as i64 + offset_ms;
                v.max(0) as u64
            };
            let words = words.map(|ws| {
                ws.into_iter()
                    .map(|w| Word {
                        start_ms: adj(w.0),
                        end_ms: adj(w.1),
                        text: w.2,
                    })
                    .collect::<Vec<_>>()
            });
            for t in times {
                out.push(LyricLine {
                    time_ms: Some(adj(t)),
                    text: content.clone(),
                    words: words.clone(),
                });
            }
        }
    }

    out.sort_by_key(|l| l.time_ms.unwrap_or(u64::MAX));
    let synced = out.iter().any(|l| l.time_ms.is_some());
    Parsed { synced, lines: out }
}

/// 解析增强 LRC 的逐字标签：形如 [00:01.00]<00:01.00>你<00:01.50>好
/// 返回 (去标签文本, 逐字 (start, end, text) 列表)；无逐字标签时 words = None。
/// end 时间 = 下一字的 start；末尾字暂用 start+600ms，调用方按行时长兜底修正。
fn parse_word_tags(s: &str) -> (String, Option<Vec<(u64, u64, String)>>) {
    if !s.contains('<') {
        return (s.to_string(), None);
    }
    let mut text = String::with_capacity(s.len());
    let mut words: Vec<(u64, u64, String)> = Vec::new();
    let mut i = 0usize;
    while i < s.len() {
        if s[i..].starts_with('<') {
            if let Some((t, consumed)) = parse_time_prefix(&s[i + 1..]) {
                let j = i + 1 + consumed; // '>' 之后
                                          // 字文本：到下一个 '<' 为止
                let k = s[j..].find('<').map(|p| j + p).unwrap_or(s.len());
                let word = &s[j..k];
                text.push_str(word);
                words.push((t, t, word.to_string()));
                i = k;
                continue;
            }
        }
        // 非标签处的普通字符；孤立成对的 <...>（非时间）整段剥除
        if s[i..].starts_with('<') {
            if let Some(gt) = s[i..].find('>') {
                i += gt + 1;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        text.push(ch);
        i += ch.len_utf8();
    }
    if words.is_empty() {
        return (text, None);
    }
    // 回填 end：下一字起始；末尾字给 start+600ms 占位
    for w in 0..words.len() {
        if w + 1 < words.len() {
            words[w].1 = words[w + 1].0;
        } else {
            words[w].1 = words[w].0 + 600;
        }
    }
    (text, Some(words))
}

/// "mm:ss.xx" 开头解析为毫秒；返回 (毫秒, 消费到 '>' 后的字符数)
fn parse_time_prefix(s: &str) -> Option<(u64, usize)> {
    let close = s.find('>')?;
    let t = parse_time_tag(&s[..close])?;
    // 消费：close+1 个字符（含 '>')
    Some((t, close + 1))
}

// ---------- yrc / QRC 逐字歌词 → 增强 LRC ----------

/// yrc（网易云）/ QRC（QQ）逐字歌词格式：
/// 歌词行：[行起始ms,行持续ms]字(字起始ms,字持续ms,0)字(...)…字
/// —— 时间元组**跟在它所属字的后面**；但实测网易云行末常带“尾巴字”：
/// 最后一个字之后没有自己的元组（它的区间由下一行起始时间界定）。
/// 真实样例：
///   [1,4890](1,270,0)词(270,270,0)版(540,270,0)…公(4600,290,0)司
/// “司”即尾巴字——不带元组。旧解析按“元组前文本”配对，尾巴被静默丢弃，
/// 表现为行末最后一个字丢失（播放页/桌面歌词都少字）。
/// 修法：尾巴字的区间 = [前一元组 end, 行 start+行 dur]。
/// 元数据行（元信息 JSON：{"t":ms,"c":[...]} 或纯文本）跳过。
/// 转换为增强 LRC：[mm:ss.cc]<mm:ss.cc>字<mm:ss.cc>...
/// 无法解析出任何歌词行时返回 None（调用方回落行级 LRC）。
pub fn yrc_to_enhanced_lrc(yrc: &str) -> Option<String> {
    let mut out = String::new();
    for line in yrc.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('{') {
            // JSON 元数据行（v1 端点的 {"t":..,"c":[..]} 格式）不进歌词
            continue;
        }
        let rest = line.strip_prefix('[')?;
        let close = rest.find(']')?;
        let header: Vec<&str> = rest[..close].split(',').collect();
        if header.len() < 2 {
            return None;
        }
        let line_start: u64 = header[0].parse().ok()?;
        let line_dur: u64 = header[1].parse().ok()?;
        let line_end = line_start + line_dur;
        let mut body = String::new();
        let words = &rest[close + 1..];
        // 游标扫描：每个 (...) 元组配它前面紧邻的文本段；
        // 元组前无文本（起拍占位）则跳过该元组。
        let mut cursor = 0usize;
        let mut prev_end = line_start;
        while let Some(rel) = words[cursor..].find('(') {
            let lp = cursor + rel;
            let text = words[cursor..lp].trim_start();
            let rp = words[lp..].find(')')? + lp;
            // 字元组 (start,dur[,ext...])：只取前两个，容忍第三参数
            let times: Vec<&str> = words[lp + 1..rp].split(',').collect();
            if times.len() < 2 {
                return None;
            }
            let ws: u64 = times[0].parse().ok()?;
            let wd: u64 = times[1].parse().ok()?;
            if !text.is_empty() {
                let cs = fmt_lrc_time(ws);
                let ce = fmt_lrc_time(ws + wd);
                body.push_str(&format!("<{cs}>{text}<{ce}>"));
            }
            prev_end = prev_end.max(ws + wd);
            cursor = rp + 1;
        }
        // 行末尾巴字：最后一个元组之后仍残留的文本（它没有自己的元组），
        // 区间 = [前一元组 end, 行 start+行 dur]
        let tail = words[cursor..].trim_start();
        if !tail.is_empty() && !body.is_empty() {
            let cs = fmt_lrc_time(prev_end);
            let ce = fmt_lrc_time(line_end.max(prev_end));
            body.push_str(&format!("<{cs}>{tail}<{ce}>"));
        }
        if body.is_empty() {
            continue;
        }
        let ls = fmt_lrc_time(line_start);
        out.push_str(&format!("[{ls}]{body}\n"));
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// 毫秒 → LRC 时间 mm:ss.cc（百分秒，yrc/QRC 原始精度 10ms）
pub fn fmt_lrc_time(ms: u64) -> String {
    let csec = ms / 10;
    format!("{}:{:02}.{:02}", csec / 6000, (csec / 100) % 60, csec % 100)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enhanced_lrc_word_tags() {
        let line = "[00:01.00]<00:01.00>你<00:01.50>好<00:02.00>呀";
        let p = parse(line);
        assert_eq!(p.lines.len(), 1);
        let l = &p.lines[0];
        assert_eq!(l.text, "你好呀");
        let ws = l.words.as_ref().expect("words");
        assert_eq!(ws.len(), 3);
        assert_eq!(
            (ws[0].start_ms, ws[0].end_ms, ws[0].text.as_str()),
            (1000, 1500, "你")
        );
        assert_eq!((ws[1].start_ms, ws[1].end_ms), (1500, 2000));
        assert_eq!(ws[2].end_ms, 2600); // 末尾字 start+600 兜底
    }

    #[test]
    fn plain_lrc_no_words() {
        let p = parse("[00:10.00]普通歌词");
        assert_eq!(p.lines[0].text, "普通歌词");
        assert!(p.lines[0].words.is_none());
    }

    #[test]
    fn offset_applies_to_words() {
        let p = parse("[offset:500]\n[00:01.00]<00:01.00>你");
        let ws = p.lines[0].words.as_ref().unwrap();
        assert_eq!(ws[0].start_ms, 1500);
    }
}
