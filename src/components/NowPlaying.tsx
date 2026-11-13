import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Heart, Maximize2, Mic2, Minimize2, Music4 } from "lucide-react";
import { useStore } from "../store";
import { formatLyricSource, useI18n } from "../i18n";
import CoverImg from "./CoverImg";
import NowPlayingSkeuo from "./NowPlaying.skeuo";
import { api, coverSrc } from "../api";
import {
  applyLyricFill,
  clearLyricFill,
  extractPalette,
  lyricAdaptiveColors,
  lyricLineProgress,
  remoteCoverCache,
} from "../utils";

export default function NowPlaying() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "skeuo") return <NowPlayingSkeuo />;
  return <NowPlayingStandardOrLiquid isLiquid={uiTheme === "liquid"} />;
}

function NowPlayingStandardOrLiquid({ isLiquid }: { isLiquid: boolean }) {
  const { tr } = useI18n();
  const current = useStore((s) => s.current);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const fullscreen = useStore((s) => s.fullscreen);
  const toggleFullscreen = useStore((s) => s.toggleFullscreen);
  const toggleLike = useStore((s) => s.toggleLike);
  const lyrics = useStore((s) => s.lyrics);
  const lyricsLoading = useStore((s) => s.lyricsLoading);
  const lyricSource = useStore((s) => s.lyrics?.source);
  const loadLyricsByKey = useStore((s) => s.loadLyricsByKey);
  const seek = useStore((s) => s.seek);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const neteaseToggleLike = useStore((s) => s.neteaseToggleLike);
  const savedOnline = useStore((s) => s.savedOnline);
  const likedOnline = useStore((s) => s.likedOnline);
  const recentOnline = useStore((s) => s.recentOnline);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const playing = useStore((s) => s.playing);
  const lyricsShowTrans = useStore((s) => s.lyricsShowTrans);
  const setLyricsShowTrans = useStore((s) => s.setLyricsShowTrans);
  const lyricsFontScale = useStore((s) => s.lyricsFontScale);
  const setLyricsFontScale = useStore((s) => s.setLyricsFontScale);

  const anchorRef = useRef({ pos: 0, at: performance.now() });
  useEffect(() => {
    const st0 = useStore.getState();
    anchorRef.current = { pos: st0.pos, at: performance.now() };
    let lastPos = st0.pos;
    let lastPlaying = st0.playing;
    return useStore.subscribe((s) => {
      if (s.pos !== lastPos || s.playing !== lastPlaying) {
        anchorRef.current = { pos: s.pos, at: performance.now() };
        lastPos = s.pos;
        lastPlaying = s.playing;
      }
    });
  }, []);

  const scrollRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<(HTMLDivElement | null)[]>([]);
  const lastActiveRef = useRef(-1);
  const renderFrameRef = useRef<() => void>(() => {});

  const lyricsKey =
    current?.kind === "track" && current.id != null
      ? `track-${current.id}`
      : current?.kind === "netease" && current.nid != null
        ? `net-${current.nid}`
        : current?.kind === "qq" && current.qid != null
          ? `qq-${current.qid}`
          : current?.kind === "kugou" && current.kgid != null
            ? `kug-${current.kgid}`
            : current?.kind === "bilibili" && current.qid != null
              ? `bili-${current.qid}`
              : current?.kind === "navidrome" && current.qid != null
                ? `nd-${current.qid}`
                : null;

  useEffect(() => {
    if (lyricsKey) {
      loadLyricsByKey(lyricsKey);
    } else {
      useStore.setState({ lyrics: null, lyricsFor: null, lyricsLoading: false });
    }
  }, [lyricsKey, loadLyricsByKey]);

  const hasTrans = useMemo(
    () => (lyrics?.lines ?? []).some((l) => l.trans != null && l.trans !== ""),
    [lyrics]
  );

  const syncedLines = useMemo(
    () =>
      lyrics?.synced
        ? lyrics.lines
            .filter((l) => l.timeMs != null)
            .sort((a, b) => (a.timeMs ?? 0) - (b.timeMs ?? 0))
        : [],
    [lyrics]
  );

  useEffect(() => {
    if (!syncedLines.length) return;
    const tick = () => {
      const a = anchorRef.current;
      const st = useStore.getState();
      const dur = st.dur;
      const speed = st.speed;
      const raw = a.pos + (performance.now() - a.at) * speed;
      const posNow = dur > 0 ? Math.min(raw, dur) : raw;
      const lyricPos = Math.max(0, posNow + (st.lyricOffsetMs ?? 0));

      let activeIdx = -1;
      let lo = 0;
      let hi = syncedLines.length - 1;
      while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        if ((syncedLines[mid].timeMs ?? 0) <= lyricPos) {
          activeIdx = mid;
          lo = mid + 1;
        } else {
          hi = mid - 1;
        }
      }

      const prevActive = lastActiveRef.current;
      if (activeIdx !== prevActive) {
        const prevEl = lineRefs.current[prevActive];
        if (prevEl) {
          prevEl.classList.remove("active");
          prevEl.style.color = "";
          prevEl.style.fontSize = "";
          prevEl.style.fontWeight = "";
          prevEl.style.opacity = "";
          prevEl.style.filter = "";
          prevEl.style.transform = "";
          clearLyricFill(prevEl);
        }
        const el = lineRefs.current[activeIdx];
        if (el) {
          el.classList.add("active");
          el.style.color = "var(--lyric-unsung, var(--ink))";
          el.style.fontSize = "25px";
          el.style.fontWeight = "800";
          el.style.opacity = "1";
          el.style.filter = "none";
          el.style.transform = "scale(1)";
        }
        for (let i = 0; i < syncedLines.length; i++) {
          const r = lineRefs.current[i];
          if (!r || i === activeIdx) continue;
          const dist = Math.abs(i - activeIdx);
          r.style.opacity = String(Math.max(0.5, 0.78 - dist * 0.06));
          r.style.transform = `scale(${Math.max(0.94, 1 - dist * 0.015)})`;
          r.style.color =
            i === activeIdx + 1 ? "var(--lyric-next, var(--ink-2))" : "";
        }
        const cur = lineRefs.current[activeIdx];
        const container = scrollRef.current;
        if (cur && container) {
          const top =
            cur.offsetTop - container.clientHeight / 2 + cur.clientHeight / 2;
          container.scrollTo({ top, behavior: "smooth" });
        }
        lastActiveRef.current = activeIdx;
      }

      const el = lineRefs.current[activeIdx];
      if (el) {
        const line = syncedLines[activeIdx];
        const start = line.timeMs ?? 0;
        const end =
          activeIdx + 1 < syncedLines.length
            ? (syncedLines[activeIdx + 1].timeMs ?? start + 5000)
            : start + 5000;
        const fill = lyricLineProgress(line, lyricPos, end);
        applyLyricFill(el, fill);
      }
    };
    renderFrameRef.current = tick;
    if (!playing) return;
    let raf = 0;
    const loop = () => {
      raf = requestAnimationFrame(loop);
      tick();
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [syncedLines, playing]);

  useEffect(() => {
    lastActiveRef.current = -1;
    for (const r of lineRefs.current) {
      if (!r) continue;
      r.classList.remove("active");
      r.style.color = "";
      r.style.fontSize = "";
      r.style.fontWeight = "";
      r.style.opacity = "";
      r.style.filter = "";
      r.style.transform = "";
      clearLyricFill(r);
    }
  }, [syncedLines]);

  const rawCover = current?.cover ?? "";
  const isRemote = rawCover.startsWith("http://") || rawCover.startsWith("https://");
  const [remoteLocal, setRemoteLocal] = useState<string | null>(null);
  useEffect(() => {
    setRemoteLocal(null);
    if (!isRemote || !rawCover) return;
    let alive = true;
    remoteCoverCache.get(rawCover)?.then((p) => {
      if (alive && p) setRemoteLocal(p);
      return;
    });
    if (!remoteCoverCache.has(rawCover)) {
      const pr = api.fetchCover(rawCover).catch(() => null);
      remoteCoverCache.set(rawCover, pr as Promise<string>);
      pr.then((path) => {
        if (alive && path) setRemoteLocal(path);
      });
    }
    return () => {
      alive = false;
    };
  }, [rawCover, isRemote]);

  const coverUrl = rawCover
    ? isRemote
      ? remoteLocal
        ? coverSrc(remoteLocal)
        : ""
      : coverSrc(rawCover)
    : "";
  const theme = useStore((s) => s.theme);

  const [palette, setPalette] = useState<string[]>([]);
  const adaptive = palette.length >= 2 ? lyricAdaptiveColors(palette, theme) : null;
  const lyricVars = (adaptive?.vars ?? {}) as React.CSSProperties;
  const scrim = adaptive?.scrim ?? "";

  useEffect(() => {
    if (!coverUrl) {
      setPalette([]);
      return;
    }
    let alive = true;
    const getPalette = async () => {
      try {
        let colors: string[] = [];
        if (/^https?:\/\/(?!asset\.)/.test(coverUrl)) {
          try {
            colors = await api.extractCoverPalette(coverUrl);
          } catch {
            colors = [];
          }
          if (colors.length < 2) colors = await extractPalette(coverUrl, 4);
        } else {
          colors = await extractPalette(coverUrl, 4);
        }
        if (alive) setPalette(colors);
      } catch {
        if (alive) setPalette([]);
      }
    };
    getPalette();
    return () => {
      alive = false;
    };
  }, [coverUrl]);

  if (!current) return null;

  return (
    <div
      className="absolute inset-0 z-40 anim-np overflow-hidden"
      style={{ background: "var(--bg)", ...(lyricVars as React.CSSProperties) }}
    >
      {/* 背景：封面取色的流动渐变 */}
      <div className="absolute inset-0 overflow-hidden">
        {palette.length < 2 && (
          <div
            className="absolute inset-0 transition-colors duration-1000"
            style={{
              background:
                "linear-gradient(180deg, var(--backdrop-1) 0%, var(--backdrop-2) 55%, var(--bg) 100%)",
            }}
          />
        )}
        {palette.length >= 2 && (
          <>
            <div
              className="absolute -inset-[25%] dynamic-gradient-layer"
              style={{
                background: `conic-gradient(from 0deg at 30% 35%, ${palette[0]}, ${palette[1] ?? palette[0]}, ${palette[2] ?? palette[0]}, ${palette[3] ?? palette[1] ?? palette[0]}, ${palette[0]})`,
                opacity: 0.55,
                filter: "blur(120px) saturate(1.35)",
                animation: "slowSpin 40s linear infinite",
              }}
            />
            <div
              className="absolute rounded-full"
              style={{
                background: `radial-gradient(circle at center, ${palette[0]} 0%, transparent 62%)`,
                width: "56%",
                height: "52%",
                left: "4%",
                top: "-8%",
                filter: "blur(85px)",
                animation: "blobDrift 17s ease-in-out infinite",
                ["--blob-a" as string]: 0.4,
              }}
            />
            <div
              className="absolute rounded-full"
              style={{
                background: `radial-gradient(circle at center, ${palette[1]} 0%, transparent 62%)`,
                width: "52%",
                height: "48%",
                left: "48%",
                top: "46%",
                filter: "blur(95px)",
                animation: "blobDrift 21s ease-in-out infinite",
                animationDelay: "-7s",
                ["--blob-a" as string]: 0.35,
              }}
            />
          </>
        )}
        <div
          className="absolute inset-0"
          style={{
            background:
              "radial-gradient(ellipse 130% 110% at 50% 45%, transparent 55%, var(--vignette) 100%)",
          }}
        />
      </div>

      {/* 顶栏 */}
      <div className="relative flex items-center justify-between px-8 pt-12 h-24">
        <span className="text-[11px] text-[var(--ink-3)] tracking-[0.26em] flex items-center gap-2.5">
          <Music4 size={14} />
          {tr("正在播放", "NOW PLAYING")}
        </span>
        {isLiquid ? (
          <div className="flex items-center gap-1.5 px-2 py-1 rounded-full liquid-dock shadow-lg">
            <button
              className="btn-ghost w-9 h-9 !rounded-full"
              onClick={() => toggleFullscreen()}
              title={fullscreen ? tr("退出全屏", "Exit Fullscreen") : tr("全屏播放（无边框）", "Fullscreen (Borderless)")}
            >
              {fullscreen ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
            </button>
            <button
              className="btn-ghost w-9 h-9 !rounded-full"
              onClick={() => {
                if (fullscreen) toggleFullscreen(false);
                setNowPlayingOpen(false);
              }}
              title={tr("收起播放页", "Collapse Now Playing")}
            >
              <ChevronDown size={19} />
            </button>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <button
              className="btn-ghost w-10 h-10 !rounded-full"
              onClick={() => toggleFullscreen()}
              title={fullscreen ? tr("退出全屏", "Exit Fullscreen") : tr("全屏播放（无边框）", "Fullscreen (Borderless)")}
            >
              {fullscreen ? <Minimize2 size={17} /> : <Maximize2 size={17} />}
            </button>
            <button
              className="btn-ghost w-10 h-10 !rounded-full"
              onClick={() => {
                if (fullscreen) toggleFullscreen(false);
                setNowPlayingOpen(false);
              }}
              title={tr("收起播放页", "Collapse Now Playing")}
            >
              <ChevronDown size={20} />
            </button>
          </div>
        )}
      </div>

      {/* 主体 */}
      <div className="relative flex gap-14 px-14 pb-8 items-stretch h-[calc(100%-96px)]">
        {/* 左：封面 */}
        <div className="w-[40%] max-w-[440px] flex flex-col items-center justify-center gap-5">
          <div className={`relative ${isLiquid ? "group" : ""}`}>
            <div
              className={`absolute -inset-10 rounded-[48px] ${
                isLiquid
                  ? "opacity-50 blur-3xl transition-all duration-1000"
                  : "opacity-40 blur-3xl transition-colors duration-1000"
              }`}
              style={{
                background: coverUrl
                  ? `url(${coverUrl}) center/cover`
                  : "linear-gradient(135deg, var(--accent), var(--accent-strong))",
              }}
            />
            <CoverImg
              src={current.cover}
              seed={current.title}
              className={
                isLiquid
                  ? "w-[min(38vh,360px)] h-[min(38vh,360px)] rounded-[32px] shadow-[var(--cover-shadow-lg)] relative ring-1 ring-white/30 transition-transform duration-500 group-hover:scale-[1.02]"
                  : "w-[min(36vh,340px)] h-[min(36vh,340px)] rounded-[28px] shadow-[var(--cover-shadow-lg)] relative"
              }
              iconSize={56}
            />
          </div>
          <div className="text-center max-w-full np-shadowed">
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
                  className="btn-ghost w-8 h-8 !rounded-full glass"
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
                  title={
                    savedOnline[`qq-${current.qid}`]
                      ? tr("取消喜欢", "Unlike")
                      : tr("收藏到“我喜欢”", "Add to Liked Songs")
                  }
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
              {current.kind === "bilibili" && current.qid != null && (
                <button
                  className="btn-ghost w-8 h-8 !rounded-full glass"
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
                  title={
                    savedOnline[`bilibili-${current.qid}`]
                      ? tr("取消喜欢", "Unlike")
                      : tr("收藏到“我喜欢”", "Add to Liked Songs")
                  }
                >
                  <Heart
                    size={16}
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
                  className="btn-ghost w-8 h-8 !rounded-full glass"
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
                  title={
                    savedOnline[`navidrome-${current.qid}`]
                      ? tr("取消喜欢", "Unlike")
                      : tr("收藏到“我喜欢”", "Add to Liked Songs")
                  }
                >
                  <Heart
                    size={16}
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
                  className="btn-ghost w-8 h-8 !rounded-full glass"
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
                  title={
                    savedOnline[`kugou-${current.kgid}`]
                      ? tr("取消喜欢", "Unlike")
                      : tr("收藏到“我喜欢”", "Add to Liked Songs")
                  }
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
              {current.album && (
                <span className="text-[12px] text-[var(--ink-3)] truncate max-w-[200px]">
                  {current.album}
                </span>
              )}
            </div>
            {/* 歌词来源 + 字体调缩放 / 译文开关 */}
            <div className="flex items-center justify-center gap-2 mt-3 text-[11px] text-[var(--ink-3)]">
              {lyricSource && (
                <span className="flex items-center gap-1 opacity-75">
                  <Mic2 size={11} />
                  {formatLyricSource(lyricSource)}
                </span>
              )}
              {lyrics?.synced && (
                <button
                  className={`px-1.5 py-0.5 rounded transition-colors ${
                    !hasTrans
                      ? "opacity-30 cursor-not-allowed text-[var(--ink-3)]"
                      : lyricsShowTrans
                        ? "text-[var(--accent-strong)] font-semibold"
                        : "opacity-75 hover:opacity-100"
                  }`}
                  disabled={!hasTrans}
                  onClick={() => setLyricsShowTrans(!lyricsShowTrans)}
                  title={
                    !hasTrans
                      ? tr("当前歌词无翻译", "No translation available")
                      : lyricsShowTrans
                        ? tr("隐藏歌词翻译", "Hide translation")
                        : tr("显示歌词翻译", "Show translation")
                  }
                >
                  {tr("译", "TR")}
                </button>
              )}
              <span className="opacity-40">|</span>
              <button
                className="px-1 py-0.5 rounded hover:text-[var(--ink)] transition-colors font-semibold"
                onClick={() => setLyricsFontScale(lyricsFontScale - 0.1)}
                title={tr("减小歌词字号", "Decrease lyric font size")}
              >
                A-
              </button>
              <button
                className="px-1 py-0.5 rounded hover:text-[var(--ink)] transition-colors font-semibold"
                onClick={() => setLyricsFontScale(lyricsFontScale + 0.1)}
                title={tr("增大歌词字号", "Increase lyric font size")}
              >
                A+
              </button>
            </div>
          </div>
        </div>

        {/* 右：歌词 */}
        <div
          className={
            isLiquid
              ? "flex-1 min-w-0 relative h-full flex flex-col justify-center px-4 overflow-hidden"
              : "flex-1 min-w-0 relative"
          }
        >
          {scrim && (
            <div
              aria-hidden
              className="absolute inset-0 pointer-events-none"
              style={{
                background: `radial-gradient(115% 95% at 50% 50%, ${scrim} 42%, transparent 96%)`,
              }}
            />
          )}
          <div
            ref={scrollRef}
            className="relative h-full overflow-y-auto py-[28%] lyrics-mask pr-3"
            style={{ scrollbarWidth: "none", zoom: lyricsFontScale }}
          >
            {lyricsLoading && (
              <div className="text-[var(--ink-3)] text-[13px] text-center pt-20">
                {tr("正在加载歌词…", "Loading lyrics…")}
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
                  : tr("暂无歌词", "No lyrics")}
              </div>
            )}
            {syncedLines.map((l, i) => {
              const text = l.text || "···";
              return (
                <div
                  key={i}
                  ref={(el) => {
                    lineRefs.current[i] = el;
                  }}
                  onClick={() => {
                    if (l.timeMs == null) return;
                    seek(l.timeMs);
                    renderFrameRef.current();
                  }}
                  className="lyric-line lyric-fill px-4 py-[9px] text-center cursor-pointer"
                >
                  {text}
                  {lyricsShowTrans && l.trans && (
                    <div className="text-[12.5px] leading-snug mt-0.5 opacity-80">
                      {l.trans}
                    </div>
                  )}
                  <div className="lyric-fill-ov" aria-hidden>
                    <span>{text}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
