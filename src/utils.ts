import type { LyricsPayload, LyricLine, TrackMeta } from "./types";

/** 当前播放位置对应的歌词行（无同步歌词返回 null） */
export function activeLyricText(
  lyrics: LyricsPayload | null,
  pos: number
): string | null {
  if (!lyrics?.synced) return null;
  const lines = lyrics.lines
    .filter((l) => l.timeMs != null && l.text.trim() !== "")
    .sort((a, b) => (a.timeMs ?? 0) - (b.timeMs ?? 0));
  if (!lines.length) return null;
  let ans: string | null = null;
  for (const l of lines) {
    if ((l.timeMs ?? 0) <= pos) ans = l.text;
    else break;
  }
  return ans;
}

export function fmtTime(ms: number): string {
  if (!isFinite(ms) || ms <= 0) return "0:00";
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return `${h > 0 ? `${h}:` : ""}${mm}:${s.toString().padStart(2, "0")}`;
}

export function fmtSize(bytes: number): string {
  if (bytes <= 0) return "-";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function fmtDate(secs: number): string {
  if (!secs) return "-";
  const d = new Date(secs * 1000);
  return `${d.getFullYear()}-${(d.getMonth() + 1).toString().padStart(2, "0")}-${d
    .getDate()
    .toString()
    .padStart(2, "0")}`;
}

/**
 * 右键菜单/弹出层的视口夹取：保证菜单完整出现在窗口内。
 * x/y 为期望位置（光标或锚点右下角），menuW/menuH 为菜单尺寸，
 * 返回夹取后的坐标（不出右/下边缘，且不为负）。
 */
export function clampMenuPos(
  x: number,
  y: number,
  menuW = 200,
  menuH = 240
): { x: number; y: number } {
  const maxX = Math.max(8, window.innerWidth - menuW - 8);
  const maxY = Math.max(8, window.innerHeight - menuH - 8);
  return {
    x: Math.min(Math.max(8, x), maxX),
    y: Math.min(Math.max(8, y), maxY),
  };
}

/**
 * 菜单渲染后实测尺寸并校正位置（替代估算宽高）：
 * 首帧按估算夹取渲染，下一帧量 offsetWidth/offsetHeight 重新夹取，
 * 菜单项数量变化（如展开歌单列表）也能贴边不出屏。返回 null = 尚未校正。
 */
export function refineMenuPos(
  el: HTMLElement,
  x: number,
  y: number
): { x: number; y: number } | null {
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  if (!w || !h) return null;
  return clampMenuPos(x, y, w, h);
}

export function hashStr(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = (h * 31 + s.charCodeAt(i)) | 0;
  }
  return Math.abs(h);
}

/** 根据字符串生成稳定的占位渐变（暖色系：陶土 / 琥珀 / 赭石） */
export function gradientFor(seed: string): string {
  const h = 15 + (hashStr(seed || "?") % 40);
  const h2 = h + 18;
  return `linear-gradient(135deg, hsl(${h}, 52%, 46%), hsl(${h2}, 58%, 32%))`;
}

/** 从封面提取多个代表色（色相分桶取加权色心）
 * asset:// 协议（本地文件）不受同源限制，无需 crossOrigin；
 * http 封面若跨域被 CORS 允许也可采样，失败则静默回退。 */
