import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Heart, Maximize2, Mic2, Minimize2 } from "lucide-react";
import { useStore } from "../store";
import { formatLyricSource, useI18n } from "../i18n";
import CoverImg from "./CoverImg";
import VUMeter from "./VUMeter";
import Tonearm from "./Tonearm";
import LyricMatchModal from "./LyricMatchModal";
import { api, coverSrc } from "../api";
import {
  applyLyricFill,
  clearLyricFill,
  extractPalette,
  lyricLineProgress,
  remoteCoverCache,
} from "../utils";
import { setFxWanted } from "../fx";

export default function NowPlaying() {
  const { tr } = useI18n();
  const current = useStore((s) => s.current);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const fullscreen = useStore((s) => s.fullscreen);
  const toggleFullscreen = useStore((s) => s.toggleFullscreen);
  const toggleLike = useStore((s) => s.toggleLike);
  const togglePlay = useStore((s) => s.togglePlay);
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
  const theme = useStore((s) => s.theme);
  const [matchOpen, setMatchOpen] = useState(false);

  // 挂载时开启后端音频频谱分析（驱动 VU 表和电平指示），卸载时关闭
  useEffect(() => {
    setFxWanted(true, "skeuo-vu");
    return () => {
      setFxWanted(false, "skeuo-vu");
    };
  }, []);

  // 歌词逐字染色脱离 React 重渲染
  const anchorRef = useRef({ pos: 0, at: performance.now() });
  useEffect(() => {
    const st0 = useStore.getState();
    anchorRef.current = { pos: st0.pos, at: performance.now() };
    let lastPos = st0.pos;
    let lastPlaying = st0.playing;
    let lastOffset = st0.lyricOffsetMs;
    return useStore.subscribe((s) => {
      if (s.pos !== lastPos || s.playing !== lastPlaying || s.lyricOffsetMs !== lastOffset) {
        anchorRef.current = { pos: s.pos, at: performance.now() };
        lastPos = s.pos;
        lastPlaying = s.playing;
        lastOffset = s.lyricOffsetMs;
        if (!s.playing) renderFrameRef.current();
      }
    });
  }, []);

  const scrollRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<(HTMLDivElement | null)[]>([]);
  const lastActiveRef = useRef(-1);
  const renderFrameRef = useRef<() => void>(() => {});

  // 歌词键：本地曲目 / 网易云 / QQ / 酷狗 / B站 / Navidrome
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

  // 逐帧驱动歌词渲染
  useEffect(() => {
    if (!syncedLines.length) return;
    const tick = () => {
      const a = anchorRef.current;
      const st = useStore.getState();
      const dur = st.dur;
      const speed = st.speed;
      const raw = a.pos + (st.playing ? (performance.now() - a.at) * speed : 0);
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
          el.style.fontSize = "24px";
          el.style.fontWeight = "800";
          el.style.opacity = "1";
          el.style.filter = "none";
          el.style.transform = "scale(1.02)";
        }
        for (let i = 0; i < syncedLines.length; i++) {
          const r = lineRefs.current[i];
          if (!r || i === activeIdx) continue;
          const dist = Math.abs(i - activeIdx);
          r.style.opacity = String(Math.max(0.42, 0.78 - dist * 0.08));
          r.style.transform = `scale(${Math.max(0.92, 1 - dist * 0.018)})`;
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
    tick();
    if (!playing) return;
    let raf = 0;
    const loop = () => {
      raf = requestAnimationFrame(loop);
      tick();
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [syncedLines, playing]);

  // 重置行样式
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

  // 封面与调色板
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

  const [palette, setPalette] = useState<string[]>([]);

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

  const isLiked =
    current.kind === "track" && current.id != null
      ? !!current.liked
      : current.kind === "netease" && current.nid != null
        ? !!(savedOnline[`netease-${current.nid}`] || neteaseLiked[current.nid])
        : current.kind === "qq" && current.qid != null
          ? !!savedOnline[`qq-${current.qid}`]
          : current.kind === "kugou" && current.kgid != null
            ? !!savedOnline[`kugou-${current.kgid}`]
            : current.kind === "bilibili" && current.qid != null
              ? !!savedOnline[`bilibili-${current.qid}`]
              : current.kind === "navidrome" && current.qid != null
                ? !!savedOnline[`navidrome-${current.qid}`]
                : false;

  const handleToggleLike = () => {
    if (current.kind === "track" && current.id != null) {
      toggleLike(current.id);
    } else if (current.kind === "netease" && current.nid != null) {
      toggleLikeOnline({
        kind: "netease",
        id: current.nid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: "",
        vip: false,
      });
    } else if (current.kind === "qq" && current.qid != null) {
      const entry = [...likedOnline, ...recentOnline].find(
        (e) => e.kind === "qq" && e.onlineId === current.qid
      );
      toggleLikeOnline({
        kind: "qq",
        id: current.qid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: entry?.mediaMid ?? "",
        vip: entry?.vip ?? false,
      });
    } else if (current.kind === "kugou" && current.kgid != null) {
      const entry = [...likedOnline, ...recentOnline].find(
        (e) => e.kind === "kugou" && e.onlineId === current.kgid
      );
      toggleLikeOnline({
        kind: "kugou",
        id: current.kgid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: "",
        vip: entry?.vip ?? false,
      });
    } else if (current.kind === "bilibili" && current.qid != null) {
      toggleLikeOnline({
        kind: "bilibili",
        id: current.qid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: "",
        vip: false,
      });
    } else if (current.kind === "navidrome" && current.qid != null) {
      toggleLikeOnline({
        kind: "navidrome",
        id: current.qid,
        name: current.title,
        artist: current.artist,
        album: current.album,
        cover: current.cover,
        durationMs: current.durationMs,
        mediaMid: "",
        vip: false,
      });
    }
  };

  const platformBadge =
    current.kind === "netease"
      ? tr("网易云音乐", "NetEase Cloud Music")
      : current.kind === "qq"
        ? tr("QQ音乐", "QQ Music")
        : current.kind === "kugou"
          ? tr("酷狗音乐", "KuGou Music")
          : current.kind === "bilibili"
            ? tr("B站音频", "Bilibili Audio")
            : current.kind === "navidrome"
              ? "Navidrome"
              : tr("本地音源", "Local Audio");

  return (
    <div
      className="absolute inset-0 z-40 anim-np overflow-hidden select-none flex flex-col"
      style={{ background: "var(--bg)" }}
    >
      {/* 拟物机架拉丝金属底板 */}
      <div className="absolute inset-0 skeuo-chassis-bg pointer-events-none" />

      {/* 胆机/仪表发光氛围衬底：由封面主色温润晕染 */}
      {palette.length >= 2 && (
        <div
          className="absolute inset-0 pointer-events-none transition-opacity duration-1000"
          style={{
            background: `radial-gradient(ellipse 80% 65% at 32% 45%, ${palette[0]}24 0%, ${palette[1]}12 48%, transparent 78%)`,
            opacity: theme === "light" ? 0.25 : 0.58,
          }}
        />
      )}

      {/* 顶栏：与下方双舱和底栏严格同宽同边距（px-6），pt-10 避让系统标题栏 */}
      <div className="relative flex items-center justify-between max-w-[1440px] mx-auto w-full px-6 pt-10 pb-3 shrink-0 z-20">
        {/* 左侧：物理运行状态灯 */}
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-3 py-1 rounded-lg bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] shadow-[var(--sk-groove-shadow)]">
            <div className={playing ? "skeuo-led-on" : "skeuo-led-off"} />
            <span className="text-[10px] font-black tracking-[0.2em] text-[var(--accent-strong)] uppercase">
              {playing ? "PHONO · 33⅓ RPM" : "DECK STANDBY"}
            </span>
          </div>
          <span className="text-[10.5px] font-mono font-semibold text-[var(--ink-3)] hidden md:inline-block tracking-[0.18em]">
            DIRECT DRIVE REFERENCE
          </span>
        </div>

        {/* 右侧：金属微倒角全屏与收起键 */}
        <div className="flex items-center gap-2">
          <button
            className="btn-ghost !w-8 !h-8 !rounded-lg"
            onClick={() => toggleFullscreen()}
            title={fullscreen ? tr("退出全屏", "Exit Fullscreen") : tr("全屏播放（无边框）", "Fullscreen (Borderless)")}
          >
            {fullscreen ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
          </button>
          <button
            className="btn-ghost !w-8 !h-8 !rounded-lg"
            onClick={() => {
              if (fullscreen) toggleFullscreen(false);
              setNowPlayingOpen(false);
            }}
            title={tr("收起播放页", "Collapse Now Playing")}
          >
            <ChevronDown size={18} />
          </button>
        </div>
      </div>

      {/* 主体机台：左右双舱严格等高对齐，外边距 px-6 与底栏 PlayerBar 严丝合缝 */}
      <div className="relative flex-1 min-h-0 flex items-center justify-center gap-6 xl:gap-8 px-6 pt-1 pb-[106px] z-10 max-w-[1440px] mx-auto w-full">
        {/* 左舱：直驱黑胶唱机工作台（与右舱严格等高） */}
        <div className="w-[43%] min-w-[350px] max-w-[490px] h-full max-h-[600px] flex flex-col shrink-0">
          <div className="skeuo-turntable-deck rounded-2xl p-5 sm:p-6 relative w-full h-full flex flex-col items-center justify-between">
            {/* 唱机台面：居中舒展，转盘 + 精密 S 型唱臂统一比例坐标系 */}
            <div className="relative flex-1 min-h-0 w-full flex items-center justify-center">
              <div className="relative w-full max-w-[420px] aspect-[1.22/1] max-h-full flex items-center justify-start pl-2 sm:pl-4">
                {/* 沉入式 CNC 铝制转盘凹槽 */}
                <div className="relative h-[94%] aspect-square rounded-full skeuo-turntable-well flex items-center justify-center shrink-0">
                  {/* 车削铝转盘外环 + 频闪测速环 */}
                  <div className="relative w-[calc(100%-10px)] h-[calc(100%-10px)] rounded-full skeuo-platter-rim flex items-center justify-center">
                    <div className="absolute inset-[2px] rounded-full skeuo-strobe-rim pointer-events-none opacity-65" />

                    {/* 等比匀速旋转黑胶唱片（暂停原地停驻，绝不缩放变形） */}
                    <div
                      className={`relative w-[calc(100%-14px)] h-[calc(100%-14px)] rounded-full skeuo-vinyl-disc skeuo-vinyl-spin flex items-center justify-center ${
                        playing ? "is-playing" : ""
                      }`}
                    >
                      {/* 黑胶双色各向异性蝴蝶领结高光 */}
                      <div
                        className="absolute inset-0 rounded-full pointer-events-none"
                        style={{
                          background:
                            "conic-gradient(from 38deg at 50% 50%, rgba(255,244,224,0.2) 0deg, rgba(255,255,255,0.03) 26deg, transparent 48deg, transparent 148deg, rgba(210,230,255,0.04) 168deg, rgba(255,250,240,0.2) 182deg, rgba(255,255,255,0.03) 208deg, transparent 230deg, transparent 330deg, rgba(255,255,255,0.03) 348deg, rgba(255,244,224,0.2) 360deg)",
                        }}
                      />

                      {/* 饱满清晰的圆形专辑封面中心纸标 */}
                      <div className="relative w-[43%] h-[43%] rounded-full overflow-hidden border-2 border-white/20 shadow-[0_0_0_2px_rgba(0,0,0,0.78),inset_0_2px_6px_rgba(0,0,0,0.85)]">
                        <CoverImg
                          src={current.cover}
                          seed={current.title}
                          className="w-full h-full object-cover"
                          iconSize={28}
                        />
                        <div className="absolute inset-0 bg-gradient-to-br from-white/10 via-transparent to-black/25 pointer-events-none" />
                      </div>

                      {/* 精致金属主轴心针 */}
                      <div className="absolute w-3.5 h-3.5 rounded-full skeuo-record-clamp flex items-center justify-center z-10 pointer-events-none">
                        <div className="w-1 h-1 rounded-full bg-[#111]" />
                      </div>
                    </div>
                  </div>
                </div>

                {/* 精密 S 型唱臂：百分比锚定在唱机台面右侧 */}
                <Tonearm
                  playing={playing}
                  className="absolute top-[1%] right-[0%] w-[29%] h-[86%] z-20"
                />

                {/* 台面右下角：真实可按的物理 START / STOP 启停微动键 */}
                <button
                  type="button"
                  onClick={() => togglePlay()}
                  title={playing ? tr("暂停", "Pause") : tr("播放", "Play")}
                  className="btn-ghost absolute bottom-1 right-1 z-20 !px-2.5 !py-1 !rounded-md flex items-center gap-1.5 text-[9px] font-mono font-bold tracking-[0.14em]"
                >
                  <span className={playing ? "skeuo-led-on !w-1.5 !h-1.5" : "skeuo-led-off !w-1.5 !h-1.5"} />
                  {playing ? "STOP" : "START"}
                </button>
              </div>
            </div>

            {/* 歌曲主信息与精简硬件控制条 */}
            <div className="w-full text-center mt-3 pt-3.5 border-t border-[var(--sk-divider)] px-1 flex flex-col items-center shrink-0">
              <h1 className="text-[19px] xl:text-[21px] font-black text-[var(--ink)] tracking-tight truncate max-w-full leading-tight">
                {current.title}
              </h1>
              <div className="text-[12.5px] text-[var(--ink-2)] font-semibold mt-1 truncate max-w-full">
                {current.artist}
              </div>

              <div className="flex items-center gap-2 mt-3 px-3 py-1.5 rounded-xl skeuo-well">
                <button
                  className="btn-ghost !w-7 !h-7 !rounded-lg relative flex items-center justify-center shrink-0"
                  onClick={handleToggleLike}
                  title={isLiked ? tr("取消收藏", "Unlike") : tr("收藏至“我喜欢”", "Add to Liked Songs")}
                >
                  <Heart
                    size={14}
                    className={
                      isLiked
                        ? "fill-[#ef4444] text-[#ef4444]"
                        : "text-[var(--ink-3)]"
                    }
                  />
                  {isLiked && (
                    <span className="skeuo-led-red absolute -top-0.5 -right-0.5 !w-1.5 !h-1.5" />
                  )}
                </button>

                <span className="text-[10.5px] font-bold text-[var(--ink-2)] px-2.5 py-0.5 rounded-md bg-[var(--shade-strong)] border border-[var(--sk-edge)] max-w-[155px] truncate">
                  {current.album ? `${platformBadge} · ${current.album}` : platformBadge}
                </span>

                <div className="w-px h-3.5 bg-[var(--sk-divider)]" />

                <button
                  disabled={!hasTrans}
                  className={`px-2 py-0.5 rounded-md text-[11px] font-bold transition-all flex items-center gap-1 ${
                    !hasTrans
                      ? "opacity-35 cursor-not-allowed text-[var(--ink-3)]"
                      : lyricsShowTrans
                        ? "bg-[var(--accent)] text-white shadow-sm"
                        : "btn-ghost"
                  }`}
                  onClick={() => setLyricsShowTrans(!lyricsShowTrans)}
                  title={
                    hasTrans
                      ? tr("显示 / 隐藏歌词翻译", "Show / Hide Lyrics Translation")
                      : tr("当前曲目无翻译", "No translation available")
                  }
                >
                  {hasTrans && (
                    <span
                      className={`!w-1.5 !h-1.5 ${
                        lyricsShowTrans ? "skeuo-led-on" : "skeuo-led-off"
                      }`}
                    />
                  )}
                  {tr("译", "TR")}
                </button>

                <div className="w-px h-3.5 bg-[var(--sk-divider)]" />

                <div className="flex items-center gap-1">
                  <button
                    className="btn-ghost !w-6 !h-6 !rounded-md text-[10.5px] font-black"
                    onClick={() => setLyricsFontScale(Math.max(0.8, lyricsFontScale - 0.1))}
                    title={tr("减小歌词字号", "Decrease lyrics font size")}
                  >
                    A-
                  </button>
                  <button
                    className="btn-ghost !w-6 !h-6 !rounded-md text-[10.5px] font-black"
                    onClick={() => setLyricsFontScale(Math.min(1.5, lyricsFontScale + 0.1))}
                    title={tr("增大歌词字号", "Increase lyrics font size")}
                  >
                    A+
                  </button>
                </div>
                {current.kind === "track" && current.id != null && (
                  <>
                    <div className="w-px h-3.5 bg-[var(--sk-divider)]" />
                    <button
                      className="btn-ghost !px-2 !h-6 !rounded-md text-[10.5px] font-bold shrink-0"
                      onClick={() => setMatchOpen(true)}
                      title={tr("手动匹配、更换或导出本地歌词", "Match, replace, or export local lyrics")}
                    >
                      {tr("匹配", "Match")}
                    </button>
                  </>
                )}
              </div>

              {lyricSource && (
                <div className="text-[9.5px] font-mono text-[var(--ink-3)] tracking-wider mt-2 opacity-70">
                  {tr("歌词来源：", "LYRICS SOURCE: ")}{formatLyricSource(lyricSource)}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* 右舱：精密声学监听歌词舱（与左舱严格等高） */}
        <div className="flex-1 min-w-0 h-full max-h-[600px] flex flex-col skeuo-acoustic-bay rounded-2xl p-4 sm:p-5 relative">
          {/* 顶舱：宽幅双通道模拟 VU 动圈电平表 */}
          <div className="shrink-0 mb-4 z-10">
            <VUMeter playing={playing} />
          </div>

          {/* 底舱：通透光学玻璃声学歌词视窗 */}
          <div className="relative flex-1 min-h-0 w-full rounded-xl overflow-hidden skeuo-lyric-viewport p-1">
            <div
              ref={scrollRef}
              className="relative h-full overflow-y-auto py-[22%] lyrics-mask px-4 text-center"
              style={{
                scrollbarWidth: "none",
                msOverflowStyle: "none",
                zoom: lyricsFontScale,
              }}
            >
              {lyricsLoading && (
                <div className="text-[var(--ink-3)] text-[13px] text-center pt-24 tracking-wider flex flex-col items-center gap-3">
                  <div className="w-6 h-6 border-2 border-[var(--accent)] border-t-transparent rounded-full animate-spin" />
                  {tr("正在校准声学歌词母带…", "Calibrating acoustic lyric stream…")}
                </div>
              )}

              {!lyricsLoading && syncedLines.length === 0 && (
                <div className="flex flex-col items-center gap-3.5 text-[var(--ink-3)] pt-[24%] select-none">
                  <div className="w-12 h-12 rounded-full bg-[var(--shade-strong)] border border-[var(--sk-groove-border)] flex items-center justify-center shadow-[var(--sk-groove-shadow)]">
                    <Mic2 size={22} className="text-[var(--ink-2)]" />
                  </div>
                  <div className="text-[13.5px] font-medium tracking-wide">
                    {lyrics && lyrics.lines.length
                      ? lyrics.lines.map((l, i) => (
                          <p
                            key={i}
                            className="text-[14px] text-[var(--ink-2)] text-center leading-relaxed"
                          >
                            {l.text}
                          </p>
                        ))
                      : tr("暂无同步歌词", "No synced lyrics")}
                  </div>
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
                    className="lyric-line lyric-fill px-4 py-[9px] text-center cursor-pointer select-none transition-all duration-200"
                  >
                    <span className="inline-block transition-transform duration-300">
                      {text}
                    </span>
                    {lyricsShowTrans && l.trans && (
                      <div className="text-[12.5px] leading-snug mt-1 opacity-70 font-normal tracking-normal">
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
      {current.kind === "track" && current.id != null && (
        <LyricMatchModal
          open={matchOpen}
          onClose={() => setMatchOpen(false)}
          trackId={current.id}
          title={current.title}
          artist={current.artist}
          durationMs={current.durationMs}
        />
      )}
    </div>
  );
}
