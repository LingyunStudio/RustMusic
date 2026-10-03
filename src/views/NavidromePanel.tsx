import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  ArrowLeft,
  Check,
  Download,
  ListPlus,
  Loader2,
  LogIn,
  MoreHorizontal,
  Play,
  Search,
  Server,
} from "lucide-react";
import { api } from "../api";
import { useStore } from "../store";
import { useLocatePillEl } from "../hooks/useLocatePillEl";
import LocateCurrentPill from "../components/LocateCurrentPill";
import type { NdAlbum, NdSong } from "../types";
import { clampMenuPos, fmtTime } from "../utils";
import Modal from "../components/Modal";

const LS_SERVER = "navidrome.ui.server";
const LS_USER = "navidrome.ui.username";

type View =
  | { page: "albums" }
  | { page: "songs" }
  | { page: "album"; id: string; name: string; artist: string }
  | { page: "search"; query: string };

/** 模块级状态快照：本组件随主视图切换（去 QQ/网易云/资料库等页面）
 *  会被整体卸载，把面板状态存在这里，重新挂载时恢复——连接状态、
 *  当前页面、列表数据、搜索词全部保留，不再闪登录页重新自动连接 */
const ndSnapshot: {
  connected: boolean;
  view: View;
  albums: NdAlbum[];
  albumSongs: NdSong[];
  allSongs: NdSong[];
  total: number;
  seg: "albums" | "songs";
  kw: string;
  searched: boolean;
  navStack: { view: View; albumSongs: NdSong[]; kw: string; searched: boolean }[];
} = {
  connected: false,
  view: { page: "albums" },
  albums: [],
  albumSongs: [],
  allSongs: [],
  total: 0,
  seg: "albums",
  kw: "",
  searched: false,
  navStack: [],
};

