import { useEffect, useState } from "react";
import { api } from "./api";
import { useStore } from "./store";
import Titlebar from "./components/Titlebar";
import Sidebar from "./components/Sidebar";
import PlayerBar from "./components/PlayerBar";
import QueuePanel from "./components/QueuePanel";
import NowPlaying from "./components/NowPlaying";
import Logo from "./components/Logo";
import ToastContainer from "./components/Toast";
import UpdateDialog from "./components/UpdateDialog";
import LibraryView from "./views/LibraryView";
import PlaylistDetail from "./views/PlaylistDetail";
import SourcesView from "./views/SourcesView";
import SettingsView from "./views/SettingsView";
import OnlineLibraryView from "./views/NeteaseView";
import { coverSrc } from "./api";
import { extractColor } from "./utils";
import { skinUri } from "./skins";

function DynamicBackdrop() {
  const cover = useStore((s) => s.current?.cover);
  const skin = useStore((s) => s.skin);
  const skinUrl = skinUri(skin);
  // http 封面直接用原始 URL，本地路径才走 asset 协议
  const url = cover
    ? cover.startsWith("http://") || cover.startsWith("https://")
      ? cover
      : coverSrc(cover)
    : "";

  useEffect(() => {
    if (!url) {
      // 无封面时用主题默认氛围色（CSS 变量按主题分支）
      document.documentElement.style.removeProperty("--glow");
      return;
    }
    let alive = true;
    const pick = async () => {
      try {
        // 远程 http 封面（QQ 等域无 CORS，canvas 会被污染）走后端取色；
        // asset:// 本地封面是 WebView 虚拟主机，后端连不上，用前端采样
        let c: string | null = null;
        if (/^https?:\/\/(?!asset\.)/.test(url)) {
          const palette = await api.extractCoverPalette(url);
          c = palette[0] ?? null;
        } else {
          c = await extractColor(url);
        }
        if (alive && c) document.documentElement.style.setProperty("--glow", c);
      } catch {
        /* 取色失败保留默认氛围色 */
      }
    };
    pick();
    return () => {
      alive = false;
    };
  }, [url]);

  return (
    <div className="absolute inset-0 overflow-hidden pointer-events-none">
      {/* 底层：皮肤背景图（启用时替代默认渐变底），或主题默认氛围 */}
      {skinUrl ? (
        <>
          <div
            className="absolute inset-0 transition-all duration-500"
            style={{
              backgroundImage: skinUrl,
              backgroundSize: "cover",
              backgroundPosition: "center",
            }}
          />
          {/* 主题纱：暗色压暗 / 浅色洗成粉彩，保证前景文字可读 */}
          <div className="absolute inset-0" style={{ background: "var(--skin-scrim)" }} />
        </>
      ) : (
        <div
          className="absolute inset-0"
          style={{
            background:
              "radial-gradient(ellipse 120% 100% at 50% -20%, var(--backdrop-1) 0%, var(--backdrop-2) 60%, var(--bg) 100%)",
          }}
        />
      )}
      {/* 封面主色微光（低饱和、低调；皮肤上保留，让壁纸跟着音乐呼吸） */}
      <div
        className="absolute -top-64 left-[12%] w-[820px] h-[640px] transition-colors duration-1000"
        style={{
          background:
            "radial-gradient(ellipse at center, var(--glow) 0%, transparent 60%)",
          opacity: skinUrl ? 0.09 : 0.16,
          filter: "blur(90px)",
        }}
      />
      <div
        className="absolute -bottom-72 -right-48 w-[760px] h-[600px]"
        style={{
          background:
            "radial-gradient(ellipse at center, var(--blob-warm) 0%, transparent 62%)",
          opacity: skinUrl ? 0.1 : 0.18,
          filter: "blur(100px)",
        }}
      />
      {/* 暗角 + 噪点 */}
      <div
        className="absolute inset-0"
        style={{
          background:
            "radial-gradient(ellipse 135% 115% at 50% 42%, transparent 52%, var(--vignette) 100%)",
        }}
      />
      <div
        className="absolute inset-0 mix-blend-overlay"
        style={{
          opacity: "var(--noise-opacity)",
          backgroundImage:
            "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2'%3E%3C/filter%3E%3Crect width='140' height='140' filter='url(%23n)' opacity='0.4'/%3E%3C/svg%3E\")",
        }}
      />
    </div>
  );
}

