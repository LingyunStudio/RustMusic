import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Disc3, Heart, Maximize2, Mic2, Minimize2, Music4 } from "lucide-react";
import { useStore } from "../store";
import { formatLyricSource, useI18n } from "../i18n";
import CoverImg from "./CoverImg";
import VUMeter from "./VUMeter";
import Tonearm from "./Tonearm";
import { api, coverSrc } from "../api";
import {
  applyLyricFill,
  clearLyricFill,
  extractPalette,
  lyricAdaptiveColors,
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

  // 挂载时开启后端音频频谱分析（驱动 VU 表和电平指示），卸载时关闭
  useEffect(() => {
    setFxWanted(true);
    return () => {
      setFxWanted(false);
    };
  }, []);

  // 歌词逐字染色脱离 React 重渲染
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
  const adaptive = palette.length >= 2 ? lyricAdaptiveColors(palette, theme) : null;
  const lyricVars = (adaptive?.vars ?? {}) as React.CSSProperties;

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
        ? !!neteaseLiked[current.nid]
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
      neteaseToggleLike(current.nid);
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
      className="absolute inset-0 z-40 anim-np overflow-hidden select-none"
      style={{
        background: "var(--bg)",
        ...(lyricVars as React.CSSProperties),
      }}
    >
      {/* 拟物机架拉丝金属底板 */}
      <div className="absolute inset-0 skeuo-chassis-bg pointer-events-none" />

      {/* 胆机/仪表发光氛围衬底：由封面主色温润晕染，杜绝生硬大色块 */}
      {palette.length >= 2 && (
        <div
          className="absolute inset-0 pointer-events-none transition-opacity duration-1000"
          style={{
            background: `radial-gradient(ellipse 90% 70% at 50% 38%, ${palette[0]}2e 0%, ${palette[1]}16 55%, transparent 80%)`,
            opacity: theme === "light" ? 0.35 : 0.65,
          }}
        />
      )}

      {/* 物理机箱四角紧固车削螺丝 */}
      <div className="skeuo-screw absolute top-3.5 left-4 z-20 pointer-events-none" />
      <div className="skeuo-screw absolute top-3.5 right-4 z-20 pointer-events-none" />

      {/* 顶栏：发烧级硬件机架式状态栏 */}
      <div className="relative flex items-center justify-between px-8 pt-2.5 h-14 z-20 border-b border-[var(--sk-divider)] shadow-[inset_0_1px_0_var(--sk-divider-hi)]">
        {/* 左侧：物理指示灯与母带工作模式 */}
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-2.5 py-1 rounded-md bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] shadow-[var(--sk-groove-shadow)]">
            <div className={playing ? "skeuo-led-on" : "skeuo-led-off"} />
            <span className="text-[10px] font-black tracking-[0.2em] text-[var(--accent-strong)] uppercase">
              {playing ? "PHONO ACTIVE · 33⅓ RPM" : "DECK STANDBY"}
            </span>
          </div>
          <span className="text-[11px] font-mono font-semibold text-[var(--ink-3)] hidden md:inline-block tracking-wider">
            HI-FI DIRECT DRIVE MONITOR
          </span>
        </div>

        {/* 中间：机架金属名牌 */}
        <div className="skeuo-nameplate px-3.5 py-1 rounded-md hidden lg:flex items-center gap-2">
          <Disc3 size={13} className="text-[var(--accent-strong)]" />
          <span className="text-[10.5px] font-black tracking-[0.25em] text-[var(--ink)]">
            RUSTMUSIC REFERENCE SYSTEM
          </span>
        </div>

        {/* 右侧：金属微倒角窗口操作键 */}
        <div className="flex items-center gap-2">
          <button
            className="titlebar-btn !w-8 !h-7 !rounded-lg"
            onClick={() => toggleFullscreen()}
            title={fullscreen ? tr("退出全屏", "Exit Fullscreen") : tr("全屏播放（无边框）", "Fullscreen (Borderless)")}
          >
            {fullscreen ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
          </button>
          <button
            className="titlebar-btn !w-8 !h-7 !rounded-lg"
            onClick={() => {
              if (fullscreen) toggleFullscreen(false);
              setNowPlayingOpen(false);
            }}
            title={tr("收起播放页", "Collapse Now Playing")}
          >
            <ChevronDown size={17} />
          </button>
        </div>
      </div>

      {/* 主体机台：双舱布局（左：直驱黑胶唱机工作台；右：精密声学监听歌词舱） */}
      <div className="relative flex flex-col lg:flex-row gap-8 xl:gap-14 px-8 xl:px-14 pt-4 pb-28 items-center justify-center h-[calc(100%-56px)] z-10 max-w-[1580px] mx-auto w-full">
        {/* 左舱：直驱黑胶唱机工作台 */}
        <div className="w-full lg:w-[48%] max-w-[540px] flex flex-col items-center justify-center shrink-0">
          {/* 拟物机架唱机台基座 */}
          <div className="skeuo-turntable-deck rounded-2xl p-5 relative w-full flex flex-col items-center">
            {/* 机台四角固定螺丝 */}
            <div className="skeuo-screw absolute top-3 left-3" />
            <div className="skeuo-screw absolute top-3 right-3" />
            <div className="skeuo-screw absolute bottom-3 left-3" />
            <div className="skeuo-screw absolute bottom-3 right-3" />

            {/* 机台顶部铭牌与测速状态 */}
            <div className="w-full flex items-center justify-between text-[10px] font-black tracking-widest text-[var(--ink-3)] px-3 mb-3 border-b border-[var(--sk-divider)] pb-2">
              <span className="flex items-center gap-1.5 text-[var(--accent-strong)]">
                <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent)]" />
                PRECISION TURNTABLE
              </span>
              <span className="font-mono">MODEL PT-9800</span>
            </div>

            {/* 唱盘与唱臂交互核心区 */}
            <div className="relative flex items-center justify-center my-2 w-full">
              {/* 实体黑胶唱片封套（厚纸板装帧、哑光光泽与压痕） */}
              <div className="relative z-10 rounded-xl p-1.5 skeuo-jacket shrink-0 -mr-16 xl:-mr-20 transition-transform duration-500 hover:scale-[1.01]">
                <div className="relative rounded-lg overflow-hidden shadow-[inset_0_1px_1px_rgba(255,255,255,0.3)]">
                  <CoverImg
                    src={current.cover}
                    seed={current.title}
                    className="w-[min(30vh,250px)] h-[min(30vh,250px)] object-cover block"
                    iconSize={48}
                  />
                  {/* 封套哑光反光层 */}
                  <div
                    className="absolute inset-0 pointer-events-none"
                    style={{
                      background:
                        "linear-gradient(135deg, rgba(255,255,255,0.2) 0%, rgba(255,255,255,0.02) 40%, transparent 60%)",
                    }}
                  />
                  {/* 书脊折痕光影 */}
                  <div className="absolute top-0 bottom-0 left-2 w-px bg-white/20 pointer-events-none" />
                </div>
              </div>

              {/* 沉入式转盘凹槽（Turntable Well） */}
              <div className="relative w-[min(34vh,280px)] h-[min(34vh,280px)] rounded-full skeuo-turntable-well flex items-center justify-center shrink-0">
                {/* 频闪测速外圈（Strobe dots rim） */}
                <div className="absolute inset-1 rounded-full skeuo-strobe-rim pointer-events-none opacity-45" />

                {/* 旋转黑胶唱片 */}
                <div
                  className={`relative w-[min(31vh,255px)] h-[min(31vh,255px)] rounded-full shadow-[0_12px_32px_rgba(0,0,0,0.85)] flex items-center justify-center transition-transform ${
                    playing ? "animate-[slowSpin_22s_linear_infinite]" : ""
                  }`}
                  style={{
                    background:
                      "repeating-radial-gradient(circle at center, #090b0e 0px, #090b0e 1px, #14181f 2px, #090b0e 3px)",
                    border: "2px solid rgba(255, 255, 255, 0.08)",
                  }}
                >
                  {/* 黑胶各向异性蝴蝶领结高光 */}
                  <div
                    className="absolute inset-0 rounded-full pointer-events-none"
                    style={{
                      background:
                        "conic-gradient(from 40deg at 50% 50%, rgba(255,255,255,0.18) 0deg, rgba(255,255,255,0.02) 30deg, transparent 50deg, transparent 150deg, rgba(255,255,255,0.02) 170deg, rgba(255,255,255,0.18) 180deg, rgba(255,255,255,0.02) 210deg, transparent 230deg, transparent 330deg, rgba(255,255,255,0.02) 350deg, rgba(255,255,255,0.18) 360deg)",
                    }}
                  />

                  {/* 唱片中心纸标 */}
                  <div className="relative w-[86px] h-[86px] rounded-full overflow-hidden border-2 border-white/20 shadow-[inset_0_2px_4px_rgba(0,0,0,0.8)]">
                    <CoverImg
                      src={current.cover}
                      seed={current.title}
                      small
                      className="w-full h-full object-cover scale-110"
                      iconSize={20}
                    />
                    <div className="absolute inset-0 bg-black/20" />
                  </div>

                  {/* 拟物实心黄铜/铬合金唱片金属压镇（Record Clamp） */}
                  <div className="absolute w-10 h-10 rounded-full skeuo-record-clamp flex items-center justify-center z-10 pointer-events-none">
                    <div className="w-2.5 h-2.5 rounded-full bg-[#111] border border-white/60 shadow-[inset_0_1px_2px_#000]" />
                  </div>
                </div>

                {/* 动态唱臂与唱针系统（播时旋入黑胶，暂停时升起归位） */}
                <Tonearm
                  playing={playing}
                  className="absolute -top-7 -right-11 z-20"
                />
              </div>
            </div>

            {/* 歌曲主信息与硬件控制仓 */}
            <div className="w-full text-center mt-3 px-2 flex flex-col items-center">
              <h1 className="text-[21px] xl:text-[23px] font-black text-[var(--ink)] tracking-tight truncate max-w-full drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)] leading-tight">
                {current.title}
              </h1>
              <div className="text-[13px] text-[var(--ink-2)] font-semibold mt-1 truncate max-w-full">
                {current.artist}
              </div>

              {/* 拟物金属凹槽集成控制台 */}
              <div className="flex items-center gap-2.5 mt-3.5 px-3.5 py-1.5 rounded-xl skeuo-well">
                {/* 物理收藏按钮（带红宝石指示灯点亮） */}
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
                        : "text-[var(--ink-3)] group-hover:text-[var(--ink-2)]"
                    }
                  />
                  {isLiked && (
                    <span className="skeuo-led-on absolute -top-0.5 -right-0.5 !w-1.5 !h-1.5" />
                  )}
                </button>

                {/* 音源/专辑金属铭牌 */}
                <span className="text-[10.5px] font-bold text-[var(--ink-2)] px-2.5 py-0.5 rounded-md bg-[var(--shade-strong)] border border-[var(--sk-edge)] max-w-[150px] truncate">
                  {current.album ? `${platformBadge} · ${current.album}` : platformBadge}
                </span>

                <div className="w-px h-3.5 bg-[var(--sk-divider)]" />

                {/* 歌词翻译硬件切换键 */}
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

                {/* 字号步进按键 */}
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
              </div>

              {lyricSource && (
                <div className="text-[9.5px] font-mono text-[var(--ink-3)] tracking-wider mt-2 opacity-70">
                  {tr("歌词来源：", "LYRICS SOURCE: ")}{formatLyricSource(lyricSource)}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* 右舱：精密声学监听歌词舱（集成双通道模拟 VU 表与平滑母带歌词流） */}
        <div className="w-full lg:flex-1 min-w-0 h-[min(72vh,580px)] flex flex-col skeuo-display rounded-2xl p-4 sm:p-5 relative shadow-[0_20px_45px_rgba(0,0,0,0.65)]">
          {/* 四角机架沉头螺栓 */}
          <div className="skeuo-screw absolute top-3 left-3 z-20 pointer-events-none" />
          <div className="skeuo-screw absolute top-3 right-3 z-20 pointer-events-none" />
          <div className="skeuo-screw absolute bottom-3 left-3 z-20 pointer-events-none" />
          <div className="skeuo-screw absolute bottom-3 right-3 z-20 pointer-events-none" />

          {/* 顶舱：双通道模拟 VU 动圈电平表与音频状态 */}
          <div className="shrink-0 mb-3 px-1 z-10">
            <VUMeter playing={playing} />
          </div>

          {/* 底舱：沉浸声学歌词视窗（无滚动条，平滑逐字推进） */}
          <div className="relative flex-1 min-h-0 w-full rounded-xl overflow-hidden skeuo-well p-1">
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
    </div>
  );
}
