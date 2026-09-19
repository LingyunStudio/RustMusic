/**
 * 桌面歌词窗口入口（独立 HTML：desktop-lyrics.html）
 *
 * 形态：背景全透明（桌面只见文字）；鼠标移入时浮现半透明玻璃底 + 播放控制；
 * 全窗皆为拖拽热区（含歌词上方空白），右下角缩放；字号随窗口宽度等比缩放。
 * 暂停时进度冻结（不做外推），卡拉OK 染色停在原地。
 * 数据链路：主窗口 store emit "dlyrics://push"（250ms/帧 + 歌词加载完成）。
 * 播放控制：emit "media://control"（主窗口 store 处理，与托盘一致）。
 */
import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { LyricLine } from "../types";
import { lyricLineProgress } from "../utils";
import "../index.css";

interface PushPayload {
  /** 歌词行；缺省 = 与上一帧相同（主窗口只在歌词变化时携带，
   *  避免 250ms 一帧全量序列化逐字歌词） */
  lines?: LyricLine[] | null;
  synced: boolean;
  pos: number;
  dur: number;
  playing: boolean;
  title: string;
  artist: string;
  /** 三色（缺省时用内置默认）：sung=已唱 unsung=未唱 next=下一句 */
  colors?: {
    sung: string;
    unsung: string;
    next: string;
  };
}

const DEFAULT_COLORS = {
  sung: "#ffc45e",
  unsung: "rgba(255,255,255,0.9)",
  next: "rgba(230,168,82,0.8)",
};

/* 入口即强制 body 透明：index.css 的 body{background:var(--bg)} 会盖掉
   desktop-lyrics.html 里的 transparent（后注入的 style 层叠优先） */
document.documentElement.style.background = "transparent";
document.body.style.background = "transparent";

async function mediaControl(action: string) {
  const { emit } = await import("@tauri-apps/api/event");
  await emit("media://control", { action });
}

