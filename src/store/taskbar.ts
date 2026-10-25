//! 任务栏歌词（Windows）：当前歌词行显示在任务栏托盘折叠图标左侧，
//! Rust 侧嵌入任务栏的原生图层窗口渲染（taskbar_lyric 模块）。
//! 与菜单栏歌词（Linux）/悬浮歌词窗口相互独立，可单独开启。
//! 窗口常驻本进程：应用退出即销毁，无需菜单栏方案的心跳/过期协议。
import { api } from "../api";
import type { Store } from "./contract";
import { lineText } from "./menubar";

const LS_KEY = "rustmusic.dlyricsTaskbar";
const LS_KEY_COLOR = "rustmusic.dlyricsTaskbarColor";
const LS_KEY_DISPLAY = "rustmusic.dlyricsTaskbarDisplay";

let enabled = false;
let color = ""; // 空 = 默认色（Rust 侧随任务栏深浅取黑/白）
let display = "all"; // "all" = 全部显示器；其它 = 显示器设备名
let lastText: string | null = null; // null = 尚未写过（下次必写）

/** 状态帧同步：文本变化才 IPC（250ms 一帧，仅行切换时产生调用） */
export function syncTaskbarLyrics(s: Pick<Store, "lyrics" | "pos" | "current">) {
  if (!enabled) return;
  const text = lineText(s);
  if (text === lastText) return;
  lastText = text;
  void api.dlyricsTaskbarLine(text, color).catch(() => {});
}

/** 歌词颜色（空 = 默认随任务栏深浅）：改后 lastText 置空，≤250ms 内生效 */
export function setTaskbarLyricsColor(c: string) {
  color = c.trim();
  try {
    localStorage.setItem(LS_KEY_COLOR, color);
  } catch {
    // localStorage 不可用：仅本次会话生效
  }
  lastText = null;
}

export function getTaskbarLyricsColor(): string {
  return color;
}

/** 显示范围（"all" = 全部显示器）：切换后 ≤200ms 内生效（定时器兜底重建窗口） */
export function setTaskbarLyricsDisplay(d: string) {
  display = d.trim() || "all";
  try {
    localStorage.setItem(LS_KEY_DISPLAY, display);
  } catch {
    // localStorage 不可用：仅本次会话生效
  }
  void api.taskbarLyricsSetDisplay(display).catch(() => {});
}

export function getTaskbarLyricsDisplay(): string {
  return display;
}

/** 开关（含持久化）。关闭时立即清空显示，窗口由 Rust 侧销毁 */
export function setTaskbarLyricsEnabled(on: boolean) {
  enabled = on;
  try {
    localStorage.setItem(LS_KEY, on ? "1" : "0");
  } catch {
    // 隐私模式等 localStorage 不可用：仅本次会话生效
  }
  lastText = null;
  void api.taskbarLyricsSetEnabled(on).catch(() => {});
}

/** 应用启动恢复上次开关状态与颜色；返回状态供 store 同步 UI */
export function initTaskbarLyrics(): boolean {
  color = localStorage.getItem(LS_KEY_COLOR) ?? "";
  display = localStorage.getItem(LS_KEY_DISPLAY) || "all";
  void api.taskbarLyricsSetDisplay(display).catch(() => {});
  const on = localStorage.getItem(LS_KEY) === "1";
  if (on) setTaskbarLyricsEnabled(true);
  return on;
}
