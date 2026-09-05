import { Heart, MoreHorizontal, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useStore } from "../store";
import type { TrackMeta } from "../types";
import { fmtTime, trackArtist, trackTitle } from "../utils";
import CoverImg from "./CoverImg";
import Modal from "./Modal";

// ---------- 曲目行右键菜单 ----------

interface MenuState {
  x: number;
  y: number;
  track: TrackMeta;
}

interface TrackListProps {
  tracks: TrackMeta[];
  emptyHint?: string;
  emptyAction?: { label: string; onClick: () => void };
  /** 播放列表视图：提供该回调时显示“从列表移除” */
  onRemoveFromPlaylist?: (tid: number) => void;
  sort?: {
    key: SortKey;
    dir: 1 | -1;
    onSort: (k: SortKey) => void;
  };
}

export type SortKey = "title" | "artist" | "album" | "duration" | "added" | "plays";

export default function TrackList({
  tracks,
  emptyHint,
  emptyAction,
  onRemoveFromPlaylist,
}: TrackListProps) {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [pickerFor, setPickerFor] = useState<TrackMeta | null>(null);
  const current = useStore((s) => s.current);
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

  const menuX = Math.min(menu?.x ?? 0, window.innerWidth - 220);
  const menuY = Math.min(menu?.y ?? 0, window.innerHeight - 340);

  if (!tracks.length) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 text-zinc-600 anim-fade">
        <div
          className="w-16 h-16 rounded-2xl flex items-center justify-center"
          style={{ background: "rgba(255,255,255,0.04)", border: "1px solid rgba(255,255,255,0.06)" }}
        >
          <Play size={26} className="text-zinc-600" />
        </div>
        <div className="text-[13.5px]">{emptyHint ?? "这里空空如也"}</div>
        {emptyAction && (
          <button className="btn-primary mt-1" onClick={emptyAction.onClick}>
            {emptyAction.label}
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto pb-4">
      <div className="px-5">
        {tracks.map((t, i) => {
          const active =
            current?.kind === "track" && current.id === t.id;
          return (
            <div
              key={t.id}
              className="group grid grid-cols-[36px_minmax(0,1fr)_minmax(0,0.6fr)_84px_92px] items-center gap-3 h-[56px] px-3 rounded-xl hover:bg-white/[0.05] transition-colors cursor-default"
              style={{ contentVisibility: "auto", containIntrinsicSize: "56px" }}
              onDoubleClick={() => playTracks(tracks, i)}
              onContextMenu={(e) => {
                e.preventDefault();
                setMenu({ x: e.clientX, y: e.clientY, track: t });
              }}
            >
              <div className="relative w-9 h-9 shrink-0">
                <CoverImg
                  src={t.cover}
                  seed={t.title}
                  className={`w-9 h-9 rounded-lg shadow ${active ? "opacity-30" : "opacity-100 group-hover:opacity-0"} transition-opacity`}
                  iconSize={14}
                />
                {active ? (
                  <div className="absolute inset-0 flex items-center justify-center">
                    <div className={`eq-bars ${useStore.getState().playing ? "" : "paused"}`}>
                      <i />
                      <i />
                      <i />
                    </div>
                  </div>
                ) : (
                  <button
                    className="absolute inset-0 rounded-lg bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center text-white"
                    onClick={() => playTracks(tracks, i)}
                  >
                    <Play size={14} className="fill-current ml-px" />
                  </button>
                )}
              </div>

              <div className="min-w-0">
                <div
                  className={`text-[13.5px] truncate ${active ? "text-[var(--dyn)] font-medium" : "text-zinc-100"}`}
                >
                  {trackTitle(t)}
                </div>
                <div className="text-[12px] text-zinc-500 truncate">
                  {trackArtist(t)}
                  {t.sampleRate > 0 && (
                    <span className="ml-1.5 text-[10.5px] text-zinc-600">
                      {t.sampleRate / 1000}kHz
                      {t.bitDepth > 0 ? `/${t.bitDepth}bit` : ""}
                    </span>
                  )}
                </div>
              </div>

              <div className="text-[12.5px] text-zinc-500 truncate">
                {t.album || "-"}
              </div>

              <div className="text-[12px] text-zinc-500 tabular-nums text-right">
                {t.format !== "" && (
                  <span className="mr-2 text-[10px] px-1.5 py-0.5 rounded bg-white/[0.06] text-zinc-400">
                    {t.format}
                  </span>
                )}
                {fmtTime(t.duration * 1000)}
              </div>

              <div className="flex items-center justify-end gap-0.5">
                <button
                  className="btn-ghost w-7 h-7 opacity-0 group-hover:opacity-100 data-[liked=true]:opacity-100"
                  data-liked={t.liked}
                  onClick={() => toggleLike(t.id)}
                  title={t.liked ? "取消喜欢" : "喜欢"}
                >
                  <Heart
                    size={14}
                    className={t.liked ? "fill-rose-500 text-rose-500" : ""}
                  />
                </button>
                <button
                  className="btn-ghost w-7 h-7 opacity-0 group-hover:opacity-100"
                  onClick={(e) => {
                    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                    setMenu({ x: r.left - 150, y: r.bottom + 4, track: t });
                  }}
                >
                  <MoreHorizontal size={15} />
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {/* 右键菜单 */}
      {menu && (
        <div
          className="fixed z-[75] w-[200px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={{ left: menuX, top: menuY }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          <MenuItem
            icon={<Play size={13} />}
            label="播放"
            onClick={() => {
              const idx = tracks.findIndex((t) => t.id === menu.track.id);
              playTracks(tracks, Math.max(0, idx));
              setMenu(null);
            }}
          />
          <MenuItem
            icon={<Play size={13} />}
            label="下一首播放"
            onClick={() => {
              playNext({ kind: "track", id: menu.track.id });
              setMenu(null);
            }}
          />
          <MenuItem
            icon={<Play size={13} />}
            label="加入队列"
            onClick={() => {
              addToQueue({ kind: "track", id: menu.track.id });
              setMenu(null);
            }}
          />
          <MenuItem
            icon={<Heart size={13} />}
            label={menu.track.liked ? "取消喜欢" : "喜欢"}
            onClick={() => {
              toggleLike(menu.track.id);
              setMenu(null);
            }}
          />
          <div className="my-1 mx-2 border-t border-white/[0.07]" />
          <MenuItem
            icon={<Play size={13} />}
            label="添加到播放列表…"
            onClick={() => {
              setPickerFor(menu.track);
              setMenu(null);
            }}
          />
          {onRemoveFromPlaylist && (
            <MenuItem
              icon={<Play size={13} />}
              label="从此列表移除"
              onClick={() => {
                onRemoveFromPlaylist(menu.track.id);
                setMenu(null);
              }}
            />
          )}
        </div>
      )}

      {/* 添加到播放列表弹窗 */}
      <Modal
        open={!!pickerFor}
        onClose={() => setPickerFor(null)}
        title="添加到播放列表"
        width={380}
      >
        <div className="flex flex-col gap-1.5 max-h-[280px] overflow-y-auto">
          {playlists.map((p) => (
            <button
              key={p.id}
              className="h-10 px-3 rounded-lg text-left text-[13px] text-zinc-200 hover:bg-white/[0.07] flex items-center justify-between transition-colors"
              onClick={async () => {
                if (pickerFor) await useStore.getState().addToPlaylist(p.id, pickerFor.id);
                setPickerFor(null);
              }}
            >
              <span className="truncate">{p.name}</span>
              <span className="text-[11px] text-zinc-500">{p.trackIds.length} 首</span>
            </button>
          ))}
          {!playlists.length && (
            <div className="text-[12.5px] text-zinc-500 py-2">还没有播放列表，创建一个吧</div>
          )}
        </div>
        <div className="flex gap-2 mt-3">
          <input
            type="text"
            value={newPlName}
            onChange={(e) => setNewPlName(e.target.value)}
            placeholder="新播放列表名称"
            className="flex-1 h-9 rounded-lg bg-white/[0.06] border border-white/[0.09] px-3 text-[13px] focus:border-white/25"
            onKeyDown={async (e) => {
              if (e.key === "Enter" && newPlName.trim() && pickerFor) {
                await createPlaylist(newPlName.trim());
                const pls = useStore.getState().playlists;
                const created = pls[pls.length - 1];
                if (created) await useStore.getState().addToPlaylist(created.id, pickerFor.id);
                setNewPlName("");
                setPickerFor(null);
              }
            }}
          />
          <button
            className="btn-secondary"
            onClick={async () => {
              if (!newPlName.trim() || !pickerFor) return;
              await createPlaylist(newPlName.trim());
              const pls = useStore.getState().playlists;
              const created = pls[pls.length - 1];
              if (created) await useStore.getState().addToPlaylist(created.id, pickerFor.id);
              setNewPlName("");
              setPickerFor(null);
            }}
          >
            创建并添加
          </button>
        </div>
      </Modal>
    </div>
  );
}

function MenuItem({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-zinc-300 hover:bg-white/[0.08] hover:text-white transition-colors text-left"
      onClick={onClick}
    >
      <span className="text-zinc-500">{icon}</span>
      {label}
    </button>
  );
}