export async function extractPalette(url: string, count = 4): Promise<string[]> {
  try {
    const img = new Image();
    // 不设 crossOrigin：asset 协议不需要；远程封面按需带
    if (/^https?:/.test(url)) img.crossOrigin = "anonymous";
    img.src = url;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = 36;
    c.height = 36;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    if (!ctx) return [];
    ctx.drawImage(img, 0, 0, 36, 36);
    const data = ctx.getImageData(0, 0, 36, 36).data;
    if (!data || data.length < 4) return [];
    // 按色相分桶，桶内取加权最优色
    const buckets = new Map<number, { r: number; g: number; b: number; w: number }>();
    for (let i = 0; i < data.length; i += 4) {
      const r = data[i], g = data[i + 1], b = data[i + 2];
      if (data[i + 3] < 128) continue;
      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      const sat = max === 0 ? 0 : (max - min) / max;
      const lum = (r * 0.3 + g * 0.6 + b * 0.1) / 255;
      if (lum < 0.06 || lum > 0.97) continue; // 过暗/过亮跳过
      let hue = 0;
      const d = max - min;
      if (d !== 0) {
        if (max === r) hue = ((g - b) / d) % 6;
        else if (max === g) hue = (b - r) / d + 2;
        else hue = (r - g) / d + 4;
        hue *= 60;
        if (hue < 0) hue += 360;
      }
      const bucket = Math.floor(hue / 45);
      const w = 0.4 + sat; // 饱和度高的颜色权重更大
      const cur = buckets.get(bucket) ?? { r: 0, g: 0, b: 0, w: 0 };
      cur.r += r * w; cur.g += g * w; cur.b += b * w; cur.w += w;
      buckets.set(bucket, cur);
    }
    const colors = [...buckets.values()]
      .sort((a, b) => b.w - a.w)
      .slice(0, count)
      .map((x) => {
        const r = x.r / x.w / 255;
        const g = x.g / x.w / 255;
        const b = x.b / x.w / 255;
        const max = Math.max(r, g, b);
        const min = Math.min(r, g, b);
        const l = (max + min) / 2;
        const d = max - min;
        let h = 0;
        let s2 = 0;
        if (d !== 0) {
          s2 = d / (1 - Math.abs(2 * l - 1));
          if (max === r) h = ((g - b) / d) % 6;
          else if (max === g) h = (b - r) / d + 2;
          else h = (r - g) / d + 4;
          h *= 60;
          if (h < 0) h += 360;
        }
        // 输出归一化 hsl：强制增饱和、亮度压低（浅色封面也产出浓艳色）
        s2 = Math.min(1, Math.max(s2, 0.55) * 1.1); // 饱和度至少 55%
        const lOut = Math.min(0.85, Math.max(l, 0.66)); // 亮度 66%–85%（清透）
        return `hsl(${Math.round(h)}, ${Math.round(s2 * 100)}%, ${Math.round(lOut * 100)}%)`;
      });
    if (!colors.length) return [];
    // 全部色彩作为底色还是太浅时（如纯白封面）：注入互补的浓色作伴
    return colors;
  } catch {
    return [];
  }
}

/** 从封面提取主色，用于动态氛围光 */
export async function extractColor(url: string): Promise<string | null> {
  try {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.src = url;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = 24;
    c.height = 24;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0, 24, 24);
    const data = ctx.getImageData(0, 0, 24, 24).data;
    let best = { score: -1, r: 129, g: 140, b: 248 };
    for (let i = 0; i < data.length; i += 4) {
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const max = Math.max(r, g, b);
      const min = Math.min(r, g, b);
      const sat = max === 0 ? 0 : (max - min) / max;
      const lum = (r * 0.3 + g * 0.6 + b * 0.1) / 255;
      const score = sat * 1.2 + (1 - Math.abs(lum - 0.55));
      if (score > best.score) best = { score, r, g, b };
    }
    return `rgb(${best.r}, ${best.g}, ${best.b})`;
  } catch {
    return null;
  }
}

export function trackTitle(t: TrackMeta): string {
  return t.title || t.path.split(/[\\/]/).pop() || "未知曲目";
}

export function trackArtist(t: TrackMeta): string {
  return t.artist || "未知艺术家";
}

export function matchSearch(t: TrackMeta, q: string): boolean {
  if (!q) return true;
  const s = q.toLowerCase();
  return (
    t.title.toLowerCase().includes(s) ||
    t.artist.toLowerCase().includes(s) ||
    t.album.toLowerCase().includes(s)
  );
}

/** 单个字符的"演唱权重"：CJK/全角字符占更多视觉宽度，权重高于拉丁字母 */
function charWeight(c: string): number {
  const code = c.codePointAt(0) ?? 0;
  if (code >= 0x1100 && (code <= 0x115f || code === 0x3000)) return 2; // CJK 标点/全角空格
  if (code >= 0x2e80 && code <= 0xa4cf) return 2; // CJK 部首-彝文
  if (code >= 0xac00 && code <= 0xd7a3) return 2; // 谚文音节
  if (code >= 0xf900 && code <= 0xfaff) return 2; // CJK 兼容
  if (code >= 0xff00 && code <= 0xff60) return 2; // 全角形式
  if (c === " ") return 0.4; // 空格快
  return 1;
}

/**
 * 歌词行染色推进比例（0~1）：卡拉OK 染色随演唱节奏走。
 * 1) 有逐字时间戳（yrc/QRC/增强 LRC）：按当前时间落在哪个字、
 *    字内按线性插值，精确贴合实际演唱（句内快慢不均）。
 * 2) 无逐字数据：按"每字耗时 ∝ 字符宽度权重"分配行时长，
 *    比匀速好得多（汉字唱得慢、空格标点瞬时掠过）。
 * 行结束时间取下一行起始（与换行判定一致）。
 */
