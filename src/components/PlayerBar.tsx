import {
  ChevronUp,
  Heart,
  ListMusic,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useMemo, useRef, useState, useEffect } from "react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import Slider from "./Slider";
import { activeLyricText, fmtTime } from "../utils";

const SPEEDS = [0.75, 1, 1.25, 1.5, 2];

/** 竖向音量弹出条（点击音量图标显示，离开自动收起） */
function VolumePopover({
  volume,
  onSet,
  onClose,
}: {
  volume: number;
  onSet: (v: number) => void;
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
        className="absolute bottom-[48px] right-0 z-[61] w-11 rounded-2xl p-2 flex justify-center"
        style={{
          background: "var(--bar-glass)",
          backdropFilter: "blur(8px)",
          border: "1px solid var(--bar-line)",
          boxShadow: "var(--bar-shadow)",
        }}
      >
        <div
          className="relative w-1.5 rounded-full bg-[var(--shade-strong)] cursor-pointer touch-none"
          style={{ height: H }}
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId);
            onPointer(e);
          }}
          onPointerMove={(e) => {
            if (e.buttons !== 1) return;
            onPointer(e);
          }}
        >
          <div
            className="absolute left-0 right-0 bottom-0 rounded-full"
            style={{
              height: Math.max(2, H - y),
              background: "linear-gradient(180deg, var(--accent-strong), var(--accent))",
            }}
          />
          <div
            className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 w-3.5 h-3.5 rounded-full bg-white shadow"
            style={{ top: y }}
          />
        </div>
      </div>
    </>
  );
}

