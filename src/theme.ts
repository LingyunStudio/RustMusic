/** 主题系统：浅色/暗色 + 强调色选择，localStorage 持久化 */

export interface AccentColor {
  key: string;
  label: string;
  base: string; // 主色
  strong: string; // 亮一档（浅色模式可作主色）
  onLight: string; // 浅色模式下按钮文字色
}

export const ACCENTS: AccentColor[] = [
  { key: "amber", label: "琥珀", base: "#f0a24a", strong: "#ffc470", onLight: "#fff" },
  { key: "tangerine", label: "橙黄", base: "#e8823f", strong: "#ff9d5c", onLight: "#fff" },
  { key: "coral", label: "珊瑚", base: "#e8674a", strong: "#ff8f6e", onLight: "#fff" },
  { key: "rose", label: "玫红", base: "#d94f6e", strong: "#f4758f", onLight: "#fff" },
  { key: "crimson", label: "绛紫", base: "#b04a8f", strong: "#cd6ba9", onLight: "#fff" },
  { key: "violet", label: "紫罗兰", base: "#8e6fd8", strong: "#ab93ea", onLight: "#fff" },
  { key: "iris", label: "鸢尾", base: "#6d6ee0", strong: "#8f90ea", onLight: "#fff" },
  { key: "ocean", label: "海蓝", base: "#3f8fd4", strong: "#6bace8", onLight: "#fff" },
  { key: "sky", label: "天青", base: "#3ba6c9", strong: "#63c0dd", onLight: "#fff" },
  { key: "teal", label: "青碧", base: "#3aa896", strong: "#5fc0af", onLight: "#fff" },
  { key: "jade", label: "翡翠", base: "#2f9e7f", strong: "#52b89b", onLight: "#fff" },
  { key: "moss", label: "苔绿", base: "#7ba85f", strong: "#9cc285", onLight: "#fff" },
  { key: "olive", label: "橄榄", base: "#8f9a4d", strong: "#adb866", onLight: "#fff" },
  { key: "graphite", label: "石墨", base: "#6b7280", strong: "#8b95a3", onLight: "#fff" },
  { key: "wine", label: "酒红", base: "#a83f55", strong: "#c65d72", onLight: "#fff" },
];

const CUSTOM_KEY_PREFIX = "custom:";
const CUSTOM_HEX_KEY = "rustmusic.accent.custom";

// ---------- 桌面歌词配色 ----------

const DLYRICS_COLOR_KEY = "rustmusic.dlyrics.colors";

export interface DesktopLyricsColors {
  sung: string; // 已唱（卡拉OK 染色）
  unsung: string; // 未唱（当前行底色）
  next: string; // 下一句小字
}

export const DLYRICS_DEFAULT_COLORS: DesktopLyricsColors = {
  sung: "#ffc45e",
  unsung: "rgba(255,255,255,0.9)",
  next: "rgba(230,168,82,0.8)",
};

export function loadDesktopLyricsColors(): DesktopLyricsColors {
  try {
    const raw = localStorage.getItem(DLYRICS_COLOR_KEY);
    if (!raw) return DLYRICS_DEFAULT_COLORS;
    const v = JSON.parse(raw) as Partial<DesktopLyricsColors>;
    return {
      sung: v.sung || DLYRICS_DEFAULT_COLORS.sung,
      unsung: v.unsung || DLYRICS_DEFAULT_COLORS.unsung,
      next: v.next || DLYRICS_DEFAULT_COLORS.next,
    };
  } catch {
    return DLYRICS_DEFAULT_COLORS;
  }
}

export function saveDesktopLyricsColors(c: DesktopLyricsColors) {
  localStorage.setItem(DLYRICS_COLOR_KEY, JSON.stringify(c));
}

/** 解析自定义色 key（"custom:#RRGGBB"）为 hex；非法返回 null */
export function parseCustomAccent(key: string): string | null {
  if (!key.startsWith(CUSTOM_KEY_PREFIX)) return null;
  const hex = key.slice(CUSTOM_KEY_PREFIX.length);
  return /^#[0-9a-fA-F]{6}$/.test(hex) ? hex : null;
}

/** 读取上次的自定义色（用于取色器回显） */
export function loadCustomAccentHex(): string {
  return localStorage.getItem(CUSTOM_HEX_KEY) ?? "#f0a24a";
}

export function saveCustomAccentHex(hex: string) {
  localStorage.setItem(CUSTOM_HEX_KEY, hex);
}

/** hex → HSL，用于给自定义色派生浅色档和文字对比色 */
function hexToHsl(hex: string): [number, number, number] {
  const [r0, g0, b0] = hexToRgb(hex).map((v) => v / 255) as [number, number, number];
  const max = Math.max(r0, g0, b0);
  const min = Math.min(r0, g0, b0);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === r0) h = ((g0 - b0) / d + (g0 < b0 ? 6 : 0)) / 6;
  else if (max === g0) h = ((b0 - r0) / d + 2) / 6;
  else h = ((r0 - g0) / d + 4) / 6;
  return [h, s, l];
}

function hslToHex(h: number, s: number, l: number): string {
  const f = (n: number) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    const v = l - a * Math.max(-1, Math.min(k - 3, Math.min(9 - k, 1)));
    return Math.round(v * 255)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

const THEME_KEY = "rustmusic.theme";
const ACCENT_KEY = "rustmusic.accent";

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

export function applyTheme(theme: "dark" | "light") {
  document.documentElement.dataset.theme = theme;
}

export function applyAccent(key: string) {
  const customHex = parseCustomAccent(key);
  const acc = customHex
    ? (() => {
        // 自定义色：亮色档 = HSL 亮度 +0.12；文字色按亮度阈值自动黑/白
        const [h, s, l] = hexToHsl(customHex);
        return {
          base: customHex,
          strong: hslToHex(h, Math.max(s * 0.9, 0), Math.min(l + 0.12, 0.94)),
          onLight: l > 0.72 ? "#5a3a10" : "#fff",
        };
      })()
    : ACCENTS.find((a) => a.key === key) ?? ACCENTS[0];
  const root = document.documentElement.style;
  root.setProperty("--accent", acc.base);
  root.setProperty("--accent-strong", acc.strong);
  root.setProperty("--accent-soft", `rgba(${hexToRgb(acc.base).join(", ")}, 0.35)`);
  root.setProperty("--accent-weak", `rgba(${hexToRgb(acc.base).join(", ")}, 0.14)`);
  // 浅色模式按钮文字对比色
  root.setProperty("--accent-on", acc.onLight);
}

export function loadTheme(): "dark" | "light" {
  const t = localStorage.getItem(THEME_KEY);
  // 液态玻璃 UI 以浅色为默认主题
  return t === "dark" ? "dark" : "light";
}

export function loadAccent(): string {
  return localStorage.getItem(ACCENT_KEY) ?? "amber";
}

export function saveTheme(theme: "dark" | "light") {
  localStorage.setItem(THEME_KEY, theme);
}

export function saveAccent(key: string) {
  localStorage.setItem(ACCENT_KEY, key);
}

/** 初始化（应用启动时调用一次） */
export function initTheme() {
  const theme = loadTheme();
  applyTheme(theme);
  applyAccent(loadAccent());
}
