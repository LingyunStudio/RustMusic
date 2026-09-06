import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Heart, Mic2, Music4 } from "lucide-react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import { coverSrc } from "../api";
import { extractPalette } from "../utils";

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
        : current?.kind === "qq" && current.qid != null
          ? `qq-${current.qid}`
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

  // 封面取色 → 动态渐变背景
  const [palette, setPalette] = useState<string[]>([]);
  useEffect(() => {
    if (!coverUrl) {
      setPalette([]);
      return;
    }
    let alive = true;
    extractPalette(coverUrl, 4).then((colors) => {
      if (alive) setPalette(colors);
    });
    return () => {
      alive = false;
    };
  }, [coverUrl]);

  return (
    <div className="absolute inset-0 z-40 anim-np overflow-hidden">
      {/* 背景：封面多色采样的动态渐变 */}
      <div className="absolute inset-0 overflow-hidden">
        <div
          className="absolute inset-0 transition-colors duration-1000"
          style={{
            background:
              "linear-gradient(180deg, var(--backdrop-1) 0%, var(--backdrop-2) 55%, var(--bg) 100%)",
          }}
        />
        {palette.slice(0, 4).map((c, i) => (
          <div
            key={i}
            className="absolute rounded-full anim-Drift"
            style={{
              background: `radial-gradient(circle at center, ${c} 0%, transparent 62%)`,
              width: `${44 + i * 12}%`,
              height: `${40 + i * 10}%`,
              left: `${8 + i * 22}%`,
              top: `${i % 2 === 0 ? -6 + i * 8 : 44 - i * 6}%`,
              filter: "blur(90px)",
              opacity: 0.4,
              animation: "blobDrift 16s ease-in-out infinite",
              animationDelay: `${-i * 4}s`,
              ["--blob-a" as string]: 0.42 - i * 0.06,
              transition: "background 1.2s ease",
            }}
          />
        ))}
        <div
          className="absolute inset-0"
          style={{
            background:
              "radial-gradient(ellipse 130% 110% at 50% 45%, transparent 55%, var(--vignette) 100%)",
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
            <div className="text-[13px] text-[var(--ink-2)] mt-1 truncate">
              {current.artist}
            </div>
            <div className="flex items-center justify-center gap-2.5 mt-2.5">
              {current.kind === "track" && current.id != null && (
                <button
                  className="btn-ghost w-8 h-8 !rounded-full glass"
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
                  className="btn-ghost w-8 h-8 !rounded-full glass"
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
              {current.kind === "qq" && (
                <span className="text-[11px] text-[var(--ink-3)] px-2.5 py-0.5 rounded-full bg-[var(--shade)]">
                  QQ音乐
                </span>
              )}
              {current.kind === "track" && (
                <span className="text-[11px] text-[var(--ink-2)] px-2.5 py-0.5 rounded-full bg-[var(--shade)]">
                  {current.album || "未知专辑"}
                </span>
              )}
              {current.kind === "netease" && (
                <span className="text-[11px] text-[var(--accent)] px-2.5 py-0.5 rounded-full bg-[rgba(240,162,74,0.12)]">
                  网易云 · {current.album || "在线曲库"}
                </span>
              )}
              {current.kind === "qq" && current.album && (
                <span className="text-[11px] text-[var(--ink-2)] px-2.5 py-0.5 rounded-full bg-[var(--shade)]">
                  {current.album}
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
