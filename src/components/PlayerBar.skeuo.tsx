import {
  PictureInPicture2,
  ChevronUp,
  Heart,
  ListMusic,
  Pause,
  Play,
  FastForward,
  Repeat,
  Repeat1,
  Rewind,
  Shuffle,
  SkipBack,
  SkipForward,
  Timer,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import Slider from "./Slider";
import { fmtTime } from "../utils";

const SPEEDS = [0.75, 1, 1.25, 1.5, 2];

/** 左侧文案：未打开播放页时显示当前歌词行（随 pos 换行，溢出走 marquee），否则曲名。
 *  pos 是 250ms 一帧的高频更新——隔离在这个小组件里，播放条主体不再每帧重渲染 */
function TrackTitleLyric() {
  const pos = useStore((s) => s.pos);
  const lyrics = useStore((s) => s.lyrics);
  const current = useStore((s) => s.current);
  const nowPlayingOpen = useStore((s) => s.nowPlayingOpen);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);

  // 播放栏歌词：当前行（未打开播放页时也加载并展示）。
  // 是否滚动不再靠字宽估算，而是渲染后实测：容器 scrollWidth > clientWidth 即滚。
  // 时间行在歌词加载时预计算一次（此前每 250ms 一帧 filter+sort 重建数组），
  // 当前行用二分定位
  const timedLines = useMemo(
    () =>
      lyrics?.synced
        ? lyrics.lines
            .filter((l) => l.timeMs != null && l.text.trim() !== "")
            .sort((a, b) => (a.timeMs ?? 0) - (b.timeMs ?? 0))
        : [],
    [lyrics]
  );
  const activeLyric = useMemo(() => {
    if (nowPlayingOpen || !timedLines.length) return null;
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
    return ans >= 0 ? { text: timedLines[ans].text } : null;
  }, [nowPlayingOpen, timedLines, pos]);

  // 实测溢出：文字真实宽度超出可视宽度时启用 marquee（下一帧测，本帧可能未布局）
  const lyricWrapRef = useRef<HTMLDivElement>(null);
  const [lyricOverflow, setLyricOverflow] = useState(false);
  const overflowText = activeLyric?.text;
  useEffect(() => {
    const el = lyricWrapRef.current;
    if (!el || overflowText == null) {
      setLyricOverflow(false);
      return;
    }
    const measure = () => {
      const e2 = lyricWrapRef.current;
      if (e2) setLyricOverflow(e2.scrollWidth > e2.clientWidth + 1);
    };
    requestAnimationFrame(measure);
    // 窗口拉宽/缩窄后溢出状态要重算（否则该滚的停滚、该停的仍滚）
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [overflowText]);

  if (!current) return null;
  return activeLyric != null ? (
    <div
      ref={lyricWrapRef}
      className={`text-[13px] font-medium text-[var(--accent-strong)] cursor-pointer overflow-hidden whitespace-nowrap ${
        lyricOverflow ? "marquee-wrap" : ""
      }`}
      onClick={() => setNowPlayingOpen(!nowPlayingOpen)}
      title="点击展开播放页"
    >
      {lyricOverflow ? (
        <span
          className="marquee-inner"
          style={{
            ["--dur" as string]: `${Math.max(
              9,
              activeLyric.text.length * 0.55
            )}s`,
          }}
        >
          {activeLyric.text}
          <span className="inline-block w-14" />
          {activeLyric.text}
          <span className="inline-block w-14" />
        </span>
      ) : (
        <span className="inline-block">{activeLyric.text}</span>
      )}
    </div>
  ) : (
    <div
      className="text-[13.5px] font-semibold text-[var(--ink)] truncate cursor-pointer hover:text-[var(--accent-strong)] transition-colors"
      onClick={() => setNowPlayingOpen(!nowPlayingOpen)}
    >
      {current.title}
    </div>
  );
}

/** 中部进度区：时间 + 滑块 + 悬停时间气泡。pos/dur 的高频订阅隔离于此。
 *  拖动期数值完全留在本地（Slider 视觉本就本地预览）：此前每个 pointermove
 *  写全局 store，Playhead/TrackTitleLyric/队列面板全部逐帧重渲染 */
function Playhead({ fallbackTotal }: { fallbackTotal: number }) {
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const seek = useStore((s) => s.seek);
  const setScrubbing = useStore((s) => s.setScrubbing);
  const total = dur || fallbackTotal || 0;
  const [dragVal, setDragVal] = useState<number | null>(null);
  const shown = dragVal ?? pos;
  // 悬停时间气泡：非拖动时跟随鼠标位置预览对应时间
  const [hoverRatio, setHoverRatio] = useState<number | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const ratioOf = (clientX: number) => {
    const el = barRef.current;
    if (!el) return 0;
    const rect = el.getBoundingClientRect();
    return rect.width > 0 ? Math.min(1, Math.max(0, (clientX - rect.left) / rect.width)) : 0;
  };
  return (
    <div className="w-full max-w-[540px] flex items-center gap-3">
      <span className="text-[11px] text-[var(--ink-3)] tabular-nums w-9 text-right">
        {fmtTime(shown)}
      </span>
      <div
        ref={barRef}
        className="relative flex-1"
        onPointerMove={(e) => {
          if (dragVal != null) return;
          setHoverRatio(ratioOf(e.clientX));
        }}
        onPointerLeave={() => setHoverRatio(null)}
      >
        <Slider
          value={pos}
          max={total || 1}
          onChange={(v) => {
            if (dragVal == null) {
              setScrubbing(true);
              setHoverRatio(null);
            }
            setDragVal(v);
          }}
          onCommit={(v) => {
            setDragVal(null);
            seek(v);
          }}
          // 拖动被系统中断：不 seek，松开 scrubbing 让引擎 pos 事件恢复回写
          onCancel={() => {
            setDragVal(null);
            setScrubbing(false);
          }}
          className="w-full"
        />
        {hoverRatio != null && total > 0 && (
          <span
            className="pointer-events-none absolute -top-7 -translate-x-1/2 text-[10.5px] px-1.5 py-0.5 rounded-md text-[var(--ink)] glass-strong tabular-nums"
            style={{ left: `${hoverRatio * 100}%` }}
          >
            {fmtTime(hoverRatio * total)}
          </span>
        )}
      </div>
      <span className="text-[11px] text-[var(--ink-3)] tabular-nums w-9">
        {fmtTime(total)}
      </span>
    </div>
  );
}

/** 竖向音量弹出条（点击音量图标显示，点击其他区域收起；顶部可静音） */
function VolumePopover({
  volume,
  onSet,
  muted,
  onToggleMute,
  onClose,
}: {
  volume: number;
  onSet: (v: number) => void;
  muted: boolean;
  onToggleMute: () => void;
  onClose: () => void;
}) {
  const H = 140;
  const y = (1 - volume) * H;
  const onPointer = (e: React.PointerEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = 1 - (e.clientY - rect.top) / rect.height;
    onSet(Math.min(1, Math.max(0, ratio)));
  };
  return (
    <>
      {/* 点击其他区域收起 */}
      <div className="fixed inset-0 z-[60]" onClick={onClose} />
      <div
        className="absolute bottom-[48px] right-0 z-[61] w-12 rounded-2xl p-2.5 flex flex-col items-center gap-2 glass-strong"
        style={{ boxShadow: "var(--bar-shadow)" }}
      >
        {/* 拟物静音按键 */}
        <button
          className="btn-ghost !w-8 !h-6 !text-[10px] !font-bold"
          onClick={() => {
            onToggleMute();
          }}
          title={muted ? "取消静音" : "静音"}
        >
          {muted ? "有声" : "静音"}
        </button>

        {/* 拟物调音台竖向凹槽 */}
        <div
          className="relative w-3 rounded-full cursor-pointer touch-none"
          style={{
            height: H,
            background: "var(--sk-groove-bg)",
            border: "1px solid var(--sk-groove-border)",
            boxShadow: "var(--sk-groove-shadow)",
          }}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            e.currentTarget.setPointerCapture(e.pointerId);
            onPointer(e);
          }}
          onPointerMove={(e) => {
            if (e.buttons !== 1) return;
            onPointer(e);
          }}
          onPointerUp={(e) => e.currentTarget.releasePointerCapture(e.pointerId)}
          onPointerCancel={(e) => e.currentTarget.releasePointerCapture(e.pointerId)}
        >
          {/* 发光填充柱 */}
          <div
            className="absolute left-0.5 right-0.5 bottom-0.5 rounded-full"
            style={{
              height: Math.max(2, H - y),
              background: "linear-gradient(180deg, var(--accent-strong), var(--accent))",
              boxShadow: "0 0 6px var(--accent-soft)",
            }}
          />
          {/* 拟物横向调音台推钮 */}
          <div
            className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 w-4 h-3 rounded-[3px] flex items-center justify-center transition-transform"
            style={{
              top: y,
              background: "var(--sk-knob-metal)",
              border: "1px solid var(--sk-edge)",
              boxShadow: "var(--sk-knob-shadow)",
            }}
          >
            <span className="w-2.5 h-[1px] bg-red-600 shadow-[0_0_2px_red]" />
          </div>
        </div>
      </div>
    </>
  );
}

