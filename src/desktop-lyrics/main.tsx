/**
 * 桌面歌词窗口入口（独立 HTML：desktop-lyrics.html）
 *
 * 形态：背景全透明（桌面只见文字）；鼠标移入时浮现半透明玻璃底 + 播放控制；
 * 全窗皆为拖拽热区（含歌词上方空白），右下角缩放；字号随窗口宽度等比缩放。
 * 暂停时进度冻结（不做外推），卡拉OK 染色停在原地。
 * 数据链路：主窗口 store emit "dlyrics://push"（250ms/帧 + 歌词加载完成）。
 * 播放控制：emit "media://control"（主窗口 store 处理，与托盘一致）。
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { LyricLine } from "../types";
import { estimateLyricWidth, lyricLineProgress } from "../utils";
import "../index.css";

interface PushPayload {
  /** 歌词行；缺省 = 与上一帧相同（主窗口只在歌词变化时携带，
   *  避免 250ms 一帧全量序列化逐字歌词） */
  lines?: LyricLine[] | null;
  synced: boolean;
  pos: number;
  dur: number;
  playing: boolean;
  /** 播放速度（外推用；缺省 = 1） */
  speed?: number;
  title: string;
  artist: string;
  /** 三色（缺省时用内置默认）：sung=已唱 unsung=未唱 next=下一句 */
  colors?: {
    sung: string;
    unsung: string;
    next: string;
  };
}

const DEFAULT_COLORS = {
  sung: "#ffc45e",
  unsung: "rgba(255,255,255,0.9)",
  next: "rgba(230,168,82,0.8)",
};

/* 入口即强制 body 透明：index.css 的 body{background:var(--bg)} 会盖掉
   desktop-lyrics.html 里的 transparent（后注入的 style 层叠优先） */
document.documentElement.style.background = "transparent";
document.body.style.background = "transparent";

/* Linux：WebKitGTK 的窗口透明在部分显卡/驱动（虚拟机 vmwgfx 等）上不可用，
   透明背景会渲染成整块灰/黑阴影。Linux 一律用实色底（悬浮栏形态），
   显示效果在任何机器上都确定；Windows 保持纯悬浮文字形态不变 */
const IS_LINUX = !/windows/i.test(navigator.userAgent);
// 虚拟机（后端已禁用 WebKit 合成）走透明悬浮形态：URL 带 transparent=1
const TRANSPARENT =
  !IS_LINUX || new URLSearchParams(location.search).get("transparent") === "1";

/** 置顶偏好持久化 key（歌词窗口与主窗口同源，localStorage 共享） */
const ALWAYS_TOP_KEY = "rustmusic.dlyrics.alwaysTop";
/** 调试钩子：localStorage 置 rustmusic.dlyrics.debugHover=1 时控制条常显
 *  （无头环境截屏验证布局用；正常用户不会设，无行为影响） */
const DEBUG_HOVER = (() => {
  try {
    return localStorage.getItem("rustmusic.dlyrics.debugHover") === "1";
  } catch {
    return false;
  }
})();

async function mediaControl(action: string) {
  const { emit } = await import("@tauri-apps/api/event");
  await emit("media://control", { action });
}

