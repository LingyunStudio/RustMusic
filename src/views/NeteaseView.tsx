import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  ArrowLeft,
  Cloud,
  Heart,
  Info,
  ListChecks,
  ListPlus,
  LogOut,
  MoreHorizontal,
  Play,
  Search,
  ShieldCheck,
  Loader2,
  Shuffle,
  X,
  Check,
  Download,
  History,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { useVirtualWindow } from "../hooks/useVirtualWindow";
import type {
  KgSong,
  NeteaseTrack,
  OnlineNavSnapshot,
  OnlineRecState,
  QqSong,
} from "../types";
import { clampMenuPos, fmtTime } from "../utils";
import Modal from "../components/Modal";

type Source = "netease" | "qq" | "kugou";

interface OnlineRow {
  kind: Source;
  id: number | string;
  name: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
  vip: boolean;
  mediaMid: string;
}

const qqCover = (albumMid: string) =>
  albumMid
    ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${albumMid}.jpg`
    : "";

/** 推荐内容状态 OnlineRecState 定义在 types.ts / store：跳转歌手/专辑页后
 *  返回时按源还原当时的推荐列表（此前存组件 state，跳转即丢） */

/** 在线搜索历史（localStorage，跨会话保留，最多 12 条） */
const SEARCH_HISTORY_KEY = "rustmusic_search_history";
const loadSearchHistory = (): string[] => {
  try {
    const raw = localStorage.getItem(SEARCH_HISTORY_KEY);
    const list = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(list)
      ? list.filter((x): x is string => typeof x === "string")
      : [];
  } catch {
    return [];
  }
};

export default function OnlineLibraryView({ source }: { source: Source }) {
  const qqPage = useStore((s) => s.qqPage);
  const kugouPage = useStore((s) => s.kugouPage);
  const neteaseResults = useStore((s) => s.neteaseResults);
  const neteaseSearching = useStore((s) => s.neteaseSearching);
  const neteaseSearched = useStore((s) => s.neteaseSearched);
  const neteaseTotal = useStore((s) => s.neteaseTotal);
  const qqResults = useStore((s) => s.qqResults);
  const qqSearching = useStore((s) => s.qqSearching);
  const qqSearched = useStore((s) => s.qqSearched);
  const kugouResults = useStore((s) => s.kugouResults);
  const kugouSearching = useStore((s) => s.kugouSearching);
  const kugouSearched = useStore((s) => s.kugouSearched);
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const savedOnline = useStore((s) => s.savedOnline);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);
  const addOnlineToPlaylist = useStore((s) => s.addOnlineToPlaylist);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const playNetease = useStore((s) => s.playNetease);
  const playQq = useStore((s) => s.playQq);
  const playKugou = useStore((s) => s.playKugou);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const neteaseSearch = useStore((s) => s.neteaseSearch);
  const kugouSearch = useStore((s) => s.kugouSearch);
  const qqSearch = useStore((s) => s.qqSearch);
  const neteaseLoggedIn = useStore((s) => s.neteaseLoggedIn);
  const neteaseNickname = useStore((s) => s.neteaseNickname);
  const qqLoggedIn = useStore((s) => s.qqLoggedIn);
  const qqNickname = useStore((s) => s.qqNickname);
  const neteaseLogout = useStore((s) => s.neteaseLogout);
  const qqLogout = useStore((s) => s.qqLogout);
  const playlists = useStore((s) => s.playlists);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const importNeteasePlaylist = useStore((s) => s.importNeteasePlaylist);
  const importQqPlaylist = useStore((s) => s.importQqPlaylist);
  const importAllPlaylists = useStore((s) => s.importAllPlaylists);
  const openDetailPage = useStore((s) => s.openDetailPage);
  const toast = useStore((s) => s.toast);

  const [kw, setKw] = useState(useStore.getState().lastKw[source] ?? "");
  const [menu, setMenu] = useState<{ x: number; y: number; row: OnlineRow } | null>(
    null
  );
  // 推荐态：随机歌单/榜单加载后接管列表展示，搜索时清空回到搜索态。
  // 存在 store（按源）：跳转歌手/专辑页再返回时还原当时的推荐列表
  const rec = useStore((s) => s.onlineRec[source]);
  const setRec = (r: OnlineRecState | null) =>
    useStore.getState().setOnlineRec(source, r);
  const [recLoading, setRecLoading] = useState(false);
  const [toplists, setToplists] = useState<
    { id: number; name: string; cover: string }[]
  >([]);
  // 批量选择：key = `${kind}-${id}`，与行渲染 key 一致
  const [sel, setSel] = useState<Set<string>>(new Set());
  // 批量加入播放列表的待选行（非空时打开歌单选择弹窗）
  const [pickerRows, setPickerRows] = useState<OnlineRow[] | null>(null);
  const [batchMode, setBatchMode] = useState(false);
  const [searchHistory, setSearchHistory] = useState<string[]>(loadSearchHistory);

  // ---------- 视图内导航历史（点歌手/专辑/搜索后可返回上一页） ----------
  // 栈存 store（按源）：跳转歌手/专辑页往返后，之前的搜索历史依然可逐步返回
  const navStack = useStore((s) => s.onlineNav[source]);
  const [qrSource, setQrSource] = useState<Source | null>(null);
  const [newPlName, setNewPlName] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importList, setImportList] = useState<
    { id: number; name: string; trackCount: number }[] | null
  >(null);
  const [importing, setImporting] = useState(false);
  const [importProgress, setImportProgress] = useState<{
    done: number;
    total: number;
    name: string;
  } | null>(null);

  const searching =
    source === "netease"
      ? neteaseSearching
      : source === "qq"
        ? qqSearching
        : kugouSearching;
  const searched =
    source === "netease"
      ? neteaseSearched
      : source === "qq"
        ? qqSearched
        : kugouSearched;
  // 酷狗匿名可用，无需登录
  const loggedIn =
    source === "netease" ? neteaseLoggedIn : source === "qq" ? qqLoggedIn : true;
  const nickname =
    source === "netease"
      ? neteaseNickname
      : source === "qq"
        ? qqNickname
        : "免费畅听";
  const sourceName =
    source === "netease" ? "网易云" : source === "qq" ? "QQ 音乐" : "酷狗";
  // 随机推荐 / 榜单目前只接了网易云与 QQ（匿名接口），酷狗无此能力
  const recommendable = source === "netease" || source === "qq";

  // 推荐接口预取：榜单 chip 数据量小、匿名可拉，进视图静默加载；
  // 失败只影响推荐入口，不打扰搜索主流程。
  // 注意：此 effect 只在挂载时执行一次（App 对三个源是独立挂载的实例）。
  // 不要在这里清导航栈/推荐内容——它们按源存 store，清掉会毁掉
  // 「跳转歌手/专辑页 → 返回」的还原链（此前正是这个自毁导致无法返回）
  useEffect(() => {
    setKw(useStore.getState().lastKw[source] ?? "");
    if (!recommendable) return;
    let dead = false;
    (async () => {
      try {
        if (source === "netease") {
          const r = await api.neteaseToplists();
          if (!dead)
            setToplists(
              (r.toplists ?? []).map((t) => ({
                id: t.id,
                name: t.name,
                cover: t.cover,
              }))
            );
        } else {
          const r = await api.qqToplists();
          if (!dead)
            setToplists(
              (r.toplists ?? []).map((t) => ({
                id: t.id,
                name: t.title,
                cover: t.pic,
              }))
            );
        }
      } catch {
        /* 榜单入口拿不到就隐藏，不报错 */
      }
    })();
    return () => {
      dead = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source]);

  // 推荐曲目写入 store 缓存：与搜索结果一致，保证“下一首播放/加入队列”
  // 在从未播放过的情况下也能解析出曲目信息
  const cacheRecSongs = (r: OnlineRecState) => {
    if (r.netease) {
      useStore.setState((s) => ({
        neteaseCache: {
          ...s.neteaseCache,
          ...Object.fromEntries(r.netease!.map((t) => [t.id, t])),
        },
      }));
    } else if (r.qq) {
      useStore.setState((s) => ({
        qqCache: {
          ...s.qqCache,
          ...Object.fromEntries(r.qq!.map((t) => [t.id, t])),
        },
      }));
    }
  };

  const loadRandom = async () => {
    if (recLoading) return;
    setRecLoading(true);
    try {
      if (source === "netease") {
        const r = await api.neteaseRandomPlaylist();
        const next: OnlineRecState = {
          origin: "random",
          title: r.name,
          cover: r.cover,
          subtitle: r.creator,
          playlistId: r.id,
          netease: r.songs,
        };
        cacheRecSongs(next);
        setRec(next);
      } else {
        const r = await api.qqRandomPlaylist();
        const next: OnlineRecState = {
          origin: "random",
          title: r.name,
          cover: r.cover,
          subtitle: r.creator,
          playlistId: r.id,
          qq: r.songs,
        };
        cacheRecSongs(next);
        setRec(next);
      }
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setRecLoading(false);
    }
  };

  const loadToplist = async (t: { id: number; name: string; cover: string }) => {
    if (recLoading) return;
    setRecLoading(true);
    try {
      const r =
        source === "netease"
          ? await api.neteaseToplistTracks(t.id)
          : await api.qqToplistTracks(t.id);
      const next: OnlineRecState =
        source === "netease"
          ? {
              origin: "top",
              title: t.name,
              cover: t.cover,
              subtitle: `${sourceName}官方榜单`,
              // 网易云榜单 ID 即歌单 ID，可整单收藏
              playlistId: t.id,
              netease: r.songs as NeteaseTrack[],
            }
          : {
              origin: "top",
              title: t.name,
              cover: t.cover,
              subtitle: `${sourceName}官方榜单`,
              qq: r.songs as QqSong[],
            };
      cacheRecSongs(next);
      setRec(next);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setRecLoading(false);
    }
  };

  // 每日推荐 / 私人 FM：仅网易云且已登录可用
  const loadNeteaseFeed = async (
    kind: "daily" | "fm",
    loader: () => Promise<{ songs: NeteaseTrack[] }>
  ) => {
    if (recLoading) return;
    if (!neteaseLoggedIn) {
      toast("请先扫码登录网易云账号", "error");
      return;
    }
    setRecLoading(true);
    try {
      const r = await loader();
      const next: OnlineRecState =
        kind === "daily"
          ? {
              origin: "daily",
              title: "每日推荐",
              cover: "",
              subtitle: "根据你的听歌口味生成",
              netease: r.songs,
            }
          : {
              origin: "fm",
              title: "私人 FM",
              cover: "",
              subtitle: "换个批次继续听",
              netease: r.songs,
            };
      cacheRecSongs(next);
      setRec(next);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setRecLoading(false);
    }
  };

  // 把当前推荐歌单整单收藏为本地播放列表（复用导入合并逻辑，去重不乱序）
  const saveRecPlaylist = async () => {
    if (!rec?.playlistId || recLoading) return;
    try {
      if (source === "netease") await importNeteasePlaylist(rec.playlistId, rec.title);
      else await importQqPlaylist(rec.playlistId, rec.title);
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const rows: OnlineRow[] = useMemo(() => {
    // 推荐态优先：随机歌单/榜单与搜索结果同构，行渲染与操作全部复用
    if (rec?.netease) {
      return rec.netease.map((t) => ({
        kind: "netease" as const,
        id: t.id,
        name: t.name,
        artist: t.ar.map((a) => a.name).join(" / "),
        album: t.al?.name ?? "",
        cover: t.al?.picUrl ?? "",
        durationMs: t.dt,
        vip: t.fee === 1,
        mediaMid: "",
      }));
    }
    if (rec?.qq) {
      return rec.qq.map((t) => ({
        kind: "qq" as const,
        id: t.id,
        name: t.name,
        artist: t.singer,
        album: t.album,
        cover: qqCover(t.albumMid),
        durationMs: t.durationMs,
        vip: t.vip,
        mediaMid: t.mediaMid,
      }));
    }
    if (source === "netease") {
      return neteaseResults.map((t: NeteaseTrack) => ({
        kind: "netease" as const,
        id: t.id,
        name: t.name,
        artist: t.ar.map((a) => a.name).join(" / "),
        album: t.al?.name ?? "",
        cover: t.al?.picUrl ?? "",
        durationMs: t.dt,
        vip: t.fee === 1,
        mediaMid: "",
      }));
    }
    if (source === "kugou") {
      return kugouResults.map((t) => ({
        kind: "kugou" as const,
        id: t.id,
        name: t.name,
        artist: t.singer,
        album: t.album,
        cover: t.cover,
        durationMs: t.durationMs,
        vip: t.vip,
        mediaMid: "",
      }));
    }
    return qqResults.map((t: QqSong) => ({
      kind: "qq" as const,
      id: t.id,
      name: t.name,
      artist: t.singer,
      album: t.album,
      cover: qqCover(t.albumMid),
      durationMs: t.durationMs,
      vip: t.vip,
      mediaMid: t.mediaMid,
    }));
  }, [rec, source, neteaseResults, qqResults, kugouResults]);

  // 记录搜索历史（去重置顶，最多 12 条，localStorage 跨会话）
  const rememberSearch = (text: string) => {
    const kwTrim = text.trim();
    if (!kwTrim) return;
    setSearchHistory((prev) => {
      const next = [kwTrim, ...prev.filter((x) => x !== kwTrim)].slice(0, 12);
      try {
        localStorage.setItem(SEARCH_HISTORY_KEY, JSON.stringify(next));
      } catch {
        /* 隐私模式等场景写不进就算了 */
      }
      return next;
    });
  };

  // 发起搜索前快照当前视图（推荐内容 + 各源搜索结果），供返回按钮恢复
  const snapshotNav = (): OnlineNavSnapshot => ({
    rec,
    kw,
    neteaseResults,
    neteaseTotal,
    neteaseSearched,
    qqResults,
    qqSearched,
    qqPage,
    kugouResults,
    kugouSearched,
    kugouPage,
  });
  const pushNav = () => useStore.getState().pushOnlineNav(source, snapshotNav());

  const goBack = () => {
    const prev = useStore.getState().popOnlineNav(source);
    if (!prev) return;
    setRec(prev.rec);
    setKw(prev.kw);
    setSel(new Set());
    useStore.setState({
      neteaseResults: prev.neteaseResults,
      neteaseTotal: prev.neteaseTotal,
      neteaseSearched: prev.neteaseSearched,
      qqResults: prev.qqResults,
      qqSearched: prev.qqSearched,
      qqPage: prev.qqPage,
      kugouResults: prev.kugouResults,
      kugouSearched: prev.kugouSearched,
      kugouPage: prev.kugouPage,
    });
  };

  const submit = (query?: string) => {
    const q = (typeof query === "string" ? query : kw).trim();
    if (!q) return;
    pushNav();
    setRec(null);
    setSel(new Set());
    rememberSearch(q);
    useStore.getState().setLastKw(source, q);
    if (source === "netease") neteaseSearch(q);
    else if (source === "qq") qqSearch(q);
    else kugouSearch(q);
  };

  // 搜索结果随“加载更多”无上限增长：窗口化渲染（行高 60px 恒定）
  const ROW_H = 60;
  const win = useVirtualWindow(rows.length, ROW_H);

  const playRow = (i: number) => {
    if (rec) {
      // 推荐态：队列 = 当前推荐列表（曲目信息已在加载时写入缓存）
      if (rec.netease) playNetease(rec.netease, i);
      else if (rec.qq) playQq(rec.qq, i);
      return;
    }
    if (source === "netease") playNetease(neteaseResults, i);
    else if (source === "kugou") playKugou(kugouResults, i);
    else playQq(qqResults, i);
  };

  const menuAction = (row: OnlineRow, action: "play" | "next" | "queue") => {
    if (action === "play") {
      const idx = Math.max(
        0,
        rows.findIndex((t) => t.kind === row.kind && t.id === row.id)
      );
      playRow(idx);
      return;
    }
    const q =
      row.kind === "netease"
        ? { kind: "netease" as const, id: row.id as number }
        : { kind: row.kind as "qq" | "kugou", id: row.id as string };
    if (action === "next") playNext(q);
    else addToQueue(q);
  };

  // ---------- 批量操作（多选行 → 加入歌单 / 队列 / 下载） ----------
  const selKeyOf = (row: OnlineRow) => `${row.kind}-${row.id}`;
  const toggleSel = (row: OnlineRow) =>
    setSel((prev) => {
      const next = new Set(prev);
      const k = selKeyOf(row);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next;
    });
  const selectedRows = rows.filter((r) => sel.has(selKeyOf(r)));

  const batchQueue = () => {
    for (const row of selectedRows) menuAction(row, "queue");
    toast(`已把 ${selectedRows.length} 首加入队列`, "success");
    setSel(new Set());
    setBatchMode(false);
  };

  const batchDownload = async () => {
    const list = [...selectedRows];
    setSel(new Set());
    setBatchMode(false);
    for (const row of list) {
      try {
        await downloadOnline({
          kind: row.kind,
          id: row.id,
          name: row.name,
          artist: row.artist,
          album: row.album,
          cover: row.cover,
          durationMs: row.durationMs,
          mediaMid: row.mediaMid,
        });
      } catch (e) {
        toast(`下载「${row.name}」失败：${String(e)}`, "error");
      }
    }
  };

  // 批量加入播放列表：弹出歌单选择（选择后逐条写入，store 内部按在线条目去重）
  const batchAddToPlaylist = () => {
    if (!selectedRows.length) return;
    setPickerRows(selectedRows);
  };

  const hasMore =
    !rec &&
    (source === "netease"
      ? rows.length < neteaseTotal
      : source === "kugou"
        ? rows.length > 0 && rows.length === 30 * kugouPage
        : rows.length > 0 && rows.length === 30 * qqPage);

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {/* 头部：标题 | 搜索 | 登录（同一行区域） */}
      <header className="px-8 pt-7 pb-4">
        <div className="flex items-end justify-between gap-5">
          <div className="min-w-0 anim-rise shrink-0">
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-3)] tracking-[0.24em] mb-2">
              <Cloud size={13} />
              在线曲库
            </div>
            <h1 className="text-[30px] font-extrabold leading-none tracking-tight text-[var(--ink)]">
              搜一下，就能听
            </h1>
            <div className="flex items-center gap-3 mt-3 text-[12.5px] text-[var(--ink-2)]">
              <span className="tabular-nums">{rows.length} 条结果</span>
            </div>
          </div>

          {/* 搜索（与标题同行）；有导航历史时显示返回按钮 */}
          <div className="flex items-center gap-2.5 flex-1 max-w-[520px] mb-1">
            {navStack.length > 0 && (
              <button
                className="btn-ghost w-10 h-10 shrink-0"
                onClick={goBack}
                title="返回上一页"
              >
                <ArrowLeft size={16} />
              </button>
            )}
            <div className="relative flex-1">
              <Search
                size={14}
                className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
              />
              <input
                type="text"
                value={kw}
                onChange={(e) => setKw(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && submit()}
                placeholder={
                  source === "netease"
                    ? "搜索网易云曲库…"
                    : source === "qq"
                      ? "搜索 QQ 音乐曲库…"
                      : "搜索酷狗曲库…"
                }
                className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] pl-9 pr-3 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)] focus:border-[rgba(240,162,74,0.45)] transition-colors"
              />
            </div>
            <button
              className="btn-primary h-10"
              onClick={() => submit()}
              disabled={searching}
            >
              {searching ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Search size={13} />
              )}
              搜索
            </button>
            <button
              className={`btn-ghost w-10 h-10 ${
                batchMode ? "text-[var(--accent)]" : ""
              }`}
              onClick={() => {
                setBatchMode(!batchMode);
                setSel(new Set());
              }}
              title={batchMode ? "退出多选" : "批量选择：加入播放列表 / 队列 / 下载"}
            >
              <ListChecks size={17} />
            </button>
          </div>

          {/* 登录状态（酷狗免登录） */}
          <div className="shrink-0 flex flex-col items-end gap-2 mb-1">
            {source === "kugou" ? (
              <span
                className="chip text-[var(--ink-2)]"
                style={{ background: "var(--shade)" }}
                title="酷狗暂不支持登录：免费曲目可直接播放，标有 VIP 的曲目暂时无法播放"
              >
                <Info size={13} />
                登录暂未支持 · 仅非 VIP 曲目可播
              </span>
            ) : loggedIn ? (
              <>
                <span
                  className="chip text-[var(--accent-strong)]"
                  style={{ background: "rgba(240,162,74,0.12)" }}
                  title={`已登录${sourceName}账号`}
                >
                  <ShieldCheck size={13} />
                  {nickname || "已登录"}
                </span>
                <div className="flex items-center gap-2">
                  <button
                    className="btn-secondary !py-1.5 !px-3"
                    onClick={async () => {
                      setImportList(null);
                      setImportOpen(true);
                      try {
                        const list =
                          source === "netease"
                            ? await api.neteaseUserPlaylists()
                            : await api.qqUserPlaylists();
                        setImportList(list);
                      } catch (e) {
                        setImportOpen(false);
                        toast(String(e), "error");
                      }
                    }}
                  >
                    导入歌单
                  </button>
                  <button
                    className="btn-secondary !py-1.5 !px-3"
                    onClick={() => (source === "netease" ? neteaseLogout() : qqLogout())}
                  >
                    退出
                  </button>
                </div>
              </>
            ) : (
              <button
                className="btn-secondary !py-1.5 !px-3"
                onClick={() => setQrSource(source)}
              >
                扫码登录
              </button>
            )}
          </div>
        </div>
      </header>

      {/* 结果列表（底边界抬到播放条上方，留 4px 空隙：播放条总占位 64+16+4=84px） */}
      <div className="flex-1 min-h-0 flex flex-col px-6 pb-[86px]">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {/* 推荐横幅：随机歌单/榜单接管列表时的来源说明与操作 */}
          {rec && (
            <div className="flex items-center gap-4 px-5 py-4 border-b border-[var(--line)] shrink-0">
              {rec.cover ? (
                <img
                  src={rec.cover}
                  alt=""
                  className="w-14 h-14 rounded-xl object-cover shadow-[var(--cover-shadow-sm)] shrink-0"
                  draggable={false}
                />
              ) : (
                <div
                  className="w-14 h-14 rounded-xl shrink-0 flex items-center justify-center"
                  style={{ background: "rgba(243,233,216,0.07)" }}
                >
                  <Shuffle size={18} className="text-[var(--ink-3)]" />
                </div>
              )}
              <div className="min-w-0 flex-1">
                <div className="text-[15px] font-bold text-[var(--ink)] truncate">
                  {rec.title}
                </div>
                <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
                  {rec.subtitle ? `${rec.subtitle} · ` : ""}
                  {rows.length} 首 · 双击播放，配合底部随机播放打乱顺序
                </div>
              </div>
              <button
                className="btn-secondary !py-1.5 !px-3 shrink-0"
                onClick={() =>
                  rec.origin === "daily"
                    ? loadNeteaseFeed("daily", api.neteaseDailyRecommend)
                    : rec.origin === "fm"
                      ? loadNeteaseFeed("fm", api.neteasePersonalFm)
                      : loadRandom()
                }
                disabled={recLoading}
                title={
                  rec.origin === "daily"
                    ? "刷新每日推荐"
                    : rec.origin === "fm"
                      ? "换一批 FM 歌曲"
                      : "随机换一个推荐歌单"
                }
              >
                {recLoading ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Shuffle size={13} />
                )}
                换一批
              </button>
              {rec.playlistId != null && (
                <button
                  className="btn-secondary !py-1.5 !px-3 shrink-0"
                  onClick={saveRecPlaylist}
                  title="收藏这个歌单到本地播放列表（与已有列表合并去重）"
                >
                  <ListPlus size={13} />
                  收藏歌单
                </button>
              )}
              <button
                className="btn-ghost w-8 h-8 shrink-0"
                onClick={() => {
                  setRec(null);
                  setSel(new Set());
                }}
                title="退出推荐"
              >
                <X size={15} />
              </button>
            </div>
          )}
          {/* 批量操作栏：多选模式下显示（选择数为 0 时只提示） */}
          {batchMode && (
            <div className="flex items-center gap-2 px-5 py-2.5 border-b border-[var(--line)] shrink-0 flex-wrap">
              <span className="text-[12.5px] text-[var(--ink-2)] mr-1">
                已选 {sel.size} 首
              </span>
              <button
                className="btn-secondary !py-1.5 !px-3"
                disabled={!sel.size}
                onClick={batchAddToPlaylist}
              >
                <ListPlus size={13} />
                加入播放列表
              </button>
              <button
                className="btn-secondary !py-1.5 !px-3"
                disabled={!sel.size}
                onClick={batchQueue}
              >
                <Play size={13} />
                加入队列
              </button>
              <button
                className="btn-secondary !py-1.5 !px-3"
                disabled={!sel.size}
                onClick={batchDownload}
              >
                <Download size={13} />
                下载到本地
              </button>
              <button
                className="btn-ghost !py-1.5 !px-3 text-[12.5px]"
                onClick={() => setSel(new Set())}
                disabled={!sel.size}
              >
                清除选择
              </button>
              <span className="text-[11px] text-[var(--ink-3)] ml-auto">
                点击行勾选 / 取消，再点右上角图标退出
              </span>
            </div>
          )}
          {rows.length > 0 && (
            <div className="grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-10 px-5 border-b border-[var(--line)] text-[10.5px] text-[var(--ink-3)] tracking-[0.18em]">
              <span className="text-center">序号</span>
              <span>歌曲</span>
              <span>专辑</span>
              <span className="text-right">时长</span>
              <span className="text-right">操作</span>
            </div>
          )}
          <div
            ref={win.containerRef}
            onScroll={win.onScroll}
            className="flex-1 min-h-0 overflow-y-auto px-2.5 pt-2.5 pb-[90px]"
          >
            {searching && (
              <div className="flex items-center justify-center gap-2.5 text-[var(--ink-3)] text-[13px] pt-16">
                <Loader2 size={15} className="animate-spin" />
                正在搜索…
              </div>
            )}
            {!searching && searched && rows.length === 0 && (
              <div className="text-center text-[var(--ink-3)] text-[13px] pt-16">
                没有找到相关歌曲
              </div>
            )}
            {!searched && !searching && !rec && (
              <div className="flex flex-col items-center justify-center gap-4 pt-16 anim-fade">
                <div className="relative">
                  <div
                    className="absolute -inset-8 rounded-full"
                    style={{
                      background:
                        "radial-gradient(circle, var(--accent) 0%, transparent 65%)",
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
                    <Cloud size={30} className="text-[var(--ink-2)]" />
                  </div>
                </div>
                <div className="text-[13.5px] text-[var(--ink-2)]">
                  搜索在线曲库，双击即可播放
                </div>
                <div className="text-[11.5px] text-[var(--ink-3)] max-w-[440px] text-center leading-relaxed">
                  {sourceName}
                  免费曲目可直接播放；扫码登录自己的账号后按账号权益播放（含会员曲目）。请支持正版。
                </div>
                {recommendable && (
                  <>
                    <div className="flex items-center gap-2 mt-1">
                      <button
                        className="btn-primary h-9 !px-5"
                        onClick={loadRandom}
                        disabled={recLoading}
                      >
                        {recLoading ? (
                          <Loader2 size={13} className="animate-spin" />
                        ) : (
                          <Shuffle size={13} />
                        )}
                        随便听听
                      </button>
                      {source === "netease" && neteaseLoggedIn && (
                        <>
                          <button
                            className="btn-secondary h-9 !px-4"
                            onClick={() =>
                              loadNeteaseFeed("daily", api.neteaseDailyRecommend)
                            }
                            disabled={recLoading}
                            title="根据你的听歌口味每天更新（需登录）"
                          >
                            每日推荐
                          </button>
                          <button
                            className="btn-secondary h-9 !px-4"
                            onClick={() =>
                              loadNeteaseFeed("fm", api.neteasePersonalFm)
                            }
                            disabled={recLoading}
                            title="私人 FM，按批次换歌（需登录）"
                          >
                            私人 FM
                          </button>
                        </>
                      )}
                    </div>
                    {toplists.length > 0 && (
                      <div className="flex flex-wrap justify-center gap-1.5 max-w-[620px]">
                        {toplists.map((t) => (
                          <button
                            key={t.id}
                            className="chip text-[var(--ink-2)] hover:text-[var(--accent-strong)] transition-colors"
                            style={{ background: "var(--shade)" }}
                            disabled={recLoading}
                            onClick={() => loadToplist(t)}
                            title={`查看${sourceName}${t.name}`}
                          >
                            {t.name}
                          </button>
                        ))}
                      </div>
                    )}
                  </>
                )}
                {searchHistory.length > 0 && (
                  <div className="flex flex-wrap justify-center items-center gap-1.5 max-w-[620px] mt-1">
                    <span className="flex items-center gap-1 text-[11px] text-[var(--ink-3)]">
                      <History size={12} />
                      最近搜索
                    </span>
                    {searchHistory.slice(0, 8).map((h) => (
                      <button
                        key={h}
                        className="chip text-[var(--ink-3)] hover:text-[var(--ink)] transition-colors max-w-[140px]"
                        style={{ background: "var(--shade)" }}
                        onClick={() => {
                          setKw(h);
                          submit(h);
                        }}
                        title={`搜索「${h}」`}
                      >
                        <span className="truncate">{h}</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
            {rec && rows.length === 0 && recLoading && (
              <div className="flex items-center justify-center gap-2.5 text-[var(--ink-3)] text-[13px] pt-16">
                <Loader2 size={15} className="animate-spin" />
                正在挑选推荐内容…
              </div>
            )}

            <div style={{ height: win.start * ROW_H }} aria-hidden />
            {rows.slice(win.start, win.end).map((t, k) => {
              const i = win.start + k;
              const active =
                current?.kind === t.kind &&
                (t.kind === "qq"
                  ? current.qid === t.id
                  : t.kind === "kugou"
                    ? current.kgid === t.id
                    : t.kind === "netease"
                      ? current.nid === t.id
                      : false);
              const saved = !!savedOnline[`${t.kind}-${t.id}`];
              const checked = sel.has(`${t.kind}-${t.id}`);
              return (
                <div
                  key={`${t.kind}-${t.id}`}
                  style={{ ["--row-idx" as string]: Math.min(i, 12) }}
                  className={`${i < 24 ? "anim-row" : ""} group grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-[60px] px-4 rounded-[13px] transition-colors cursor-default ${
                    batchMode
                      ? checked
                        ? "bg-[var(--accent-weak)]"
                        : "hover:bg-[var(--shade-hover)]"
                      : active
                        ? "bg-[var(--accent-weak)]"
                        : "hover:bg-[var(--shade-hover)]"
                  }`}
                  onClick={
                    batchMode
                      ? (ev) => {
                          ev.stopPropagation();
                          toggleSel(t);
                        }
                      : undefined
                  }
                  onDoubleClick={() => !batchMode && playRow(i)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: e.clientX, y: e.clientY, row: t });
                  }}
                >
                  <div className="relative h-11 flex items-center justify-center">
                    {batchMode ? (
                      <span
                        className={`w-[18px] h-[18px] rounded-[5px] border flex items-center justify-center transition-colors ${
                          checked
                            ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                            : "border-[var(--line)] group-hover:border-[var(--accent)]"
                        }`}
                      >
                        {checked && <Check size={13} strokeWidth={3} />}
                      </span>
                    ) : (
                      <>
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
                              : "bg-[var(--accent)] text-[var(--accent-on)]"
                          }`}
                          onClick={() => playRow(i)}
                          title="播放"
                        >
                          <Play size={14} className="fill-current ml-px" />
                        </button>
                      </>
                    )}
                  </div>

                  <div className="flex items-center gap-4 min-w-0">
                    {t.cover ? (
                      <img
                        src={t.cover}
                        alt=""
                        className="hover-lift w-11 h-11 rounded-xl object-cover shadow-[var(--cover-shadow-sm)] shrink-0"
                        draggable={false}
                      />
                    ) : (
                      <div
                        className="w-11 h-11 rounded-xl shrink-0"
                        style={{ background: "rgba(243,233,216,0.07)" }}
                      />
                    )}
                    <div className="min-w-0">
                      <div
                        className={`text-[13.5px] truncate flex items-center gap-2 ${
                          active
                            ? "text-[var(--accent-strong)] font-semibold"
                            : "text-[var(--ink)]"
                        }`}
                      >
                        <span className="truncate">{t.name}</span>
                        {t.vip && (
                          <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--accent-weak)] text-[var(--accent-strong)] font-bold shrink-0">
                            VIP
                          </span>
                        )}
                        {active && (
                          <span className="shrink-0 inline-flex">
                            <span className={`eq-bars ${playing ? "" : "paused"}`}>
                              <i />
                              <i />
                              <i />
                            </span>
                          </span>
                        )}
                      </div>
                      <button
                        className={`block text-left text-[12px] truncate mt-1 max-w-full hover:text-[var(--accent-strong)] transition-colors ${
                          active ? "text-[var(--accent-strong)]" : "text-[var(--ink-3)]"
                        }`}
                        onClick={(ev) => {
                          ev.stopPropagation();
                          openDetailPage("artist", t.artist);
                        }}
                        title={`查看歌手：${t.artist || "未知艺术家"}`}
                      >
                        {t.artist || "未知艺术家"}
                      </button>
                    </div>
                  </div>

                  <button
                    className="block text-left text-[12.5px] text-[var(--ink-3)] truncate max-w-full hover:text-[var(--accent-strong)] transition-colors"
                    onClick={(ev) => {
                      ev.stopPropagation();
                      openDetailPage("album", t.album);
                    }}
                    title={`查看专辑：${t.album || "未知专辑"}`}
                  >
                    {t.album || "未知专辑"}
                  </button>

                  <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                    {fmtTime(t.durationMs)}
                  </div>

                  <div className="flex items-center justify-end gap-1 pr-1">
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleLikeOnline({
                          kind: t.kind,
                          id: t.id,
                          name: t.name,
                          artist: t.artist,
                          album: t.album,
                          cover: t.cover,
                          durationMs: t.durationMs,
                          mediaMid: t.mediaMid,
                          vip: t.vip,
                        });
                      }}
                      title={saved ? "取消喜欢" : "收藏到“我喜欢”"}
                    >
                      <Heart
                        size={15}
                        className={
                          saved
                            ? "fill-[#e0533f] text-[#e0533f]"
                            : "opacity-0 group-hover:opacity-100"
                        }
                      />
                    </button>
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => {
                        e.stopPropagation();
                        menuAction(t, "next");
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
                      onClick={(e) => {
                        const r = (
                          e.currentTarget as HTMLElement
                        ).getBoundingClientRect();
                        setMenu({ x: r.right - 200, y: r.bottom + 4, row: t });
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

            <div style={{ height: Math.max(0, rows.length - win.end) * ROW_H }} aria-hidden />
            {hasMore && (
              <div className="flex justify-center pt-1 pb-2">
                <button
                  className="btn-secondary !py-1.5 !px-4"
                  disabled={searching}
                  onClick={() =>
                    source === "netease"
                      ? neteaseSearch(kw, true)
                      : source === "kugou"
                        ? kugouSearch(kw, true)
                        : qqSearch(kw, true)
                  }
                >
                  {searching ? <Loader2 size={13} className="animate-spin" /> : null}
                  加载更多
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 右键菜单 */}
      {menu &&
        createPortal(
        <div
          className="fixed z-[75] w-[200px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={(() => {
            const p = clampMenuPos(menu.x, menu.y, 200, 240);
            return { left: p.x, top: p.y };
          })()}
          onMouseDown={(e) => e.stopPropagation()}
          onMouseLeave={() => setMenu(null)}
        >
          {[
            { label: "播放", action: "play" as const },
            { label: "下一首播放", action: "next" as const },
            { label: "加入队列", action: "queue" as const },
          ].map((item) => (
            <button
              key={item.action}
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                menuAction(menu.row, item.action);
                setMenu(null);
              }}
            >
              <Play size={13} /> {item.label}
            </button>
          ))}
          <div className="my-1 mx-2 border-t border-[var(--line)]" />
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              setPickerRows([menu.row]);
              setMenu(null);
            }}
          >
            <Heart size={13} /> 添加到播放列表…
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              toggleLikeOnline({
                kind: menu.row.kind,
                id: menu.row.id,
                name: menu.row.name,
                artist: menu.row.artist,
                album: menu.row.album,
                cover: menu.row.cover,
                durationMs: menu.row.durationMs,
                mediaMid: menu.row.mediaMid,
                vip: menu.row.vip,
              });
              setMenu(null);
            }}
          >
            <Heart size={13} /> 收藏到“我喜欢”
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              downloadOnline({
                kind: menu.row.kind,
                id: menu.row.id,
                name: menu.row.name,
                artist: menu.row.artist,
                album: menu.row.album,
                cover: menu.row.cover,
                durationMs: menu.row.durationMs,
                mediaMid: menu.row.mediaMid,
              });
              setMenu(null);
            }}
          >
            <Play size={13} /> 下载到本地
          </button>
        </div>,
        document.body
      )}

      {/* 添加到播放列表弹窗（单行右键 / 批量多选共用） */}
      <Modal
        open={!!pickerRows}
        onClose={() => setPickerRows(null)}
        title={
          pickerRows && pickerRows.length > 1
            ? `添加 ${pickerRows.length} 首到播放列表`
            : "添加到播放列表"
        }
        width={380}
      >
        <div className="flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
          {playlists.map((p) => (
            <button
              key={p.id}
              className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
              onClick={async () => {
                if (pickerRows) {
                  for (const r of pickerRows) {
                    await addOnlineToPlaylist(p.id, r);
                  }
                }
                setPickerRows(null);
              }}
            >
              <span className="truncate">{p.name}</span>
              <span className="text-[11px] text-[var(--ink-2)]">
                {p.entries.length} 首
              </span>
            </button>
          ))}
          {!playlists.length && (
            <div className="text-[12.5px] text-[var(--ink-2)] py-2">
              还没有播放列表，在下方创建
            </div>
          )}
        </div>
        <div className="flex gap-2 mt-3">
          <input
            type="text"
            value={newPlName}
            onChange={(e) => setNewPlName(e.target.value)}
            placeholder="新播放列表名称"
            className="flex-1 h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-3 text-[13px] focus:border-[var(--line)] outline-none"
          />
          <button
            className="btn-secondary"
            onClick={async () => {
              if (!newPlName.trim() || !pickerRows?.length) return;
              const pid = await createPlaylist(newPlName.trim());
              if (pid >= 0) {
                for (const r of pickerRows) {
                  await addOnlineToPlaylist(pid, r);
                }
              }
              setNewPlName("");
              setPickerRows(null);
            }}
          >
            创建并添加
          </button>
        </div>
      </Modal>

      {/* 导入歌单弹窗（网易云 / QQ 音乐） */}
      <Modal
        open={importOpen}
        onClose={() => setImportOpen(false)}
        title={`导入${sourceName}歌单`}
        width={400}
      >
        {importList === null ? (
          <div className="flex items-center justify-center gap-2 text-[13px] text-[var(--ink-2)] py-6">
            <Loader2 size={15} className="animate-spin" /> 获取中…
          </div>
        ) : (
          <>
            {importList.length > 1 && (
              <>
                {/* 与列表行同构的低调样式：accent 图标+文字，右侧计数，行下加分隔线 */}
                <button
                  disabled={importing}
                  className="h-10 px-3 rounded-lg text-left text-[13px] hover:bg-[var(--shade)] flex items-center justify-between transition-colors disabled:opacity-60"
                  onClick={async () => {
                    if (importing || !importList.length) return;
                    setImporting(true);
                    // 逐个导入，弹窗内实时显示进度；结束后由 store 统一
                    // 刷新列表并汇总提示（含失败个数）
                    // 酷狗无登录态、导入入口不渲染，这里只会是 netease/qq
                    await importAllPlaylists(
                      source === "kugou" ? "netease" : source,
                      importList,
                      setImportProgress
                    );
                    setImporting(false);
                    setImportProgress(null);
                    setImportOpen(false);
                  }}
                >
                  {importProgress ? (
                    <span className="flex items-center gap-2 text-[var(--ink-2)] min-w-0">
                      <Loader2 size={14} className="animate-spin shrink-0 text-[var(--accent)]" />
                      <span className="truncate">
                        正在导入 {importProgress.done + 1}/{importProgress.total}
                        ：「{importProgress.name}」
                      </span>
                    </span>
                  ) : (
                    <>
                      <span className="flex items-center gap-2 text-[var(--accent)] font-medium">
                        <ListPlus size={15} />
                        全部导入
                      </span>
                      <span className="text-[11px] text-[var(--ink-3)] shrink-0 ml-3">
                        共 {importList.length} 个歌单
                      </span>
                    </>
                  )}
                </button>
                <div className="border-b border-[var(--line)] mb-2" aria-hidden />
              </>
            )}
            <div className="flex flex-col gap-1.5 max-h-[320px] overflow-y-auto">
              {importList.map((p) => (
                <button
                  key={p.id}
                  disabled={importing}
                  className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors disabled:opacity-50"
                  onClick={async () => {
                    setImporting(true);
                    if (source === "netease") await importNeteasePlaylist(p.id, p.name);
                    else await importQqPlaylist(p.id, p.name);
                    setImporting(false);
                    setImportOpen(false);
                  }}
                >
                  <span className="truncate">{p.name}</span>
                  <span className="text-[11px] text-[var(--ink-2)] shrink-0 ml-3">
                    {p.trackCount} 首
                  </span>
                </button>
              ))}
              {!importList.length && (
                <div className="text-[12.5px] text-[var(--ink-2)] py-2">账号下没有歌单</div>
              )}
            </div>
          </>
        )}
        <div className="text-[10.5px] text-[var(--ink-3)] mt-3 leading-relaxed">
          导入的歌单以在线条目保存：播放时按账号权益实时获取播放链接，不占用本地磁盘。
        </div>
      </Modal>

      {/* 扫码登录弹窗 */}
      <QrLoginModal source={qrSource} onClose={() => setQrSource(null)} />
    </div>
  );
}

