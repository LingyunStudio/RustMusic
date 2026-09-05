import { useEffect, useMemo, useRef } from "react";
import { ChevronDown, Heart, Mic2, Music4 } from "lucide-react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import { coverSrc } from "../api";
import { extractColor } from "../utils";

export default function NowPlaying() {
  const current = useStore((s) => s.current);
  const pos = useStore((s) => s.pos);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const toggleLike = useStore((s) => s.toggleLike);
  const lyrics = useStore((s) => s.lyrics);
  const lyricsLoading = useStore((s) => s.lyricsLoading);
  const loadLyrics = useStore((s) => s.loadLyrics);
  const seek = useStore((s) => s.seek);

  const scrollRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<(HTMLDivElement | null)[]>([]);

  const trackId = current?.kind === "track" ? current.id : null;

  useEffect(() => {
    if (trackId != null) {
      loadLyrics(trackId);
    } else {
      // 在线音源等无歌词场景，清掉上一首的歌词
      useStore.setState({ lyrics: null, lyricsFor: null, lyricsLoading: false });
    }
  }, [trackId, loadLyrics]);

  const syncedLines = useMemo(
    () =>
      lyrics?.synced
        ? lyrics.lines
            .filter((l) => l.timeMs != null)
            .sort((a, b) => (a.timeMs ?? 0) - (b.timeMs ?? 0))
        : [],
    [lyrics]
  );

  const activeIdx = useMemo(() => {
    if (!syncedLines.length) return -1;
    let lo = 0;
    let hi = syncedLines.length - 1;
    let ans = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if ((syncedLines[mid].timeMs ?? 0) <= pos) {
        ans = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return ans;
  }, [syncedLines, pos]);

  useEffect(() => {
    const el = lineRefs.current[activeIdx];
    const container = scrollRef.current;
    if (el && container) {
      const top = el.offsetTop - container.clientHeight / 2 + el.clientHeight / 2;
      container.scrollTo({ top, behavior: "smooth" });
    }
  }, [activeIdx]);

  if (!current) return null;
  const coverUrl = current.cover ? coverSrc(current.cover) : "";

  return (
    <div className="absolute inset-0 z-40 anim-np overflow-hidden">
      {/* 背景：封面模糊 + 渐变遮罩 */}
      <div className="absolute inset-0 overflow-hidden">
        {coverUrl && (
          <img
            src={coverUrl}
            alt=""
            className="absolute inset-0 w-full h-full object-cover scale-150 opacity-25"
            style={{ filter: "blur(56px)" }}
            draggable={false}
          />
        )}
        <div className="absolute inset-0 bg-gradient-to-b from-black/55 via-black/45 to-black/70" />
      </div>

      {/* 顶栏 */}
      <div className="relative flex items-center justify-between px-6 h-14">
        <span className="text-[12px] text-zinc-300/70 tracking-widest flex items-center gap-2">
          <Music4 size={14} />
          正在播放
        </span>
        <button className="btn-ghost w-9 h-9 !rounded-full" onClick={() => setNowPlayingOpen(false)}>
          <ChevronDown size={20} />
        </button>
      </div>

      {/* 主体 */}
      <div className="relative flex gap-10 px-10 pb-6 items-stretch h-[calc(100%-56px)]">
        {/* 左：封面 */}
        <div className="w-[42%] max-w-[440px] flex flex-col items-center justify-center gap-7">
          <div className="relative group">
            <div
              className="absolute -inset-8 rounded-[40px] opacity-45 blur-3xl transition-opacity"
              style={{
                background: coverUrl
                  ? `url(${coverUrl}) center/cover`
                  : "linear-gradient(135deg,#6366f1,#22d3ee)",
              }}
            />
            <CoverImg
              src={current.cover}
              seed={current.title}
              className="w-[min(38vh,360px)] h-[min(38vh,360px)] rounded-3xl shadow-[0_30px_80px_rgba(0,0,0,0.65)] relative"
              iconSize={56}
            />
          </div>
          <div className="text-center max-w-full">
            <div className="text-[22px] font-bold text-white truncate">
              {current.title}
            </div>
            <div className="text-[14px] text-zinc-400 mt-1 truncate">{current.artist}</div>
            <div className="flex items-center justify-center gap-2 mt-4">
              {current.kind === "track" && current.id != null && (
                <button
                  className="btn-ghost w-9 h-9 !rounded-full bg-white/[0.06]"
                  onClick={() => toggleLike(current.id!)}
                >
                  <Heart
                    size={17}
                    className={current.liked ? "fill-rose-500 text-rose-500" : ""}
                  />
                </button>
              )}
              {current.kind === "track" && (
                <span className="text-[11px] text-zinc-500 px-2.5 py-1 rounded-full bg-white/[0.05]">
                  {current.album || "未知专辑"}
                </span>
              )}
              {current.kind === "url" && (
                <span className="text-[11px] text-cyan-300/80 px-2.5 py-1 rounded-full bg-cyan-400/10">
                  在线音源
                </span>
              )}
            </div>
          </div>
        </div>

        {/* 右：歌词 */}
        <div className="flex-1 min-w-0 relative">
          <div className="absolute inset-y-0 -left-6 w-6 bg-gradient-to-r from-transparent to-white/[0.02] pointer-events-none" />
          <div
            ref={scrollRef}
            className="h-full overflow-y-auto py-[28%] lyrics-mask pr-2"
            style={{ scrollbarWidth: "none" }}
          >
            {lyricsLoading && (
              <div className="text-zinc-500 text-[13px] text-center pt-20">
                正在加载歌词…
              </div>
            )}
            {!lyricsLoading && syncedLines.length === 0 && (
              <div className="flex flex-col items-center gap-3 text-zinc-600 pt-[30%]">
                <Mic2 size={26} />
                {lyrics && lyrics.lines.length
                  ? lyrics.lines.map((l, i) => (
                      <p key={i} className="text-[15px] text-zinc-400 text-center leading-relaxed">
                        {l.text}
                      </p>
                    ))
                  : "暂无歌词"}
              </div>
            )}
            {syncedLines.map((l, i) => (
              <div
                key={i}
                ref={(el) => {
                  lineRefs.current[i] = el;
                }}
                onClick={() => l.timeMs != null && seek(l.timeMs)}
                className={`px-3 py-[9px] text-center cursor-pointer transition-all duration-300 rounded-lg ${
                  i === activeIdx
                    ? "text-white text-[24px] font-bold scale-[1.02]"
                    : "text-zinc-400/60 text-[19px] hover:text-zinc-200"
                }`}
                style={{ transformOrigin: "center" }}
              >
                {l.text || "···"}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
