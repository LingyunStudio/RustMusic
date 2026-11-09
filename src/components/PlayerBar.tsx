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

/** 中部进度区：时间 + 滑块 + 悬停时间气泡。pos/dur 的高频订阅隔离于此。 */
function Playhead({ fallbackTotal }: { fallbackTotal: number }) {
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const seek = useStore((s) => s.seek);
  const setScrubbing = useStore((s) => s.setScrubbing);
  const total = dur || fallbackTotal || 0;
  const [dragVal, setDragVal] = useState<number | null>(null);
  const shown = dragVal ?? pos;
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

/** 竖向音量弹出条 */
function VolumePopover({
  volume,
  onSet,
  muted,
  onToggleMute,
  onClose,
  uiTheme,
}: {
  volume: number;
  onSet: (v: number) => void;
  muted: boolean;
  onToggleMute: () => void;
  onClose: () => void;
  uiTheme: string;
}) {
  const H = 140;
  const y = (1 - volume) * H;
  const isSkeuo = uiTheme === "skeuo";
  const isLiquid = uiTheme === "liquid";

  const onPointer = (e: React.PointerEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = 1 - (e.clientY - rect.top) / rect.height;
    onSet(Math.min(1, Math.max(0, ratio)));
  };

  return (
    <>
      <div className="fixed inset-0 z-[60]" onClick={onClose} />
      <div
        className={`absolute ${
          isLiquid ? "bottom-[52px]" : "bottom-[48px]"
        } right-0 z-[61] ${
          isLiquid || isSkeuo ? "w-12 p-2.5 gap-2" : "w-11 p-2 gap-1.5"
        } rounded-2xl flex flex-col items-center glass-strong`}
        style={isLiquid ? undefined : { boxShadow: "var(--bar-shadow)" }}
      >
        {isSkeuo ? (
          <button
            className="btn-ghost !w-8 !h-6 !text-[10px] !font-bold"
            onClick={onToggleMute}
            title={muted ? "取消静音" : "静音"}
          >
            {muted ? "有声" : "静音"}
          </button>
        ) : isLiquid ? (
          <button
            className="liquid-pill !py-0.5 !px-2 text-[10px] font-semibold text-[var(--ink)]"
            onClick={onToggleMute}
            title={muted ? "取消静音" : "静音"}
          >
            {muted ? "有声" : "静音"}
          </button>
        ) : (
          <button
            className="w-7 h-6 rounded-lg flex items-center justify-center text-[10px] font-semibold text-[var(--ink-2)] hover:bg-[var(--shade-strong)] hover:text-[var(--ink)] transition-colors"
            onClick={onToggleMute}
            title={muted ? "取消静音" : "静音"}
          >
            {muted ? "有声" : "静音"}
          </button>
        )}

        <div
          className={`relative ${
            isSkeuo
              ? "w-3"
              : isLiquid
              ? "w-2 border border-[var(--line)] shadow-inner"
              : "w-1.5"
          } rounded-full cursor-pointer touch-none ${
            !isSkeuo ? "bg-[var(--shade-strong)]" : ""
          }`}
          style={
            isSkeuo
              ? {
                  height: H,
                  background: "var(--sk-groove-bg)",
                  border: "1px solid var(--sk-groove-border)",
                  boxShadow: "var(--sk-groove-shadow)",
                }
              : { height: H }
          }
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
          <div
            className={`absolute ${
              isSkeuo ? "left-0.5 right-0.5 bottom-0.5" : "left-0 right-0 bottom-0"
            } rounded-full`}
            style={{
              height: Math.max(isLiquid ? 3 : 2, H - y),
              background: "linear-gradient(180deg, var(--accent-strong), var(--accent))",
              boxShadow: isLiquid
                ? "0 0 8px var(--accent-soft)"
                : isSkeuo
                ? "0 0 6px var(--accent-soft)"
                : undefined,
            }}
          />
          {isSkeuo ? (
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
          ) : (
            <div
              className={`absolute left-1/2 -translate-x-1/2 -translate-y-1/2 ${
                isLiquid
                  ? "w-4 h-4 shadow-[0_2px_8px_rgba(0,0,0,0.4),0_0_10px_var(--accent-soft)] border border-white/80"
                  : "w-3.5 h-3.5 shadow"
              } rounded-full bg-white`}
              style={{ top: y }}
            />
          )}
        </div>
      </div>
    </>
  );
}

/** 定时停止弹出菜单 */
function SleepPopover({
  sleepAt,
  onSet,
  onMode,
  stopAfterTrack,
  stopAfterQueue,
  onClose,
  uiTheme,
}: {
  sleepAt: number | null;
  onSet: (min: number | null) => void;
  onMode: (mode: "track" | "queue" | null) => void;
  stopAfterTrack: boolean;
  stopAfterQueue: boolean;
  onClose: () => void;
  uiTheme: string;
}) {
  const isLiquid = uiTheme === "liquid";
  const remainMin = sleepAt
    ? Math.max(1, Math.ceil((sleepAt - Date.now()) / 60000))
    : null;
  const [custom, setCustom] = useState("");
  const applyCustom = () => {
    const n = Math.round(Number(custom));
    if (!isFinite(n) || n <= 0) return;
    onSet(Math.min(1440, n));
    onClose();
  };
  return (
    <>
      <div className="fixed inset-0 z-[60]" onClick={onClose} />
      <div
        className={`absolute ${
          isLiquid ? "bottom-[52px] w-[172px] p-2" : "bottom-[48px] w-[168px] p-1.5"
        } right-0 z-[61] rounded-2xl flex flex-col glass-strong`}
        style={isLiquid ? undefined : { boxShadow: "var(--bar-shadow)" }}
      >
        {remainMin != null && (
          <div className="px-2.5 py-1 text-[11px] text-[var(--accent-strong)] font-semibold">
            剩余约 {remainMin} 分钟
          </div>
        )}
        {[15, 30, 45, 60, 90].map((m) => (
          <button
            key={m}
            className={`w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left ${
              isLiquid ? "transition-colors" : ""
            }`}
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
            isLiquid ? "transition-colors " : ""
          }${
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
            isLiquid ? "transition-colors " : ""
          }${
            stopAfterQueue ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
          }`}
          onClick={() => {
            onMode("queue");
            onClose();
          }}
        >
          播完当前列表后停止
        </button>
        <div className="flex items-center gap-1 px-1 pt-1.5">
          <input
            type="number"
            min={1}
            max={1440}
            value={custom}
            onChange={(e) => setCustom(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && applyCustom()}
            placeholder="自定义(分钟)"
            className={
              isLiquid
                ? "w-full min-w-0 h-7 rounded-lg liquid-input px-2 text-[12px] text-[var(--ink)] placeholder:text-[var(--ink-3)] outline-none"
                : "w-full min-w-0 h-7 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-2 text-[12px] text-[var(--ink)] placeholder:text-[var(--ink-3)] outline-none focus:border-[var(--accent)]"
            }
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
  const uiTheme = useStore((s) => s.uiTheme);
  const isSkeuo = uiTheme === "skeuo";
  const isLiquid = uiTheme === "liquid";

  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
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
    <div
      className={`anim-bar absolute bottom-0 right-0 z-50 ${
        isLiquid
          ? `pl-3 pr-2 pb-2 pt-1 ${centered ? "left-0" : "left-[250px]"}`
          : `px-6 pb-4 pt-1 ${centered ? "left-0" : "left-[236px]"}`
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
        className={
          isSkeuo
            ? `h-[74px] rounded-[18px] flex items-center pl-4 pr-5 gap-4 transition-opacity duration-300 ${
                nowPlayingOpen ? "opacity-80 hover:opacity-100" : ""
              }`
            : isLiquid
            ? `h-[70px] rounded-2xl flex items-center pl-5 pr-5 gap-5 transition-all duration-300 liquid-dock relative z-10 ${
                nowPlayingOpen ? "opacity-90 hover:opacity-100" : ""
              }`
            : `h-[64px] rounded-[18px] flex items-center pl-5 pr-6 gap-5 transition-opacity duration-300 ${
                nowPlayingOpen ? "opacity-80 hover:opacity-100" : ""
              }`
        }
        style={
          isLiquid
            ? undefined
            : isSkeuo
            ? {
                background: "var(--bar-glass)",
                border: "1px solid var(--bar-line)",
                boxShadow: "var(--bar-shadow)",
              }
            : {
                background: "var(--bar-glass)",
                boxShadow: "var(--bar-shadow)",
              }
        }
      >
        {/* 曲目信息 */}
        {isSkeuo ? (
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
                    className="w-[42px] h-[42px] rounded-[6px]"
                    iconSize={20}
                  />
                  <div className="absolute inset-0 bg-black/45 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                    <ChevronUp
                      size={18}
                      className={`text-[var(--ink)] transition-transform ${nowPlayingOpen ? "rotate-180" : ""}`}
                    />
                  </div>
                </button>
                <div className="min-w-0 flex-1">
                  <TrackTitleLyric />
                  <div className="text-[11.5px] text-[var(--ink-2)] truncate mt-0.5 flex items-center gap-1.5">
                    <span className="truncate">{current.artist}</span>
                    {current.quality && (
                      <span
                        className="shrink-0 text-[9px] font-bold px-1.5 py-px rounded-full bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] text-[var(--accent-strong)]"
                        title="当前播放音质"
                      >
                        {current.quality}
                      </span>
                    )}
                  </div>
                </div>
                {/* 各种音源的喜欢按键 */}
                {current.kind === "track" && current.id != null && (
                  <button
                    className="btn-ghost !w-7 !h-7 shrink-0"
                    onClick={() => toggleLike(current.id!)}
                    title={current.liked ? "取消喜欢" : "喜欢"}
                  >
                    <Heart
                      size={15}
                      className={current.liked ? "fill-[#e0533f] text-[#e0533f]" : "text-[var(--ink-3)]"}
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
                      size={15}
                      className={neteaseLiked[current.nid] ? "fill-[#e0533f] text-[#e0533f]" : "text-[var(--ink-3)]"}
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
                      size={15}
                      className={savedOnline[`qq-${current.qid}`] ? "fill-[#e0533f] text-[#e0533f]" : "text-[var(--ink-3)]"}
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
                      size={15}
                      className={savedOnline[`bilibili-${current.qid}`] ? "fill-[#e0533f] text-[#e0533f]" : "text-[var(--ink-3)]"}
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
                      size={15}
                      className={savedOnline[`navidrome-${current.qid}`] ? "fill-[#e0533f] text-[#e0533f]" : "text-[var(--ink-3)]"}
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
                      size={15}
                      className={savedOnline[`kugou-${current.kgid}`] ? "fill-[#e0533f] text-[#e0533f]" : "text-[var(--ink-3)]"}
                    />
                  </button>
                )}
              </>
            ) : (
              <div className="skeuo-display-screen rounded-[8px] p-2 flex items-center gap-2.5 w-full">
                <div className="w-[32px] h-[32px] rounded-[6px] bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] flex items-center justify-center text-[var(--ink-3)] shadow-[var(--sk-groove-shadow)]">
                  <Play size={14} className="fill-current ml-0.5 opacity-40" />
                </div>
                <div className="text-[12px] font-bold tracking-wider text-[var(--ink-3)] uppercase">NO DISC</div>
              </div>
            )}
          </div>
        ) : (
          <div className="flex items-center gap-4 w-[260px] min-w-[200px]">
            {current ? (
              <>
                <button
                  className="group relative shrink-0"
                  onClick={() => setNowPlayingOpen(!nowPlayingOpen)}
                  title={nowPlayingOpen ? "收起播放页" : "展开播放页"}
                >
                  <CoverImg
                    src={current.cover}
                    seed={current.title}
                    small
                    className={
                      isLiquid
                        ? "w-[46px] h-[46px] rounded-xl shadow-[var(--cover-shadow-sm)] transition-transform group-hover:scale-105"
                        : "w-[44px] h-[44px] rounded-[10px] shadow-[var(--cover-shadow-sm)]"
                    }
                    iconSize={20}
                  />
                  <div
                    className={`absolute inset-0 ${
                      isLiquid ? "rounded-[13px] bg-black/40" : "rounded-xl bg-black/45"
                    } opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center`}
                  >
                    <ChevronUp
                      size={18}
                      className={`text-[var(--ink)] transition-transform ${nowPlayingOpen ? "rotate-180" : ""}`}
                    />
                  </div>
                </button>
                <div className="min-w-0">
                  <TrackTitleLyric />
                  <div className="text-[12px] text-[var(--ink-3)] truncate mt-1 flex items-center gap-2">
                    <span className="truncate">{current.artist}</span>
                    {current.quality && (
                      <span
                        className={
                          isLiquid
                            ? "shrink-0 text-[9.5px] font-semibold px-2 py-0.5 rounded-full text-[var(--accent-strong)] border border-[var(--accent-soft)]"
                            : "shrink-0 text-[9.5px] font-semibold px-1.5 py-px rounded text-[var(--accent-strong)]"
                        }
                        style={{ background: "var(--accent-weak)" }}
                        title="当前播放音质"
                      >
                        {current.quality}
                      </span>
                    )}
                  </div>
                </div>
                {current.kind === "track" && current.id != null && (
                  <button
                    className="btn-ghost w-8 h-8 shrink-0"
                    onClick={() => toggleLike(current.id!)}
                    title={current.liked ? "取消喜欢" : "喜欢"}
                  >
                    <Heart
                      size={16}
                      className={current.liked ? "fill-[#e0533f] text-[#e0533f]" : ""}
                    />
                  </button>
                )}
                {current.kind === "netease" && current.nid != null && (
                  <button
                    className="btn-ghost w-8 h-8 shrink-0"
                    onClick={() => neteaseToggleLike(current.nid!)}
                    title={neteaseLiked[current.nid] ? "取消收藏" : "收藏到“我喜欢”"}
                  >
                    <Heart
                      size={16}
                      className={neteaseLiked[current.nid] ? "fill-[#e0533f] text-[#e0533f]" : ""}
                    />
                  </button>
                )}
                {current.kind === "qq" && current.qid != null && (
                  <button
                    className="btn-ghost w-8 h-8 shrink-0"
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
                      size={16}
                      className={savedOnline[`qq-${current.qid}`] ? "fill-[#e0533f] text-[#e0533f]" : ""}
                    />
                  </button>
                )}
                {current.kind === "bilibili" && current.qid != null && (
                  <button
                    className="btn-ghost w-8 h-8 shrink-0"
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
                      size={16}
                      className={savedOnline[`bilibili-${current.qid}`] ? "fill-[#e0533f] text-[#e0533f]" : ""}
                    />
                  </button>
                )}
                {current.kind === "navidrome" && current.qid != null && (
                  <button
                    className="btn-ghost w-8 h-8 shrink-0"
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
                      size={16}
                      className={savedOnline[`navidrome-${current.qid}`] ? "fill-[#e0533f] text-[#e0533f]" : ""}
                    />
                  </button>
                )}
                {current.kind === "kugou" && current.kgid != null && (
                  <button
                    className="btn-ghost w-8 h-8 shrink-0"
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
                      size={16}
                      className={savedOnline[`kugou-${current.kgid}`] ? "fill-[#e0533f] text-[#e0533f]" : ""}
                    />
                  </button>
                )}
              </>
            ) : (
              <>
                <div
                  className={`w-[44px] h-[44px] ${isLiquid ? "rounded-xl" : "rounded-[10px]"} flex items-center justify-center`}
                  style={{
                    background: "rgba(243,233,216,0.05)",
                    border: "1px solid var(--line)",
                  }}
                >
                  <Play size={18} className="text-[var(--ink-3)]" />
                </div>
                <div className="text-[13px] text-[var(--ink-3)]">未在播放</div>
              </>
            )}
          </div>
        )}

        {/* 中部控制 */}
        <div className="flex-1 flex flex-col items-center justify-center gap-1 min-w-0">
          <div className="flex items-center gap-3">
            <button
              className={`btn-ghost ${isSkeuo ? "w-[32px] h-[32px]" : "w-[30px] h-[30px]"} ${shuffle ? "!text-[var(--accent)]" : ""}`}
              onClick={toggleShuffle}
              title="随机播放"
            >
              <Shuffle size={15} />
            </button>
            <button
              className={`btn-ghost ${isSkeuo ? "w-[32px] h-[32px]" : "w-[30px] h-[30px]"}`}
              onClick={prev}
              title="上一首"
            >
              <SkipBack size={16} className="fill-current" />
            </button>
            <button
              className={`btn-ghost ${isSkeuo ? "w-[32px] h-[32px]" : "w-[30px] h-[30px]"} disabled:opacity-40`}
              disabled={!current}
              onClick={() => seekBackward(5000)}
              title="快退 5 秒（←）"
            >
              <Rewind size={16} className="fill-current" />
            </button>
            {isSkeuo ? (
              <button
                className="w-[40px] h-[40px] rounded-full flex items-center justify-center text-[var(--accent-on)] transition-transform hover:scale-105 active:scale-95 shadow-[var(--sk-raised-shadow)]"
                style={{
                  background: "linear-gradient(135deg, var(--accent-strong) 0%, var(--accent) 60%, var(--accent) 120%)",
                  border: "1px solid var(--sk-edge)",
                  boxShadow: "var(--sk-raised-shadow), 0 0 12px var(--accent-soft)",
                }}
                onClick={togglePlay}
                title="播放 / 暂停（空格）"
              >
                {playing ? (
                  <Pause size={18} className="fill-current" />
                ) : (
                  <Play size={18} className="fill-current ml-0.5" />
                )}
              </button>
            ) : isLiquid ? (
              <button
                className="w-[42px] h-[42px] play-btn shrink-0"
                onClick={togglePlay}
                title="播放 / 暂停（空格）"
              >
                {playing ? (
                  <Pause size={18} className="fill-current" />
                ) : (
                  <Play size={18} className="fill-current ml-0.5" />
                )}
              </button>
            ) : (
              <button
                className="w-[38px] h-[38px] rounded-full flex items-center justify-center text-[var(--accent-on)] hover:scale-105 active:scale-95 transition-transform"
                style={{
                  background: "linear-gradient(135deg, var(--accent-strong) 0%, var(--accent) 60%, var(--accent) 120%)",
                  boxShadow: "0 6px 20px -4px var(--accent-soft)",
                }}
                onClick={togglePlay}
                title="播放 / 暂停（空格）"
              >
                {playing ? (
                  <Pause size={17} className="fill-current" />
                ) : (
                  <Play size={17} className="fill-current ml-0.5" />
                )}
              </button>
            )}
            <button
              className={`btn-ghost ${isSkeuo ? "w-[32px] h-[32px]" : "w-[30px] h-[30px]"} disabled:opacity-40`}
              disabled={!current}
              onClick={() => seekForward(5000)}
              title="快进 5 秒（→）"
            >
              <FastForward size={16} className="fill-current" />
            </button>
            <button
              className={`btn-ghost ${isSkeuo ? "w-[32px] h-[32px]" : "w-[30px] h-[30px]"}`}
              onClick={() => next(false)}
              title="下一首"
            >
              <SkipForward size={16} className="fill-current" />
            </button>
            <button
              className={`btn-ghost relative ${isSkeuo ? "w-[32px] h-[32px]" : "w-[30px] h-[30px]"} ${
                repeat !== "off" ? (isSkeuo ? "!text-[var(--accent-strong)]" : "!text-[var(--accent)]") : ""
              }`}
              onClick={() =>
                setRepeat(repeat === "off" ? "all" : repeat === "all" ? "one" : "off")
              }
              title={repeat === "off" ? "列表循环" : repeat === "all" ? "单曲循环" : "关闭循环"}
            >
              {repeat === "one" ? <Repeat1 size={15} /> : <Repeat size={15} />}
              {isSkeuo && repeat !== "off" && (
                <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />
              )}
            </button>
          </div>
          <Playhead fallbackTotal={current?.durationMs ?? 0} />
        </div>

        {/* 右侧控制 */}
        <div className="flex items-center gap-2 w-[260px] min-w-[200px] justify-end">
          {/* 桌面歌词开关 */}
          <button
            className={`btn-ghost ${isSkeuo ? "w-[34px] h-[34px] text-[12px]" : "w-9 h-9 text-[13px]"} font-bold ${
              desktopLyricsOn ? "!text-[var(--accent)]" : ""
            }`}
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
            词
          </button>
          <button
            className={`btn-ghost relative ${isSkeuo ? "w-[34px] h-[34px] text-[11px]" : "w-9 h-9 text-[11.5px]"} font-bold tabular-nums`}
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
            <span className={isSkeuo && speed !== 1 ? "text-[var(--accent-strong)]" : !isSkeuo && speed !== 1 ? "!text-[var(--accent)]" : ""}>
              {speed}x
            </span>
            {isSkeuo && speed !== 1 && (
              <span className="skeuo-led-on absolute top-1 right-1 !w-[4px] !h-[4px]" />
            )}
          </button>

          {/* 音量 */}
          <div className="relative flex items-center">
            <button
              ref={volBtnRef}
              className={`btn-ghost ${isSkeuo ? "w-[34px] h-[34px]" : "w-9 h-9"} shrink-0 ${
                volOpen
                  ? isSkeuo
                    ? "!text-[var(--accent-strong)] shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]"
                    : "!text-[var(--accent)]"
                  : ""
              }`}
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
                uiTheme={uiTheme}
              />
            )}
          </div>

          {/* 定时停止 */}
          <div className="relative flex items-center">
            <button
              className={`btn-ghost relative ${isSkeuo ? "w-[34px] h-[34px]" : "w-9 h-9"} shrink-0 ${
                sleepAt != null || sleepStopAfterTrack || sleepStopAfterQueue
                  ? isSkeuo
                    ? "!text-[var(--accent-strong)] shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]"
                    : "!text-[var(--accent)]"
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
              {isSkeuo && (sleepAt != null || sleepStopAfterTrack || sleepStopAfterQueue) && (
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
                uiTheme={uiTheme}
              />
            )}
          </div>

          <button
            className={`btn-ghost relative ${isSkeuo ? "w-[34px] h-[34px]" : "w-9 h-9"} ${miniPlayer ? "!text-[var(--accent)]" : ""}`}
            onClick={() => setMiniPlayer(!miniPlayer)}
            title="极简小窗模式"
          >
            <PictureInPicture2 size={isSkeuo ? 15 : 16} />
          </button>

          <button
            className={`btn-ghost relative ${isSkeuo ? "w-[34px] h-[34px]" : "w-9 h-9"} ${
              queueOpen
                ? isSkeuo
                  ? "!text-[var(--accent-strong)] shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]"
                  : "!text-[var(--accent)]"
                : ""
            }`}
            onClick={() => setQueueOpen(!queueOpen)}
            title="播放队列"
          >
            <ListMusic size={16} />
            {queue.length > 1 && (
              <span
                className={`absolute -top-1 -right-1 text-[9.5px] text-[var(--accent-on)] rounded-full min-w-[15px] leading-[15px] font-bold text-center px-0.5 ${
                  isSkeuo ? "border border-black/40 shadow" : ""
                }`}
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