export function lyricLineProgress(
  line: LyricLine,
  posIn: number,
  lineEnd: number
): number {
  const start = line.timeMs ?? 0;
  const end = Math.max(start + 400, lineEnd);
  if (end <= start) return 0;
  const pos = Math.min(end, Math.max(start, posIn));

  const words = line.words?.filter((w) => w.text.length > 0);
  if (words && words.length) {
    // 逐字时间戳路径：每个字在其 [start,end] 内线性点亮，
    // 染色边界精确贴合实际演唱节奏（句内快慢不均）
    let sungWeight = 0;
    let totalWeight = 0;
    for (const w of words) {
      const weight = [...w.text].reduce((s, c) => s + charWeight(c), 0);
      totalWeight += weight;
      if (pos >= w.endMs) sungWeight += weight;
      else if (pos > w.startMs)
        sungWeight += weight * ((pos - w.startMs) / (w.endMs - w.startMs || 1));
    }
    if (totalWeight <= 0) return 0;
    return Math.min(1, sungWeight / totalWeight);
  }

  // 无逐字数据：把行时长按字宽权重分摊到每个字（汉字唱得久、空格标点瞬时掠过），
  // 得到伪逐字时间轴后与真实逐字走同一条插值路径
  const chars = [...line.text];
  if (!chars.length) return 0;
  const weights = chars.map(charWeight);
  const total = weights.reduce((s, w) => s + w, 0);
  if (total <= 0) return 0;
  let cursor = start;
  let sungWeight = 0;
  for (let i = 0; i < chars.length; i++) {
    const wEnd = cursor + (weights[i] / total) * (end - start);
    if (pos >= wEnd) sungWeight += weights[i];
    else if (pos > cursor)
      sungWeight += weights[i] * ((pos - cursor) / (wEnd - cursor || 1));
    cursor = wEnd;
  }
  return Math.min(1, sungWeight / total);
}

// ---------- 播放页歌词自适应配色 ----------

/** 解析 "hsl(H, S%, L%)" 字符串 → [h(0-360), s(0-1), l(0-1)]；非法返回 null */
export function parseHsl(str: string): [number, number, number] | null {
  const m = str.match(/hsl\(\s*([\d.]+)(?:deg)?[,\s]+([\d.]+)%[,\s]+([\d.]+)%\s*\)/i);
  if (!m) return null;
  return [parseFloat(m[1]), parseFloat(m[2]) / 100, parseFloat(m[3]) / 100];
}

export function hslToCss(h: number, s: number, l: number): string {
  return `hsl(${Math.round(h)}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%)`;
}

/** HSL → WCAG 相对亮度（0=黑，1=白） */
export function hslRelLum(h: number, s: number, l: number): number {
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  const lin = (v: number) =>
    v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  return 0.2126 * lin(f(0)) + 0.7152 * lin(f(8)) + 0.0722 * lin(f(4));
}

export interface LyricAdaptiveColors {
  /** 写入播放页容器的 CSS 变量（--lyric-unsung/dim/next/sung/shadow） */
  vars: Record<string, string>;
  /** 歌词区柔边遮罩颜色（rgba）；空串 = 不需要遮罩 */
  scrim: string;
}

/**
 * 播放页歌词自适应配色（有封面调色板时启用）。
 *
 * 背景建模：歌词背后的亮度 = 主题底色(45%) ⊕ 对流渐变层(调色板 55%)。
 * 渐变层持续旋转、光斑漂移，歌词区背后的颜色会在调色板各色之间游走——
 * 因此用「最亮/最暗区域」的最坏情况对比度决策，而不是平均亮度
 * （旧策略按平均一刀切选黑/白字，旋转到亮色光斑时白字直接看不清）。
 *
 * 策略：
 * - 近白文字为默认（Apple Music 式观感）；近黑文字只在明亮背景上
 *   「决定性更优」（对最暗区域 ≥4.5:1 且明显强于白字）时启用；
 * - 与 4.5:1 的对比缺口按比例三档补偿：文字阴影加强、次行/远行
 *   透明度提高、歌词区加柔边遮罩（scrim）；
 * - 已唱染色取调色板饱和度最高的一色，沿亮度轴压到「与未唱文字
 *   可区分、与最亮/最暗背景区域都可读」的区间——染色跟封面走，
 *   不再固定强调色（同色系封面上强调色会隐形）。
 */
