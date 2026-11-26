//! 应用内可自定义快捷键配置、序列化、跨平台展示与按键匹配
import { PLATFORM } from "./platform";

export type ShortcutAction =
  | "playPause"
  | "prevTrack"
  | "nextTrack"
  | "seekBackward"
  | "seekForward"
  | "volumeUp"
  | "volumeDown"
  | "toggleMute"
  | "toggleDesktopLyrics"
  | "focusSearch";

export interface ShortcutMeta {
  id: ShortcutAction;
  zh: string;
  en: string;
  defaultCombo: string;
  allowRepeat?: boolean;
}

export const SHORTCUT_LIST: ShortcutMeta[] = [
  { id: "playPause", zh: "播放 / 暂停", en: "Play / Pause", defaultCombo: "Space" },
  { id: "prevTrack", zh: "上一首", en: "Previous Track", defaultCombo: "Mod+ArrowLeft" },
  { id: "nextTrack", zh: "下一首", en: "Next Track", defaultCombo: "Mod+ArrowRight" },
  { id: "seekBackward", zh: "快退 5 秒", en: "Rewind 5s", defaultCombo: "ArrowLeft", allowRepeat: true },
  { id: "seekForward", zh: "快进 5 秒", en: "Forward 5s", defaultCombo: "ArrowRight", allowRepeat: true },
  { id: "volumeUp", zh: "增大音量", en: "Volume Up", defaultCombo: "ArrowUp", allowRepeat: true },
  { id: "volumeDown", zh: "减小音量", en: "Volume Down", defaultCombo: "ArrowDown", allowRepeat: true },
  { id: "toggleMute", zh: "静音切换", en: "Toggle Mute", defaultCombo: "KeyM" },
  { id: "toggleDesktopLyrics", zh: "桌面歌词开关 / 解锁", en: "Toggle / Unlock Desktop Lyrics", defaultCombo: "KeyL" },
  { id: "focusSearch", zh: "聚焦搜索框", en: "Focus Search", defaultCombo: "Mod+KeyF" },
];

export type ShortcutMap = Record<ShortcutAction, string>;

const STORAGE_KEY = "rustmusic.shortcuts.v1";

export function defaultShortcuts(): ShortcutMap {
  const map = {} as ShortcutMap;
  for (const item of SHORTCUT_LIST) {
    map[item.id] = item.defaultCombo;
  }
  return map;
}

function readStored(): ShortcutMap {
  const base = defaultShortcuts();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return base;
    const parsed = JSON.parse(raw) as Partial<Record<ShortcutAction, unknown>>;
    for (const item of SHORTCUT_LIST) {
      const v = parsed[item.id];
      if (typeof v === "string") {
        base[item.id] = v;
      }
    }
  } catch {
    /* ignore malformed storage */
  }
  return base;
}

let currentMap: ShortcutMap = readStored();
let recording = false;
const listeners = new Set<() => void>();

function notify() {
  for (const fn of listeners) fn();
}

export function getShortcuts(): ShortcutMap {
  return currentMap;
}

export function subscribeShortcuts(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function setShortcutRecording(on: boolean) {
  recording = on;
}

export function isShortcutRecording(): boolean {
  return recording;
}

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(currentMap));
  } catch {}
}

export function setShortcut(action: ShortcutAction, combo: string) {
  currentMap = { ...currentMap, [action]: combo };
  persist();
  notify();
}

export function resetShortcut(action: ShortcutAction) {
  const def = SHORTCUT_LIST.find((s) => s.id === action)?.defaultCombo ?? "";
  setShortcut(action, def);
}

export function resetAllShortcuts() {
  currentMap = defaultShortcuts();
  persist();
  notify();
}

export function hasCustomShortcuts(map: ShortcutMap = currentMap): boolean {
  return SHORTCUT_LIST.some((item) => map[item.id] !== item.defaultCombo);
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/** 从 KeyboardEvent 提取规范组合键字符串（如 "Mod+KeyF" / "Space" / "Shift+ArrowRight"）；仅修饰键返回 null */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (!e.code || MODIFIER_CODES.has(e.code)) return null;
  const parts: string[] = [];
  const isMac = PLATFORM === "macos";
  if (isMac ? e.metaKey : e.ctrlKey) parts.push("Mod");
  if (isMac && e.ctrlKey) parts.push("Ctrl");
  if (!isMac && e.metaKey) parts.push("Meta");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(e.code);
  return parts.join("+");
}

/** 精确判断 KeyboardEvent 是否命中某条已配置快捷键 */
export function matchShortcut(e: KeyboardEvent, combo: string): boolean {
  if (!combo) return false;
  const actual = comboFromEvent(e);
  return actual !== null && actual === combo;
}

/** 查找与指定组合键冲突的其它动作 */
export function findShortcutConflict(
  action: ShortcutAction,
  combo: string,
  map: ShortcutMap = currentMap
): ShortcutMeta | null {
  if (!combo) return null;
  for (const item of SHORTCUT_LIST) {
    if (item.id !== action && map[item.id] === combo) {
      return item;
    }
  }
  return null;
}

/** 将规范键位字符串格式化为人类可读的键帽数组（跨平台显示 Ctrl / ⌘） */
export function formatComboParts(combo: string): string[] {
  if (!combo) return [];
  const isMac = PLATFORM === "macos";
  return combo.split("+").map((part) => {
    if (part === "Mod") return isMac ? "⌘" : "Ctrl";
    if (part === "Alt") return isMac ? "⌥" : "Alt";
    if (part === "Shift") return isMac ? "⇧" : "Shift";
    if (part === "Meta") return isMac ? "⌘" : "Win";
    if (part === "Space") return "Space";
    if (part === "ArrowLeft") return "←";
    if (part === "ArrowRight") return "→";
    if (part === "ArrowUp") return "↑";
    if (part === "ArrowDown") return "↓";
    if (part.startsWith("Key") && part.length === 4) return part.slice(3);
    if (part.startsWith("Digit") && part.length === 6) return part.slice(5);
    if (part === "Minus") return "-";
    if (part === "Equal") return "=";
    if (part === "BracketLeft") return "[";
    if (part === "BracketRight") return "]";
    if (part === "Backslash") return "\\";
    if (part === "Semicolon") return ";";
    if (part === "Quote") return "'";
    if (part === "Comma") return ",";
    if (part === "Period") return ".";
    if (part === "Slash") return "/";
    if (part === "Backquote") return "`";
    return part;
  });
}
