import {
  Clock3,
  Disc3,
  Heart,
  Library,
  ListMusic,
  Loader2,
  Plus,
  Radio,
  Settings,
} from "lucide-react";
import { useStore } from "../store";
import type { ViewName } from "../types";

const NAV: { key: ViewName; label: string; icon: typeof Library }[] = [
  { key: "library", label: "资料库", icon: Library },
  { key: "liked", label: "我喜欢", icon: Heart },
  { key: "recent", label: "最近播放", icon: Clock3 },
  { key: "sources", label: "在线音源", icon: Radio },
];

export default function Sidebar() {
  const view = useStore((s) => s.view);
  const viewParam = useStore((s) => s.viewParam);
  const setView = useStore((s) => s.setView);
  const playlists = useStore((s) => s.playlists);
  const scan = useStore((s) => s.scan);
  const tracks = useStore((s) => s.tracks);

  const likedCount = tracks.filter((t) => t.liked).length;

  return (
    <aside className="w-[216px] shrink-0 flex flex-col gap-1 px-3 pt-2 pb-3 overflow-y-auto relative z-20">
      <nav className="flex flex-col gap-0.5">
        {NAV.map(({ key, label, icon: Icon }) => {
          const active = view === key;
          return (
            <button
              key={key}
              onClick={() => setView(key)}
              className={`h-9 px-3 rounded-[10px] flex items-center gap-2.5 text-[13px] transition-all ${
                active
                  ? "bg-white/[0.09] text-white font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.06)]"
                  : "text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]"
              }`}
            >
              <Icon
                size={16}
                strokeWidth={1.9}
                className={active ? "text-[var(--dyn)]" : ""}
              />
              {label}
              {key === "liked" && likedCount > 0 && (
                <span className="ml-auto text-[11px] text-zinc-500">{likedCount}</span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="mt-4 mb-1 px-3 flex items-center justify-between">
        <span className="text-[11px] font-medium text-zinc-500 tracking-wider">播放列表</span>
        <button
          className="btn-ghost w-6 h-6"
          title="新建播放列表"
          onClick={() => {
            const name = window.prompt("播放列表名称：");
            if (name) useStore.getState().createPlaylist(name.trim());
          }}
        >
          <Plus size={14} />
        </button>
      </div>

      <div className="flex flex-col gap-0.5">
        {playlists.map((p) => {
          const active = view === "playlist" && viewParam === p.id;
          return (
            <button
              key={p.id}
              onClick={() => setView("playlist", p.id)}
              className={`h-8 px-3 rounded-[9px] flex items-center gap-2.5 text-[12.5px] transition-all ${
                active
                  ? "bg-white/[0.09] text-white"
                  : "text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]"
              }`}
            >
              <ListMusic size={14} className={active ? "text-[var(--dyn)]" : "text-zinc-500"} />
              <span className="truncate">{p.name}</span>
              <span className="ml-auto text-[11px] text-zinc-600">{p.trackIds.length}</span>
            </button>
          );
        })}
        {!playlists.length && (
          <div className="px-3 py-1.5 text-[12px] text-zinc-600">还没有播放列表</div>
        )}
      </div>

      <div className="mt-auto flex flex-col gap-0.5">
        <button
          onClick={() => setView("settings")}
          className={`h-9 px-3 rounded-[10px] flex items-center gap-2.5 text-[13px] transition-all ${
            view === "settings"
              ? "bg-white/[0.09] text-white"
              : "text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]"
          }`}
        >
          <Settings size={16} strokeWidth={1.9} />
          设置
        </button>
        <div className="h-8 px-3 flex items-center gap-2 text-[11.5px] text-zinc-600">
          {scan.active ? (
            <>
              <Loader2 size={12} className="animate-spin text-[var(--dyn)]" />
              <span className="truncate">
                正在扫描 {scan.total ? `${scan.done}/${scan.total}` : "…"}
              </span>
            </>
          ) : (
            <>
              <Disc3 size={12} />
              <span>{tracks.length} 首曲目</span>
            </>
          )}
        </div>
      </div>
    </aside>
  );
}
