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

/// yrc（网易云）/ QRC（QQ）逐字歌词 → 增强 LRC。
/// yrc/QRC 行格式（真实数据字节级实测）：
///   [行起始ms,行持续ms](字起始ms,字持续ms[,0])字(…)字…
/// 时间元组在字**前面**，配它**后面**的文本段：首元组起始=行首、
/// 末元组结束=行尾（30 行真实 yrc 统计：元组数=字数、行首/行尾
/// 偏差全为 0，元组精确铺满整行）。此前按"元组配前面文本"解析，
/// 等于每个字都用下一个字的时间点亮——高亮滞后一字、行首空拍、
/// 行末字被当作"无元组的尾巴字"拉伸到行尾，逐字歌词整体不准。
/// 元数据行（元信息 JSON：{"t":ms,"c":[...]}、[t:0] 等）跳过。
/// 单行解析失败只丢弃该行，一行坏不拖累整首。
/// 无法解析出任何歌词行时返回 None（调用方回落行级 LRC）。
pub fn yrc_to_enhanced_lrc(yrc: &str) -> Option<String> {
    let mut out = String::new();
    for line in yrc.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('{') {
            // JSON 元数据行（v1 端点的 {"t":..,"c":[..]} 格式）不进歌词
            continue;
        }
        // 单行解析：任何畸形行（未闭合括号、非数字时间、纯文本行）只丢弃
        // 该行——与 KRC 版行为一致，一行坏不拖累整首逐字歌词降级行级
        if let Some(lrc) = yrc_line_to_enhanced(line) {
            out.push_str(&lrc);
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// 单条 yrc/QRC 歌词行 → 一行增强 LRC（含换行）；解析失败返回 None（跳过该行）
fn yrc_line_to_enhanced(line: &str) -> Option<String> {
    let rest = line.strip_prefix('[')?;
    let close = rest.find(']')?;
    let header: Vec<&str> = rest[..close].split(',').collect();
    if header.len() < 2 {
        return None;
    }
    let line_start: u64 = header[0].trim().parse().ok()?;
    // 行持续仅作行头格式校验：逐字元组自带起止，行尾时间无需使用
    header[1].trim().parse::<u64>().ok()?;
    let words_raw = &rest[close + 1..];

    // 游标扫描：每个 (start,dur[,x]) 元组配它**后面**紧邻的文本段
    // （真实格式见 yrc_to_enhanced_lrc 注释；元组前的零散文本规范上
    // 不存在，有也无处挂时间轴，跳过）
    let mut body = String::new();
    let mut cursor = 0usize;
    while let Some(rel) = words_raw[cursor..].find('(') {
        let lp = cursor + rel;
        let rp = match words_raw[lp..].find(')') {
            Some(p) => lp + p,
            None => break, // 未闭合元组：其后无法再配对
        };
        // 字元组 (start,dur[,ext...])：只取前两个，容忍第三参数；
        // 畸形元组只跳过自身，不拖累整行
        let times: Vec<&str> = words_raw[lp + 1..rp].split(',').collect();
        let Ok(ws) = times[0].trim().parse::<u64>() else {
            cursor = rp + 1;
            continue;
        };
        let Some(wd) = times.get(1).and_then(|t| t.trim().parse::<u64>().ok()) else {
            cursor = rp + 1;
            continue;
        };
        // 字文本：到下一个 '(' 或行尾
        let start = rp + 1;
        let end = words_raw[start..].find('(').map(|p| start + p).unwrap_or(words_raw.len());
        let text = words_raw[start..end].trim();
        if !text.is_empty() {
            let cs = fmt_lrc_time(ws);
            let ce = fmt_lrc_time(ws + wd);
            body.push_str(&format!("<{cs}>{text}<{ce}>"));
        }
        cursor = end;
    }
    if body.is_empty() {
        return None;
    }
    let ls = fmt_lrc_time(line_start);
    Some(format!("[{ls}]{body}\n"))
}

/// KRC（酷狗逐字歌词）→ 增强 LRC。KRC 行格式（解密解压后）：
///   [行起始ms,行持续ms]<字起始ms,字持续ms,0>字<字起始,字持续,0>字…
/// 与 yrc/QRC 相反，时间元组跟在字**前面**（尖括号包裹）；头部另有
/// [ti:]/[ar:]/[offset:0] 等元数据行（带冒号），逐行跳过。
/// 注意：krcs 线上下载数据的**字时间是相对行首的偏移**（多首歌 × 全部
/// 候选实测），字起始需加回行起始才是绝对毫秒——与 yrc/QRC 的绝对
/// 字时间不同。漏加的话每行词时间都落在 0~行时长 的小窗里，前端按
/// 绝对播放进度比对会把整行瞬间判为已唱完，逐词染色失效。
/// 无法解析出任何歌词行时返回 None（调用方回落行级 LRC）。
pub fn krc_to_enhanced_lrc(krc: &str) -> Option<String> {
    let mut out = String::new();
    for line in krc.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        // 元数据行：[ti:xxx] / [offset:0] —— 冒号形式，跳过
        let Some(rest) = line.strip_prefix('[') else { continue };
        let Some(close) = rest.find(']') else { continue };
        let header = &rest[..close];
        let (h0, h1) = match header.split_once(',') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => continue, // 无逗号 = 元数据行
        };
        let Ok(line_start) = h0.parse::<u64>() else { continue };
        // 行持续时长仅作行头格式校验（词时间是行内偏移，转换后无需行尾时间）
        let Ok(_) = h1.parse::<u64>() else { continue };
        let body = &rest[close + 1..];

        // 游标扫描：<s,d[,x]>text 段；元组在字前面
        let mut words: Vec<(u64, u64, String)> = Vec::new();
        let mut cursor = 0usize;
        while let Some(lt) = body[cursor..].find('<') {
            let lp = cursor + lt;
            let Some(rp) = body[lp..].find('>') else { break };
            let times: Vec<&str> = body[lp + 1..lp + rp].split(',').collect();
            if times.len() < 2 {
                break;
            }
            let (Ok(ws), Ok(wd)) = (times[0].trim().parse::<u64>(), times[1].trim().parse::<u64>())
            else {
                break;
            };
            // 字文本：到下一个 '<' 或行尾
            let start = lp + rp + 1;
            let end = body[start..].find('<').map(|p| start + p).unwrap_or(body.len());
            let text = &body[start..end];
            if !text.is_empty() {
                // 字时间是行内偏移，加回行起始才是绝对毫秒（见函数注释）
                words.push((line_start + ws, line_start + ws + wd, text.to_string()));
            }
            cursor = end;
        }
        if words.is_empty() {
            continue;
        }
        let mut body = String::new();
        for (ws, we, text) in words {
            body.push_str(&format!(
                "<{}>{}<{}>",
                fmt_lrc_time(ws),
                text,
                fmt_lrc_time(we)
            ));
        }
        out.push_str(&format!("[{}]{body}\n", fmt_lrc_time(line_start)));
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
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

    #[test]
    fn krc_to_enhanced() {
        // 线上 krcs 真实格式：词时间是相对行首的偏移（行首 17662，首词 0）
        let krc = "[id:$00000000$]\n[ar:测试]\n[offset:0]\n[17662,2870]<0,187,0>跟<187,234,0>着<421,234,0>希望<655,234,0>去<889,234,0>闯\n[21120,2930]<0,210,0>只<210,210,0>有";
        let out = krc_to_enhanced_lrc(krc).expect("converted");
        let p = parse(&out);
        assert!(p.synced);
        assert_eq!(p.lines.len(), 2);
        let l0 = &p.lines[0];
        // 增强-LRC 中间格式精度为 10ms（与 yrc/QRC 转换一致）
        assert_eq!(l0.time_ms, Some(17660));
        assert_eq!(l0.text, "跟着希望去闯");
        let ws = l0.words.as_ref().expect("words");
        // `<start>字<end>` 格式经 parse 会产出字与空字交替的词表（与 yrc/QRC 转换一致）
        let real: Vec<_> = ws.iter().filter(|w| !w.text.is_empty()).collect();
        assert_eq!(real.len(), 5);
        // 词时间必须加回行首成为绝对毫秒：漏加则落在 0~2870 小窗内
        assert_eq!(real[0].text, "跟");
        assert_eq!(real[0].start_ms, 17660);
        assert_eq!(real[0].end_ms, 17840); // = 下一词（着）的起始
        assert_eq!(real[2].text, "希望");
        assert_eq!(real[2].start_ms, 18080);
        assert_eq!(p.lines[1].text, "只有");
        assert_eq!(p.lines[1].words.as_ref().unwrap()[0].start_ms, 21120);
    }

    #[test]
    fn krc_all_metadata_returns_none() {
        assert!(krc_to_enhanced_lrc("[ti:歌]\n[ar:人]\n[offset:0]\n").is_none());
    }
}
