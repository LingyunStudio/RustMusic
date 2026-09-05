import { useEffect } from "react";
import { api } from "./api";
import { useStore } from "./store";
import Titlebar from "./components/Titlebar";
import Sidebar from "./components/Sidebar";
import PlayerBar from "./components/PlayerBar";
import QueuePanel from "./components/QueuePanel";
import NowPlaying from "./components/NowPlaying";
import ToastContainer from "./components/Toast";
import LibraryView from "./views/LibraryView";
import PlaylistDetail from "./views/PlaylistDetail";
import SourcesView from "./views/SourcesView";
import SettingsView from "./views/SettingsView";
import { coverSrc } from "./api";
import { extractColor } from "./utils";

function DynamicBackdrop() {
  const cover = useStore((s) => s.current?.cover);
  const url = cover ? coverSrc(cover) : "";

  useEffect(() => {
    if (!url) {
      document.documentElement.style.setProperty("--glow", "#c2570f");
      return;
    }
    let alive = true;
    extractColor(url).then((c) => {
      if (alive && c) document.documentElement.style.setProperty("--glow", c);
    });
    return () => {
      alive = false;
    };
  }, [url]);

  return (
    <div className="absolute inset-0 overflow-hidden pointer-events-none">
      {/* 暖色底 */}
      <div
        className="absolute inset-0"
        style={{
          background:
            "radial-gradient(ellipse 120% 100% at 50% -20%, #241a10 0%, #14100b 45%, #0f0c09 100%)",
        }}
      />
      {/* 封面主色微光（低饱和、低调） */}
      <div
        className="absolute -top-64 left-[12%] w-[820px] h-[640px] transition-colors duration-1000"
        style={{
          background:
            "radial-gradient(ellipse at center, var(--glow) 0%, transparent 60%)",
          opacity: 0.16,
          filter: "blur(90px)",
        }}
      />
      <div
        className="absolute -bottom-72 -right-48 w-[760px] h-[600px]"
        style={{
          background:
            "radial-gradient(ellipse at center, #8a3d1f 0%, transparent 62%)",
          opacity: 0.18,
          filter: "blur(100px)",
        }}
      />
      {/* 暗角 + 噪点 */}
      <div
        className="absolute inset-0"
        style={{
          background:
            "radial-gradient(ellipse 135% 115% at 50% 42%, transparent 52%, rgba(5,3,2,0.55) 100%)",
        }}
      />
      <div
        className="absolute inset-0 opacity-30 mix-blend-overlay"
        style={{
          backgroundImage:
            "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.15'/%3E%3C/svg%3E\")",
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

  useEffect(() => {
    useStore.getState().init();
  }, []);

  // 空格键 播放/暂停
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      if (e.code === "Space") {
        e.preventDefault();
        useStore.getState().togglePlay();
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
      <div className="h-full flex flex-col items-center justify-center gap-4 bg-[#0b0b10]">
        <div
          className="w-14 h-14 rounded-2xl flex items-center justify-center shadow-2xl animate-pulse"
          style={{ background: "linear-gradient(135deg, #ffc470, #e8823f)" }}
        >
          <div
            className="ml-1"
            style={{
              width: 0,
              height: 0,
              borderTop: "11px solid transparent",
              borderBottom: "11px solid transparent",
              borderLeft: "17px solid white",
            }}
          />
        </div>
        <div className="text-[13px] text-zinc-500">RustMusic 正在启动…</div>
      </div>
    );
  }

  return (
    <div className="h-full relative overflow-hidden bg-[#0b0b10]">
      <DynamicBackdrop />

      <div className="relative h-full flex flex-col">
        <Titlebar />

        <div className="relative flex flex-1 min-h-0">
          <Sidebar />

          <main className="flex-1 min-w-0 flex flex-col relative">
            {view === "library" && <LibraryView mode="library" />}
            {view === "liked" && <LibraryView mode="liked" />}
            {view === "recent" && <LibraryView mode="recent" />}
            {view === "playlist" && <PlaylistDetail id={viewParam} />}
            {view === "sources" && <SourcesView />}
            {view === "settings" && <SettingsView />}
          </main>

          {queueOpen && <QueuePanel />}

          {nowPlayingOpen && <NowPlaying />}
        </div>

        <PlayerBar />
      </div>

      <ToastContainer />
    </div>
  );
}
