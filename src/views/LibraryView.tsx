import { useMemo, useState } from "react";
import { Clock3, Heart, Library, Play, Shuffle } from "lucide-react";
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

const META: Record<Mode, { title: string; sub: string; icon: typeof Library }> = {
  library: { title: "资料库", sub: "你的全部音乐", icon: Library },
  liked: { title: "我喜欢", sub: "收藏的心动之歌", icon: Heart },
  recent: { title: "最近播放", sub: "刚刚听过的旋律", icon: Clock3 },
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
    else if (mode === "recent") list = tracks.filter((t) => t.lastPlayed > 0);
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
          return a.album.localeCompare(b.album, "zh") || a.trackNo - b.trackNo;
        case "duration":
          return a.duration - b.duration;
        case "plays":
          return a.lastPlayed - b.lastPlayed || a.playCount - b.playCount;
      }
    };
    return [...list].sort((a, b) => dir * cmp(a, b));
  }, [tracks, mode, search, sortKey, dir]);

  const { title, sub, icon: Icon } = META[mode];
  const totalSecs = Math.floor(filtered.reduce((a, t) => a + t.duration, 0));
  const totalDesc =
    totalSecs >= 3600
      ? `${Math.floor(totalSecs / 3600)} 小时 ${Math.floor((totalSecs % 3600) / 60)} 分钟`
      : `${Math.floor(totalSecs / 60)} 分钟`;

  const onSort = (k: SortKey) => {
    if (k === sortKey) setDir((d) => (d === 1 ? -1 : 1));
    else {
      setSortKey(k);
      setDir(k === "title" || k === "artist" || k === "album" ? 1 : -1);
    }
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {/* 头部 */}
      <header className="px-8 pt-7 pb-5">
        <div className="flex items-end justify-between gap-6">
          <div className="min-w-0 anim-rise">
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-3)] tracking-[0.24em] mb-2">
              <Icon size={13} />
              {sub}
              {mode === "library" && scan.active && (
                <span className="text-[var(--accent)] tracking-normal flex items-center gap-1.5">
                  · <LoaderSpin /> 扫描中 {scan.total ? `${scan.done}/${scan.total}` : ""}
                </span>
              )}
            </div>
            <h1 className="text-[30px] font-extrabold leading-none tracking-tight text-[var(--ink)]">
              {title}
            </h1>
            <div className="flex items-center gap-3 mt-3 text-[12.5px] text-[var(--ink-2)]">
              <span className="tabular-nums">{filtered.length} 首</span>
              {totalSecs > 0 && (
                <>
                  <span className="w-1 h-1 rounded-full bg-[var(--ink-3)]" />
                  <span className="tabular-nums">{totalDesc}</span>
                </>
              )}
            </div>
            {mode === "library" && !folders.length && !scan.active && (
              <p className="text-[12.5px] text-[var(--ink-3)] mt-3">
                添加音乐文件夹，或直接把文件 / 文件夹拖进窗口
              </p>
            )}
          </div>

          {filtered.length > 0 && (
            <div className="flex items-center gap-3 shrink-0 anim-rise">
              <button
                className="btn-secondary"
                onClick={() =>
                  playTracks(filtered, Math.floor(Math.random() * filtered.length))
                }
              >
                <Shuffle size={14} />
                随机播放
              </button>
              <button className="btn-primary" onClick={() => playTracks(filtered, 0)}>
                <Play size={14} className="fill-current" />
                播放全部
              </button>
            </div>
          )}
        </div>

        {/* 排序 */}
        <div className="flex items-center gap-2 mt-5">
          <span className="text-[11.5px] text-[var(--ink-3)] mr-1">排序</span>
          {SORTS.map((s) => (
            <button
              key={s.key}
              onClick={() => onSort(s.key)}
              className={`chip ${
                sortKey === s.key
                  ? "bg-[rgba(240,162,74,0.14)] text-[var(--accent-strong)] font-medium"
                  : "text-[var(--ink-3)] hover:text-[var(--ink-2)] hover:bg-white/[0.05]"
              }`}
            >
              {s.label}
              {sortKey === s.key && (
                <span className="ml-0.5">{dir === 1 ? "↑" : "↓"}</span>
              )}
            </button>
          ))}
        </div>
      </header>

      {/* 曲目卡片 */}
      <div className="flex-1 min-h-0 flex flex-col px-6 pb-4">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {filtered.length > 0 && (
            <div className="grid grid-cols-[56px_minmax(200px,460px)_minmax(140px,300px)_92px_96px] items-center gap-4 h-10 px-5 border-b border-[var(--line)] text-[10.5px] text-[var(--ink-3)] tracking-[0.18em]">
              <span className="text-center">序号</span>
              <span>歌曲</span>
              <span>专辑</span>
              <span className="text-right">时长</span>
              <span className="text-right">操作</span>
            </div>
          )}
          <TrackList
            tracks={filtered}
            inCard
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
        </div>
      </div>
    </div>
  );
}

function LoaderSpin() {
  return (
    <svg
      className="animate-spin"
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
    >
      <path d="M21 12a9 9 0 1 1-6.219-8.56" />
    </svg>
  );
}
