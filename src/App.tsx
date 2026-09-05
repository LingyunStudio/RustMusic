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
      document.documentElement.style.setProperty("--dyn", "#818cf8");
      return;
    }
    let alive = true;
    extractColor(url).then((c) => {
      if (alive && c) document.documentElement.style.setProperty("--dyn", c);
    });
    return () => {
      alive = false;
    };
  }, [url]);

  return (
    <div className="absolute inset-0 overflow-hidden pointer-events-none">
      {url && (
        <img
          src={url}
          alt=""
          draggable={false}
          className="absolute inset-0 w-full h-full object-cover scale-150 opacity-[0.08]"
          style={{ filter: "blur(48px)" }}
        />
      )}
      <div
        className="absolute -top-56 left-[8%] w-[720px] h-[620px] transition-colors duration-1000"
        style={{
          background:
            "radial-gradient(ellipse at center, var(--dyn) 0%, transparent 62%)",
          opacity: 0.14,
        }}
      />
      <div
        className="absolute -bottom-64 -right-40 w-[760px] h-[640px]"
        style={{
          background:
            "radial-gradient(ellipse at center, #22d3ee 0%, transparent 62%)",
          opacity: 0.1,
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
          style={{ background: "linear-gradient(135deg, #6366f1, #22d3ee)" }}
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
