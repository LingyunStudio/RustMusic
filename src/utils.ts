import type { LyricsPayload, TrackMeta } from "./types";

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
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
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

/** 从封面提取多个代表色（k-means 简化版：分桶取最饱和/最亮的色心） */
export async function extractPalette(url: string, count = 4): Promise<string[]> {
  try {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.src = url;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = 36;
    c.height = 36;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    if (!ctx) return [];
    ctx.drawImage(img, 0, 0, 36, 36);
    const data = ctx.getImageData(0, 0, 36, 36).data;
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
      .map((x) => `rgb(${Math.round(x.r / x.w)}, ${Math.round(x.g / x.w)}, ${Math.round(x.b / x.w)})`);
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
