import { Heart, MoreHorizontal, Play } from "lucide-react";
import { useEffect, useState } from "react";
import { useStore } from "../store";
import type { TrackMeta } from "../types";
import { fmtTime, trackArtist, trackTitle } from "../utils";
import CoverImg from "./CoverImg";
import Modal from "./Modal";

interface MenuState {
  x: number;
  y: number;
  track: TrackMeta;
}

interface TrackListProps {
  tracks: TrackMeta[];
  inCard?: boolean;
  emptyHint?: string;
  emptyAction?: { label: string; onClick: () => void };
  onRemoveFromPlaylist?: (tid: number) => void;
}

export type SortKey = "title" | "artist" | "album" | "duration" | "added" | "plays";

export default function TrackList({
  tracks,
  inCard,
  emptyHint,
  emptyAction,
  onRemoveFromPlaylist,
}: TrackListProps) {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [pickerFor, setPickerFor] = useState<TrackMeta | null>(null);
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const playTracks = useStore((s) => s.playTracks);
  const toggleLike = useStore((s) => s.toggleLike);
  const addToQueue = useStore((s) => s.addToQueue);
  const playNext = useStore((s) => s.playNext);
  const playlists = useStore((s) => s.playlists);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const [newPlName, setNewPlName] = useState("");

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("mousedown", close);
    window.addEventListener("wheel", close);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("wheel", close);
    };
  }, [menu]);

  const menuX = Math.min(menu?.x ?? 0, window.innerWidth - 230);
  const menuY = Math.min(menu?.y ?? 0, window.innerHeight - 350);

  if (!tracks.length) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-5 anim-fade">
        <div className="relative">
          <div
            className="absolute -inset-8 rounded-full"
            style={{
              background: "radial-gradient(circle, var(--accent) 0%, transparent 65%)",
              opacity: 0.14,
            }}
          />
          <div
            className="relative w-[76px] h-[76px] rounded-3xl flex items-center justify-center"
            style={{
              background:
                "linear-gradient(135deg, rgba(243,233,216,0.1), rgba(243,233,216,0.03))",
              border: "1px solid rgba(243,233,216,0.12)",
            }}
          >
            <Play size={30} className="text-[var(--ink-2)] ml-1" />
          </div>
        </div>
        <div className="text-[13.5px] text-[var(--ink-2)]">{emptyHint ?? "这里空空如也"}</div>
        {emptyAction && (
          <button className="btn-primary mt-1" onClick={emptyAction.onClick}>
            {emptyAction.label}
          </button>
        )}
      </div>
    );
  }

  return (
    <div className={`flex-1 min-h-0 overflow-y-auto ${inCard ? "px-2.5 py-2.5" : "px-5 pb-4"}`}>
      {tracks.map((t, i) => {
        const active = current?.kind === "track" && current.id === t.id;
        return (
          <div
            key={t.id}
            className={`group grid grid-cols-[64px_minmax(0,1fr)_minmax(0,0.5fr)_120px_104px] items-center gap-4 h-[64px] px-4 rounded-2xl transition-colors duration-150 cursor-default ${
              active ? "bg-[rgba(240,162,74,0.1)]" : "hover:bg-white/[0.045]"
            }`}
            onDoubleClick={() => playTracks(tracks, i)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ x: e.clientX, y: e.clientY, track: t });
            }}
          >
            {/* 序号 / 播放 */}
            <div className="relative h-12 flex items-center justify-center">
              <span
                className={`text-[12.5px] tabular-nums transition-opacity ${
                  active
                    ? "text-[var(--accent)] font-bold opacity-100 group-hover:opacity-0"
                    : "text-[var(--ink-3)] group-hover:opacity-0"
                }`}
              >
                {String(i + 1).padStart(2, "0")}
              </span>
              {active && (
                <div className="absolute inset-0 flex items-center justify-center group-hover:opacity-0">
                  <div className={`eq-bars ${playing ? "" : "paused"}`}>
                    <i />
                    <i />
                    <i />
                  </div>
                </div>
              )}
              <button
                className={`absolute inset-0 m-auto w-9 h-9 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all hover:scale-105 ${
                  active ? "text-[var(--accent)]" : "bg-[var(--ink)] text-[#241505]"
                }`}
                onClick={() => (active ? useStore.getState().togglePlay() : playTracks(tracks, i))}
                title={active ? "播放 / 暂停" : "播放"}
              >
                <Play size={14} className="fill-current ml-px" />
              </button>
            </div>

            {/* 封面 + 标题 */}
            <div className="flex items-center gap-4 min-w-0">
              <CoverImg
                src={t.cover}
                seed={t.title}
                className="w-11 h-11 rounded-xl shadow-[0_4px_14px_rgba(0,0,0,0.45)] shrink-0"
                iconSize={16}
              />
              <div className="min-w-0">
                <div
                  className={`text-[13.5px] truncate ${
                    active ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
                  }`}
                >
                  {trackTitle(t)}
                </div>
                <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
                  {trackArtist(t)}
                </div>
              </div>
            </div>

            {/* 专辑 */}
            <div className="text-[12.5px] text-[var(--ink-3)] truncate">
              {t.album || "未知专辑"}
            </div>

            {/* 格式 / 时长（分列排布，间距固定） */}
            <div className="flex items-center justify-end gap-3">
              <span className="text-[10.5px] px-2 py-[3px] rounded-md bg-white/[0.07] text-[var(--ink-2)] font-semibold tracking-wider">
                {t.format || "AUDIO"}
              </span>
              <span className="text-[12.5px] text-[var(--ink-2)] tabular-nums w-10 text-right">
                {fmtTime(t.duration * 1000)}
              </span>
            </div>

            {/* 操作 */}
            <div className="flex items-center justify-end gap-1 pr-1">
              <button
                className="btn-ghost w-8 h-8"
                onClick={() => toggleLike(t.id)}
                title={t.liked ? "取消喜欢" : "喜欢"}
              >
                <Heart
                  size={15}
                  className={
                    t.liked
                      ? "fill-[#e0533f] text-[#e0533f]"
                      : "opacity-0 group-hover:opacity-100"
                  }
                />
              </button>
              <button
                className="btn-ghost w-8 h-8"
                onClick={(e) => {
                  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                  setMenu({ x: r.left - 180, y: r.bottom + 6, track: t });
                }}
              >
                <MoreHorizontal size={16} className="opacity-0 group-hover:opacity-100" />
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