/** 定时停止弹出菜单（点击时钟图标显示，点击其他区域收起）。
 *  含“播完当前歌曲/列表后停止”两个档位——音乐播放器最高频的定时诉求 */
function SleepPopover({
  sleepAt,
  onSet,
  onMode,
  stopAfterTrack,
  stopAfterQueue,
  onClose,
}: {
  sleepAt: number | null;
  onSet: (min: number | null) => void;
  onMode: (mode: "track" | "queue" | null) => void;
  stopAfterTrack: boolean;
  stopAfterQueue: boolean;
  onClose: () => void;
}) {
  const remainMin = sleepAt
    ? Math.max(1, Math.ceil((sleepAt - Date.now()) / 60000))
    : null;
  const [custom, setCustom] = useState("");
  const applyCustom = () => {
    const n = Math.round(Number(custom));
    if (!isFinite(n) || n <= 0) return;
    // 上限 24 小时，防止手滑输入超长值
    onSet(Math.min(1440, n));
    onClose();
  };
  return (
    <>
      {/* 点击其他区域收起 */}
      <div className="fixed inset-0 z-[60]" onClick={onClose} />
      <div
        className="absolute bottom-[48px] right-0 z-[61] w-[168px] rounded-2xl p-1.5 flex flex-col glass-strong"
        style={{ boxShadow: "var(--bar-shadow)" }}
      >
        {remainMin != null && (
          <div className="px-2.5 py-1 text-[11px] text-[var(--accent-strong)] font-semibold">
            剩余约 {remainMin} 分钟
          </div>
        )}
        {[15, 30, 45, 60, 90].map((m) => (
          <button
            key={m}
            className="w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              onSet(m);
              onClose();
            }}
          >
            {m} 分钟后停止
          </button>
        ))}
        <div className="my-1 h-px bg-[var(--line)]" />
        <button
          className={`w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] hover:bg-[var(--shade-strong)] text-left ${
            stopAfterTrack ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
          }`}
          onClick={() => {
            onMode("track");
            onClose();
          }}
        >
          播完当前歌曲后停止
        </button>
        <button
          className={`w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] hover:bg-[var(--shade-strong)] text-left ${
            stopAfterQueue ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
          }`}
          onClick={() => {
            onMode("queue");
            onClose();
          }}
        >
          播完当前列表后停止
        </button>
        {/* 自定义时长（分钟）：回车或点确定生效 */}
        <div className="flex items-center gap-1 px-1 pt-1.5">
          <input
            type="number"
            min={1}
            max={1440}
            value={custom}
            onChange={(e) => setCustom(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && applyCustom()}
            placeholder="自定义(分钟)"
            className="w-full min-w-0 h-7 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-2 text-[12px] text-[var(--ink)] placeholder:text-[var(--ink-3)] outline-none focus:border-[var(--accent)]"
          />
          <button
            className="btn-secondary !py-1 !px-2 text-[11px] shrink-0 disabled:opacity-50"
            onClick={applyCustom}
            disabled={!custom.trim() || !isFinite(Number(custom)) || Number(custom) <= 0}
            title="按输入的分钟数定时"
          >
            确定
          </button>
        </div>
        {(sleepAt != null || stopAfterTrack || stopAfterQueue) && (
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] text-[#e0533f] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              onSet(null);
              onMode(null);
              onClose();
            }}
          >
            取消定时
          </button>
        )}
      </div>
    </>
  );
}

