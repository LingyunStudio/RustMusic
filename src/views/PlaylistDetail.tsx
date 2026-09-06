import { useMemo, useState } from "react";
import {
  Heart,
  ListMusic,
  MoreHorizontal,
  Play,
  Shuffle,
  Trash2,
  Cloud,
} from "lucide-react";
import { useStore } from "../store";
import { ConfirmModal } from "../components/Dialogs";
import { fmtTime, matchSearch } from "../utils";
import CoverImg from "../components/CoverImg";
import Modal from "../components/Modal";
import type { PlaylistEntryMeta } from "../types";

type DetailRow =
  | {
      entry: PlaylistEntryMeta;
      kind: "track";
      id: number;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "netease";
      id: number;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "qq";
      id: string;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    };

export default function PlaylistDetail({ id }: { id: number }) {
  const playlists = useStore((s) => s.playlists);
  const tracks = useStore((s) => s.tracks);
  const search = useStore((s) => s.search);
  const playing = useStore((s) => s.playing);
  const playEntries = useStore((s) => s.playEntries);
  const removeFromPlaylist = useStore((s) => s.removeFromPlaylist);
  const removePlaylistEntryRow = useStore((s) => s.removePlaylistEntryRow);
  const toggleLike = useStore((s) => s.toggleLike);
  const saveOnline = useStore((s) => s.saveOnline);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const deletePlaylist = useStore((s) => s.deletePlaylist);
  const current = useStore((s) => s.current);
  const [confirmDel, setConfirmDel] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; e: PlaylistEntryMeta } | null>(
    null
  );

  const pl = playlists.find((p) => p.id === id);

  const list = useMemo<DetailRow[]>(() => {
    if (!pl) return [];
    const byId = new Map(tracks.map((t) => [t.id, t]));
    return pl.entries
      .filter((e) => matchSearchEntry(e, search))
      .map((e): DetailRow => {
        const base = {
          entry: e,
          name: e.title,
          artist: e.artist,
          album: e.album,
          cover: e.cover,
          durationMs: e.duration * 1000,
        };
        if (e.kind === "local") {
          const t = e.trackId != null ? byId.get(e.trackId) : undefined;
          return {
            ...base,
            kind: "track",
            id: e.trackId ?? 0,
            name: t ? t.title : e.title,
            artist: t ? t.artist : e.artist,
            album: t ? t.album : e.album,
            cover: t ? t.cover : e.cover,
            durationMs: t ? t.duration * 1000 : e.duration * 1000,
            liked: t?.liked ?? false,
          };
        }
        if (e.kind === "netease") {
          return {
            ...base,
            kind: "netease",
            id: Number(e.onlineId ?? 0),
            liked: false,
          };
        }
        return {
          ...base,
          kind: "qq",
          id: e.onlineId ?? "",
          liked: false,
        };
      });
  }, [pl, tracks, search]);

  if (!pl) {
    return (
      <div className="flex-1 flex items-center justify-center text-zinc-500">
        播放列表不存在
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="pt-5 pb-4 px-5 flex gap-5 items-end">
        <div
          className="w-[104px] h-[104px] rounded-2xl shadow-xl flex items-center justify-center shrink-0"
          style={{
            background:
              "linear-gradient(135deg, hsl(35, 55%, 45%), hsl(15, 60%, 32%))",
          }}
        >
          <ListMusic size={34} className="text-white/85" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[11px] text-zinc-500 tracking-wider mb-1">播放列表</div>
          <h1 className="text-[24px] font-bold truncate">{pl.name}</h1>
          <div className="text-[12.5px] text-zinc-500 mt-1.5">
            {list.length} 首曲目
          </div>
          <div className="flex items-center gap-2 mt-3">
            <button className="btn-primary" onClick={() => playEntries(list.map((r) => r.entry), 0)}>
              <Play size={13} className="fill-current" />
              播放全部
            </button>
            <button
              className="btn-secondary"
              onClick={() =>
                playEntries(
                  list.map((r) => r.entry),
                  Math.floor(Math.random() * Math.max(1, list.length))
                )
              }
            >
              <Shuffle size={13} />
              随机
            </button>
            <button
              className="btn-secondary !text-rose-300/80 hover:!bg-rose-500/15"
              onClick={() => setConfirmDel(true)}
            >
              <Trash2 size={13} />
              删除列表
            </button>
          </div>
        </div>
      </header>

      <ConfirmModal
        open={confirmDel}
        title="删除播放列表"
        danger
        confirmText="删除"
        onClose={() => setConfirmDel(false)}
        onConfirm={() => deletePlaylist(pl.id)}
      >
        确定删除播放列表「{pl.name}」？列表中的曲目不会被删除。
      </ConfirmModal>

      <div className="flex-1 min-h-0 flex flex-col px-6 pb-4">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {list.length === 0 ? (
            <div className="flex-1 flex items-center justify-center text-[13px] text-zinc-500">
              列表里还没有歌曲（可在在线曲库右键添加）
            </div>
          ) : (
            <div className="flex-1 min-h-0 overflow-y-auto px-2.5 py-2.5">
              {list.map((r, i) => {
                const active =
                current != null &&
                current.kind === r.kind &&
                (r.kind === "qq"
                  ? current.qid === r.id
                  : current.id === (r.id as number));
                return (
                  <div
                    key={r.entry.rowid}
                    className={`group grid grid-cols-[56px_minmax(200px,460px)_minmax(140px,300px)_92px_136px] items-center gap-4 h-[60px] px-4 rounded-2xl transition-colors cursor-default ${
                      active ? "bg-[rgba(240,162,74,0.1)]" : "hover:bg-white/[0.045]"
                    }`}
                    onDoubleClick={() => playEntries(list.map((x) => x.entry), i)}
                    onContextMenu={(ev) => {
                      ev.preventDefault();
                      setMenu({ x: ev.clientX, y: ev.clientY, e: r.entry });
                    }}
                  >
                    <div className="relative h-11 flex items-center justify-center">
                      <span
                        className={`text-[12.5px] tabular-nums transition-opacity ${
                          active
                            ? "text-[var(--accent)] font-bold opacity-100 group-hover:opacity-0"
                            : "text-[var(--ink-3)] group-hover:opacity-0"
                        }`}
                      >
                        {String(i + 1).padStart(2, "0")}
                      </span>
                      <button
                        className={`absolute inset-0 m-auto w-9 h-9 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all hover:scale-105 ${
                          active
                            ? "text-[var(--accent)]"
                            : "bg-[var(--ink)] text-[#241505]"
                        }`}
                        onClick={() => playEntries(list.map((x) => x.entry), i)}
                      >
                        <Play size={14} className="fill-current ml-px" />
                      </button>
                    </div>

                    <div className="flex items-center gap-4 min-w-0">
                      <CoverImg
                        src={r.cover}
                        seed={r.name}
                        className="w-11 h-11 rounded-xl shadow-[0_4px_14px_rgba(0,0,0,0.45)] shrink-0"
                        iconSize={16}
                      />
                      <div className="min-w-0">
                        <div
                          className={`text-[13.5px] truncate flex items-center gap-2 ${
                            active
                              ? "text-[var(--accent-strong)] font-semibold"
                              : "text-[var(--ink)]"
                          }`}
                        >
                          <span className="truncate">{r.name}</span>
                          {r.kind !== "track" && (
                            <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-white/[0.08] text-[var(--ink-3)] font-medium shrink-0 flex items-center gap-1">
                              <Cloud size={9} />
                              {r.kind === "netease" ? "网易云" : "QQ音乐"}
                            </span>
                          )}
                        </div>
                        <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
                          {r.artist || "未知艺术家"}
                        </div>
                      </div>
                    </div>

                    <div className="text-[12.5px] text-[var(--ink-3)] truncate">
                      {r.album || "未知专辑"}
                    </div>

                    <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                      {fmtTime(r.durationMs)}
                    </div>

                    <div className="flex items-center justify-end gap-1 pr-1">
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(e) => {
                          e.stopPropagation();
                          if (r.kind === "track") toggleLike(r.id);
                          else
                            saveOnline({
                              kind: r.kind,
                              id: r.id,
                              name: r.name,
                              artist: r.artist,
                              album: r.album,
                              cover: r.cover,
                              durationMs: r.durationMs,
                            });
                        }}
                        title={
                          r.kind === "track"
                            ? r.liked
                              ? "取消喜欢"
                              : "喜欢"
                            : "收藏到“我喜欢”（下载到本机）"
                        }
                      >
                        <Heart
                          size={15}
                          className={
                            r.kind === "track" && r.liked
                              ? "fill-[#e0533f] text-[#e0533f]"
                              : "opacity-0 group-hover:opacity-100"
                          }
                        />
                      </button>
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(e) => {
                          e.stopPropagation();
                          if (r.kind === "track")
                            playNext({ kind: "track", id: r.id });
                          else if (r.kind === "netease")
                            playNext({ kind: "netease", id: r.id as number });
                          else playNext({ kind: "qq", id: r.id as string });
                        }}
                        title="下一首播放"
                      >
                        <Play
                          size={14}
                          className="opacity-0 group-hover:opacity-100"
                        />
                      </button>
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(ev) => {
                          const rect = (
                            ev.currentTarget as HTMLElement
                          ).getBoundingClientRect();
                          setMenu({
                            x: rect.left - 150,
                            y: rect.bottom + 6,
                            e: r.entry,
                          });
                        }}
                      >
                        <MoreHorizontal
                          size={16}
                          className="opacity-0 group-hover:opacity-100"
                        />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* 条目菜单 */}
      {menu && (
        <div
          className="fixed z-[75] w-[200px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={{ left: menu.x, top: menu.y }}
          onMouseDown={(ev) => ev.stopPropagation()}
          onMouseLeave={() => setMenu(null)}
        >
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-zinc-200 hover:bg-white/[0.08] text-left"
            onClick={() => {
              const idx = list.findIndex((r) => r.entry.rowid === menu.e.rowid);
              playEntries(
                list.map((r) => r.entry),
                Math.max(0, idx)
              );
              setMenu(null);
            }}
          >
            <Play size={13} /> 播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-zinc-200 hover:bg-white/[0.08] text-left"
            onClick={() => {
              const r = list.find((x) => x.entry.rowid === menu.e.rowid);
              if (r) {
                if (r.kind === "track") playNext({ kind: "track", id: r.id });
                else if (r.kind === "netease") playNext({ kind: "netease", id: r.id });
                else playNext({ kind: "qq", id: r.id });
              }
              setMenu(null);
            }}
          >
            <Play size={13} /> 下一首播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-rose-300 hover:bg-white/[0.08] text-left"
            onClick={() => {
              removePlaylistEntryRow(menu.e.rowid);
              if (menu.e.kind === "local" && menu.e.trackId != null)
                removeFromPlaylist(pl.id, menu.e.trackId);
              setMenu(null);
            }}
          >
            <Trash2 size={13} /> 从列表移除
          </button>
        </div>
      )}
    </div>
  );
}

function matchSearchEntry(
  e: PlaylistEntryMeta,
  q: string
): boolean {
  if (!q) return true;
  const s = q.toLowerCase();
  return (
    e.title.toLowerCase().includes(s) ||
    e.artist.toLowerCase().includes(s) ||
    e.album.toLowerCase().includes(s)
  );
}
