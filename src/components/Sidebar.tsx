import {
  Clock3,
  Cloud,
  Disc3,
  Heart,
  Library,
  ListMusic,
  Loader2,
  Plus,
  Radio,
  Settings,
  DiscAlbum,
} from "lucide-react";
import { useState } from "react";
import { useStore } from "../store";
import { InputModal } from "./Dialogs";
import CoverImg from "./CoverImg";
import type { ViewName } from "../types";

const NAV: { key: ViewName; label: string; icon: typeof Library }[] = [
  { key: "library", label: "资料库", icon: Library },
  { key: "liked", label: "我喜欢", icon: Heart },
  { key: "recent", label: "最近播放", icon: Clock3 },
  { key: "netease", label: "网易云", icon: Cloud },
  { key: "qq", label: "QQ音乐", icon: DiscAlbum },
  { key: "sources", label: "在线音源", icon: Radio },
];

export default function Sidebar() {
  const view = useStore((s) => s.view);
  const viewParam = useStore((s) => s.viewParam);
  const setView = useStore((s) => s.setView);
  const playlists = useStore((s) => s.playlists);
  const scan = useStore((s) => s.scan);
  const tracks = useStore((s) => s.tracks);
  const [plModalOpen, setPlModalOpen] = useState(false);

  const likedCount = tracks.filter((t) => t.liked).length;
  const totalDuration = tracks.reduce((acc, t) => acc + t.duration, 0);
  const hours = Math.floor(totalDuration / 3600);
  const mins = Math.floor((totalDuration % 3600) / 60);

  return (
    <aside className="w-[236px] shrink-0 flex flex-col px-4 pt-3 pb-4 overflow-y-auto relative z-20">
      {/* 品牌区 */}
      <div className="flex items-center gap-3.5 px-3 pt-2 pb-6">
        <div
          className="w-11 h-11 rounded-[14px] flex items-center justify-center flex-shrink-0"
          style={{
            background: "linear-gradient(135deg, #ffc470 0%, #f0a24a 50%, #d96c2f 120%)",
            boxShadow: "0 8px 24px -6px rgba(240, 162, 74, 0.45)",
          }}
        >
          <div
            className="ml-[2px]"
            style={{
              width: 0,
              height: 0,
              borderTop: "7px solid transparent",
              borderBottom: "7px solid transparent",
              borderLeft: "11px solid #241505",
            }}
          />
        </div>
        <div className="min-w-0">
          <div className="text-[15px] font-bold tracking-wide leading-tight text-[var(--ink)]">
            RustMusic
          </div>
          <div className="text-[10px] text-[var(--ink-3)] tracking-[0.22em] mt-0.5">
            HI-FI PLAYER
          </div>
        </div>
      </div>

      {/* 主导航 */}
      <nav className="flex flex-col gap-1.5">
        {NAV.map(({ key, label, icon: Icon }) => {
          const active = view === key;
          return (
            <button
              key={key}
              onClick={() => setView(key)}
              className={`relative h-11 pl-4 pr-3 rounded-xl flex items-center gap-3.5 text-[13.5px] transition-all duration-200 ${
                active
                  ? "text-[var(--ink)] font-semibold"
                  : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-white/[0.05]"
              }`}
              style={
                active
                  ? {
                      background: "rgba(240, 162, 74, 0.12)",
                      border: "1px solid rgba(240, 162, 74, 0.22)",
                    }
                  : { border: "1px solid transparent" }
              }
            >
              {active && (
                <span
                  className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full"
                  style={{ background: "var(--accent)" }}
                />
              )}
              <Icon
                size={17}
                strokeWidth={1.9}
                className={active ? "text-[var(--accent)]" : ""}
              />
              {label}
              {key === "liked" && likedCount > 0 && (
                <span className="ml-auto text-[11.5px] text-[var(--ink-3)] tabular-nums">
                  {likedCount}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* 播放列表 */}
      <div className="mt-7 mb-2 px-4 flex items-center justify-between">
        <span className="text-[10.5px] font-semibold text-[var(--ink-3)] tracking-[0.18em]">
          播放列表
        </span>
        <button
          className="btn-ghost w-7 h-7 rounded-lg"
          title="新建播放列表"
          onClick={() => setPlModalOpen(true)}
        >
          <Plus size={15} />
        </button>
      </div>

      <div className="flex flex-col gap-1">
        {playlists.map((p) => {
          const active = view === "playlist" && viewParam === p.id;
          return (
            <button
              key={p.id}
              onClick={() => setView("playlist", p.id)}
              className={`h-10 pl-4 pr-3 rounded-xl flex items-center gap-3 text-[13px] transition-all ${
                active
                  ? "bg-white/[0.08] text-[var(--ink)]"
                  : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-white/[0.05]"
              }`}
            >
              <ListMusic
                size={15}
                className={active ? "text-[var(--accent)]" : "text-[var(--ink-3)]"}
              />
              <span className="truncate">{p.name}</span>
              <span className="ml-auto text-[11.5px] text-[var(--ink-3)] tabular-nums">
                {p.entries.length}
              </span>
            </button>
          );
        })}
        {!playlists.length && (
          <button
            onClick={() => setPlModalOpen(true)}
            className="mx-1 h-10 rounded-xl border border-dashed border-[rgba(243,233,216,0.14)] text-[12px] text-[var(--ink-3)] hover:text-[var(--ink-2)] hover:border-[rgba(243,233,216,0.26)] transition-colors"
          >
            + 创建播放列表
          </button>
        )}
      </div>

      {/* 底部：扫描状态 + 设置 */}
      <div className="mt-auto flex flex-col gap-3">
        {scan.active && (
          <div className="flex items-center gap-2 px-4 text-[12px] text-[var(--accent)]">
            <Loader2 size={13} className="animate-spin" />
            <span className="truncate">
              扫描中 {scan.total ? `${scan.done}/${scan.total}` : "…"}
            </span>
          </div>
        )}

        <button
          onClick={() => setView("settings")}
          className={`h-11 px-4 rounded-xl flex items-center gap-3.5 text-[13.5px] transition-all ${
            view === "settings"
              ? "bg-white/[0.08] text-[var(--ink)]"
              : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-white/[0.05]"
          }`}
        >
          <Settings size={17} strokeWidth={1.9} />
          设置
        </button>
      </div>

      <InputModal
        open={plModalOpen}
        title="新建播放列表"
        placeholder="播放列表名称"
        confirmText="创建"
        onClose={() => setPlModalOpen(false)}
        onConfirm={(name) => useStore.getState().createPlaylist(name)}
      />
    </aside>
  );
}
