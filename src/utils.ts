import type { TrackMeta } from "./types";

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

/** 根据字符串生成稳定的占位渐变 */
export function gradientFor(seed: string): string {
  const h = hashStr(seed || "?") % 360;
  return `linear-gradient(135deg, hsl(${h}, 55%, 42%), hsl(${(h + 50) % 360}, 65%, 28%))`;
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