function DesktopLyrics() {
  const [data, setData] = useState<PushPayload>({
    lines: null,
    synced: false,
    pos: 0,
    dur: 0,
    playing: false,
    title: "",
    artist: "",
  });
  const [locked, setLocked] = useState(false);
  const [hover, setHover] = useState(false);
  // 置顶状态：以窗口真实状态为唯一事实源（后端创建时设了置顶，但
  // WM 对"隐藏后 re-show"的窗口可能丢弃 always-on-top——猜出来的
  // 初始值会和实际相反，按钮金色"已置顶"实际却在别的窗口下面）。
  // 启动 effect 里查询真实值并回写 localStorage
  const [alwaysTop, setAlwaysTop] = useState(true);
  // 窗口内尺寸（缩放时字号等比跟随：以宽度 800 为基准）
  const [winW, setWinW] = useState(800);
  const posRef = useRef(0);
  const posAtRef = useRef(performance.now());
  const [, setRenderTick] = useState(0);

  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    import("@tauri-apps/api/event").then(async ({ listen, emit }) => {
      if (disposed) return;
      return listen<PushPayload>("dlyrics://push", (e) => {
        posAtRef.current = performance.now();
        posRef.current = e.payload.pos;
        // 合并而非替换：lines 缺省时沿用上一帧（主窗口瘦身推送）
        setData((prev) => ({ ...prev, ...e.payload }));
      }).then((unregister) => {
        // 监听注册完成后再向主窗口握手：主窗口收到 ready 会立即补推一帧
        // 全量状态（含歌词）。此前是先 emit ready 再注册 listen——补推的帧
        // 可能赶在注册完成前到达而丢失，窗口整首歌卡在“纯音乐”
        void emit("dlyrics://ready");
        return unregister;
      });
    }).then((u) => {
      if (disposed) u?.();
      else un = u as unknown as (() => void) | undefined;
    });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  // 主窗口按 L 解锁：后端只恢复鼠标事件，不通知本窗口——锁定后 🔒 按钮自身
  // 不可点，这条事件是恢复 locked 状态（控制条/缩放手柄）的唯一途径。
  // 本窗口 toggleLock 发出的同名事件也会到达这里，值一致，无副作用
  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    import("@tauri-apps/api/event").then(({ listen }) => {
      if (disposed) return;
      return listen<{ locked: boolean }>("dlyrics://lock", (e) => {
        if (e.payload.locked) setHover(false);
        setLocked(e.payload.locked);
      });
    }).then((u) => {
      if (disposed) u?.();
      else un = u as unknown as (() => void) | undefined;
    });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  // 窗口尺寸监听（缩放 → 字号基准）。初始尺寸主动查一次：位置记忆恢复的
  // 窗口宽度未必是 800，等不到第一次 resize 事件的话字号基准会一直错
  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      if (disposed) return;
      const w = getCurrentWindow();
      try {
        const factor = await w.scaleFactor();
        const size = await w.innerSize();
        if (!disposed) setWinW(size.width / factor);
      } catch {
        /* 查询失败保持默认基准，resize 后自然纠正 */
      }
      return w.onResized(({ payload }) => {
        setWinW(payload.width / (window.devicePixelRatio || 1));
      });
    }).then((u) => {
      if (disposed) u?.();
      else un = u as unknown as (() => void) | undefined;
    });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  // hover 判定：mouseenter/mouseleave/mousemove 事件（跟随真实光标）+
  // 空闲兜底（3s 无任何鼠标活动即收起半透明底）。绝不读光标位置：
  // cursorPosition 取的是 X 服务器指针，XTest 合成输入会把该指针永久
  // 卡死在窗口内某点（真实鼠标是 Wayland 指针，影响不到它），轮询式
  // 判定会因此永远认为"光标在窗口内"——灰底永远不消失
  const lastMouseAt = useRef(0);
  useEffect(() => {
    const mark = () => {
      lastMouseAt.current = Date.now();
    };
    // 光标在窗口内停止移动时 X11 不再派发 mousemove，这里主动记一次
    // "光标曾在窗口内"，之后的收起判定只依赖"多久没有新活动"
    mark();
    window.addEventListener("mousemove", mark);
    window.addEventListener("mouseout", mark);
    const timer = setInterval(() => {
      // 800ms 无活动即收起：离开窗口后灰底很快消失；光标静止在窗口内
      // 会被 mouseout/Enter 交叉刷新，不会误收
      if (Date.now() - lastMouseAt.current > 800) setHover(false);
    }, 300);
    return () => {
      window.removeEventListener("mousemove", mark);
      window.removeEventListener("mouseout", mark);
      clearInterval(timer);
    };
  }, []);

  // 启动：以窗口真实置顶状态为准对齐 UI 与偏好记忆。
  // 显隐语义下窗口常驻，"隐藏再显示"后 WM 可能丢弃 always-on-top：
  // 真实为真 → 再断言一次（修复丢失）；真实为假 → 撤销创建时的硬编码
  // 置顶（否则按钮显示已置顶、窗口却在别窗口下面）
  useEffect(() => {
    let disposed = false;
    import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const w = getCurrentWindow();
        const real = await w.isAlwaysOnTop().catch(() => true);
        if (disposed) return;
        setAlwaysTop(real);
        try {
          localStorage.setItem(ALWAYS_TOP_KEY, real ? "1" : "0");
        } catch {}
        if (real) await w.setAlwaysOnTop(true).catch(() => {});
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, []);

  // rAF 插值循环：仅播放中运行（暂停时彻底停掉循环，渲染进程得以空闲，
  // Chromium 才有机会回收图片/渲染缓存；恢复播放由 data.playing 依赖触发重建）
  useEffect(() => {
    if (!data.playing) return;
    let raf = 0;
    let last = 0;
    const loop = (t: number) => {
      if (t - last > 50) {
        last = t;
        setRenderTick((n) => n + 1);
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [data.playing]);

  // 外推封顶在曲目时长：主窗口 WebView 挂起期间推送中断、恢复后锚点时间戳
  // 涵盖了整段挂起时长，不封顶的话进度会瞬间冲到很远（表现为“没声音但歌词狂飙”）
  // 外推按倍速缩放：2x 播放下按 1x 墙钟外推会让染色持续滞后并锯齿跳变
  const speed = data.speed ?? 1;
  const rawPos =
    data.pos + (data.playing ? (performance.now() - posAtRef.current) * speed : 0);
  const posNow = data.dur > 0 ? Math.min(rawPos, data.dur) : rawPos;

  // 有时间轴的行（与播放栏一致按时间排序：个别歌词源乱序，不排会跳行）。
  // memo 化：lines 引用在两次推送之间稳定，避免 20fps 渲染每帧重建数组
  const syncedLines = useMemo(
    () =>
      (data.lines ?? [])
        .filter((l) => l.timeMs != null && l.text.trim())
        .sort((a, b) => (a.timeMs ?? 0) - (b.timeMs ?? 0)),
    [data.lines]
  );
  let activeIdx = -1;
  if (data.synced && syncedLines.length) {
    for (let i = 0; i < syncedLines.length; i++) {
      if (syncedLines[i].timeMs! <= posNow) activeIdx = i;
      else break;
    }
  }
  const active = activeIdx >= 0 ? syncedLines[activeIdx] : null;
  const next =
    activeIdx >= 0 && activeIdx + 1 < syncedLines.length
      ? syncedLines[activeIdx + 1]
      : null;

  // 染色推进：逐字时间戳按实际演唱节奏；行级 LRC 按字宽加权
  const fillRatio = active
    ? lyricLineProgress(
        active,
        posNow,
        next ? next.timeMs! : Math.max(active.timeMs! + 8000, data.dur)
      )
    : 0;

  // 字号随窗口宽度等比缩放（800 为基准）；超宽行再按视觉宽度折减——
  // 塞不下的行硬裁会切掉半句词，宁可缩小完整显示
  const scale = Math.max(0.6, Math.min(2, winW / 800));
  const lineUnits = active ? estimateLyricWidth(active.text) : 0;
  // 一个"半角字宽"约 0.55*fontSize：窗口能容纳的视觉宽度单位数
  const maxUnits = Math.max(14, (winW - 48) / (34 * scale * 0.55));
  const fitScale = lineUnits > maxUnits ? Math.max(0.5, maxUnits / lineUnits) : 1;
  const mainFont = Math.round(34 * scale * fitScale);
  const nextFont = Math.round(15 * scale);
  // 三色（设置页可改，随 push 载荷实时更新）
  const colors = { ...DEFAULT_COLORS, ...(data.colors ?? {}) };

  // 无当前行可显示时的兜底：曲名·艺术家（前奏、纯音乐、无时间轴歌词），
  // 什么都没推过来才显示占位文案
  const headerText =
    data.title && data.artist
      ? `${data.title} · ${data.artist}`
      : data.title || data.artist;
  const idleText = data.playing ? "纯音乐，请欣赏" : "等待播放…";

  const close = async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const { emit } = await import("@tauri-apps/api/event");
    // 通知主窗口同步“词”按钮状态（主窗口可见时即时不等查询）
    await emit("dlyrics://closed", {});
    await invoke("desktop_lyrics_close");
  };
  const toggleLock = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const { emit } = await import("@tauri-apps/api/event");
    const next = !locked;
    await getCurrentWindow().setIgnoreCursorEvents(next);
    // 同步锁定状态给主窗口（“词”按钮提示/行为依赖）
    await emit("dlyrics://lock", { locked: next });
    // 锁定后鼠标穿透，mouseleave 不会再触发：必须同步清掉 hover，
    // 否则半透明底和控制条会永久残留
    if (next) setHover(false);
    setLocked(next);
  };
  const toggleTop = async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    const next = !alwaysTop;
    try {
      await w.setAlwaysOnTop(next);
    } catch {
      return; // 状态切换失败：保持原状态，不给假反馈
    }
    // 置顶偏好双写：localStorage 供本窗口 UI 快显，DB 供后端 re-show
    // 时断言（localStorage 在 WebKit 数据目录，DB 更可靠）
    try {
      localStorage.setItem(ALWAYS_TOP_KEY, next ? "1" : "0");
    } catch {}
    import("@tauri-apps/api/core")
      .then(({ invoke }) =>
        invoke("desktop_lyrics_set_always_top", { on: next })
      )
      .catch(() => {});
    setAlwaysTop(next);
    // 置顶切换触发 remap 后鼠标事件链可能短暂中断：3s 空闲兜底会收起
    // 灰底；光标真实移开的 mouseleave 也会正常触发
  };
  const startResize = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    void (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().startResizeDragging("SouthEast");
    })();
  };

  return (
    // 全窗拖拽热区（含歌词上方空白）；hover 才浮现半透明底与控制条。
    // 拖动走系统 WM 协议（data-tauri-drag-region）——原生、零 IPC 压力
    <div
      data-tauri-drag-region
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onMouseMove={() => setHover(true)}
      className="h-full w-full flex flex-col select-none relative"
      style={{
        background: TRANSPARENT
          ? "transparent"
          : hover
            ? "#241d15"
            : "#1a1510",
        borderRadius: TRANSPARENT ? 0 : 0,
        border: TRANSPARENT ? undefined : "1px solid rgba(255, 244, 220, 0.06)",
      }}
    >
      {/* 右下角缩放手柄 */}      {/* 歌词主体：hover 时只给"歌词内容"包一条紧凑玻璃底，静止时纯悬浮
          文字。控制条药丸 absolute 锚定在玻璃条正上方 8px——按钮贴着歌词，
          不再有顶部预留区的遥远距离；absolute 不占布局，歌词零跳动。
          pointer-events-none 让空白处点击穿透到根层 drag-region 拖动 */}
      {/* 底部锚定：桌面歌词常规形态。SE 角缩放拖的是底边/右边，内容
          钉在底边 → 拖动缩放时按钮与歌词纹丝不动（居中布局会整组跟着
          窗口高度重新居中，看起来像按钮在"缩放"）。
          不加 overflow-hidden：控制条药丸在布局流内，窗口边界自然裁剪 */}
      <div className="flex-1 min-h-0 flex flex-col justify-end pb-1.5 pointer-events-none">
        <div className="relative flex flex-col items-center self-center w-fit">
        {!locked && (hover || DEBUG_HOVER) && (
          <div
            data-tauri-drag-region
            className="mb-2 flex items-center gap-0.5 px-1.5 py-1 rounded-2xl pointer-events-auto z-10"
            style={{
              background: "rgba(20, 16, 12, 0.55)",
              border: "1px solid rgba(255,255,255,0.16)",
            }}
          >
          <button
            className="text-[15px] w-8 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={() => mediaControl("prev")}
            title="上一首"
          >
            ⏮
          </button>
          <button
            className="text-[15px] w-8 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: colors.sung }}
            onClick={() => mediaControl("toggle")}
            title={data.playing ? "暂停" : "播放"}
          >
            {data.playing ? "⏸" : "▶"}
          </button>
          <button
            className="text-[15px] w-8 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={() => mediaControl("next")}
            title="下一首"
          >
            ⏭
          </button>
          <span className="w-px h-4 mx-1" style={{ background: "rgba(255,255,255,0.2)" }} />
          <button
            className="text-[13px] w-7 h-7 rounded-lg flex items-center justify-center transition-colors"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={toggleLock}
            title="锁定：鼠标穿透歌词窗口（控制条随之隐藏），主窗口按 L 或托盘菜单解锁"
          >
            🔓
          </button>
          <button
            className="text-[13px] w-7 h-7 rounded-lg flex items-center justify-center transition-colors"
            style={
              alwaysTop
                ? { background: colors.sung, color: "#1a1206" }
                : { color: "rgba(255,255,255,0.85)" }
            }
            onClick={toggleTop}
            title={alwaysTop ? "已置顶（点击取消）" : "置顶（点击后窗口总在最前）"}
          >
            📌
          </button>
          <button
            className="text-[13px] w-7 h-7 rounded-lg hover:brightness-125 flex items-center justify-center"
            style={{ color: "rgba(255,255,255,0.85)" }}
            onClick={close}
            title="关闭"
          >
            ✕
          </button>
          </div>
        )}
        <div
          className="flex flex-col items-center gap-1 px-5 py-2.5 rounded-2xl pointer-events-none"
          style={{
            background: TRANSPARENT
              ? hover
                ? "rgba(14, 10, 6, 0.55)"
                : "transparent"
              : "transparent",
            border: TRANSPARENT && hover ? "1px solid rgba(255,255,255,0.14)" : "1px solid transparent",
          }}
        >
        {data.synced && active ? (
          <>
            <div
              className="relative max-w-full overflow-hidden"
              style={{ textShadow: "0 2px 14px rgba(0,0,0,0.55)" }}
            >
              <div
                className="whitespace-nowrap"
                style={{
                  fontSize: mainFont,
                  fontWeight: 800,
                  color: colors.unsung,
                  letterSpacing: "0.02em",
                  lineHeight: 1.25,
                }}
              >
                {active.text}
              </div>
              <div
                className="absolute inset-0"
                style={{ clipPath: `inset(0 ${100 - fillRatio * 100}% 0 0)` }}
                aria-hidden
              >
                <div
                  className="whitespace-nowrap"
                  style={{
                    fontSize: mainFont,
                    fontWeight: 800,
                    color: colors.sung,
                    letterSpacing: "0.02em",
                    lineHeight: 1.25,
                  }}
                >
                  {active.text}
                </div>
              </div>
            </div>
            {/* 副行：有翻译优先显示翻译（三行同屏会超高被裁，翻译比预览下一句更有用） */}
            {active.trans ? (
              <div
                className="whitespace-nowrap max-w-full overflow-hidden"
                style={{
                  fontSize: nextFont,
                  color: colors.next,
                  textShadow: "0 1px 6px rgba(0,0,0,0.45)",
                }}
              >
                {active.trans}
              </div>
            ) : next ? (
              <div
                className="whitespace-nowrap max-w-full overflow-hidden"
                style={{
                  fontSize: nextFont,
                  color: colors.next,
                  textShadow: "0 1px 6px rgba(0,0,0,0.45)",
                }}
              >
                {next.text}
              </div>
            ) : null}
          </>
        ) : headerText ? (
          <div
            className="whitespace-nowrap max-w-full overflow-hidden"
            style={{
              fontSize: Math.round(26 * scale),
              fontWeight: 700,
              color: "rgba(255,255,255,0.85)",
              textShadow: "0 2px 12px rgba(0,0,0,0.5)",
            }}
          >
            {headerText}
          </div>
        ) : (
          <div
            className="whitespace-nowrap"
            style={{
              fontSize: Math.round(26 * scale),
              fontWeight: 700,
              color: "rgba(255,255,255,0.85)",
              textShadow: "0 2px 12px rgba(0,0,0,0.5)",
            }}
          >
            {idleText}
          </div>
        )}
        </div>
        {/* 缩放手柄：贴在歌词框自身右下角（跟随歌词框而非窗口角） */}
        {!locked && (
          <div
            onMouseDown={startResize}
            className="absolute right-0 bottom-0 w-[20px] h-[20px] cursor-nwse-resize z-10 flex items-center justify-center pointer-events-auto"
            title="拖动缩放"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" style={{ opacity: hover || DEBUG_HOVER ? 0.9 : 0 }}>
              <path
                d="M9 1.5 L9 9 L1.5 9"
                stroke="rgba(255,255,255,0.6)"
                strokeWidth="1.5"
                fill="none"
              />
            </svg>
          </div>
        )}
        </div>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<DesktopLyrics />);
