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
import { useMemo } from "react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import Slider from "./Slider";
import { activeLyricText, fmtTime } from "../utils";

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
  const queueOpen = useStore((s) => s.queueOpen);
  const nowPlayingOpen = useStore((s) => s.nowPlayingOpen);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const neteaseToggleLike = useStore((s) => s.neteaseToggleLike);
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
  // 播放栏歌词：当前行（未打开播放页时也加载并展示）
  // needsScroll 按可见宽度估算：约 13px 字号下，中文≈13px/字、ASCII≈7px/字
  const activeLyric = useMemo(() => {
    if (nowPlayingOpenFlag) return null;
    const text = activeLyricText(lyrics, pos);
    if (text == null) return null;
    const width = [...text].reduce(
      (acc, ch) => acc + (ch.charCodeAt(0) > 0x2e80 ? 13 : 7),
      0
    );
    return { text, needsScroll: width > 210 };
  }, [nowPlayingOpenFlag, lyrics, pos]);

  return (
    <div className="absolute bottom-0 left-0 right-0 z-50 px-4 pb-4 pt-1">
      {download && (
        <div className="absolute left-8 right-8 top-0 h-[3px] bg-white/[0.07] rounded-full overflow-hidden">
          <div
            className="h-full transition-all duration-300 rounded-full"
            style={{
              width: `${download.pct}%`,
              background: "var(--accent)",
            }}
          />
        </div>
      )}

      <div className="h-[88px] glass-strong rounded-[22px] flex items-center pl-5 pr-6 gap-5">
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
                  className="w-[56px] h-[56px] rounded-xl shadow-[0_8px_24px_rgba(0,0,0,0.5)]"
                  iconSize={20}
                />
                <div className="absolute inset-0 rounded-xl bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                  <ChevronUp
                    size={18}
                    className={`text-white transition-transform ${nowPlayingOpen ? "rotate-180" : ""}`}
                  />
                </div>
              </button>
              <div className="min-w-0">
                {activeLyric != null ? (
                  <div
                    className="text-[13px] font-medium text-[var(--accent-strong)] cursor-pointer marquee-wrap"
                    onClick={() => setNowPlayingOpen(!nowPlayingOpen)}
                    title="点击展开播放页"
                  >
                    {activeLyric.needsScroll ? (
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
                      style={{ background: "rgba(240,162,74,0.14)" }}
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
            </>
          ) : (
            <>
              <div
                className="w-[56px] h-[56px] rounded-xl flex items-center justify-center"
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

        {/* 中部控制 */}
        <div className="flex-1 flex flex-col items-center justify-center gap-1.5 min-w-0">
          <div className="flex items-center gap-4">
            <button
              className={`btn-ghost w-8 h-8 ${shuffle ? "!text-[var(--accent)]" : ""}`}
              onClick={toggleShuffle}
              title="随机播放"
            >
              <Shuffle size={15} />
            </button>
            <button className="btn-ghost w-8 h-8" onClick={prev} title="上一首">
              <SkipBack size={17} className="fill-current" />
            </button>
            <button
              className="w-11 h-11 rounded-full flex items-center justify-center text-[#241505] hover:scale-105 active:scale-95 transition-transform"
              style={{
                background: "linear-gradient(135deg, #ffc470 0%, #f0a24a 60%, #e8823f 120%)",
                boxShadow: "0 6px 20px -4px rgba(240,162,74,0.5)",
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
            <button className="btn-ghost w-8 h-8" onClick={() => next(false)} title="下一首">
              <SkipForward size={17} className="fill-current" />
            </button>
            <button
              className={`btn-ghost w-8 h-8 ${repeat !== "off" ? "!text-[var(--accent)]" : ""}`}
              onClick={() =>
                setRepeat(repeat === "off" ? "all" : repeat === "all" ? "one" : "off")
              }
              title={repeat === "off" ? "列表循环" : repeat === "all" ? "单曲循环" : "关闭循环"}
            >
              {repeat === "one" ? <Repeat1 size={16} /> : <Repeat size={16} />}
            </button>
          </div>
          <div className="w-full max-w-[540px] flex items-center gap-3">
            <span className="text-[11px] text-[var(--ink-3)] tabular-nums w-9 text-right">
              {fmtTime(pos)}
            </span>
            <Slider
              value={pos}
              max={total || 1}
              onChange={(v) => useStore.setState({ pos: v })}
              onCommit={(v) => seek(v)}
              className="flex-1"
            />
            <span className="text-[11px] text-[var(--ink-3)] tabular-nums w-9">
              {fmtTime(total)}
            </span>
          </div>
        </div>

        {/* 右侧控制 */}
        <div className="flex items-center gap-2 w-[260px] min-w-[200px] justify-end">
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
          <div className="flex items-center gap-2 w-[104px] mx-1">
            <button
              className="btn-ghost w-8 h-8 shrink-0"
              onClick={() => setVolume(volume === 0 ? 0.8 : 0)}
              title="静音"
            >
              <VolIcon size={15} />
            </button>
            <Slider value={volume} max={1} onChange={setVolume} className="flex-1" thick={4} />
          </div>
          <button
            className={`btn-ghost relative w-9 h-9 ${queueOpen ? "!text-[var(--accent)]" : ""}`}
            onClick={() => setQueueOpen(!queueOpen)}
            title="播放队列"
          >
            <ListMusic size={16} />
            {queue.length > 1 && (
              <span
                className="absolute -top-1 -right-1 text-[9.5px] text-[#241505] rounded-full min-w-[15px] leading-[15px] font-bold text-center px-0.5"
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
