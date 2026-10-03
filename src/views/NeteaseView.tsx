import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  ArrowLeft,
  Cloud,
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
  Download,
  History,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { useVirtualWindow } from "../hooks/useVirtualWindow";
import { useLocatePill } from "../hooks/useLocatePill";
import LocateCurrentPill from "../components/LocateCurrentPill";
import type { NeteaseTrack, OnlineNavSnapshot, OnlineRecState, QqSong, QueueItem } from "../types";
import { fmtTime } from "../utils";
import { ImportPlaylistModal } from "./online/ImportPlaylistModal";
import { PlaylistPickerModal } from "./online/PlaylistPickerModal";
import { QrLoginModal } from "./online/QrLoginModal";
import { RowMenu } from "./online/RowMenu";
import { TrackRowItem } from "./online/TrackRowItem";
import {
  loadSearchHistory,
  SEARCH_HISTORY_KEY,
  qqCover,
  type OnlineRow,
  type Source,
} from "./online/shared";


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
  const kugouLoggedIn = useStore((s) => s.kugouLoggedIn);
  const kugouNickname = useStore((s) => s.kugouNickname);
  const neteaseLogout = useStore((s) => s.neteaseLogout);
  const qqLogout = useStore((s) => s.qqLogout);
  const kugouLogout = useStore((s) => s.kugouLogout);
  const playlists = useStore((s) => s.playlists);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const importNeteasePlaylist = useStore((s) => s.importNeteasePlaylist);
  const importQqPlaylist = useStore((s) => s.importQqPlaylist);
  const importKugouPlaylist = useStore((s) => s.importKugouPlaylist);
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
  // “创建并添加”进行中：双击会创建两个同名歌单（后端不按名去重）
  const [creatingPl, setCreatingPl] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [importList, setImportList] = useState<
    { id: number | string; name: string; trackCount: number }[] | null
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
  // 酷狗搜索/播放匿名可用；登录用于 VIP 曲目按会员权益播放
  const loggedIn =
    source === "netease"
      ? neteaseLoggedIn
      : source === "qq"
        ? qqLoggedIn
        : kugouLoggedIn;
  const nickname =
    source === "netease"
      ? neteaseNickname
      : source === "qq"
        ? qqNickname
        : kugouNickname;
  const sourceName =
    source === "netease" ? "网易云" : source === "qq" ? "QQ 音乐" : "酷狗";
  // 推荐入口（榜单/随便听听）三源都接了：酷狗为匿名接口
  const recommendable = true;

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
        } else if (source === "qq") {
          const r = await api.qqToplists();
          if (!dead)
            setToplists(
              (r.toplists ?? []).map((t) => ({
                id: t.id,
                name: t.title,
                cover: t.pic,
              }))
            );
        } else {
          const r = await api.kugouToplists();
          if (!dead)
            setToplists(
              (r.toplists ?? []).map((t) => ({
                id: t.id,
                name: t.name,
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
    } else if (r.kugou) {
      useStore.setState((s) => ({
        kugouCache: {
          ...s.kugouCache,
          ...Object.fromEntries(r.kugou!.map((t) => [t.id, t])),
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
      } else if (source === "qq") {
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
      } else {
        const r = await api.kugouRandomPlaylist();
        const next: OnlineRecState = {
          origin: "random",
          title: r.name,
          cover: r.cover,
          subtitle: r.creator,
          playlistId: r.id,
          kugou: r.songs,
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

  const loadToplist = async (
    t: { id: number; name: string; cover: string },
    page = 1,
  ): Promise<QueueItem[]> => {
    if (recLoading) return [];
    setRecLoading(true);
    try {
      const append = page > 1 && rec?.origin === "top" && rec.topId === t.id;
      const merge = <T,>(old: T[] | undefined, add: T[]): T[] => [
        ...(old ?? []),
        ...add,
      ];
      const next: OnlineRecState =
        source === "netease"
          ? await (async () => {
              const r = await api.neteaseToplistTracks(t.id, page);
              const prev = append ? rec : undefined;
              return {
                origin: "top" as const,
                title: t.name,
                cover: t.cover,
                subtitle: `${sourceName}官方榜单`,
                // 网易云榜单 ID 即歌单 ID，可整单收藏
                playlistId: t.id,
                topId: t.id,
                recPage: page,
                recHasMore: r.hasMore,
                netease: merge(prev?.netease, r.songs as NeteaseTrack[]),
              };
            })()
          : source === "qq"
            ? await (async () => {
                const r = await api.qqToplistTracks(t.id, page);
                const prev = append ? rec : undefined;
                return {
                  origin: "top" as const,
                  title: t.name,
                  cover: t.cover,
                  subtitle: `${sourceName}官方榜单`,
                  topId: t.id,
                  recPage: page,
                  recHasMore: r.hasMore,
                  qq: merge(prev?.qq, r.songs as QqSong[]),
                };
              })()
            : await (async () => {
                const r = await api.kugouToplistTracks(t.id, page);
                const prev = append ? rec : undefined;
                return {
                  origin: "top" as const,
                  title: t.name,
                  cover: t.cover,
                  subtitle: `${sourceName}官方榜单`,
                  topId: t.id,
                  recPage: page,
                  recHasMore: r.hasMore,
                  kugou: merge(prev?.kugou, r.songs),
                };
              })();
      cacheRecSongs(next);
      setRec(next);
      // 本页新增行 → 队列项（自动续页时追加进播放队列；prevLen 与上面
      // merge 用的同一个 rec 快照，slice 结果恰为本次接口返回的新增）
      const prevLen =
        source === "netease"
          ? (append ? rec?.netease?.length ?? 0 : 0)
          : source === "qq"
            ? (append ? rec?.qq?.length ?? 0 : 0)
            : (append ? rec?.kugou?.length ?? 0 : 0);
      const merged =
        source === "netease" ? next.netease : source === "qq" ? next.qq : next.kugou;
      return (merged ?? []).slice(prevLen).map((t) =>
        source === "netease"
          ? ({ kind: "netease", id: t.id } as QueueItem)
          : ({ kind: source, id: t.id } as QueueItem)
      );
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setRecLoading(false);
    }
    return [];
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
      if (source === "netease") await importNeteasePlaylist(rec.playlistId as number, rec.title);
      else if (source === "qq") await importQqPlaylist(rec.playlistId as number, rec.title);
      else await importKugouPlaylist(String(rec.playlistId), rec.title);
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
    if (rec?.kugou) {
      return rec.kugou.map((t) => ({
        kind: "kugou" as const,
        id: t.id,
        name: t.name,
        artist: t.singer,
        album: t.album,
        cover: t.cover,
        durationMs: t.durationMs,
        vip: t.vip,
        mediaMid: String(t.albumAudioId ?? ""),
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
        // mediaMid 通道复用：存专辑音频 ID，收藏/下载/恢复播放都要用
        mediaMid: String(t.albumAudioId ?? ""),
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

  // 队列自动续页：把当前列表（搜索结果 / 榜单）的"加载下一页"注册给队列。
  // 播到队列最后一条时 store 触发本回调，加载的新行追加进队列——最后一首
  // 自然播完后无缝继续。tail 校验防止队列被替换后过期回调误续页。
  const rowsRef = useRef(rows);
  rowsRef.current = rows;
  const recRef = useRef(rec);
  recRef.current = rec;
  const loadToplistRef = useRef(loadToplist);
  loadToplistRef.current = loadToplist;
  const registerQueueExtender = () => {
    const tail = rowsRef.current[rowsRef.current.length - 1];
    if (!tail) return;
    let expectedTail = String(tail.id);
    const tailKind = tail.kind;
    const kwAtPlay = kw;
    const srcAtPlay = source;
    useStore.getState().setQueueExtender(async () => {
      const st = useStore.getState();
      const last = st.queue[st.queue.length - 1];
      if (!last || last.kind !== tailKind || String(last.id) !== expectedTail) {
        return null; // 队列尾部已变（换队列/手动增删）：本上下文过期
      }
      const rec = recRef.current;
      if (rec) {
        // 歌单/榜单模式：榜单有分页（加载更多），歌单/每日推荐整单加载无续页
        if (rec.origin !== "top" || !rec.recHasMore || rec.topId == null) return null;
        const fresh = await loadToplistRef.current(
          { id: rec.topId, name: rec.title, cover: rec.cover },
          (rec.recPage ?? 1) + 1,
        );
        if (!fresh.length) return null;
        expectedTail = String(fresh[fresh.length - 1].id);
        return fresh;
      }
      // 搜索结果模式：hasMore 判定与"加载更多"按钮一致
      if (srcAtPlay === "netease") {
        if (st.neteaseResults.length >= st.neteaseTotal) return null;
        const before = st.neteaseResults.length;
        await neteaseSearch(kwAtPlay, true);
        const fresh = useStore.getState().neteaseResults.slice(before);
        if (!fresh.length) return null;
        expectedTail = String(fresh[fresh.length - 1].id);
        return fresh.map((t) => ({ kind: "netease", id: t.id }) as QueueItem);
      }
      if (srcAtPlay === "qq") {
        if (st.qqResults.length === 0 || st.qqResults.length !== 30 * st.qqPage) return null;
        const before = st.qqResults.length;
        await qqSearch(kwAtPlay, true);
        const fresh = useStore.getState().qqResults.slice(before);
        if (!fresh.length) return null;
        expectedTail = String(fresh[fresh.length - 1].id);
        return fresh.map((t) => ({ kind: "qq", id: t.id }) as QueueItem);
      }
      if (st.kugouResults.length === 0 || st.kugouResults.length !== 30 * st.kugouPage) return null;
      const before = st.kugouResults.length;
      await kugouSearch(kwAtPlay, true);
      const fresh = useStore.getState().kugouResults.slice(before);
      if (!fresh.length) return null;
      expectedTail = String(fresh[fresh.length - 1].id);
      return fresh.map((t) => ({ kind: "kugou", id: t.id }) as QueueItem);
    });
  };

  // 当前播放行的下标（网易比 nid、酷狗比 kgid、QQ 比 qid）——
  // 搜索结果与歌单/榜单共用（药丸两种模式都要工作）
  const currentIdx = useMemo(
    () =>
      rows.findIndex(
        (t) =>
          current?.kind === t.kind &&
          (t.kind === "netease"
            ? current.nid === t.id
            : t.kind === "kugou"
              ? current.kgid === t.id
              : current.qid === t.id)
      ),
    [rows, current]
  );

  // 打开歌单/榜单（rec 变化）时定位到当前播放行；不在列表则回到顶部。
  // rows 由 rec 同步派生（setRec 在数据就绪后调用），此时 rows 已是新列表
  useLayoutEffect(() => {
    const el = win.containerRef.current;
    if (!el || !rec) return;
    if (currentIdx >= 0) win.scrollToIndex(currentIdx);
    else el.scrollTop = 0;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rec]);

  // 当前播放行滚出可视区时浮出「回到当前歌曲」药丸
  const pill = useLocatePill({
    containerRef: win.containerRef,
    rowHeight: ROW_H,
    currentIndex: currentIdx,
  });

  const playRow = (i: number) => {
    registerQueueExtender();
    if (rec) {
      // 推荐态：队列 = 当前推荐列表（曲目信息已在加载时写入缓存）
      if (rec.netease) playNetease(rec.netease, i);
      else if (rec.qq) playQq(rec.qq, i);
      else if (rec.kugou) playKugou(rec.kugou, i);
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

          {/* 登录状态（酷狗扫码登录用于 VIP 曲目按权益播放） */}
          <div className="shrink-0 flex flex-col items-end gap-2 mb-1">
            {loggedIn ? (
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
                            : source === "qq"
                              ? await api.qqUserPlaylists()
                              : await api.kugouUserPlaylists();
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
                    onClick={() =>
                      source === "netease"
                        ? neteaseLogout()
                        : source === "qq"
                          ? qqLogout()
                          : kugouLogout()
                    }
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
        <div className="relative glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
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
              const checked = sel.has(`${t.kind}-${t.id}`);
              return (
                <TrackRowItem
                  key={`${t.kind}-${t.id}`}
                  row={t}
                  index={i}
                  batchMode={batchMode}
                  checked={checked}
                  onToggleSel={() => toggleSel(t)}
                  onPlay={() => playRow(i)}
                  onNext={() => menuAction(t, "next")}
                  onLike={() =>
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
                    })
                  }
                  onMore={(e) => {
                    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                    setMenu({ x: r.right - 200, y: r.bottom + 4, row: t });
                  }}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: e.clientX, y: e.clientY, row: t });
                  }}
                  onOpenDetail={openDetailPage}
                />
              );
            })}
            <div style={{ height: Math.max(0, rows.length - win.end) * ROW_H }} aria-hidden />
            {rec?.origin === "top" && rec.recHasMore && (
              <div className="flex justify-center pt-1 pb-2">
                <button
                  className="btn-secondary !py-1.5 !px-4"
                  disabled={recLoading}
                  onClick={() =>
                    rec.topId != null &&
                    loadToplist(
                      { id: rec.topId, name: rec.title, cover: rec.cover },
                      (rec.recPage ?? 1) + 1,
                    )
                  }
                >
                  {recLoading ? <Loader2 size={13} className="animate-spin" /> : null}
                  加载更多
                </button>
              </div>
            )}
            {hasMore && !rec && (
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
          <LocateCurrentPill show={pill.show} onClick={pill.locate} className="absolute bottom-3 left-1/2 -translate-x-1/2" />
        </div>
      </div>

      {/* 右键菜单 */}
      {menu && (
        <RowMenu
          menu={menu}
          onClose={() => setMenu(null)}
          menuAction={menuAction}
          onAddToPlaylist={(row) => setPickerRows([row])}
        />
      )}

      {/* 添加到播放列表弹窗（单行右键 / 批量多选共用） */}
      <PlaylistPickerModal rows={pickerRows} onClose={() => setPickerRows(null)} />

      {/* 导入歌单弹窗（网易云 / QQ 音乐） */}
      <ImportPlaylistModal
        open={importOpen}
        onClose={() => setImportOpen(false)}
        list={importList}
        source={source}
        sourceName={sourceName}
      />

      {/* 扫码登录弹窗 */}
      <QrLoginModal source={qrSource} onClose={() => setQrSource(null)} />
    </div>
  );
}