function DesktopLyrics() {
  const [data, setData] = useState<PushPayload>({
    lines: null,
    synced: false,
    pos: 0,
    dur: 0,
    playing: false,
    title: "",
    artist: "",
  });
  const [locked, setLocked] = useState(false);
  const [hover, setHover] = useState(false);
  const [alwaysTop, setAlwaysTop] = useState(true);
  // 窗口内尺寸（缩放时字号等比跟随：以宽度 800 为基准）
  const [winW, setWinW] = useState(800);
  const posRef = useRef(0);
  const posAtRef = useRef(performance.now());
  const [, setRenderTick] = useState(0);

  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    import("@tauri-apps/api/event").then(({ listen }) => {
      if (disposed) return;
      return listen<PushPayload>("dlyrics://push", (e) => {
        posAtRef.current = performance.now();
        posRef.current = e.payload.pos;
        // 合并而非替换：lines 缺省时沿用上一帧（主窗口瘦身推送）
        setData((prev) => ({ ...prev, ...e.payload }));
      });
    }).then((u) => {
      if (disposed) u?.();
      else un = u as unknown as (() => void) | undefined;
    });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  // 窗口尺寸监听（缩放 → 字号基准）
  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      if (disposed) return;
      return getCurrentWindow().onResized(({ payload }) => {
        setWinW(payload.width / (window.devicePixelRatio || 1));
      });
    }).then((u) => {
      if (disposed) u?.();
      else un = u as unknown as (() => void) | undefined;
    });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  // rAF 插值循环：仅播放中推进（暂停时静止，修复暂停仍更新的 bug）
  useEffect(() => {
    let raf = 0;
    let last = 0;
    const loop = (t: number) => {
      if (data.playing && t - last > 50) {
        last = t;
        setRenderTick((n) => n + 1);
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [data.playing]);

  const posNow =
    data.pos + (data.playing ? performance.now() - posAtRef.current : 0);

  const syncedLines = (data.lines ?? []).filter(
    (l) => l.timeMs != null && l.text.trim()
  );
  let activeIdx = -1;
  if (data.synced && syncedLines.length) {
    for (let i = 0; i < syncedLines.length; i++) {
      if (syncedLines[i].timeMs! <= posNow) activeIdx = i;
      else break;
    }
  }
  const active = activeIdx >= 0 ? syncedLines[activeIdx] : null;
  const next =
    activeIdx >= 0 && activeIdx + 1 < syncedLines.length
      ? syncedLines[activeIdx + 1]
      : null;

  // 染色推进：逐字时间戳按实际演唱节奏；行级 LRC 按字宽加权
  const fillRatio = active
    ? lyricLineProgress(
        active,
        posNow,
        next ? next.timeMs! : Math.max(active.timeMs! + 8000, data.dur)
      )
    : 0;

  // 字号随窗口宽度等比缩放（800 为基准）
  const scale = Math.max(0.6, Math.min(2, winW / 800));
  const mainFont = Math.round(34 * scale);
  const nextFont = Math.round(15 * scale);
  // 三色（设置页可改，随 push 载荷实时更新）
  const colors = { ...DEFAULT_COLORS, ...(data.colors ?? {}) };

  const close = async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("desktop_lyrics_close");
  };
  const toggleLock = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const next = !locked;
    await getCurrentWindow().setIgnoreCursorEvents(next);
    // 锁定后鼠标穿透，mouseleave 不会再触发：必须同步清掉 hover，
    // 否则半透明底和控制条会永久残留
    if (next) setHover(false);
    setLocked(next);
  };
  const toggleTop = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    await w.setAlwaysOnTop(!alwaysTop);
    setAlwaysTop(!alwaysTop);
  };
  const startResize = async (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().startResizeDragging("SouthEast");
  };

  const idleText = data.playing ? "纯音乐，请欣赏" : "等待播放…";

  return (
    // 全窗拖拽热区（含歌词上方空白）；hover 才浮现半透明底与控制条
    <div
      data-tauri-drag-region
      className="h-full w-full flex flex-col justify-center select-none relative transition-colors duration-200"
      style={{
        background: hover ? "rgba(16, 12, 8, 0.42)" : "transparent",
        backdropFilter: hover ? "blur(10px)" : undefined,
        borderRadius: 14,
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
    >
      {/* 播放控制条（hover 时浮现，顶行居中） */}
      {!locked && hover && (
        <div
          className="absolute top-2 left-1/2 -translate-x-1/2 z-10 flex items-center gap-0.5 px-1.5 py-1 rounded-2xl"
          style={{
            background: "rgba(20, 16, 12, 0.55)",
            border: "1px solid rgba(255,255,255,0.16)",
          }}
        >
          <button
            className="text-[15px] w-8 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={() => mediaControl("prev")}
            title="上一首"
          >
            ⏮
          </button>
          <button
            className="text-[15px] w-8 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: colors.sung }}
            onClick={() => mediaControl("toggle")}
            title={data.playing ? "暂停" : "播放"}
          >
            {data.playing ? "⏸" : "▶"}
          </button>
          <button
            className="text-[15px] w-8 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={() => mediaControl("next")}
            title="下一首"
          >
            ⏭
          </button>
          <span className="w-px h-4 mx-1" style={{ background: "rgba(255,255,255,0.2)" }} />
          <button
            className="text-[13px] w-7 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={toggleLock}
            title="锁定（鼠标穿透，主窗口按 L 解锁）"
          >
            🔒
          </button>
          <button
            className="text-[13px] w-7 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: alwaysTop ? colors.sung : "rgba(255,255,255,0.85)" }}
            onClick={toggleTop}
            title="置顶"
          >
            📌
          </button>
          <button
            className="text-[13px] w-7 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={close}
            title="关闭"
          >
            ✕
          </button>
        </div>
      )}

      {/* 右下角缩放手柄 */}
      {!locked && (
        <div
          onPointerDown={startResize}
          className="absolute bottom-1 right-1 w-[20px] h-[20px] cursor-nwse-resize z-10 flex items-center justify-center"
          title="拖动缩放"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" style={{ opacity: hover ? 0.9 : 0 }}>
            <path
              d="M9 1.5 L9 9 L1.5 9"
              stroke="rgba(255,255,255,0.6)"
              strokeWidth="1.5"
              fill="none"
            />
          </svg>
        </div>
      )}

      {/* 歌词主体（pointer-events-none：点击穿透到父层 drag-region，
          全区任意位置可拖；控制条/手柄单独恢复交互） */}
      <div className="flex flex-col items-center gap-1.5 px-6 pointer-events-none">
        {data.synced && active ? (
          <>
            <div
              className="relative max-w-full"
              style={{ textShadow: "0 2px 14px rgba(0,0,0,0.55)" }}
            >
              <div
                className="whitespace-nowrap"
                style={{
                  fontSize: mainFont,
                  fontWeight: 800,
                  color: colors.unsung,
                  letterSpacing: "0.02em",
                  lineHeight: 1.25,
                }}
              >
                {active.text}
              </div>
              <div
                className="absolute inset-0"
                style={{ clipPath: `inset(0 ${100 - fillRatio * 100}% 0 0)` }}
                aria-hidden
              >
                <div
                  className="whitespace-nowrap"
                  style={{
                    fontSize: mainFont,
                    fontWeight: 800,
                    color: colors.sung,
                    letterSpacing: "0.02em",
                    lineHeight: 1.25,
                  }}
                >
                  {active.text}
                </div>
              </div>
            </div>
            {next && (
              <div
                className="whitespace-nowrap max-w-full overflow-hidden"
                style={{
                  fontSize: nextFont,
                  color: colors.next,
                  textShadow: "0 1px 6px rgba(0,0,0,0.45)",
                }}
              >
                {next.text}
              </div>
            )}
          </>
        ) : (
          <div
            className="whitespace-nowrap"
            style={{
              fontSize: Math.round(26 * scale),
              fontWeight: 700,
              color: "rgba(255,255,255,0.85)",
              textShadow: "0 2px 12px rgba(0,0,0,0.5)",
            }}
          >
            {idleText}
          </div>
        )}
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<DesktopLyrics />);
