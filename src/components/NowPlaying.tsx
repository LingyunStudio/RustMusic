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
  const loadLyricsByKey = useStore((s) => s.loadLyricsByKey);
  const seek = useStore((s) => s.seek);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const neteaseToggleLike = useStore((s) => s.neteaseToggleLike);

  const scrollRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<(HTMLDivElement | null)[]>([]);

  // 歌词键：本地曲目 / 网易云在线曲目
  const lyricsKey =
    current?.kind === "track" && current.id != null
      ? `track-${current.id}`
      : current?.kind === "netease" && current.nid != null
        ? `net-${current.nid}`
        : null;

  useEffect(() => {
    if (lyricsKey) {
      loadLyricsByKey(lyricsKey);
    } else {
      useStore.setState({ lyrics: null, lyricsFor: null, lyricsLoading: false });
    }
  }, [lyricsKey, loadLyricsByKey]);

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
      {/* 背景：近实心暖色底 + 封面微光 */}
      <div className="absolute inset-0 overflow-hidden">
        <div
          className="absolute inset-0"
          style={{
            background:
              "linear-gradient(180deg, #1a1309 0%, #120e09 52%, #0b0906 100%)",
          }}
        />
        {coverUrl && (
          <img
            src={coverUrl}
            alt=""
            className="absolute inset-0 w-full h-full object-cover scale-150 opacity-[0.12]"
            style={{ filter: "blur(72px)" }}
            draggable={false}
          />
        )}
        <div
          className="absolute inset-0"
          style={{
            background:
              "radial-gradient(ellipse 90% 70% at 30% 40%, rgba(240,162,74,0.07) 0%, transparent 60%)",
          }}
        />
      </div>

      {/* 顶栏 */}
      <div className="relative flex items-center justify-between px-8 h-16">
        <span className="text-[11px] text-[var(--ink-3)] tracking-[0.26em] flex items-center gap-2.5">
          <Music4 size={14} />
          正在播放
        </span>
        <button
          className="btn-ghost w-10 h-10 !rounded-full"
          onClick={() => setNowPlayingOpen(false)}
        >
          <ChevronDown size={20} />
        </button>
      </div>

      {/* 主体 */}
      <div className="relative flex gap-14 px-14 pb-8 items-stretch h-[calc(100%-64px)]">
        {/* 左：封面 */}
        <div className="w-[40%] max-w-[440px] flex flex-col items-center justify-center gap-5">
          <div className="relative">
            <div
              className="absolute -inset-10 rounded-[48px] opacity-40 blur-3xl transition-colors duration-1000"
              style={{
                background: coverUrl
                  ? `url(${coverUrl}) center/cover`
                  : "linear-gradient(135deg,#f0a24a,#e0533f)",
              }}
            />
            <CoverImg
              src={current.cover}
              seed={current.title}
              className="w-[min(36vh,340px)] h-[min(36vh,340px)] rounded-[28px] shadow-[0_36px_90px_rgba(0,0,0,0.7)] relative"
              iconSize={56}
            />
          </div>
          <div className="text-center max-w-full">
            <div className="text-[22px] font-bold text-[var(--ink)] truncate">
              {current.title}
            </div>
            <div className="text-[13.5px] text-[var(--ink-2)] mt-1.5 truncate">
              {current.artist}
            </div>
            <div className="flex items-center justify-center gap-3 mt-3.5">
              {current.kind === "track" && current.id != null && (
                <button
                  className="btn-ghost w-9 h-9 !rounded-full glass"
                  onClick={() => toggleLike(current.id!)}
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
                  className="btn-ghost w-9 h-9 !rounded-full glass"
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
              {current.kind === "track" && (
                <span className="text-[11.5px] text-[var(--ink-2)] px-3 py-1 rounded-full bg-white/[0.06]">
                  {current.album || "未知专辑"}
                </span>
              )}
              {current.kind === "netease" && (
                <span className="text-[11.5px] text-[var(--accent)] px-3 py-1 rounded-full bg-[rgba(240,162,74,0.12)]">
                  网易云 · {current.album || "在线曲库"}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* 右：歌词 */}
        <div className="flex-1 min-w-0 relative">
          <div
            ref={scrollRef}
            className="h-full overflow-y-auto py-[28%] lyrics-mask pr-3"
            style={{ scrollbarWidth: "none" }}
          >
            {lyricsLoading && (
              <div className="text-[var(--ink-3)] text-[13px] text-center pt-20">
                正在加载歌词…
              </div>
            )}
            {!lyricsLoading && syncedLines.length === 0 && (
              <div className="flex flex-col items-center gap-4 text-[var(--ink-3)] pt-[30%]">
                <Mic2 size={28} />
                {lyrics && lyrics.lines.length
                  ? lyrics.lines.map((l, i) => (
                      <p
                        key={i}
                        className="text-[15px] text-[var(--ink-2)] text-center leading-relaxed"
                      >
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
                className={`px-4 py-[10px] text-center cursor-pointer transition-all duration-300 rounded-2xl ${
                  i === activeIdx
                    ? "text-[var(--accent-strong)] text-[24px] font-bold scale-[1.02]"
                    : "text-[rgba(243,233,216,0.4)] text-[19px] hover:text-[var(--ink-2)]"
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