export function lyricAdaptiveColors(
  palette: string[],
  theme: string
): LyricAdaptiveColors {
  const bgLum = theme === "light" ? 0.87 : 0.04;
  const parsed = palette
    .map(parseHsl)
    .filter((x): x is [number, number, number] => x != null);
  if (parsed.length < 2) return { vars: {}, scrim: "" };

  const regionLum = (p: number) => bgLum * 0.45 + p * 0.55;
  const lums = parsed.map(([h, s, l]) => hslRelLum(h, s, l));
  const minR = regionLum(Math.min(...lums));
  const maxR = regionLum(Math.max(...lums));
  const eff = regionLum(lums.reduce((a, b) => a + b, 0) / lums.length);
  // 白字 vs 最亮区域、近黑字 vs 最暗区域的最坏情况对比度
  const cWhite = 1.05 / (maxR + 0.05);
  const cDark = (minR + 0.05) / 0.07;
  const dark = cDark > cWhite * 1.3 && cDark >= 4.5;
  const strength = Math.min(1, Math.max(0.15, (Math.max(cWhite, cDark) - 1.6) / 3.4));
  const lack = 1 - strength;

  // 对比越弱：文字越实、远行越亮、阴影越重
  const unsungA = (0.95 + 0.05 * strength).toFixed(2);
  const nextA = (0.8 + 0.12 * strength).toFixed(2);
  const dimA = (0.88 - 0.2 * strength).toFixed(2);
  const sh = 0.2 + 0.32 * lack;

  const vars: Record<string, string> = dark
    ? {
        "--lyric-unsung": `rgba(21, 24, 31, ${unsungA})`,
        "--lyric-dim": `rgba(21, 24, 31, ${dimA})`,
        "--lyric-next": `rgba(21, 24, 31, ${nextA})`,
        "--lyric-shadow": `0 1px 2px rgba(255, 255, 255, ${(sh * 0.7).toFixed(2)}), 0 2px 18px rgba(255, 255, 255, ${sh.toFixed(2)})`,
      }
    : {
        "--lyric-unsung": `rgba(255, 255, 255, ${unsungA})`,
        "--lyric-dim": `rgba(255, 255, 255, ${dimA})`,
        "--lyric-next": `rgba(255, 255, 255, ${nextA})`,
        "--lyric-shadow": `0 1px 2px rgba(10, 12, 16, ${(sh * 0.7).toFixed(2)}), 0 2px 18px rgba(10, 12, 16, ${sh.toFixed(2)})`,
      };

  // 已唱染色：取调色板饱和度最高的一色，保色相，用二分法把 WCAG 相对
  // 亮度钉到目标值。目标值自适应背景亮度带：
  // - 带（minR+0.14 ~ maxR-0.14）非空 → 取带中心，离两端最远；
  // - 窄带（背景亮度均匀，最常见）→ 逃到带外：白字优先压深（与白字
  //   对比自然拉大），深不动（minR 已很低）则提亮；黑字压深。
  // 相对亮度对 l 单调，二分必收敛；黄色系 relLum 天然偏高会自动压深，
  // 色相不丢，染色始终跟封面走。
  const [h, s0] = [...parsed].sort((a, b) => b[1] - a[1])[0];
  const s = Math.min(0.95, Math.max(s0, 0.65));
  let target: number;
  if (maxR - 0.14 > minR + 0.14) {
    target = (minR + maxR) / 2;
  } else if (dark) {
    target = Math.max(0.1, minR - 0.18);
  } else {
    const deeper = minR - 0.18;
    target = deeper >= 0.16 ? deeper : Math.min(0.55, maxR + 0.18);
  }
  // 与文字色的可区分约束：白字下染色 ≤0.55，黑字下 0.10~0.45
  target = dark ? Math.max(0.1, Math.min(0.45, target)) : Math.min(0.55, Math.max(0.12, target));
  let lo = 0.05;
  let hi = 0.95;
  for (let i = 0; i < 14; i++) {
    const mid = (lo + hi) / 2;
    if (hslRelLum(h, s, mid) < target) lo = mid;
    else hi = mid;
  }
  vars["--lyric-sung"] = hslToCss(h, s, (lo + hi) / 2);

  // 柔边遮罩：仅对比不足时上（越缺越浓，上限 ~0.33）
  const scrimA = Math.max(0, 0.82 - strength) * 0.5;
  const scrim =
    scrimA > 0.02
      ? dark
        ? `rgba(255, 255, 255, ${scrimA.toFixed(2)})`
        : `rgba(10, 12, 16, ${scrimA.toFixed(2)})`
      : "";
  return { vars, scrim };
}
