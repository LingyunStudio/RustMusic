//! 菜单栏歌词（Linux / GNOME 顶栏）。
//! 当前歌词行文本经 dlyrics_menubar_line 写入会话运行时文件
//!（$XDG_RUNTIME_DIR/rustmusic-menubar.json），GNOME Shell 扩展
//!（rustmusic-lyrics@lingyunstudio，应用内可一键安装）每 400ms 轮询显示。
//! 与悬浮歌词窗口相互独立：悬浮窗未开时也可单独工作。
import { api } from "../api";
import type { Store } from "./contract";

const LS_KEY = "rustmusic.dlyricsMenubar";
const LS_KEY_COLOR = "rustmusic.dlyricsMenubarColor";

let enabled = false;
let color = ""; // 空 = 顶栏主题默认色
let lastText: string | null = null; // null = 尚未写过（下次必写）
let heartbeat: ReturnType<typeof setInterval> | null = null;

/** 顶栏文本：同步歌词=当前行（行间/前奏为空）；未同步=♪ 标题；无曲目=空 */
export function lineText(s: Pick<Store, "lyrics" | "pos" | "current">): string {
  const cur = s.current;
  if (!cur) return "";
  const lines = s.lyrics?.lines ?? [];
  if (s.lyrics?.synced && lines.length > 0) {
    // 当前行二分（行数据有序），前奏/间奏返回空（顶栏隐藏）
    let idx = -1;
    let lo = 0;
    let hi = lines.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if ((lines[mid].timeMs ?? 0) <= s.pos) {
        idx = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    if (idx < 0) return "";
    return lines[idx].text.trim();
  }
  const title = cur.title?.trim() ?? "";
  return title ? `♪ ${title}` : "";
}

/** 状态帧同步：文本变化才落盘（250ms 一帧，仅行切换时产生 IPC） */
export function syncMenubarLyrics(s: Pick<Store, "lyrics" | "pos" | "current">) {
  if (!enabled) return;
  const text = lineText(s);
  if (text === lastText) return;
  lastText = text;
  void api.dlyricsMenubarLine(text, color).catch(() => {});
}

/** 歌词颜色（空 = 顶栏主题默认）：改后 lastText 置空，≤250ms 或一次心跳内生效 */
export function setMenubarLyricsColor(c: string) {
  color = c.trim();
  try {
    localStorage.setItem(LS_KEY_COLOR, color);
  } catch {
    // localStorage 不可用：仅本次会话生效
  }
  lastText = null;
}

export function getMenubarLyricsColor(): string {
  return color;
}

/** 开关（含持久化）。开启时启动 5s 心跳：扩展 15s 收不到更新即隐藏，
 *  应用退出/崩溃后顶栏不残留过期歌词 */
export function setMenubarLyricsEnabled(on: boolean) {
  enabled = on;
  try {
    localStorage.setItem(LS_KEY, on ? "1" : "0");
  } catch {
    // 隐私模式等 localStorage 不可用：仅本次会话生效
  }
  if (on) {
    lastText = null;
    if (heartbeat == null) {
      heartbeat = setInterval(() => {
        if (lastText != null) void api.dlyricsMenubarLine(lastText, color).catch(() => {});
      }, 5000);
    }
  } else if (heartbeat != null) {
    clearInterval(heartbeat);
    heartbeat = null;
    lastText = null;
    void api.dlyricsMenubarLine("", color).catch(() => {});
  }
}

/** 应用启动恢复上次开关状态与颜色；返回状态供 store 同步 UI */
export function initMenubarLyrics(): boolean {
  color = localStorage.getItem(LS_KEY_COLOR) ?? "";
  const on = localStorage.getItem(LS_KEY) === "1";
  if (on) setMenubarLyricsEnabled(true);
  return on;
}
