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
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import Slider from "./Slider";
import { fmtTime } from "../utils";

const SPEEDS = [0.75, 1, 1.25, 1.5, 2];

export default function PlayerBar() {
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
  const queueOpen = useStore((s) => s.queueOpen);

  const total = dur || current?.durationMs || 0;
  const VolIcon = volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;

  return (
    <div className="h-[92px] shrink-0 glass-strong border-t border-white/[0.07] relative z-30 flex items-center px-4 gap-4">
      {download && (
        <div className="absolute left-0 right-0 -top-[3px] h-[3px] bg-white/5 overflow-hidden">
          <div
            className="h-full transition-all duration-300"
            style={{
              width: `${download.pct}%`,
              background: "linear-gradient(90deg, var(--dyn), #a5b4fc)",
            }}
          />
        </div>
      )}

      {/* 曲目信息 */}
      <div className="flex items-center gap-3 w-[240px] min-w-[180px]">
        {current ? (
          <>
            <button
              className="group relative shrink-0"
              onClick={() => setNowPlayingOpen(true)}
              title="展开播放页"
            >
              <CoverImg
                src={current.cover}
                seed={current.title}
                className="w-[54px] h-[54px] rounded-xl shadow-lg"
                iconSize={20}
              />
              <div className="absolute inset-0 rounded-xl bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                <ChevronUp size={18} className="text-white" />
              </div>
            </button>
            <div className="min-w-0">
              <div
                className="text-[13px] font-medium text-zinc-100 truncate cursor-pointer hover:underline"
                onClick={() => setNowPlayingOpen(true)}
              >
                {current.title}
              </div>
              <div className="text-[12px] text-zinc-500 truncate">{current.artist}</div>
            </div>
            {current.kind === "track" && current.id != null && (
              <button
                className="btn-ghost w-7 h-7 shrink-0"
                onClick={() => toggleLike(current.id!)}
                title={current.liked ? "取消喜欢" : "喜欢"}
              >
                <Heart
                  size={15}
                  className={current.liked ? "fill-rose-500 text-rose-500" : ""}
                />
              </button>
            )}
          </>
        ) : (
          <>
            <div className="w-[54px] h-[54px] rounded-xl bg-white/[0.04] border border-white/[0.06] flex items-center justify-center">
              <Play size={18} className="text-zinc-600" />
            </div>
            <div className="text-[13px] text-zinc-600">未在播放</div>
          </>
        )}
      </div>

      {/* 中部控制 */}
      <div className="flex-1 flex flex-col items-center justify-center gap-1.5 min-w-0">
        <div className="flex items-center gap-2">
          <button
            className={`btn-ghost w-8 h-8 ${shuffle ? "text-[var(--dyn)]" : ""}`}
            onClick={toggleShuffle}
            title="随机播放"
          >
            <Shuffle size={15} />
          </button>
          <button className="btn-ghost w-8 h-8" onClick={prev} title="上一首">
            <SkipBack size={16} className="fill-current" />
          </button>
          <button
            className="w-10 h-10 rounded-full bg-white text-zinc-900 flex items-center justify-center shadow-[0_4px_16px_rgba(0,0,0,0.4)] hover:scale-105 active:scale-95 transition-transform"
            onClick={togglePlay}
            title="播放 / 暂停（空格）"
          >
            {playing ? (
              <Pause size={17} className="fill-current" />
            ) : (
              <Play size={17} className="fill-current ml-0.5" />
            )}
          </button>
          <button className="btn-ghost w-8 h-8" onClick={() => next(false)} title="下一首">
            <SkipForward size={16} className="fill-current" />
          </button>
          <button
            className={`btn-ghost w-8 h-8 ${
              repeat !== "off" ? "text-[var(--dyn)]" : ""
            }`}
            onClick={() => setRepeat(repeat === "off" ? "all" : repeat === "all" ? "one" : "off")}
            title={repeat === "off" ? "列表循环" : repeat === "all" ? "单曲循环" : "关闭循环"}
          >
            {repeat === "one" ? <Repeat1 size={15} /> : <Repeat size={15} />}
          </button>
        </div>
        <div className="w-full max-w-[560px] flex items-center gap-2.5">
          <span className="text-[11px] text-zinc-500 tabular-nums w-9 text-right">
            {fmtTime(pos)}
          </span>
          <Slider
            value={pos}
            max={total || 1}
            onChange={(v) => useStore.setState({ pos: v })}
            onCommit={(v) => seek(v)}
            className="flex-1"
          />
          <span className="text-[11px] text-zinc-500 tabular-nums w-9">
            {fmtTime(total)}
          </span>
        </div>
      </div>

      {/* 右侧控制 */}
      <div className="flex items-center gap-1 w-[240px] min-w-[180px] justify-end">
        <button
          className={`btn-ghost w-8 h-8 text-[11px] font-semibold tabular-nums ${
            speed !== 1 ? "text-[var(--dyn)]" : ""
          }`}
          onClick={() => {
            const i = SPEEDS.indexOf(speed);
            setSpeed(SPEEDS[(i + 1) % SPEEDS.length]);
          }}
          title="播放速度"
        >
          {speed}x
        </button>
        <div className="flex items-center gap-1.5 w-[110px]">
          <button
            className="btn-ghost w-7 h-7 shrink-0"
            onClick={() => setVolume(volume === 0 ? 0.8 : 0)}
            title="静音"
          >
            <VolIcon size={15} />
          </button>
          <Slider value={volume} max={1} onChange={setVolume} className="flex-1" thick={4} />
        </div>
        <button
          className={`btn-ghost relative w-8 h-8 ${queueOpen ? "text-[var(--dyn)]" : ""}`}
          onClick={() => setQueueOpen(!queueOpen)}
          title="播放队列"
        >
          <ListMusic size={16} />
          {queue.length > 1 && (
            <span className="absolute -top-0.5 -right-0.5 text-[9px] bg-[var(--dyn)] text-white rounded-full min-w-[14px] leading-[14px] font-bold text-center">
              {queue.length}
            </span>
          )}
        </button>
      </div>
    </div>
  );
}
