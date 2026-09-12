import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Heart, Maximize2, Mic2, Minimize2, Music4 } from "lucide-react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";
import { api, coverSrc } from "../api";
import { extractPalette, lyricLineProgress } from "../utils";

export default function NowPlaying() {
  const current = useStore((s) => s.current);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const fullscreen = useStore((s) => s.fullscreen);
  const toggleFullscreen = useStore((s) => s.toggleFullscreen);
  const toggleLike = useStore((s) => s.toggleLike);
  const lyrics = useStore((s) => s.lyrics);
  const lyricsLoading = useStore((s) => s.lyricsLoading);
  const loadLyricsByKey = useStore((s) => s.loadLyricsByKey);
  const seek = useStore((s) => s.seek);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const neteaseToggleLike = useStore((s) => s.neteaseToggleLike);
  const savedOnline = useStore((s) => s.savedOnline);
  const likedOnline = useStore((s) => s.likedOnline);
  const recentOnline = useStore((s) => s.recentOnline);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const playing = useStore((s) => s.playing);

  // 歌词逐字染色完全脱离 React 渲染（同桌面歌词）：
  // - 不订阅 s.pos（250ms 一次会让整页重渲染，挤压歌词渲染帧预算），
  //   锚点在 store 订阅回调里写 ref；
  // - 行样式/染色边界全部由 rAF 直接写 DOM，循环只在播放中运行。
  const anchorRef = useRef({ pos: 0, at: performance.now() });
  useEffect(() => {
    // 挂载即对齐当前进度（播放中打开播放页不用等下一个 pos 事件）
    const st0 = useStore.getState();
    anchorRef.current = { pos: st0.pos, at: performance.now() };
    // 只有 pos/playing 真正变化才移动锚点：订阅整个 store 的话，
    // toast/下载进度等无关 set 也会重置锚点时间戳，外推被反复拉回，
    // 染色边界出现回跳抖动
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

  // 逐帧驱动：外推进度 → 定位当前行 → 行样式直接写 DOM。
  // 播放中才跑循环（暂停时染色冻结，不空转——同桌面歌词）
  useEffect(() => {
    if (!playing || !syncedLines.length) return;
    let raf = 0;
    const tick = () => {
      raf = requestAnimationFrame(tick);
      const a = anchorRef.current;
      const posNow = a.pos + (performance.now() - a.at);

      // 当前行二分（行数据是排好序的）
      let activeIdx = -1;
      let lo = 0;
      let hi = syncedLines.length - 1;
      while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        if ((syncedLines[mid].timeMs ?? 0) <= posNow) {
          activeIdx = mid;
          lo = mid + 1;
        } else {
          hi = mid - 1;
        }
      }

      const prevActive = lastActiveRef.current;
      if (activeIdx !== prevActive) {
        // 行切换：旧当前行还原成普通行样式（染色层随 .active 移除而隐藏）
        const prevEl = lineRefs.current[prevActive];
        if (prevEl) {
          prevEl.classList.remove("active");
          prevEl.style.color = "";
          prevEl.style.fontSize = "";
          prevEl.style.fontWeight = "";
          prevEl.style.opacity = "";
          prevEl.style.filter = "";
          prevEl.style.transform = "";
          prevEl.style.removeProperty("--fill");
        }
        // 新当前行升级样式（will-change 合成层由 .active 类自动挂上）
        const el = lineRefs.current[activeIdx];
        if (el) {
          el.classList.add("active");
          // 当前行文字色 = 「未唱」自定义色（空时回落主题正文色，CSS 变量即时生效）
          el.style.color = "var(--lyric-unsung, var(--ink))";
          el.style.fontSize = "25px";
          el.style.fontWeight = "800";
          el.style.opacity = "1";
          el.style.filter = "none";
          el.style.transform = "scale(1)";
        }
        // 周边行透明度/缩放（只在新行附近的几行，代价小）；下一句行用「下一句」色
        for (let i = 0; i < syncedLines.length; i++) {
          const r = lineRefs.current[i];
          if (!r || i === activeIdx) continue;
          const dist = Math.abs(i - activeIdx);
          r.style.opacity = String(Math.max(0.4, 0.75 - dist * 0.07));
          r.style.transform = `scale(${Math.max(0.94, 1 - dist * 0.015)})`;
          r.style.color =
            i === activeIdx + 1 ? "var(--lyric-next, var(--ink-2))" : "";
        }
        // 滚动到中心（只在行切换时触发一次，不逐帧滚动）
        const cur = lineRefs.current[activeIdx];
        const container = scrollRef.current;
        if (cur && container) {
          const top =
            cur.offsetTop - container.clientHeight / 2 + cur.clientHeight / 2;
          container.scrollTo({ top, behavior: "smooth" });
        }
        lastActiveRef.current = activeIdx;
      }

      // 当前行染色边界：与桌面歌词同款插值（逐字时间戳精确贴合演唱）
      const el = lineRefs.current[activeIdx];
      if (el) {
        const line = syncedLines[activeIdx];
        const start = line.timeMs ?? 0;
        const end =
          activeIdx + 1 < syncedLines.length
            ? (syncedLines[activeIdx + 1].timeMs ?? start + 5000)
            : start + 5000;
        const fill = lyricLineProgress(line, posNow, end);
        el.style.setProperty("--fill", `${(fill * 100).toFixed(2)}%`);
      }
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [syncedLines, playing]);

  // 换曲 / 歌词重载：重置行内残留样式（rAF 循环会重建新状态）
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
      r.style.removeProperty("--fill");
    }
  }, [syncedLines]);

  const coverUrl = current ? (current.cover ? coverSrc(current.cover) : "") : "";

  // 封面取色 → 动态渐变背景
  // 远程 http(s) 封面一律走后端提取（QQ 的 y.gtimg.cn 无 CORS，前端 canvas 会被污染）；
  // asset:// 本地封面是 WebView 虚拟主机，后端 ureq 连不上，只能前端采样
  const [palette, setPalette] = useState<string[]>([]);
  useEffect(() => {
    if (!coverUrl) {
      setPalette([]);
      return;
    }
    let alive = true;
    const getPalette = async () => {
      try {
        const colors = /^https?:\/\/(?!asset\.)/.test(coverUrl)
          ? await api.extractCoverPalette(coverUrl)
          : await extractPalette(coverUrl, 4);
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

  // 注意：hooks 必须在条件 return 之前调用（React Hooks 规则）
  // 背景流动动画全走 CSS @keyframes（slowSpin 纯 transform，合成器执行）：
  // 此前用 rAF 每帧写 style.filter（120px blur 常驻 + 每帧重设），
  // 是播放页歌词掉帧的元凶之一
  if (!current) return null;

  return (
    <div className="absolute inset-0 z-40 anim-np overflow-hidden" style={{ background: "var(--bg)" }}>
      {/* 背景：封面取色的流动渐变（色相旋转 + 光斑漂移） */}
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
            {/* 大对流渐变层：缓慢旋转（CSS keyframes 纯 transform，合成器执行；
                不再用 rAF 每帧写 filter——那会持续重光栅化 120px blur，
                挤压歌词渲染的帧预算） */}
            <div
              className="absolute -inset-[25%] dynamic-gradient-layer"
              style={{
                background: `conic-gradient(from 0deg at 30% 35%, ${palette[0]}, ${palette[1] ?? palette[0]}, ${palette[2] ?? palette[0]}, ${palette[3] ?? palette[1] ?? palette[0]}, ${palette[0]})`,
                opacity: 0.55,
                filter: "blur(120px) saturate(1.35)",
                animation: "slowSpin 40s linear infinite",
              }}
            />
            {/* 双光斑漂移层 */}
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

      {/* 顶栏（标题栏浮在渐变之上；全屏时窗口按钮被隐藏，提供自身控制） */}
      <div className="relative flex items-center justify-between px-8 pt-12 h-24">
        <span className="text-[11px] text-[var(--ink-3)] tracking-[0.26em] flex items-center gap-2.5">
          <Music4 size={14} />
          正在播放
        </span>
        <div className="flex items-center gap-2">
          <button
            className="btn-ghost w-10 h-10 !rounded-full"
            onClick={() => toggleFullscreen()}
            title={fullscreen ? "退出全屏" : "全屏播放（无边框）"}
          >
            {fullscreen ? <Minimize2 size={17} /> : <Maximize2 size={17} />}
          </button>
          <button
            className="btn-ghost w-10 h-10 !rounded-full"
            onClick={() => {
              if (fullscreen) toggleFullscreen(false);
              setNowPlayingOpen(false);
            }}
            title="收起播放页"
          >
            <ChevronDown size={20} />
          </button>
        </div>
      </div>

      {/* 主体 */}
      <div className="relative flex gap-14 px-14 pb-8 items-stretch h-[calc(100%-96px)]">
        {/* 左：封面 */}
        <div className="w-[40%] max-w-[440px] flex flex-col items-center justify-center gap-5">
          <div className="relative">
            <div
              className="absolute -inset-10 rounded-[48px] opacity-40 blur-3xl transition-colors duration-1000"
              style={{
                background: coverUrl
                  ? `url(${coverUrl}) center/cover`
                  : "linear-gradient(135deg, var(--accent), var(--accent-strong))",
              }}
            />
            <CoverImg
              src={current.cover}
              seed={current.title}
              className="w-[min(36vh,340px)] h-[min(36vh,340px)] rounded-[28px] shadow-[var(--cover-shadow-lg)] relative"
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
              {current.kind === "track" && (
                <span className="text-[11px] text-[var(--ink-2)] px-2.5 py-0.5 rounded-full bg-[var(--shade)]">
                  {current.album || "未知专辑"}
                </span>
              )}
              {current.kind === "netease" && (
                <span className="text-[11px] text-[var(--accent)] px-2.5 py-0.5 rounded-full bg-[var(--accent-weak)]">
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
            {/* Apple Music 式歌词：当前行逐字点亮。染色边界由 rAF 循环写
                --fill 变量（clipPath 叠加层，与桌面歌词同机制），React 不参与
                逐帧渲染；active/透明度/缩放也是 rAF 直接写 style。
                染色层是真实 DOM 双层：外层铺满本行对齐布局，内层 inline-block
                收缩到文字实际宽度——clip 百分比基准是文字而非整行，
                居中行的逐字进度才不会被左右空白稀释（桌面歌词同构） */}
            {syncedLines.map((l, i) => {
              const text = l.text || "···";
              return (
                <div
                  key={i}
                  ref={(el) => {
                    lineRefs.current[i] = el;
                  }}
                  onClick={() => l.timeMs != null && seek(l.timeMs)}
                  className="lyric-line lyric-fill px-4 py-[9px] text-center cursor-pointer"
                >
                  {text}
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
