import { useMemo, useState } from "react";
import { Clock3, Heart, Library, ListMusic, Play, Shuffle } from "lucide-react";
import { useStore } from "../store";
import TrackList, { type SortKey } from "../components/TrackList";
import { matchSearch } from "../utils";
import type { TrackMeta } from "../types";

type Mode = "library" | "liked" | "recent";

const SORTS: { key: SortKey; label: string }[] = [
  { key: "added", label: "添加时间" },
  { key: "title", label: "标题" },
  { key: "artist", label: "艺术家" },
  { key: "album", label: "专辑" },
  { key: "duration", label: "时长" },
  { key: "plays", label: "播放次数" },
];

const META: Record<Mode, { title: string; icon: typeof Library }> = {
  library: { title: "资料库", icon: Library },
  liked: { title: "我喜欢", icon: Heart },
  recent: { title: "最近播放", icon: Clock3 },
};

export default function LibraryView({ mode }: { mode: Mode }) {
  const tracks = useStore((s) => s.tracks);
  const search = useStore((s) => s.search);
  const scan = useStore((s) => s.scan);
  const playTracks = useStore((s) => s.playTracks);
  const folders = useStore((s) => s.folders);
  const addFolderByDialog = useStore((s) => s.addFolderByDialog);
  const [sortKey, setSortKey] = useState<SortKey>(mode === "recent" ? "plays" : "added");
  const [dir, setDir] = useState<1 | -1>(-1);

  const filtered = useMemo(() => {
    let list: TrackMeta[];
    if (mode === "liked") list = tracks.filter((t) => t.liked);
    else if (mode === "recent")
      list = tracks.filter((t) => t.lastPlayed > 0);
    else list = tracks;
    list = list.filter((t) => matchSearch(t, search));

    const cmp = (a: TrackMeta, b: TrackMeta) => {
      switch (sortKey) {
        case "added":
          return a.id - b.id;
        case "title":
          return a.title.localeCompare(b.title, "zh");
        case "artist":
          return (
            a.artist.localeCompare(b.artist, "zh") ||
            a.album.localeCompare(b.album, "zh") ||
            a.trackNo - b.trackNo
          );
        case "album":
          return (
            a.album.localeCompare(b.album, "zh") || a.trackNo - b.trackNo
          );
        case "duration":
          return a.duration - b.duration;
        case "plays":
          return a.lastPlayed - b.lastPlayed || a.playCount - b.playCount;
      }
    };
    return [...list].sort((a, b) => dir * cmp(a, b));
  }, [tracks, mode, search, sortKey, dir]);

  const { title, icon: Icon } = META[mode];

  const onSort = (k: SortKey) => {
    if (k === sortKey) setDir((d) => (d === 1 ? -1 : 1));
    else {
      setSortKey(k);
      setDir(k === "title" || k === "artist" || k === "album" ? 1 : -1);
    }
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="pt-5 pb-3 px-5">
        <div className="flex items-end justify-between">
          <div>
            <h1 className="text-[22px] font-bold flex items-center gap-2.5">
              <Icon size={20} className="text-[var(--dyn)]" />
              {title}
              <span className="text-[13px] font-normal text-zinc-500 mb-0.5">
                {filtered.length} 首
              </span>
            </h1>
            {mode === "library" && !folders.length && (
              <p className="text-[12.5px] text-zinc-500 mt-1">
                添加音乐文件夹，或直接把文件 / 文件夹拖进窗口
              </p>
            )}
          </div>
          {filtered.length > 0 && (
            <div className="flex items-center gap-2">
              <button
                className="btn-secondary"
                onClick={() => playTracks(filtered, Math.floor(Math.random() * filtered.length))}
              >
                <Shuffle size={13} />
                随机播放
              </button>
              <button className="btn-primary" onClick={() => playTracks(filtered, 0)}>
                <Play size={13} className="fill-current" />
                播放全部
              </button>
            </div>
          )}
        </div>

        <div className="flex items-center gap-1 mt-3.5">
          <span className="text-[11px] text-zinc-600 mr-1">排序</span>
          {SORTS.map((s) => (
            <button
              key={s.key}
              onClick={() => onSort(s.key)}
              className={`h-6.5 px-2.5 py-1 rounded-full text-[11.5px] transition-colors ${
                sortKey === s.key
                  ? "bg-white/[0.1] text-zinc-100"
                  : "text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.05]"
              }`}
            >
              {s.label}
              {sortKey === s.key && (
                <span className="ml-1">{dir === 1 ? "↑" : "↓"}</span>
              )}
            </button>
          ))}
          {mode === "library" && scan.active && (
            <span className="ml-2 text-[11.5px] text-[var(--dyn)]">
              扫描中… {scan.total ? `${scan.done}/${scan.total}` : ""}
            </span>
          )}
        </div>
      </header>

      <TrackList
        tracks={filtered}
        emptyHint={
          mode === "library"
            ? "资料库还是空的"
            : mode === "liked"
              ? "还没有喜欢的音乐"
              : "还没有播放记录"
        }
        emptyAction={
          mode === "library"
            ? { label: "添加音乐文件夹", onClick: addFolderByDialog }
            : mode === "liked"
              ? {
                  label: "去资料库逛逛",
                  onClick: () => useStore.getState().setView("library"),
                }
              : undefined
        }
      />
      {mode === "library" && !tracks.length && !scan.active && folders.length > 0 && (
        <div className="px-5 pb-4 text-[12px] text-zinc-600">
          已有 {folders.length} 个音乐文件夹但未找到音频，点击设置里重新扫描试试
        </div>
      )}
    </div>
  );
}