export default function PlayerBar({ centered = false }: { centered?: boolean }) {
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const volume = useStore((s) => s.volume);
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
  const lyrics = useStore((s) => s.lyrics);
  const nowPlayingOpenFlag = useStore((s) => s.nowPlayingOpen);
  const togglePlay = useStore((s) => s.togglePlay);
  const next = useStore((s) => s.next);
  const prev = useStore((s) => s.prev);
  const seek = useStore((s) => s.seek);
  const setVolume = useStore((s) => s.setVolume);
  const setSpeed = useStore((s) => s.setSpeed);
  const setRepeat = useStore((s) => s.setRepeat);
  const toggleShuffle = useStore((s) => s.toggleShuffle);
  const toggleLike = useStore((s) => s.toggleLike);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const setQueueOpen = useStore((s) => s.setQueueOpen);

  const total = dur || current?.durationMs || 0;
  const VolIcon = volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;
  // 播放栏歌词：当前行（未打开播放页时也加载并展示）。
  // 是否滚动不再靠字宽估算，而是渲染后实测：容器 scrollWidth > clientWidth 即滚。
  const activeLyric = useMemo(() => {
    if (nowPlayingOpenFlag) return null;
    const text = activeLyricText(lyrics, pos);
    if (text == null) return null;
    return { text };
  }, [nowPlayingOpenFlag, lyrics, pos]);

  // 实测溢出：文字真实宽度超出可视宽度时启用 marquee
  const lyricWrapRef = useRef<HTMLDivElement>(null);
  const [lyricOverflow, setLyricOverflow] = useState(false);
  useEffect(() => {
    const el = lyricWrapRef.current;
    if (!el || !activeLyric) {
      setLyricOverflow(false);
      return;
    }
    // 需要在下一帧测：本帧文字可能还没布局
    requestAnimationFrame(() => {
      const e2 = lyricWrapRef.current;
      if (e2) setLyricOverflow(e2.scrollWidth > e2.clientWidth + 1);
    });
  }, [activeLyric?.text]);

  const [volOpen, setVolOpen] = useState(false);
  const volBtnRef = useRef<HTMLButtonElement>(null);

  return (
    // 与右侧内容区（玻璃卡片）对齐：左偏移侧栏宽度、右边距与卡片外边距一致
    <div
      className={`anim-bar absolute bottom-0 right-0 z-50 px-6 pb-4 pt-1 ${
        centered ? "left-0" : "left-[236px]"
      }`}
    >
      {download && (
        <div className="absolute left-8 right-8 top-0 h-[3px] bg-[var(--shade)] rounded-full overflow-hidden">
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
        className={`glass-sheen h-[64px] rounded-[18px] flex items-center pl-5 pr-6 gap-5 transition-opacity duration-300 ${
          nowPlayingOpen ? "opacity-80 hover:opacity-100" : ""
        }`}
        style={{
          background: "var(--bar-glass)",
          backdropFilter: "blur(16px)",
          WebkitBackdropFilter: "blur(16px)",
          border: "1px solid var(--bar-line)",
          boxShadow: "var(--bar-shadow)",
        }}
      >
        {/* 曲目信息 */}
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
                  className="w-[44px] h-[44px] rounded-[10px] shadow-[var(--cover-shadow-sm)]"
                  iconSize={20}
                />
                <div className="absolute inset-0 rounded-xl bg-black/45 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                  <ChevronUp
                    size={18}
                    className={`text-[var(--ink)] transition-transform ${nowPlayingOpen ? "rotate-180" : ""}`}
                  />
                </div>
              </button>
              <div className="min-w-0">
                {activeLyric != null ? (
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
                )}
                <div className="text-[12px] text-[var(--ink-3)] truncate mt-1 flex items-center gap-2">
                  <span className="truncate">{current.artist}</span>
                  {current.quality && (
                    <span
                      className="shrink-0 text-[9.5px] font-semibold px-1.5 py-px rounded text-[var(--accent-strong)]"
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
                    className={
                      current.liked ? "fill-[#e0533f] text-[#e0533f]" : ""
                    }
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
                  className="btn-ghost w-8 h-8 shrink-0"
                  onClick={() => {
                    // mediaMid/vip 不在 current 里：从已入库的在线条目（最近播放/收藏）取
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
                    className={
                      savedOnline[`qq-${current.qid}`]
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
                className="w-[44px] h-[44px] rounded-[10px] flex items-center justify-center"
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

        {/* 中部控制（内容总高 40+16+4=60px，在 64px 条高内垂直居中） */}
        <div className="flex-1 flex flex-col items-center justify-center gap-1 min-w-0">
          <div className="flex items-center gap-4">
            <button
              className={`btn-ghost w-[30px] h-[30px] ${shuffle ? "!text-[var(--accent)]" : ""}`}
              onClick={toggleShuffle}
              title="随机播放"
            >
              <Shuffle size={15} />
            </button>
            <button className="btn-ghost w-[30px] h-[30px]" onClick={prev} title="上一首">
              <SkipBack size={16} className="fill-current" />
            </button>
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
            <button className="btn-ghost w-[30px] h-[30px]" onClick={() => next(false)} title="下一首">
              <SkipForward size={16} className="fill-current" />
            </button>
            <button
              className={`btn-ghost w-[30px] h-[30px] ${repeat !== "off" ? "!text-[var(--accent)]" : ""}`}
              onClick={() =>
                setRepeat(repeat === "off" ? "all" : repeat === "all" ? "one" : "off")
              }
              title={repeat === "off" ? "列表循环" : repeat === "all" ? "单曲循环" : "关闭循环"}
            >
              {repeat === "one" ? <Repeat1 size={15} /> : <Repeat size={15} />}
            </button>
          </div>
          <div className="w-full max-w-[540px] flex items-center gap-3">
            <span className="text-[11px] text-[var(--ink-3)] tabular-nums w-9 text-right">
              {fmtTime(pos)}
            </span>
            <Slider
              value={pos}
              max={total || 1}
              onChange={(v) => useStore.setState({ pos: v, scrubbing: true })}
              onCommit={(v) => {
                useStore.setState({ scrubbing: true });
                seek(v);
              }}
              className="flex-1"
            />
            <span className="text-[11px] text-[var(--ink-3)] tabular-nums w-9">
              {fmtTime(total)}
            </span>
          </div>
        </div>

        {/* 右侧控制 */}
        <div className="flex items-center gap-2 w-[260px] min-w-[200px] justify-end">
          {/* 桌面歌词开关（与速度按钮同尺寸，开启时点亮强调色） */}
          <button
            className={`btn-ghost w-9 h-9 text-[13px] font-bold ${
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
            className={`btn-ghost w-9 h-9 text-[11.5px] font-bold tabular-nums ${
              speed !== 1 ? "!text-[var(--accent)]" : ""
            }`}
            onClick={() => {
              const i = SPEEDS.indexOf(speed);
              setSpeed(SPEEDS[(i + 1) % SPEEDS.length]);
            }}
            title="播放速度"
          >
            {speed}x
          </button>
          {/* 音量：仅图标，点击弹出竖向音量条 */}
          <div className="relative flex items-center">
            <button
              ref={volBtnRef}
              className={`btn-ghost w-9 h-9 shrink-0 ${volOpen ? "!text-[var(--accent)]" : ""}`}
              onClick={() => setVolOpen((v) => !v)}
              title={volume === 0 ? "取消静音" : "音量"}
            >
              <VolIcon size={15} />
            </button>
            {volOpen && (
              <VolumePopover
                volume={volume}
                onSet={setVolume}
                onClose={() => setVolOpen(false)}
              />
            )}
          </div>
          <button
            className={`btn-ghost relative w-9 h-9 ${queueOpen ? "!text-[var(--accent)]" : ""}`}
            onClick={() => setQueueOpen(!queueOpen)}
            title="播放队列"
          >
            <ListMusic size={16} />
            {queue.length > 1 && (
              <span
                className="absolute -top-1 -right-1 text-[9.5px] text-[var(--accent-on)] rounded-full min-w-[15px] leading-[15px] font-bold text-center px-0.5"
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
