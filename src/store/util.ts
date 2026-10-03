//! store 内部共享的小工具（无领域归属）。
import { loadDesktopLyricsColors, type DesktopLyricsColors } from "../theme";
import type { Store } from "./contract";

/** 逐字节比较（挂起恢复的刷新用）：数据未变化时保持旧引用，避免整页重渲染闪烁 */
export function jsonEq(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** 推一帧歌词/进度给桌面歌词窗口（事件式，窗口不存在时 emit 静默无副作用）。
 *  歌词行数组只在变化时随帧携带（逐字歌词可达几十 KB，250ms 一帧全量序列化
 *  是持续的 GC 压力）；歌词窗口对缺失的 lines 字段沿用上一次的值。 */
let lastPushedLines: unknown = undefined; // undefined = 尚未推过（下次必带 lines）
export async function pushDesktopLyrics(
  s: Pick<
    Store,
    "lyrics" | "pos" | "dur" | "playing" | "current" | "desktopLyricsOn"
  > & { dlyricsColors?: DesktopLyricsColors }
) {
  if (!s.desktopLyricsOn) return;
  try {
    const lines = s.lyrics?.lines ?? null;
    const includeLines = lines !== lastPushedLines;
    lastPushedLines = lines;
    const { emit } = await import("@tauri-apps/api/event");
    emit(
      "dlyrics://push",
      {
        ...(includeLines ? { lines } : {}),
        synced: s.lyrics?.synced ?? false,
        pos: s.pos,
        dur: s.dur,
        playing: s.playing,
        title: s.current?.title ?? "",
        artist: s.current?.artist ?? "",
        colors: s.dlyricsColors ?? loadDesktopLyricsColors(),
      } as Record<string, unknown>
    );
  } catch {
    // 桌面歌词窗口未开/已关：忽略
  }
}

/** 桌面歌词窗口就绪/新开后强制下一帧携带完整歌词 */
export function resetDesktopLyricsFrame() {
  lastPushedLines = undefined;
}
