//! 极简独立小窗模式（Mini Player）
//! 窗口缩成桌面独立悬浮小窗（~360x118），带实时同步歌词、整窗拖拽、全置顶、还原、最小化与完整播控。
import {
  Heart,
  Maximize2,
  Minus,
  Music2,
  Pause,
  Pin,
  PinOff,
  Play,
  SkipBack,
  SkipForward,
  X,
} from "lucide-react";
import React, { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import { fmtTime } from "../utils";

/** 实时滚动/跑马灯歌词栏组件（隔离 pos 高频刷新，避免小窗主体每帧重渲染） */
function MiniLyricBar({ onRestore }: { onRestore: () => void }) {
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const lyrics = useStore((s) => s.lyrics);
  const current = useStore((s) => s.current);
  const lyricsShowTrans = useStore((s) => s.lyricsShowTrans);

  // 预排序时间戳歌词
  const timedLines = useMemo(
    () =>
      lyrics?.synced
        ? lyrics.lines
            .filter((l) => l.timeMs != null && l.text.trim() !== "")
            .sort((a, b) => (a.timeMs ?? 0) - (b.timeMs ?? 0))
        : [],
    [lyrics]
  );

  // 二分查找定位当前行
  const activeLyric = useMemo(() => {
    if (!timedLines.length) return null;
    let lo = 0;
    let hi = timedLines.length - 1;
    let ans = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if ((timedLines[mid].timeMs ?? 0) <= pos) {
        ans = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return ans >= 0 ? timedLines[ans] : null;
  }, [timedLines, pos]);

  const lyricText = activeLyric?.text?.trim();
  const lyricTrans = lyricsShowTrans && activeLyric?.trans?.trim();

  const displayText = lyricText
    ? lyricTrans
      ? `${lyricText}  (${lyricTrans})`
      : lyricText
    : current
    ? "♪ 纯音乐 / 暂无滚动歌词"
    : "双击此处或右上角还原主窗口";

  // 歌词超长跑马灯：利用独立不可见标尺精确测量文字真实宽度
  const containerRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLSpanElement>(null);
  const [overflow, setOverflow] = useState(false);

  useEffect(() => {
    const check = () => {
      const container = containerRef.current;
      const measure = measureRef.current;
      if (!container || !measure || !displayText) {
        setOverflow(false);
        return;
      }
      // 真实文本宽度超过容器可视宽度时激活跑马灯
      setOverflow(measure.offsetWidth > container.clientWidth + 2);
    };

    check();
    const id = requestAnimationFrame(check);
    window.addEventListener("resize", check);
    return () => {
      cancelAnimationFrame(id);
      window.removeEventListener("resize", check);
    };
  }, [displayText]);

  return (
    <div
      onDoubleClick={onRestore}
      title="双击还原主窗口"
      className="mx-2.5 mb-1.5 px-2.5 h-[25px] rounded-md bg-black/5 dark:bg-white/5 border border-black/5 dark:border-white/10 flex items-center gap-1.5 overflow-hidden pointer-events-auto cursor-pointer"
    >
      <Music2 size={11} className="text-[var(--accent)] shrink-0 opacity-80" />
      {/* 离屏标尺：与实际文字拥有完全一致的字号字体，用于准确计算文本在当前样式下的真实物理宽度 */}
      <span
        ref={measureRef}
        aria-hidden="true"
        className="fixed -left-[9999px] -top-[9999px] text-[11px] font-medium whitespace-nowrap pointer-events-none opacity-0 select-none"
      >
        {displayText}
      </span>
      <div
        ref={containerRef}
        className={`flex-1 min-w-0 overflow-hidden text-[11px] font-medium text-[var(--accent-strong)] select-text ${
          overflow ? "marquee-wrap" : ""
        }`}
        title={displayText}
      >
        {overflow ? (
          <span
            key={displayText}
            className="marquee-inner inline-flex items-center whitespace-nowrap"
            style={{
              ["--dur" as string]: `${Math.max(6, displayText.length * 0.38)}s`,
            }}
          >
            <span>{displayText}</span>
            <span className="inline-block w-12 shrink-0" />
            <span>{displayText}</span>
            <span className="inline-block w-12 shrink-0" />
          </span>
        ) : (
          <span className="truncate block leading-tight">{displayText}</span>
        )}
      </div>
      <div className="text-[9.5px] font-mono text-[var(--ink-3)] tabular-nums shrink-0 opacity-75">
        {fmtTime(pos)} / {fmtTime(dur || current?.durationMs || 0)}
      </div>
    </div>
  );
}

export default function MiniPlayer() {
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const togglePlay = useStore((s) => s.togglePlay);
  const next = useStore((s) => s.next);
  const prev = useStore((s) => s.prev);
  const setMiniPlayer = useStore((s) => s.setMiniPlayer);
  const uiTheme = useStore((s) => s.uiTheme);

  // 进度条与寻道
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const seek = useStore((s) => s.seek);
  const setScrubbing = useStore((s) => s.setScrubbing);
  const total = dur || current?.durationMs || 0;

  const [scrubRatio, setScrubRatio] = useState<number | null>(null);
  const barRef = useRef<HTMLDivElement>(null);

  // 置顶状态
  const [pinned, setPinned] = useState(true);

  useEffect(() => {
    let disposed = false;
    import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        const top = await win.isAlwaysOnTop().catch(() => true);
        if (!disposed) setPinned(top);
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, []);

  const handleTogglePin = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const nextPin = !pinned;
    setPinned(nextPin);
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().setAlwaysOnTop(nextPin);
    } catch {}
  };

  const handleRestore = (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setMiniPlayer(false);
  };

  const handleMinimize = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().minimize();
    } catch {}
  };

  const handleClose = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch {}
  };

  // 收藏/喜欢状态
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const neteaseToggleLike = useStore((s) => s.neteaseToggleLike);
  const savedOnline = useStore((s) => s.savedOnline);
  const likedOnline = useStore((s) => s.likedOnline);
  const recentOnline = useStore((s) => s.recentOnline);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const toggleLike = useStore((s) => s.toggleLike);

  const isLiked = useMemo(() => {
    if (!current) return false;
    if (current.kind === "track") return !!current.liked;
    if (current.kind === "netease" && current.nid) return !!neteaseLiked[current.nid];
    if (current.kind === "qq" && current.qid) {
      return (
        !!savedOnline[`qq-${current.qid}`] ||
        likedOnline.some((e) => e.kind === "qq" && e.onlineId === current.qid)
      );
    }
    if (current.kind === "kugou" && current.kgid) {
      return (
        !!savedOnline[`kugou-${current.kgid}`] ||
        likedOnline.some((e) => e.kind === "kugou" && e.onlineId === current.kgid)
      );
    }
    if (current.kind === "bilibili" && current.qid) {
      return (
        !!savedOnline[`bilibili-${current.qid}`] ||
        likedOnline.some((e) => e.kind === "bilibili" && e.onlineId === current.qid)
      );
    }
    if (current.kind === "navidrome" && current.qid) {
      return (
        !!savedOnline[`navidrome-${current.qid}`] ||
        likedOnline.some((e) => e.kind === "navidrome" && e.onlineId === current.qid)
      );
    }
    return false;
  }, [current, neteaseLiked, savedOnline, likedOnline]);

  const handleToggleLike = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!current) return;
    if (current.kind === "track" && current.id != null) {
      toggleLike(current.id);
    } else if (current.kind === "netease" && current.nid != null) {
      neteaseToggleLike(current.nid);
    } else if (current.kind === "qq" && current.qid != null) {
      const entry = [...likedOnline, ...recentOnline].find(
        (it) => it.kind === "qq" && it.onlineId === current.qid
      );
      toggleLikeOnline({
        kind: "qq",
        id: current.qid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: entry?.mediaMid ?? "",
        vip: entry?.vip ?? false,
      });
    } else if (current.kind === "kugou" && current.kgid != null) {
      const entry = [...likedOnline, ...recentOnline].find(
        (it) => it.kind === "kugou" && it.onlineId === current.kgid
      );
      toggleLikeOnline({
        kind: "kugou",
        id: current.kgid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: "",
        vip: entry?.vip ?? false,
      });
    } else if (current.kind === "bilibili" && current.qid != null) {
      toggleLikeOnline({
        kind: "bilibili",
        id: current.qid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
      });
    } else if (current.kind === "navidrome" && current.qid != null) {
      toggleLikeOnline({
        kind: "navidrome",
        id: current.qid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
      });
    }
  };

  // 进度拖动
  const getRatio = (clientX: number) => {
    const el = barRef.current;
    if (!el) return 0;
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0) return 0;
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const ratio = getRatio(e.clientX);
    setScrubbing(true);
    setScrubRatio(ratio);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (scrubRatio != null) {
      setScrubRatio(getRatio(e.clientX));
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (scrubRatio != null) {
      const targetPos = Math.round(scrubRatio * total);
      seek(targetPos);
      setScrubRatio(null);
      setScrubbing(false);
    }
  };

  const displayPos = scrubRatio != null ? Math.round(scrubRatio * total) : pos;
  const progressPercent = total > 0 ? Math.min(100, Math.max(0, (displayPos / total) * 100)) : 0;

  return (
    <div
      data-tauri-drag-region
      onDoubleClick={handleRestore}
      title="双击还原主窗口 · 拖拽移动"
      className="mini-player w-full h-full relative select-none flex flex-col justify-between overflow-hidden cursor-move"
    >
      {/* 拟物主题沉头螺栓 */}
      {uiTheme === "skeuo" && (
        <>
          <div className="skeuo-screw absolute top-1.5 left-1.5 z-10 pointer-events-none" />
          <div className="skeuo-screw absolute bottom-2 left-1.5 z-10 pointer-events-none" />
        </>
      )}

      {/* 上半部：封面 + 歌曲信息 + 播控/窗口按钮 */}
      <div className="flex items-center gap-3 px-3 pt-2 pb-0.5 pointer-events-none">
        {/* 左侧封面 */}
        <div
          className="mini-cover relative w-[48px] h-[48px] shrink-0 rounded-[8px] overflow-hidden group cursor-pointer shadow-md pointer-events-auto"
          onClick={(e) => {
            e.stopPropagation();
            togglePlay();
          }}
          onDoubleClick={handleRestore}
          title={playing ? "点击暂停 (双击还原主窗口)" : "点击播放 (双击还原主窗口)"}
        >
          <CoverImg
            src={current?.cover}
            seed={current?.title ?? ""}
            className="w-full h-full object-cover"
            small
            iconSize={20}
          />
          <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
            {playing ? (
              <Pause size={17} className="text-white fill-current" />
            ) : (
              <Play size={17} className="text-white fill-current ml-0.5" />
            )}
          </div>
        </div>

        {/* 中间曲目信息 */}
        <div
          className="flex-1 min-w-0 pr-1 flex flex-col justify-center pointer-events-auto cursor-default"
          onDoubleClick={handleRestore}
        >
          <div
            className="text-[12.5px] font-semibold text-[var(--ink)] truncate leading-tight select-text"
            title={current?.title ?? "RustMusic"}
          >
            {current?.title ?? "暂无播放曲目"}
          </div>

          <div className="flex items-center gap-1.5 mt-1 min-w-0">
            <span
              className="text-[11px] text-[var(--ink-2)] truncate max-w-[115px] select-text"
              title={current?.artist ?? ""}
            >
              {current?.artist || "双击还原完整窗口"}
            </span>
            {current?.quality && (
              <span className="mini-quality shrink-0 px-1 py-0.2 text-[8.5px] font-bold text-[var(--accent-strong)] rounded">
                {current.quality}
              </span>
            )}
          </div>
        </div>

        {/* 右侧：顶层窗口动作 + 底层播控 */}
        <div className="shrink-0 flex flex-col justify-between items-end h-[50px] pointer-events-auto">
          {/* 顶排窗口控制按钮 */}
          <div className="flex items-center gap-0.5 -mt-0.5">
            <button
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--shade)] transition-colors"
              onClick={handleTogglePin}
              title={pinned ? "取消置顶" : "固定在最前"}
            >
              {pinned ? (
                <Pin size={11} className="text-[var(--accent)] fill-current" />
              ) : (
                <PinOff size={11} className="text-[var(--ink-3)]" />
              )}
            </button>
            <button
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--shade)] text-[var(--ink-2)] hover:text-[var(--accent)] transition-colors"
              onClick={handleRestore}
              title="还原完整主窗口"
            >
              <Maximize2 size={11} />
            </button>
            <button
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--shade)] text-[var(--ink-3)] hover:text-[var(--ink)] transition-colors"
              onClick={handleMinimize}
              title="最小化"
            >
              <Minus size={11} />
            </button>
            <button
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-red-500 hover:text-white text-[var(--ink-3)] transition-colors"
              onClick={handleClose}
              title="关闭"
            >
              <X size={12} />
            </button>
          </div>

          {/* 底排播控按钮 */}
          <div className="flex items-center gap-1">
            <button
              className="w-6 h-6 flex items-center justify-center rounded hover:bg-[var(--shade)] transition-colors"
              onClick={handleToggleLike}
              title={isLiked ? "取消喜欢" : "喜欢"}
            >
              <Heart
                size={13}
                className={
                  isLiked
                    ? "fill-[#e0533f] text-[#e0533f]"
                    : "text-[var(--ink-3)] hover:text-[var(--ink)]"
                }
              />
            </button>
            <button
              className="w-6 h-6 flex items-center justify-center rounded hover:bg-[var(--shade)] text-[var(--ink-2)] hover:text-[var(--ink)] transition-colors"
              onClick={(e) => {
                e.stopPropagation();
                prev();
              }}
              title="上一首"
            >
              <SkipBack size={13} className="fill-current" />
            </button>
            <button
              className="mini-play w-[26px] h-[26px] rounded-full flex items-center justify-center shrink-0 cursor-pointer shadow-sm"
              onClick={(e) => {
                e.stopPropagation();
                togglePlay();
              }}
              title="播放/暂停 (空格)"
            >
              {playing ? (
                <Pause size={12} className="fill-current" />
              ) : (
                <Play size={12} className="fill-current ml-0.5" />
              )}
            </button>
            <button
              className="w-6 h-6 flex items-center justify-center rounded hover:bg-[var(--shade)] text-[var(--ink-2)] hover:text-[var(--ink)] transition-colors"
              onClick={(e) => {
                e.stopPropagation();
                next();
              }}
              title="下一首"
            >
              <SkipForward size={13} className="fill-current" />
            </button>
          </div>
        </div>
      </div>

      {/* 中部：实时滚动歌词栏 */}
      <MiniLyricBar onRestore={handleRestore} />

      {/* 底部交互式进度条 */}
      <div
        ref={barRef}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={() => {
          setScrubRatio(null);
          setScrubbing(false);
        }}
        className="group relative h-[3px] hover:h-[5px] w-full bg-black/10 dark:bg-white/10 cursor-pointer transition-all shrink-0"
        title={`进度：${fmtTime(displayPos)} / ${fmtTime(total)}`}
      >
        <div
          className="h-full bg-[var(--accent)] transition-[width] duration-100 ease-linear pointer-events-none"
          style={{ width: `${progressPercent}%` }}
        />
        {/* 滑动小白点 */}
        <div
          className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-2 h-2 rounded-full bg-white shadow opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"
          style={{ left: `${progressPercent}%` }}
        />
      </div>
    </div>
  );
}