function QrLoginModal({
  source,
  onClose,
}: {
  source: Source | null;
  onClose: () => void;
}) {
  const open = source != null;
  const [qr, setQr] = useState("");
  const [status, setStatus] = useState<
    "loading" | "waiting" | "scanned" | "expired" | "error"
  >("loading");
  const [errMsg, setErrMsg] = useState("");
  const neteaseSetLogin = useStore((s) => s.neteaseSetLogin);
  const qqSetLogin = useStore((s) => s.qqSetLogin);
  const toast = useStore((s) => s.toast);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  const startPolling = (src: Source, identifier: string) => {
    timerRef.current = setInterval(async () => {
      try {
        const c =
          src === "netease"
            ? await api.neteaseQrCheck(identifier)
            : await api.qqQrCheck(identifier);
        if (c.status === "waiting") return;
        if (c.status === "scanned") {
          setStatus("scanned");
          return;
        }
        if (c.status === "expired") {
          stopPolling();
          setStatus("expired");
          return;
        }
        if (c.status === "success") {
          stopPolling();
          if (src === "netease") {
            neteaseSetLogin(true, c.nickname ?? "");
            useStore.getState().neteaseSyncLikes();
          } else {
            qqSetLogin(true, c.nickname ?? "");
          }
          toast(`登录成功：${c.nickname ?? ""}`, "success");
          onClose();
        }
      } catch (e) {
        stopPolling();
        setStatus("error");
        setErrMsg(String(e));
      }
    }, 1600);
  };

  const create = async () => {
    if (!source) return;
    stopPolling();
    setStatus("loading");
    setErrMsg("");
    try {
      if (source === "netease") {
        const r = await api.neteaseQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("netease", r.key);
      } else {
        const r = await api.qqQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("qq", r.qrsig);
      }
    } catch (e) {
      setStatus("error");
      setErrMsg(String(e));
    }
  };

  useEffect(() => {
    if (open) create();
    else {
      stopPolling();
      setQr("");
      setStatus("loading");
    }
    return stopPolling;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const sourceName = source === "qq" ? "QQ 音乐" : "网易云";

  return (
    <Modal open={open} onClose={onClose} title={`扫码登录${sourceName}`} width={360}>
      <div className="flex flex-col items-center gap-4 py-2">
        <div className="w-[240px] h-[240px] rounded-2xl bg-white flex items-center justify-center overflow-hidden">
          {qr ? (
            <img
              src={qr}
              alt="二维码"
              className="w-full h-full"
              draggable={false}
            />
          ) : status === "error" ? (
            <span className="text-[12px] text-rose-500 px-4 text-center">
              {errMsg}
            </span>
          ) : (
            <Loader2 size={26} className="animate-spin text-[var(--ink-2)]" />
          )}
        </div>
        <div className="text-[12.5px] text-[var(--ink-2)] text-center leading-relaxed">
          {status === "loading"
            ? "正在生成二维码…"
            : status === "waiting"
              ? `打开${sourceName === "QQ 音乐" ? "QQ" : "网易云"} App 扫一扫`
              : status === "scanned"
                ? "已在手机上确认，请在手机上点击登录"
                : status === "expired"
                  ? "二维码已过期"
                  : `出错了：${errMsg}`}
          {status === "expired" && (
            <button className="btn-secondary !py-1.5 !px-3 ml-2" onClick={create}>
              刷新
            </button>
          )}
        </div>
        <div className="text-[10.5px] text-[var(--ink-3)] text-center leading-relaxed max-w-[300px]">
          登录凭证仅保存在本机设置中，用于按你的账号权益获取播放链接；RustMusic
          不提供任何绕过会员/版权限制的能力。
        </div>
      </div>
      <div className="flex justify-end mt-3">
        <button className="btn-secondary" onClick={onClose}>
          关闭
        </button>
      </div>
    </Modal>
  );
}
