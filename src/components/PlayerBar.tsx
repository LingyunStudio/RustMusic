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
  Timer,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import Slider from "./Slider";
import { activeLyricText, fmtTime } from "../utils";

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
  const activeLyric = (() => {
    if (nowPlayingOpen) return null;
    const text = activeLyricText(lyrics, pos);
    return text == null ? null : { text };
  })();

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
    requestAnimationFrame(() => {
      const e2 = lyricWrapRef.current;
      if (e2) setLyricOverflow(e2.scrollWidth > e2.clientWidth + 1);
    });
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

/** 中部进度区：时间 + 滑块。pos/dur 的高频订阅隔离于此 */
function Playhead({ fallbackTotal }: { fallbackTotal: number }) {
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const seek = useStore((s) => s.seek);
  const total = dur || fallbackTotal || 0;
  return (
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
        className="absolute bottom-[48px] right-0 z-[61] w-11 rounded-2xl p-2 flex flex-col items-center gap-1.5"
        style={{
          background: "var(--bar-glass)",
          backdropFilter: "blur(var(--bar-blur, 8px))",
          border: "1px solid var(--bar-line)",
          boxShadow: "var(--bar-shadow)",
        }}
      >
        {/* 静音切换：记忆静音前的音量 */}
        <button
          className="w-7 h-6 rounded-lg flex items-center justify-center text-[10px] font-semibold text-[var(--ink-2)] hover:bg-[var(--shade-strong)] hover:text-[var(--ink)] transition-colors"
          onClick={() => {
            onToggleMute();
          }}
          title={muted ? "取消静音" : "静音"}
        >
          {muted ? "有声" : "静音"}
        </button>
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

/** 定时停止弹出菜单（点击时钟图标显示，点击其他区域收起） */
function SleepPopover({
  sleepAt,
  onSet,
  onClose,
}: {
  sleepAt: number | null;
  onSet: (min: number | null) => void;
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
        className="absolute bottom-[48px] right-0 z-[61] w-[168px] rounded-2xl p-1.5 flex flex-col"
        style={{
          background: "var(--bar-glass)",
          backdropFilter: "blur(var(--bar-blur, 8px))",
          border: "1px solid var(--bar-line)",
          boxShadow: "var(--bar-shadow)",
        }}
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
        {sleepAt != null && (
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] text-[#e0533f] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              onSet(null);
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
  const setVolume = useStore((s) => s.setVolume);
  const setSpeed = useStore((s) => s.setSpeed);
  const setRepeat = useStore((s) => s.setRepeat);
  const toggleShuffle = useStore((s) => s.toggleShuffle);
  const toggleLike = useStore((s) => s.toggleLike);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const setQueueOpen = useStore((s) => s.setQueueOpen);

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
          backdropFilter: "blur(var(--bar-blur, 16px))",
          WebkitBackdropFilter: "blur(var(--bar-blur, 16px))",
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
                <TrackTitleLyric />
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
              {current.kind === "kugou" && current.kgid != null && (
                <button
                  className="btn-ghost w-8 h-8 shrink-0"
                  onClick={() => {
                    // vip 不在 current 里：从已入库的在线条目取兜底
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
          <Playhead fallbackTotal={current?.durationMs ?? 0} />
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
          {/* 音量：点击图标弹出音量条（弹窗内可静音，记住静音前的音量） */}
          <div className="relative flex items-center">
            <button
              ref={volBtnRef}
              className={`btn-ghost w-9 h-9 shrink-0 ${volOpen ? "!text-[var(--accent)]" : ""}`}
              onClick={() => setVolOpen((v) => !v)}
              title="音量"
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
          {/* 定时停止：到点停引擎并复位 UI */}
          <div className="relative flex items-center">
            <button
              className={`btn-ghost w-9 h-9 shrink-0 ${
                sleepAt != null ? "!text-[var(--accent)]" : ""
              }`}
              onClick={() => setSleepOpen((v) => !v)}
              title={
                sleepAt != null
                  ? `定时停止：剩余约 ${Math.max(1, Math.ceil((sleepAt - Date.now()) / 60000))} 分钟`
                  : "定时停止播放"
              }
            >
              <Timer size={15} />
            </button>
            {sleepOpen && (
              <SleepPopover
                sleepAt={sleepAt}
                onSet={setSleepTimer}
                onClose={() => setSleepOpen(false)}
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
