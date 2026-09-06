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
  { key: "coral", label: "珊瑚", base: "#e8674a", strong: "#ff8f6e", onLight: "#fff" },
  { key: "rose", label: "玫红", base: "#d94f6e", strong: "#f4758f", onLight: "#fff" },
  { key: "violet", label: "紫罗兰", base: "#8e6fd8", strong: "#ab93ea", onLight: "#fff" },
  { key: "ocean", label: "海蓝", base: "#3f8fd4", strong: "#6bace8", onLight: "#fff" },
  { key: "teal", label: "青碧", base: "#3aa896", strong: "#5fc0af", onLight: "#fff" },
  { key: "moss", label: "苔绿", base: "#7ba85f", strong: "#9cc285", onLight: "#fff" },
  { key: "graphite", label: "石墨", base: "#6b7280", strong: "#8b95a3", onLight: "#fff" },
];

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

function rgbToCss(r: number, g: number, b: number): string {
  return `rgb(${r}, ${g}, ${b})`;
}

function mix(hex: string, white: number): string {
  const [r, g, b] = hexToRgb(hex);
  const f = (c: number) => Math.round(c + (255 - c) * white);
  return rgbToCss(f(r), f(g), f(b));
}

export function applyTheme(theme: "dark" | "light") {
  document.documentElement.dataset.theme = theme;
}

export function applyAccent(key: string) {
  const acc =
    ACCENTS.find((a) => a.key === key) ?? ACCENTS[0];
  const light = document.documentElement.dataset.theme === "light";
  // 浅色下用更强的一档保证对比度，暗色用基准色
  const base = light ? acc.base : acc.base;
  const strong = light ? acc.strong : acc.strong;
  const root = document.documentElement.style;
  root.setProperty("--accent", base);
  root.setProperty("--accent-strong", strong);
  root.setProperty("--accent-soft", `rgba(${hexToRgb(base).join(", ")}, 0.35)`);
  root.setProperty("--accent-weak", `rgba(${hexToRgb(base).join(", ")}, 0.14)`);
  // 浅色模式按钮文字对比色
  root.setProperty("--accent-on", acc.onLight);
  void mix;
}

export function loadTheme(): "dark" | "light" {
  const t = localStorage.getItem(THEME_KEY);
  return t === "light" ? "light" : "dark";
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
