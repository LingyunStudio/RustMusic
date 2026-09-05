import { useMemo, useState } from "react";
import { ListMusic, Play, Shuffle, Trash2 } from "lucide-react";
import { useStore } from "../store";
import TrackList from "../components/TrackList";
import { ConfirmModal } from "../components/Dialogs";
import { matchSearch } from "../utils";

export default function PlaylistDetail({ id }: { id: number }) {
  const playlists = useStore((s) => s.playlists);
  const tracks = useStore((s) => s.tracks);
  const search = useStore((s) => s.search);
  const playTracks = useStore((s) => s.playTracks);
  const removeFromPlaylist = useStore((s) => s.removeFromPlaylist);
  const deletePlaylist = useStore((s) => s.deletePlaylist);
  const [confirmDel, setConfirmDel] = useState(false);

  const pl = playlists.find((p) => p.id === id);

  const list = useMemo(() => {
    if (!pl) return [];
    const byId = new Map(tracks.map((t) => [t.id, t]));
    return pl.trackIds
      .map((tid) => byId.get(tid))
      .filter((t): t is NonNullable<typeof t> => !!t)
      .filter((t) => matchSearch(t, search));
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
              "linear-gradient(135deg, hsl(245, 60%, 45%), hsl(190, 65%, 32%))",
          }}
        >
          <ListMusic size={34} className="text-white/85" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[11px] text-zinc-500 tracking-wider mb-1">播放列表</div>
          <h1 className="text-[24px] font-bold truncate">{pl.name}</h1>
          <div className="text-[12.5px] text-zinc-500 mt-1.5">{list.length} 首曲目</div>
          <div className="flex items-center gap-2 mt-3">
            <button className="btn-primary" onClick={() => playTracks(list, 0)}>
              <Play size={13} className="fill-current" />
              播放全部
            </button>
            <button
              className="btn-secondary"
              onClick={() => playTracks(list, Math.floor(Math.random() * Math.max(1, list.length)))}
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

      <TrackList
        tracks={list}
        emptyHint="列表里还没有歌曲，右键曲目可添加"
        onRemoveFromPlaylist={(tid) => removeFromPlaylist(pl.id, tid)}
      />
    </div>
  );
}