export default function App() {
  const ready = useStore((s) => s.ready);
  const view = useStore((s) => s.view);
  const viewParam = useStore((s) => s.viewParam);
  const queueOpen = useStore((s) => s.queueOpen);
  const nowPlayingOpen = useStore((s) => s.nowPlayingOpen);
  const fullscreen = useStore((s) => s.fullscreen);

  useEffect(() => {
    useStore.getState().init();
  }, []);

  // 空格键 播放/暂停；L 切桌面歌词；Esc 退出全屏
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      if (e.code === "Space") {
        e.preventDefault();
        useStore.getState().togglePlay();
      }
      // L：桌面歌词开关（锁定时再按 = 解锁交互）
      if (e.code === "KeyL") {
        const s = useStore.getState();
        if (!s.desktopLyricsOn) s.openDesktopLyrics();
        else if (s.desktopLyricsLock) s.unlockDesktopLyrics();
        else s.closeDesktopLyrics();
      }
      if (e.key === "Escape" && useStore.getState().fullscreen) {
        useStore.getState().toggleFullscreen(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // 拖拽文件 / 文件夹导入
  useEffect(() => {
    let unbind: (() => void) | undefined;
    let disposed = false;
    (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const u = await getCurrentWebview().onDragDropEvent((ev) => {
          if (ev.payload.type === "drop") {
            const paths = ev.payload.paths ?? [];
            if (!paths.length) return;
            api
              .dropPaths(paths)
              .then((n) => {
                if (n > 0)
                  useStore
                    .getState()
                    .toast(`已导入 ${n} 项，正在扫描…`, "success");
              })
              .catch((err) => useStore.getState().toast(String(err), "error"));
          }
        });
        if (disposed) u();
        else unbind = u;
      } catch {
        /* 拖拽不可用时忽略 */
      }
    })();
    return () => {
      disposed = true;
      unbind?.();
    };
  }, []);

  if (!ready) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-4 bg-[var(--bg)]">
        <div className="animate-pulse">
          <Logo size={64} />
        </div>
        <div className="text-[13px] text-[var(--ink-2)]">RustMusic 正在启动…</div>
      </div>
    );
  }

  return (
    <div className="h-full relative overflow-hidden bg-[var(--bg)]">
      <DynamicBackdrop />

      <div className="relative h-full flex flex-col">
        {/* 全屏播放时隐藏标题栏（窗口按钮随 setFullscreen 一并由系统隐藏）。
            占位 h-3：Titlebar 的拖拽热区已改为绝对覆盖层（不占布局），
            这里保留 12px 占位使内容区位置与原先完全一致 */}
        {!fullscreen && (
          <>
            <Titlebar />
            <div className="h-3 shrink-0" />
          </>
        )}

        <div className="relative flex flex-1 min-h-0">
          <Sidebar />

          {/* 内容区最小宽度兜底：三个列表页头部（标题块 + 420px 搜索框 +
              播放按钮组）固有宽度 ~880px，被压更窄时标题会挤成竖排换行；
              列表页头部另有 flex-wrap 优雅降级。窗口更窄时这里横向滚动兜底 */}
          <main className="flex-1 min-w-[880px] min-h-0 flex flex-col relative overflow-x-auto">
            <div key={view + viewParam} className="flex-1 min-h-0 flex flex-col anim-view">
              {view === "library" && <LibraryView mode="library" />}
              {view === "liked" && <LibraryView mode="liked" />}
              {view === "recent" && <LibraryView mode="recent" />}
              {view === "playlist" && <PlaylistDetail id={viewParam} />}
              {view === "sources" && <SourcesView />}
              {view === "netease" && <OnlineLibraryView source="netease" />}
              {view === "qq" && <OnlineLibraryView source="qq" />}
              {view === "kugou" && <OnlineLibraryView source="kugou" />}
              {view === "settings" && <SettingsView />}
            </div>
          </main>
        </div>

        {/* 播放条：播放页打开时居中（不再贴右侧内容区），全屏时隐藏由 hover 唤起 */}
        {!fullscreen && <PlayerBar centered={nowPlayingOpen} />}
      </div>

      {/* 播放队列：窗口级浮层（播放条上方右侧，z-[61] 盖在播放页 z-40 /
          播放条 z-50 之上，播放页/全屏下也能弹出） */}
      {queueOpen && <QueuePanel />}

      {/* 播放页：窗口级覆盖（背景与标题栏连为一体） */}
      {nowPlayingOpen && <NowPlaying />}

      {/* 全屏播放：无边框，播放条隐藏、鼠标移至底部唤起 */}
      {fullscreen && <FullscreenPlayerBar />}

      <ToastContainer />

      {/* 自动更新弹窗：启动检查到新版本时自动弹出，设置页也可手动触发 */}
      <UpdateDialog />
    </div>
  );
}

/** 全屏播放时的唤起式播放条：贴底热区 hover 浮现，移开收回 */
function FullscreenPlayerBar() {
  const [show, setShow] = useState(false);
  return (
    <div
      className="absolute bottom-0 left-0 right-0 z-50"
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
    >
      {/* 贴底唤起热区（常驻可交互，24px 高） */}
      <div className="h-6" />
      <div
        className="px-6 pb-4 pt-1 transition-all duration-300 ease-out"
        style={{
          opacity: show ? 1 : 0,
          transform: show ? "translateY(0)" : "translateY(24px)",
          pointerEvents: show ? "auto" : "none",
        }}
      >
        <PlayerBar centered />
      </div>
    </div>
  );
}
