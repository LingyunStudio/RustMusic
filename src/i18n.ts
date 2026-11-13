import { useSyncExternalStore } from "react";

export type LanguagePref = "auto" | "zh" | "en";
export type ResolvedLang = "zh" | "en";

export const LANG_STORAGE_KEY = "rustmusic.lang";

const listeners = new Set<() => void>();

export function normalizeLanguagePref(raw?: string | null): LanguagePref {
  const s = (raw ?? "").trim().toLowerCase();
  if (s === "zh" || s.startsWith("zh-") || s.startsWith("zh_")) return "zh";
  if (s === "en" || s.startsWith("en-") || s.startsWith("en_")) return "en";
  return "auto";
}

export function detectSystemLang(): ResolvedLang {
  try {
    const primary =
      (Array.isArray(navigator.languages) && navigator.languages[0]) ||
      navigator.language ||
      "";
    return primary.toLowerCase().startsWith("zh") ? "zh" : "en";
  } catch {
    return "en";
  }
}

export function resolveLang(pref: LanguagePref): ResolvedLang {
  if (pref === "zh" || pref === "en") return pref;
  return detectSystemLang();
}

function readStoredPref(): LanguagePref {
  try {
    return normalizeLanguagePref(localStorage.getItem(LANG_STORAGE_KEY));
  } catch {
    return "auto";
  }
}

let currentPref: LanguagePref = readStoredPref();
let currentLang: ResolvedLang = resolveLang(currentPref);
let currentSnapshot = `${currentPref}:${currentLang}`;

function applyDocumentLang(lang: ResolvedLang) {
  try {
    document.documentElement.lang = lang === "en" ? "en" : "zh-CN";
  } catch {
    // non-DOM test environments
  }
}

function emitChange() {
  for (const fn of listeners) fn();
}

export function getLanguagePref(): LanguagePref {
  return currentPref;
}

export function getResolvedLang(): ResolvedLang {
  return currentLang;
}

export function isEn(): boolean {
  return currentLang === "en";
}

export function tr(zh: string, en: string): string {
  return currentLang === "en" ? en : zh;
}

export function trFor(lang: ResolvedLang, zh: string, en: string): string {
  return lang === "en" ? en : zh;
}

export function setLanguagePref(
  pref: LanguagePref,
  opts?: { persist?: boolean }
): ResolvedLang {
  const nextPref = normalizeLanguagePref(pref);
  const nextLang = resolveLang(nextPref);
  const nextSnap = `${nextPref}:${nextLang}`;

  if (opts?.persist !== false) {
    try {
      localStorage.setItem(LANG_STORAGE_KEY, nextPref);
    } catch {
      // ignore storage errors
    }
  }

  currentPref = nextPref;
  currentLang = nextLang;
  applyDocumentLang(nextLang);

  if (nextSnap !== currentSnapshot) {
    currentSnapshot = nextSnap;
    emitChange();
  }
  return nextLang;
}

export function subscribeLang(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function getSnapshot(): string {
  return currentSnapshot;
}

export function useLang(): ResolvedLang {
  useSyncExternalStore(subscribeLang, getSnapshot, getSnapshot);
  return currentLang;
}

export function useI18n() {
  useSyncExternalStore(subscribeLang, getSnapshot, getSnapshot);
  return {
    pref: currentPref,
    lang: currentLang,
    isEn: currentLang === "en",
    tr,
  };
}

let initialized = false;

/** 初始化多语言监听：跨窗口 localStorage 与 Tauri app://language-changed 同步 */
export function initI18n() {
  applyDocumentLang(currentLang);
  if (initialized) return;
  initialized = true;

  try {
    window.addEventListener("storage", (e) => {
      if (e.key === LANG_STORAGE_KEY) {
        setLanguagePref(normalizeLanguagePref(e.newValue), { persist: false });
      }
    });
  } catch {
    // ignore
  }

  // Tauri 运行时事件同步（主窗口/桌面歌词窗互相同步）
  import("@tauri-apps/api/event")
    .then(({ listen }) =>
      listen<{ language?: string; pref?: string }>("app://language-changed", (e) => {
        const raw = e.payload?.language ?? e.payload?.pref;
        if (raw) {
          setLanguagePref(normalizeLanguagePref(raw), {
            persist: false,
          });
        }
      })
    )
    .catch(() => {});
}

const QUALITY_EN_MAP: Record<string, string> = {
  "超清母带": "Master",
  "臻品母带": "Master",
  "蝰蛇母带": "Master",
  "超品母带": "Master",
  "沉浸环绕声": "Spatial",
  "臻品全景声": "Spatial",
  "蝰蛇全景声": "Spatial",
  "高清环绕声": "HD Surround",
  "臻品音质 2.0": "Premium 2.0",
  "臻品音质2.0": "Premium 2.0",
  "蝰蛇超清": "Viper HD",
  "无损": "Lossless",
  "极高": "Very High",
  "较高": "Higher",
  "标准": "Standard",
};

export function formatQualityLabel(
  q?: string | null,
  lang: ResolvedLang = currentLang
): string {
  if (!q) return "";
  if (lang !== "en") return q;
  return QUALITY_EN_MAP[q] ?? q;
}

const LYRIC_SOURCE_EN_MAP: Record<string, string> = {
  "逐字 YRC": "Word YRC",
  "逐字 QRC": "Word QRC",
  "逐字 KRC": "Word KRC",
  "行级 LRC": "Line LRC",
  "字幕 B站": "Bilibili CC",
};

export function formatLyricSource(
  src?: string | null,
  lang: ResolvedLang = currentLang
): string {
  if (!src) return "";
  if (lang !== "en") return src;
  return LYRIC_SOURCE_EN_MAP[src] ?? src;
}
