import { useEffect, useMemo, useState } from "react";
import { ArrowLeft, Play, Shuffle } from "lucide-react";
import { useStore } from "../store";
import TrackList from "../components/TrackList";
import { api } from "../api";
import type { PlaylistEntryMeta } from "../types";

const qqCover = (albumMid: string) =>
  albumMid
    ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${albumMid}.jpg`
    : "";

/** 歌手 / 专辑详情页：聚合本地匹配曲目 + 网易云/QQ/酷狗在线搜索结果，
 *  行渲染与播放、收藏、下载等操作复用 TrackList；点击行内歌手/专辑可继续跳转 */
export default function ArtistAlbumView({
  kind,
}: {
  kind: "artist" | "album";
}) {
  const name = useStore((s) => s.detailName);
  const tracks = useStore((s) => s.tracks);
  const popDetailPage = useStore((s) => s.popDetailPage);
  const openDetailPage = useStore((s) => s.openDetailPage);
  const playEntries = useStore((s) => s.playEntries);

  const [onlineLoading, setOnlineLoading] = useState(true);
  const [onlineEntries, setOnlineEntries] = useState<PlaylistEntryMeta[]>([]);

  // 本地曲库匹配（大小写不敏感的精确匹配：歌手匹配 artist/albumArtist，专辑匹配 album）
  const localMatches = useMemo(() => {
    const n = name.toLowerCase();
    if (kind === "artist") {
      return tracks.filter(
        (t) =>
          t.artist.toLowerCase() === n || t.albumArtist.toLowerCase() === n
      );
    }
    return tracks.filter((t) => t.album.toLowerCase() === n);
  }, [tracks, name, kind]);

  // 在线搜索（匿名可用；失败静默降级为只有本地结果）
  useEffect(() => {
    if (!name) return;
    let dead = false;
    setOnlineLoading(true);
    setOnlineEntries([]);
    (async () => {
      const [net, qq, kg] = await Promise.allSettled([
        api.neteaseSearch(name, 0),
        api.qqSearch(name, 1),
        api.kugouSearch(name, 1),
      ]);
      if (dead) return;
      const out: PlaylistEntryMeta[] = [];
      const seen = new Set<string>();
      const push = (e: PlaylistEntryMeta) => {
        const k = `${e.kind}:${e.onlineId}`;
        if (!seen.has(k)) {
          seen.add(k);
          out.push(e);
        }
      };
      // 单个源异常（接口变更/响应异常）只跳过该源，不影响其它源的结果
      if (net.status === "fulfilled") {
        try {
          for (const t of net.value?.songs ?? []) {
            push({
              rowid: 0,
              kind: "netease",
              trackId: null,
              onlineId: String(t.id),
              title: t.name,
              artist: t.ar.map((a) => a.name).join(" / "),
              album: t.al?.name ?? "",
              cover: t.al?.picUrl ?? "",
              duration: t.dt / 1000,
              vip: t.fee === 1,
            });
          }
        } catch {
          /* 忽略该源 */
        }
      }
      if (qq.status === "fulfilled") {
        try {
          for (const s of qq.value?.songs ?? []) {
            push({
              rowid: 0,
              kind: "qq",
              trackId: null,
              onlineId: s.id,
              title: s.name,
              artist: s.singer,
              album: s.album,
              cover: qqCover(s.albumMid),
              duration: s.durationMs / 1000,
              vip: s.vip,
              mediaMid: s.mediaMid,
            });
          }
        } catch {
          /* 忽略该源 */
        }
      }
      if (kg.status === "fulfilled") {
        try {
          for (const s of kg.value?.songs ?? []) {
            push({
              rowid: 0,
              kind: "kugou",
              trackId: null,
              onlineId: s.id,
              title: s.name,
              artist: s.singer,
              album: s.album,
              cover: s.cover,
              duration: s.durationMs / 1000,
              vip: s.vip,
            });
          }
        } catch {
          /* 忽略该源 */
        }
      }
      setOnlineEntries(out);
      setOnlineLoading(false);
    })();
    return () => {
      dead = true;
    };
  }, [name]);

  // 整页可播序列：本地在前、在线在后
  const allEntries: PlaylistEntryMeta[] = useMemo(
    () => [
      ...localMatches.map(
        (t): PlaylistEntryMeta => ({
          rowid: 0,
          kind: "local",
          trackId: t.id,
          onlineId: null,
          title: t.title,
          artist: t.artist,
          album: t.album,
          cover: t.cover,
          duration: t.duration,
        })
      ),
      ...onlineEntries,
    ],
    [localMatches, onlineEntries]
  );

  const playAll = (random: boolean) => {
    if (!allEntries.length) return;
    playEntries(
      allEntries,
      random ? Math.floor(Math.random() * allEntries.length) : 0
    );
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="px-8 pt-7 pb-5">
        <div className="flex items-end justify-between gap-5">
          <div className="min-w-0 anim-rise">
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-3)] tracking-[0.24em] mb-2">
              <button
                className="btn-ghost w-7 h-7 -ml-1.5"
                onClick={popDetailPage}
                title="返回上一页"
              >
                <ArrowLeft size={15} />
              </button>
              {kind === "artist" ? "歌手" : "专辑"}
            </div>
            <h1 className="text-[30px] font-extrabold leading-none tracking-tight text-[var(--ink)] truncate">
              {name}
            </h1>
            <div className="flex items-center gap-3 mt-3 text-[12.5px] text-[var(--ink-2)]">
              <span className="tabular-nums">
                本地 {localMatches.length} 首
              </span>
              <span className="w-1 h-1 rounded-full bg-[var(--ink-3)]" />
              <span className="tabular-nums">
                {onlineLoading ? "在线搜索中…" : `在线 ${onlineEntries.length} 首`}
              </span>
            </div>
          </div>
          {allEntries.length > 0 && (
            <div className="flex items-center gap-3 shrink-0 anim-rise">
              <button className="btn-secondary" onClick={() => playAll(true)}>
                <Shuffle size={14} />
                随机播放
              </button>
              <button className="btn-primary" onClick={() => playAll(false)}>
                <Play size={14} className="fill-current" />
                播放全部
              </button>
            </div>
          )}
        </div>
      </header>

      <div className="flex-1 min-h-0 flex flex-col px-6 pb-[86px]">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {(localMatches.length > 0 || onlineEntries.length > 0) && (
            <div className="grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-10 px-5 border-b border-[var(--line)] text-[10.5px] text-[var(--ink-3)] tracking-[0.18em]">
              <span className="text-center">序号</span>
              <span>歌曲</span>
              <span>专辑</span>
              <span className="text-right">时长</span>
              <span className="text-right">操作</span>
            </div>
          )}
          <TrackList
            tracks={localMatches}
            inCard
            onlineEntries={onlineEntries}
            onMetaClick={(field, text) => openDetailPage(field, text)}
            emptyHint={`本地曲库中没有该${kind === "artist" ? "歌手" : "专辑"}的歌曲${
              onlineLoading ? "，正在搜索在线曲库…" : "，在线曲库也没有找到"
            }`}
          />
        </div>
      </div>
    </div>
  );
}