export default function NavidromePanel() {
  const toast = useStore((s) => s.toast);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const addOnlineToPlaylist = useStore((s) => s.addOnlineToPlaylist);
  const playlists = useStore((s) => s.playlists);

  const [server, setServer] = useState(localStorage.getItem(LS_SERVER) ?? "");
  const [username, setUsername] = useState(localStorage.getItem(LS_USER) ?? "");
  const [password, setPassword] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [connected, setConnected] = useState(ndSnapshot.connected);

  const [view, setView] = useState<View>(ndSnapshot.view);
  const [albums, setAlbums] = useState<NdAlbum[]>(ndSnapshot.albums);
  const [albumSongs, setAlbumSongs] = useState<NdSong[]>(ndSnapshot.albumSongs);
  const [loading, setLoading] = useState(false);
  const [seg, setSeg] = useState<"albums" | "songs">(ndSnapshot.seg);
  const [allSongs, setAllSongs] = useState<NdSong[]>(ndSnapshot.allSongs);
  const [total, setTotal] = useState(ndSnapshot.total);

  const [kw, setKw] = useState(ndSnapshot.kw);
  const [searched, setSearched] = useState(ndSnapshot.searched);

  const [menu, setMenu] = useState<{ x: number; y: number; song: NdSong } | null>(null);
  const [pickerSong, setPickerSong] = useState<NdSong | null>(null);
  const [downloading, setDownloading] = useState(false);

  // 导航栈带数据快照：goBack 时标题与列表必须一起回退，
  // 否则"专辑 A → 搜索 → 返回"会显示专辑 A 的标题配搜索结果的列表。
  // 栈数组本体存快照里，跨卸载保留
  const navStack = useRef(ndSnapshot.navStack);
  const listRef = useRef<HTMLDivElement | null>(null);
  // 请求代次：新导航（开专辑/搜索/加载更多）使在途旧响应全部作废，
  // 防止慢的旧响应后到覆盖新结果
  const reqGen = useRef(0);

  // 每次渲染后把状态同步进模块级快照（纯赋值，开销可忽略）
  useEffect(() => {
    ndSnapshot.connected = connected;
    ndSnapshot.view = view;
    ndSnapshot.albums = albums;
    ndSnapshot.albumSongs = albumSongs;
    ndSnapshot.allSongs = allSongs;
    ndSnapshot.total = total;
    ndSnapshot.seg = seg;
    ndSnapshot.kw = kw;
    ndSnapshot.searched = searched;
  });

  useEffect(() => {
    // 快照恢复了完整状态（连接过且有数据）：直接沿用，不重新连接拉取
    if (ndSnapshot.connected && albums.length) return;
    // 首次打开：用已保存的连接信息自动连接并拉取专辑列表
    if (!server || !username) return;
    let dead = false;
    (async () => {
      setLoading(true);
      try {
        const list = await api.navidromeAlbums(server, username);
        if (!dead) {
          setAlbums(list);
          setConnected(true);
          const r = await api.navidromeAllSongs(server, username, 0);
          if (!dead) {
            setAllSongs(r.songs);
            setTotal(r.total);
          }
        }
      } catch {
        /* 未连接或失效：保持连接表单 */
      } finally {
        if (!dead) setLoading(false);
      }
    })();
    return () => {
      dead = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const connect = async () => {
    if (!server.trim() || !username.trim() || !password) {
      toast("服务器地址、用户名、密码均不能为空", "error");
      return;
    }
    setConnecting(true);
    try {
      await api.navidromeSave(server.trim(), username.trim(), password);
      const list = await api.navidromeAlbums(server.trim(), username.trim());
      localStorage.setItem(LS_SERVER, server.trim());
      localStorage.setItem(LS_USER, username.trim());
      setServer(server.trim());
      setAlbums(list);
      setConnected(true);
      try {
        const r = await api.navidromeAllSongs(server.trim(), username.trim(), 0);
        setAllSongs(r.songs);
        setTotal(r.total);
      } catch {
        /* 全曲库拉取失败不阻塞连接 */
      }
      setPassword("");
      setView({ page: "albums" });
      toast("Navidrome 已连接", "success");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setConnecting(false);
    }
  };

  const forget = async () => {
    try {
      await api.navidromeForget(server, username);
    } catch {
      /* 凭据可能已不存在 */
    }
    localStorage.removeItem(LS_SERVER);
    localStorage.removeItem(LS_USER);
    setServer("");
    setUsername("");
    setConnected(false);
    setAlbums([]);
    navStack.current = []; // 断开后返回栈失效，一并清掉
    setView({ page: "albums" });
    toast("已断开并清除保存的密码", "success");
  };

  const playSong = (s: NdSong) => {
    if (!server || !username) return;
    // 队列/收藏/播放列表都需要元数据缓存
    useStore.setState((st) => ({
      ndCache: {
        ...st.ndCache,
        [s.id]: {
          title: s.title,
          artist: s.artist,
          album: s.album,
          cover: s.coverUrl,
          durationMs: s.duration * 1000,
        },
      },
    }));
    api
      .navidromePlay(server, username, {
        id: s.id,
        title: s.title,
        artist: s.artist,
        album: s.album,
        cover: s.coverUrl,
        durationMs: s.duration * 1000,
      })
      .catch((e) => toast(String(e), "error"));
  };

  const queueItemOf = (s: NdSong) => {
    useStore.setState((st) => ({
      ndCache: {
        ...st.ndCache,
        [s.id]: {
          title: s.title,
          artist: s.artist,
          album: s.album,
          cover: s.coverUrl,
          durationMs: s.duration * 1000,
        },
      },
    }));
    return { kind: "navidrome" as const, id: s.id };
  };

  const likeRowOf = (s: NdSong) => ({
    kind: "navidrome",
    id: s.id,
    name: s.title,
    artist: s.artist,
    album: s.album,
    cover: s.coverUrl,
    durationMs: s.duration * 1000,
    mediaMid: "",
    vip: false,
  });

  const openAlbum = async (a: NdAlbum) => {
    navStack.current.push({ view, albumSongs, kw, searched });
    const gen = ++reqGen.current;
    setLoading(true);
    try {
      const r = await api.navidromeAlbumSongs(server, username, a.id);
      if (gen !== reqGen.current) return;
      setAlbumSongs(r.songs);
      setView({ page: "album", id: a.id, name: r.name, artist: r.artist });
    } catch (e) {
      if (gen === reqGen.current) toast(String(e), "error");
    } finally {
      if (gen === reqGen.current) setLoading(false);
    }
  };

  const submitSearch = async () => {
    const q = kw.trim();
    if (!q) return;
    navStack.current.push({ view, albumSongs, kw, searched });
    const gen = ++reqGen.current;
    setLoading(true);
    try {
      const r = await api.navidromeSearch(server, username, q);
      if (gen !== reqGen.current) return;
      setAlbumSongs(r);
      setView({ page: "search", query: q });
      setSearched(true);
    } catch (e) {
      if (gen === reqGen.current) toast(String(e), "error");
    } finally {
      if (gen === reqGen.current) setLoading(false);
    }
  };

  const loadMoreSongs = async (): Promise<NdSong[] | null> => {
    if (loading) return null;
    const gen = ++reqGen.current;
    setLoading(true);
    try {
      const r = await api.navidromeAllSongs(server, username, allSongs.length);
      if (gen !== reqGen.current) return null;
      setAllSongs((prev) => [...prev, ...r.songs]);
      setTotal(r.total);
      return r.songs;
    } catch (e) {
      if (gen === reqGen.current) toast(String(e), "error");
    } finally {
      if (gen === reqGen.current) setLoading(false);
    }
    return null;
  };

  const goBack = () => {
    const prev = navStack.current.pop();
    if (!prev) return;
    reqGen.current++; // 作废在途响应
    setAlbumSongs(prev.albumSongs);
    setKw(prev.kw);
    setSearched(prev.searched);
    setView(prev.view);
  };

  const downloadSong = async (s: NdSong) => {
    if (downloading) return;
    setDownloading(true);
    try {
      const name = await api.downloadOnline({
        kind: "navidrome",
        id: s.id,
        title: s.title,
        artist: s.artist,
        album: s.album,
        coverUrl: s.coverUrl,
        durationMs: s.duration * 1000,
        mediaMid: "",
      });
      await useStore.getState().refreshTracks();
      useStore.getState().toast(`已下载到资料库：${name}`, "success");
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      setDownloading(false);
    }
  };

  const rows: { s: NdSong; i: number }[] = albumSongs.map((s, i) => ({ s, i }));

  // 「回到当前歌曲」药丸 + 队列自动续页（Navidrome 全部歌曲列表）
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const current = useStore((s) => s.current);
  const pill = useLocatePillEl({
    containerRef: scrollRef,
    activeSelector: current?.kind === "navidrome" ? '[data-now-playing="1"]' : null,
  });
  const allSongsLenRef = useRef(allSongs.length);
  allSongsLenRef.current = allSongs.length;
  const totalRef = useRef(total);
  totalRef.current = total;
  const loadMoreSongsRef = useRef(loadMoreSongs);
  loadMoreSongsRef.current = loadMoreSongs;
  const locatedRef = useRef(false);
  // 首页歌曲加载完成后：正在播放的 Navidrome 歌曲若在列表中，定位到它
  useEffect(() => {
    if (locatedRef.current || !allSongsLenRef.current) return;
    locatedRef.current = true;
    requestAnimationFrame(() => {
      scrollRef.current
        ?.querySelector('[data-now-playing="1"]')
        ?.scrollIntoView({ block: "center" });
    });
  }, [allSongs.length]);
  const registerQueueExtender = (expectedTail: string) => {
    let tail = expectedTail;
    useStore.getState().setQueueExtender(async () => {
      const st = useStore.getState();
      const last = st.queue[st.queue.length - 1];
      if (!last || last.kind !== "navidrome" || String(last.id) !== tail) {
        return null; // 队列尾部已变：上下文过期
      }
      if (allSongsLenRef.current >= totalRef.current) return null;
      const fresh = await loadMoreSongsRef.current();
      if (!fresh?.length) return null;
      tail = String(fresh[fresh.length - 1].id);
      return fresh.map(queueItemOf);
    });
  };
  return (
    <div className="h-full flex flex-col min-h-0" ref={listRef}>
      <LocateCurrentPill
        show={pill.show}
        onClick={pill.locate}
        className="fixed bottom-[92px] left-1/2 -translate-x-1/2"
      />
      {!connected ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-4">
          <div
            className="w-16 h-16 rounded-2xl flex items-center justify-center"
            style={{
              background: "rgba(255,255,255,0.04)",
              border: "1px solid rgba(255,255,255,0.06)",
            }}
          >
            <Server size={26} className="text-[var(--ink-3)]" />
          </div>
          <div className="text-[13.5px] text-[var(--ink-2)]">连接你的 Navidrome 服务器</div>
          <div className="flex flex-col gap-2.5 w-[360px]">
            <input
              type="text"
              value={server}
              onChange={(e) => setServer(e.target.value)}
              placeholder="服务器地址（如 music.example.com 或 192.168.1.10:4533）"
              className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] px-3.5 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)]"
            />
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="用户名"
              className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] px-3.5 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)]"
            />
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && connect()}
              placeholder="密码（保存在 Windows 凭据管理器）"
              className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] px-3.5 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)]"
            />
            <button className="btn-primary h-10" onClick={connect} disabled={connecting}>
              {connecting ? <Loader2 size={14} className="animate-spin" /> : <LogIn size={14} />}
              连接
            </button>
          </div>
          <div className="text-[11px] text-[var(--ink-3)] max-w-[360px] text-center leading-relaxed">
            兼容 Navidrome 及所有 Subsonic API 服务器。密码仅保存在本机 Windows
            凭据管理器中，不上传、不落明文。
          </div>
        </div>
      ) : (
        <>
          {/* 头部：标题 + 搜索 + 断开 */}
          <div className="flex items-center gap-2.5 mb-3 shrink-0">
            {navStack.current.length > 0 && (
              <button
                className="btn-ghost w-10 h-10 shrink-0"
                onClick={goBack}
                title="返回上一页"
              >
                <ArrowLeft size={16} />
              </button>
            )}
            <span className="text-[12.5px] text-[var(--ink-2)] truncate max-w-[220px]" title={`${server} · ${username}`}>
              <Server size={13} className="inline mr-1.5 -mt-0.5" />
              {view.page === "album"
                ? view.name
                : view.page === "search"
                  ? `搜索：${view.query}`
                  : `专辑库 · ${username}`}
            </span>
            <div className="relative flex-1 max-w-[380px] ml-auto">
              <Search
                size={14}
                className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
              />
              <input
                type="text"
                value={kw}
                onChange={(e) => setKw(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && submitSearch()}
                placeholder="搜索服务器曲库…"
                className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] pl-9 pr-3 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)]"
              />
            </div>
            <button className="btn-primary h-10" onClick={submitSearch} disabled={loading}>
              {loading ? <Loader2 size={13} className="animate-spin" /> : <Search size={13} />}
              搜索
            </button>
            <button className="btn-secondary h-9 !px-3 text-[12px]" onClick={forget} title="断开并清除保存的密码">
              断开
            </button>
          </div>

          {/* 分段：专辑 / 歌曲 */}
          <div className="flex items-center gap-1.5 mb-3 shrink-0">
            {(
              [
                ["albums", `专辑 (${albums.length})`],
                ["songs", `歌曲 (${total || allSongs.length})`],
              ] as const
            ).map(([k, label]) => (
              <button
                key={k}
                onClick={() => {
                  setSeg(k);
                  setView({ page: k });
                  navStack.current = [];
                }}
                className={`px-3.5 py-1.5 rounded-full text-[12px] transition-colors ${
                  seg === k
                    ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                    : "text-[var(--ink-2)] hover:bg-[var(--shade-hover)]"
                }`}
              >
                {label}
              </button>
            ))}
          </div>

          {/* 内容区 */}
          <div
            ref={scrollRef}
            className="flex-1 min-h-0 overflow-y-auto"
            style={{ scrollbarWidth: "thin" }}
          >
            {loading && (
              <div className="flex items-center justify-center gap-2.5 text-[var(--ink-3)] text-[13px] pt-16">
                <Loader2 size={15} className="animate-spin" />
                加载中…
              </div>
            )}
            {!loading && view.page === "songs" && (
              <>
                <div className="pb-4">
                  {allSongs.map((s) => (
                    <div
                      key={s.id}
                      data-now-playing={
                        current?.kind === "navidrome" && current.qid === s.id ? "1" : undefined
                      }
                      className="group grid grid-cols-[36px_minmax(0,1fr)_160px_70px_120px] items-center gap-4 h-[56px] px-3 rounded-[13px] hover:bg-[var(--shade-hover)] transition-colors"
                      onDoubleClick={() => {
                        // 整表入队从该行开播（与其它列表一致）：播到最后一条时
                        // 由 queueExtender 自动加载更多并追加，无缝继续
                        const idx = allSongs.findIndex((x) => x.id === s.id);
                        useStore.getState().playNdList(allSongs, idx >= 0 ? idx : 0);
                        registerQueueExtender(String(allSongs[allSongs.length - 1].id));
                      }}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setMenu({ x: e.clientX, y: e.clientY, song: s });
                      }}
                    >
                      <button
                        className="w-8 h-8 rounded-full flex items-center justify-center text-[var(--ink-3)] group-hover:bg-[var(--accent)] group-hover:text-[var(--accent-on)] transition-colors"
                        onClick={() => playSong(s)}
                        title="播放"
                      >
                        <Play size={14} className="fill-current ml-px" />
                      </button>
                      <div className="min-w-0">
                        <div className="text-[13.5px] text-[var(--ink)] truncate">{s.title}</div>
                        <div className="text-[11.5px] text-[var(--ink-3)] truncate">{s.artist}</div>
                      </div>
                      <div className="text-[12.5px] text-[var(--ink-3)] truncate">{s.album}</div>
                      <div className="text-[12.5px] text-[var(--ink-2)] tabular-nums text-right">
                        {fmtTime(s.duration * 1000)}
                      </div>
                      <div className="flex items-center justify-end gap-1 pr-1">
                        <button
                          className="btn-ghost w-8 h-8"
                          onClick={() => downloadSong(s)}
                          title="下载到资料库"
                        >
                          <Download size={14} />
                        </button>
                        <button
                          className="btn-ghost w-8 h-8"
                          onClick={(ev) => {
                            const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
                            setMenu({ x: r.right - 200, y: r.bottom + 4, song: s });
                          }}
                        >
                          <MoreHorizontal size={16} className="opacity-0 group-hover:opacity-100" />
                        </button>
                      </div>
                    </div>
                  ))}
                  {allSongs.length < total && (
                    <div className="flex justify-center pt-2 pb-4">
                      <button className="btn-secondary !py-1.5 !px-4" onClick={loadMoreSongs}>
                        {loading ? <Loader2 size={13} className="animate-spin" /> : null}
                        加载更多（{allSongs.length}/{total}）
                      </button>
                    </div>
                  )}
                  {!allSongs.length && (
                    <div className="text-center text-[var(--ink-3)] text-[13px] pt-10">
                      服务器曲库为空
                    </div>
                  )}
                </div>
              </>
            )}
            {!loading && view.page === "albums" && (
              <div className="grid grid-cols-4 gap-3.5 pb-4">
                {albums.map((a) => (
                  <button
                    key={a.id}
                    onClick={() => openAlbum(a)}
                    className="text-left group"
                    title={`${a.name} · ${a.artist}`}
                  >
                    <div className="aspect-square rounded-xl overflow-hidden mb-2 bg-[var(--shade)]">
                      {a.coverUrl ? (
                        <img
                          src={a.coverUrl}
                          alt=""
                          className="w-full h-full object-cover group-hover:scale-[1.04] transition-transform duration-200"
                          draggable={false}
                        />
                      ) : (
                        <div className="w-full h-full flex items-center justify-center">
                          <Server size={22} className="text-[var(--ink-3)]" />
                        </div>
                      )}
                    </div>
                    <div className="text-[13px] text-[var(--ink)] truncate font-medium">{a.name}</div>
                    <div className="text-[11.5px] text-[var(--ink-3)] truncate">
                      {a.artist} · {a.songCount} 首
                    </div>
                  </button>
                ))}
                {!albums.length && (
                  <div className="col-span-4 text-center text-[var(--ink-3)] text-[13px] pt-10">
                    服务器曲库为空
                  </div>
                )}
              </div>
            )}
            {/* 专辑详情 / 搜索结果共用此块；songs 页有自己的全曲库块，
                不能落进来——否则 albumSongs 的残留行会叠在 allSongs 下面
                （表现为歌曲页多出几行，计数却是全曲库的） */}
            {!loading && (view.page === "album" || view.page === "search") && (
              <>
                {view.page === "album" && (
                  <div className="flex items-end gap-4 mb-4">
                    <div className="text-[22px] font-extrabold text-[var(--ink)]">{view.name}</div>
                    <div className="text-[12.5px] text-[var(--ink-3)] pb-1">
                      {view.artist} · {rows.length} 首
                    </div>
                  </div>
                )}
                <div className="pb-4">
                  {rows.map(({ s }) => (
                    <div
                      key={s.id}
                      className="group grid grid-cols-[36px_minmax(0,1fr)_160px_70px_120px] items-center gap-4 h-[56px] px-3 rounded-[13px] hover:bg-[var(--shade-hover)] transition-colors"
                      onDoubleClick={() => playSong(s)}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setMenu({ x: e.clientX, y: e.clientY, song: s });
                      }}
                    >
                      <button
                        className="w-8 h-8 rounded-full flex items-center justify-center text-[var(--ink-3)] group-hover:bg-[var(--accent)] group-hover:text-[var(--accent-on)] transition-colors"
                        onClick={() => playSong(s)}
                        title="播放"
                      >
                        <Play size={14} className="fill-current ml-px" />
                      </button>
                      <div className="min-w-0">
                        <div className="text-[13.5px] text-[var(--ink)] truncate">{s.title}</div>
                        <div className="text-[11.5px] text-[var(--ink-3)] truncate">{s.artist}</div>
                      </div>
                      <div className="text-[12.5px] text-[var(--ink-3)] truncate">{s.album}</div>
                      <div className="text-[12.5px] text-[var(--ink-2)] tabular-nums text-right">
                        {fmtTime(s.duration * 1000)}
                      </div>
                      <div className="flex items-center justify-end gap-1 pr-1">
                        <button
                          className="btn-ghost w-8 h-8"
                          onClick={() => downloadSong(s)}
                          title="下载到资料库"
                        >
                          <Download size={14} />
                        </button>
                        <button
                          className="btn-ghost w-8 h-8"
                          onClick={(ev) => {
                            const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
                            setMenu({ x: r.right - 200, y: r.bottom + 4, song: s });
                          }}
                        >
                          <MoreHorizontal size={16} className="opacity-0 group-hover:opacity-100" />
                        </button>
                      </div>
                    </div>
                  ))}
                  {!rows.length && (
                    <div className="text-center text-[var(--ink-3)] text-[13px] pt-10">
                      {view.page === "search" && searched ? "没有找到相关歌曲" : "专辑暂无曲目"}
                    </div>
                  )}
                </div>
              </>
            )}
          </div>
        </>
      )}

      {/* 右键菜单 */}
      {menu &&
        createPortal(
          <div
            className="fixed z-[75] w-[190px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
            style={(() => {
              const p = clampMenuPos(menu.x, menu.y, 190, 220);
              return { left: p.x, top: p.y };
            })()}
            onMouseDown={(e) => e.stopPropagation()}
            onMouseLeave={() => setMenu(null)}
          >
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                playSong(menu.song);
                setMenu(null);
              }}
            >
              <Play size={13} /> 播放
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                playNext(queueItemOf(menu.song));
                setMenu(null);
              }}
            >
              <ListPlus size={13} /> 下一首播放
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                addToQueue(queueItemOf(menu.song));
                setMenu(null);
              }}
            >
              <ListPlus size={13} /> 加入队列
            </button>
            <div className="my-1 mx-2 border-t border-[var(--line)]" />
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                toggleLikeOnline(likeRowOf(menu.song));
                setMenu(null);
              }}
            >
              <Check size={13} /> 收藏到“我喜欢”
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                setPickerSong(menu.song);
                setMenu(null);
              }}
            >
              <ListPlus size={13} /> 添加到播放列表…
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                downloadSong(menu.song);
                setMenu(null);
              }}
            >
              <Download size={13} /> 下载到资料库
            </button>
          </div>,
          document.body
        )}

      {/* 添加到播放列表 */}
      {pickerSong && (
        <Modal open onClose={() => setPickerSong(null)} title="添加到播放列表" width={380}>
          <div className="flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
            {playlists.map((p) => (
              <button
                key={p.id}
                className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
                onClick={async () => {
                  await addOnlineToPlaylist(p.id, likeRowOf(pickerSong));
                  toast(`已添加到「${p.name}」`, "success");
                  setPickerSong(null);
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
                还没有播放列表，请先在侧边栏创建
              </div>
            )}
          </div>
          <div className="flex justify-end mt-2">
            <button className="btn-secondary" onClick={() => setPickerSong(null)}>
              关闭
            </button>
          </div>
        </Modal>
      )}

      {/* 下载中提示 */}
      {downloading && (
        <div className="fixed bottom-24 right-8 z-[70]">
          <span className="chip text-[var(--ink-2)]" style={{ background: "var(--shade-strong)" }}>
            <Loader2 size={12} className="animate-spin" />
            正在下载…
          </span>
        </div>
      )}
    </div>
  );
}