export default function PlayerBar({ centered = false }: { centered?: boolean }) {
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  // pos/dur/lyrics 的高频订阅已隔离到 TrackTitleLyric / Playhead：
  // 主体只订阅低频状态，250ms 一帧的进度更新不再重渲染整个播放条
  const volume = useStore((s) => s.volume);
  const sleepAt = useStore((s) => s.sleepAt);
  const setSleepTimer = useStore((s) => s.setSleepTimer);
  const sleepStopAfterTrack = useStore((s) => s.sleepStopAfterTrack);
  const sleepStopAfterQueue = useStore((s) => s.sleepStopAfterQueue);
  const setSleepMode = useStore((s) => s.setSleepMode);
  const speed = useStore((s) => s.speed);
  const repeat = useStore((s) => s.repeat);
  const shuffle = useStore((s) => s.shuffle);
  const download = useStore((s) => s.download);
  const queue = useStore((s) => s.queue);
  const queueOpen = useStore((s) => s.queueOpen);
  const nowPlayingOpen = useStore((s) => s.nowPlayingOpen);
  const desktopLyricsOn = useStore((s) => s.desktopLyricsOn);
  const desktopLyricsLock = useStore((s) => s.desktopLyricsLock);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const neteaseToggleLike = useStore((s) => s.neteaseToggleLike);
  const savedOnline = useStore((s) => s.savedOnline);
  const likedOnline = useStore((s) => s.likedOnline);
  const recentOnline = useStore((s) => s.recentOnline);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const togglePlay = useStore((s) => s.togglePlay);
  const next = useStore((s) => s.next);
  const prev = useStore((s) => s.prev);
  const seekForward = useStore((s) => s.seekForward);
  const seekBackward = useStore((s) => s.seekBackward);
  const setVolume = useStore((s) => s.setVolume);
  const setSpeed = useStore((s) => s.setSpeed);
  const setRepeat = useStore((s) => s.setRepeat);
  const toggleShuffle = useStore((s) => s.toggleShuffle);
  const toggleLike = useStore((s) => s.toggleLike);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const setQueueOpen = useStore((s) => s.setQueueOpen);
  const miniPlayer = useStore((s) => s.miniPlayer);
  const setMiniPlayer = useStore((s) => s.setMiniPlayer);

  const VolIcon = volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;

  const [volOpen, setVolOpen] = useState(false);
  const volBtnRef = useRef<HTMLButtonElement>(null);
  const lastVolRef = useRef(0.8);
  const [sleepOpen, setSleepOpen] = useState(false);

  return (
    // 与右侧内容区（玻璃卡片）对齐：左偏移侧栏宽度、右边距与卡片外边距一致
    <div
      className={`anim-bar absolute bottom-0 right-0 z-50 px-6 pb-4 pt-1 ${
        centered ? "left-0" : "left-[236px]"
      }`}
    >
      {download && (
        <button
          className="absolute right-8 top-1 z-10 btn-ghost !w-6 !h-6 text-[10px] text-[var(--ink-3)] hover:text-[var(--ink)]"
          onClick={() => useStore.getState().cancelDownloadQueue()}
          title="取消排队中的批量下载（正在执行的当前这一个不受影响）"
        >
          取消排队
        </button>
      )}
      {download && (
        <div className="absolute left-8 right-24 top-0 h-[3px] bg-[var(--shade)] rounded-full overflow-hidden">
          <div
            className="h-full transition-all duration-300 rounded-full"
            style={{
              width: `${download.pct}%`,
              background: "var(--accent)",
            }}
          />
        </div>
      )}

      <div
        className={`h-[74px] rounded-[18px] flex items-center pl-4 pr-5 gap-4 transition-opacity duration-300 ${
          nowPlayingOpen ? "opacity-80 hover:opacity-100" : ""
        }`}
        style={{
          background: "var(--bar-glass)",
          border: "1px solid var(--bar-line)",
          boxShadow: "var(--bar-shadow)",
        }}
      >
        {/* 拟物荧光/液晶屏幕舱（曲目信息） */}
        <div className="skeuo-display rounded-[12px] p-1.5 pr-3 flex items-center gap-3 w-[270px] min-w-[200px]">
          {current ? (
            <>
              <button
                className="group relative shrink-0 rounded-[8px] overflow-hidden border border-[var(--sk-cover-border)] shadow-[var(--sk-cover-shadow)]"
                onClick={() => setNowPlayingOpen(!nowPlayingOpen)}
                title={nowPlayingOpen ? "收起播放页" : "展开播放页"}
              >
                <CoverImg
                  src={current.cover}
                  seed={current.title}
                  small
                  className="w-[42px] h-[42px] rounded-[7px]"
                  iconSize={20}
                />
                <div className="absolute inset-0 bg-black/45 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                  <ChevronUp
                    size={16}
                    className={`text-white transition-transform ${nowPlayingOpen ? "rotate-180" : ""}`}
                  />
                </div>
              </button>
              <div className="min-w-0 flex-1">
                <TrackTitleLyric />
                <div className="text-[11.5px] text-[var(--ink-2)] truncate mt-0.5 flex items-center gap-1.5">
                  <span className="truncate">{current.artist}</span>
                  {current.quality && (
                    <span
                      className="shrink-0 text-[9px] font-bold px-1.5 py-px rounded bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] text-[var(--accent-strong)] shadow-[var(--sk-groove-shadow)]"
                      title="当前播放音质"
                    >
                      {current.quality}
                    </span>
                  )}
                </div>
              </div>
              {current.kind === "track" && current.id != null && (
                <button
                  className="btn-ghost !w-7 !h-7 shrink-0"
                  onClick={() => toggleLike(current.id!)}
                  title={current.liked ? "取消喜欢" : "喜欢"}
                >
                  <Heart
                    size={14}
                    className={
                      current.liked ? "fill-[#e0533f] text-[#e0533f]" : ""
                    }
                  />
                </button>
              )}
              {current.kind === "netease" && current.nid != null && (
                <button
                  className="btn-ghost !w-7 !h-7 shrink-0"
                  onClick={() => neteaseToggleLike(current.nid!)}
                  title={neteaseLiked[current.nid] ? "取消收藏" : "收藏到“我喜欢”"}
                >
                  <Heart
                    size={14}
                    className={
                      neteaseLiked[current.nid]
                        ? "fill-[#e0533f] text-[#e0533f]"
                        : ""
                    }
                  />
                </button>
              )}
              {current.kind === "qq" && current.qid != null && (
                <button
                  className="btn-ghost !w-7 !h-7 shrink-0"
                  onClick={() => {
                    const entry = [ ...likedOnline, ...recentOnline ].find(
                      (e) => e.kind === "qq" && e.onlineId === current.qid
                    );
                    toggleLikeOnline({
                      kind: "qq",
                      id: current.qid!,
                      name: current.title,
                      artist: current.artist,
                      album: current.album,
                      cover: current.cover,
                      durationMs: current.durationMs,
                      mediaMid: entry?.mediaMid ?? "",
                      vip: entry?.vip ?? false,
                    });
                  }}
                  title={savedOnline[`qq-${current.qid}`] ? "取消喜欢" : "收藏到“我喜欢”"}
                >
                  <Heart
                    size={14}
                    className={
                      savedOnline[`qq-${current.qid}`]
                        ? "fill-[#e0533f] text-[#e0533f]"
                        : ""
                    }
                  />
                </button>
              )}
              {current.kind === "bilibili" && current.qid != null && (
                <button
                  className="btn-ghost !w-7 !h-7 shrink-0"
                  onClick={() => {
                    toggleLikeOnline({
                      kind: "bilibili",
                      id: current.qid!,
                      name: current.title,
                      artist: current.artist,
                      album: current.album,
                      cover: current.cover,
                      durationMs: current.durationMs,
                    });
                  }}
                  title={savedOnline[`bilibili-${current.qid}`] ? "取消喜欢" : "收藏到“我喜欢”"}
                >
                  <Heart
                    size={14}
                    className={
                      savedOnline[`bilibili-${current.qid}`]
                        ? "fill-[#e0533f] text-[#e0533f]"
                        : ""
                    }
                  />
                </button>
              )}
              {current.kind === "navidrome" && current.qid != null && (
                <button
                  className="btn-ghost !w-7 !h-7 shrink-0"
                  onClick={() => {
                    toggleLikeOnline({
                      kind: "navidrome",
                      id: current.qid!,
                      name: current.title,
                      artist: current.artist,
                      album: current.album,
                      cover: current.cover,
                      durationMs: current.durationMs,
                    });
                  }}
                  title={savedOnline[`navidrome-${current.qid}`] ? "取消喜欢" : "收藏到“我喜欢”"}
                >
                  <Heart
                    size={14}
                    className={
                      savedOnline[`navidrome-${current.qid}`]
                        ? "fill-[#e0533f] text-[#e0533f]"
                        : ""
                    }
                  />
                </button>
              )}
              {current.kind === "kugou" && current.kgid != null && (
                <button
                  className="btn-ghost !w-7 !h-7 shrink-0"
                  onClick={() => {
                    const entry = [...likedOnline, ...recentOnline].find(
                      (e) => e.kind === "kugou" && e.onlineId === current.kgid
                    );
                    toggleLikeOnline({
                      kind: "kugou",
                      id: current.kgid!,
                      name: current.title,
                      artist: current.artist,
                      album: current.album,
                      cover: current.cover,
                      durationMs: current.durationMs,
                      mediaMid: "",
                      vip: entry?.vip ?? false,
                    });
                  }}
                  title={savedOnline[`kugou-${current.kgid}`] ? "取消喜欢" : "收藏到“我喜欢”"}
                >
                  <Heart
                    size={14}
                    className={
                      savedOnline[`kugou-${current.kgid}`]
                        ? "fill-[#e0533f] text-[#e0533f]"
                        : ""
                    }
                  />
                </button>
              )}
            </>
          ) : (
            <>
              <div
                className="w-[42px] h-[42px] rounded-[7px] flex items-center justify-center bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] shadow-[var(--sk-groove-shadow)]"
              >
                <Play size={16} className="text-[var(--ink-3)] ml-0.5" />
              </div>
              <div className="text-[12.5px] text-[var(--ink-3)] font-mono">STANDBY</div>
            </>
          )}
        </div>

        {/* 中部控制（物理琴键与车削主播放钮） */}
        <div className="flex-1 flex flex-col items-center justify-center gap-1.5 min-w-0">
          <div className="flex items-center gap-2.5">
            <button
              className="btn-ghost relative w-[32px] h-[32px]"
              onClick={toggleShuffle}
              title="随机播放"
            >
              <Shuffle size={14} className={shuffle ? "text-[var(--accent-strong)]" : ""} />
              {shuffle && <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />}
            </button>
            <button className="btn-ghost w-[32px] h-[32px]" onClick={prev} title="上一首">
              <SkipBack size={15} className="fill-current" />
            </button>
            <button
              className="btn-ghost w-[32px] h-[32px] disabled:opacity-40"
              disabled={!current}
              onClick={() => seekBackward(5000)}
              title="快退 5 秒（←）"
            >
              <Rewind size={15} className="fill-current" />
            </button>

            {/* 拟物车削主播放/暂停旋钮 */}
            <div
              className="p-1 rounded-full border border-[var(--sk-groove-border)] shadow-[var(--sk-groove-shadow)]"
              style={{ background: "var(--sk-groove-bg)" }}
            >
              <button
                className={`skeuo-play-btn ${playing ? "playing" : ""}`}
                onClick={togglePlay}
                title="播放 / 暂停（空格）"
              >
                {playing ? (
                  <Pause size={17} className="fill-current" />
                ) : (
                  <Play size={17} className="fill-current ml-0.5" />
                )}
              </button>
            </div>

            <button
              className="btn-ghost w-[32px] h-[32px] disabled:opacity-40"
              disabled={!current}
              onClick={() => seekForward(5000)}
              title="快进 5 秒（→）"
            >
              <FastForward size={15} className="fill-current" />
            </button>
            <button className="btn-ghost w-[32px] h-[32px]" onClick={() => next(false)} title="下一首">
              <SkipForward size={15} className="fill-current" />
            </button>
            <button
              className="btn-ghost relative w-[32px] h-[32px]"
              onClick={() =>
                setRepeat(repeat === "off" ? "all" : repeat === "all" ? "one" : "off")
              }
              title={repeat === "off" ? "列表循环" : repeat === "all" ? "单曲循环" : "关闭循环"}
            >
              {repeat === "one" ? (
                <Repeat1 size={14} className="text-[var(--accent-strong)]" />
              ) : (
                <Repeat size={14} className={repeat !== "off" ? "text-[var(--accent-strong)]" : ""} />
              )}
              {repeat !== "off" && (
                <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />
              )}
            </button>
          </div>
          <Playhead fallbackTotal={current?.durationMs ?? 0} />
        </div>

        {/* 右侧控制 */}
        <div className="flex items-center gap-2 w-[270px] min-w-[200px] justify-end">
          {/* 桌面歌词开关 */}
          <button
            className="btn-ghost relative w-[34px] h-[34px] text-[12.5px] font-bold"
            onClick={() => {
              const s = useStore.getState();
              if (!s.desktopLyricsOn) s.openDesktopLyrics();
              else if (s.desktopLyricsLock) s.unlockDesktopLyrics();
              else s.closeDesktopLyrics();
            }}
            title={
              desktopLyricsOn
                ? desktopLyricsLock
                  ? "桌面歌词：已锁定（点击解锁）"
                  : "桌面歌词：点击关闭（快捷键 L）"
                : "打开桌面歌词（快捷键 L）"
            }
          >
            <span className={desktopLyricsOn ? "text-[var(--accent-strong)]" : ""}>词</span>
            {desktopLyricsOn && (
              <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />
            )}
          </button>
          <button
            className="btn-ghost relative w-[34px] h-[34px] text-[11px] font-bold tabular-nums"
            onClick={() => {
              const i = SPEEDS.indexOf(speed);
              if (i >= 0) {
                setSpeed(SPEEDS[(i + 1) % SPEEDS.length]);
              } else {
                setSpeed(SPEEDS.find((s) => s > speed) ?? SPEEDS[0]);
              }
            }}
            title="播放速度"
          >
            <span className={speed !== 1 ? "text-[var(--accent-strong)]" : ""}>{speed}x</span>
            {speed !== 1 && (
              <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />
            )}
          </button>
          {/* 音量：点击图标弹出音量条 */}
          <div className="relative flex items-center">
            <button
              ref={volBtnRef}
              className={`btn-ghost w-[34px] h-[34px] shrink-0 ${volOpen ? "!text-[var(--accent-strong)] shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]" : ""}`}
              onClick={() => setVolOpen((v) => !v)}
              onWheel={(e) => {
                setVolume(Math.min(1, Math.max(0, volume + (e.deltaY < 0 ? 0.05 : -0.05))));
              }}
              title="音量（滚轮调节）"
            >
              <VolIcon size={15} />
            </button>
            {volOpen && (
              <VolumePopover
                volume={volume}
                onSet={setVolume}
                muted={volume === 0}
                onToggleMute={() => {
                  if (volume > 0) {
                    lastVolRef.current = volume;
                    setVolume(0);
                  } else {
                    setVolume(lastVolRef.current || 0.8);
                  }
                }}
                onClose={() => setVolOpen(false)}
              />
            )}
          </div>
          {/* 定时停止 */}
          <div className="relative flex items-center">
            <button
              className={`btn-ghost relative w-[34px] h-[34px] shrink-0 ${
                sleepAt != null || sleepStopAfterTrack || sleepStopAfterQueue
                  ? "!text-[var(--accent-strong)] shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]"
                  : ""
              }`}
              onClick={() => setSleepOpen((v) => !v)}
              title={
                sleepAt != null
                  ? `定时停止：剩余约 ${Math.max(1, Math.ceil((sleepAt - Date.now()) / 60000))} 分钟`
                  : sleepStopAfterTrack
                    ? "定时停止：播完当前歌曲后停止"
                    : sleepStopAfterQueue
                      ? "定时停止：播完当前列表后停止"
                      : "定时停止播放"
              }
            >
              <Timer size={15} />
              {(sleepAt != null || sleepStopAfterTrack || sleepStopAfterQueue) && (
                <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />
              )}
            </button>
            {sleepOpen && (
              <SleepPopover
                sleepAt={sleepAt}
                onSet={setSleepTimer}
                onMode={setSleepMode}
                stopAfterTrack={sleepStopAfterTrack}
                stopAfterQueue={sleepStopAfterQueue}
                onClose={() => setSleepOpen(false)}
              />
            )}
          </div>
          {/* 播放队列 */}
          <button
            className={`btn-ghost relative w-[34px] h-[34px] ${miniPlayer ? "!text-[var(--accent)]" : ""}`}
            onClick={() => setMiniPlayer(!miniPlayer)}
            title="极简小窗模式"
          >
            <PictureInPicture2 size={15} />
          </button>
              <button
            className={`btn-ghost relative w-[34px] h-[34px] ${queueOpen ? "!text-[var(--accent-strong)] shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]" : ""}`}
            onClick={() => setQueueOpen(!queueOpen)}
            title="播放队列"
          >
            <ListMusic size={16} />
            {queue.length > 1 && (
              <span
                className="absolute -top-1 -right-1 text-[9.5px] text-[var(--accent-on)] rounded-full min-w-[15px] leading-[15px] font-bold text-center px-0.5 border border-black/40 shadow"
                style={{ background: "var(--accent)" }}
              >
                {queue.length}
              </span>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
