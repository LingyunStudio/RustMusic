import { create } from "zustand";
import { api, listenEvent, type ListenerUnbind } from "./api";
import type {
  CurrentTrack,
  DownloadState,
  Folder,
  LyricsPayload,
  NeteaseTrack,
  PlayState,
  Playlist,
  QueueItem,
  RepeatMode,
  ScanState,
  SourceItem,
  Toast,
  TrackMeta,
  ViewName,
} from "./types";

interface Store {
  ready: boolean;
  tracks: TrackMeta[];
  folders: Folder[];
  playlists: Playlist[];
  sources: SourceItem[];

  current: CurrentTrack | null;
  playing: boolean;
  pos: number;
  dur: number;
  queue: QueueItem[];
  qIndex: number;
  history: number[];
  volume: number;
  speed: number;
  repeat: RepeatMode;
  shuffle: boolean;
  eqGains: number[];
  eqEnabled: boolean;

  view: ViewName;
  viewParam: number;
  search: string;
  nowPlayingOpen: boolean;
  queueOpen: boolean;
  scan: ScanState;
  download: DownloadState | null;
  toasts: Toast[];
  lyrics: LyricsPayload | null;
  lyricsLoading: boolean;
  lyricsFor: number | null;

  // 网易云在线曲库
  neteaseResults: NeteaseTrack[];
  neteaseTotal: number;
  neteaseSearching: boolean;
  neteaseSearched: boolean;
  neteaseLoggedIn: boolean;
  neteaseNickname: string;
  neteaseCache: Record<number, NeteaseTrack>;

  init(): Promise<void>;
  toast(msg: string, type?: Toast["type"]): void;
  dismissToast(id: number): void;
  setView(v: ViewName, param?: number): void;
  setSearch(s: string): void;
  setNowPlayingOpen(v: boolean): void;
  setQueueOpen(v: boolean): void;

  refreshTracks(): Promise<void>;
  refreshFolders(): Promise<void>;
  refreshPlaylists(): Promise<void>;
  refreshSources(): Promise<void>;

  playTracks(tracks: TrackMeta[], idx: number): void;
  playSourceItem(s: SourceItem): void;
  playNetease(list: NeteaseTrack[], idx: number): void;
  playQueueIndex(i: number): void;
  togglePlay(): void;
  next(auto?: boolean): void;
  prev(): void;
  seek(ms: number): void;
  setVolume(v: number): void;
  setSpeed(v: number): void;
  setRepeat(m: RepeatMode): void;
  toggleShuffle(): void;

  toggleLike(id: number): void;
  addToQueue(item: QueueItem): void;
  playNext(item: QueueItem): void;
  removeQueueItem(i: number): void;
  clearQueue(): void;
  jumpTo(i: number): void;

  createPlaylist(name: string): Promise<void>;
  deletePlaylist(id: number): Promise<void>;
  renamePlaylist(id: number, name: string): Promise<void>;
  addToPlaylist(pid: number, tid: number): Promise<void>;
  removeFromPlaylist(pid: number, tid: number): Promise<void>;

  addFolderByDialog(): Promise<void>;
  removeFolder(id: number): Promise<void>;
  rescan(): void;
  addSource(url: string, title: string): Promise<void>;
  deleteSource(id: number): Promise<void>;
  setEq(gains: number[], enabled: boolean): void;
  clearCache(): Promise<void>;

  neteaseSearch(kw: string): Promise<void>;
  neteaseRefreshStatus(): Promise<void>;
  neteaseSetLogin(loggedIn: boolean, nickname: string): void;
  neteaseLogout(): Promise<void>;

  loadLyrics(trackId: number): Promise<void>;
  applyMediaControl(action: string, value?: number): void;
}

let toastSeq = 1;
let unbinds: ListenerUnbind[] = [];
let volumeTimer: ReturnType<typeof setTimeout> | null = null;

export const useStore = create<Store>((set, get) => ({
  ready: false,
  tracks: [],
  folders: [],
  playlists: [],
  sources: [],

  current: null,
  playing: false,
  pos: 0,
  dur: 0,
  queue: [],
  qIndex: 0,
  history: [],
  volume: 0.8,
  speed: 1,
  repeat: "off",
  shuffle: false,
  eqGains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  eqEnabled: false,

  view: "library",
  viewParam: 0,
  search: "",
  nowPlayingOpen: false,
  queueOpen: false,
  scan: { active: false, done: 0, total: 0 },
  download: null,
  toasts: [],
  lyrics: null,
  lyricsLoading: false,
  lyricsFor: null,

  neteaseResults: [],
  neteaseTotal: 0,
  neteaseSearching: false,
  neteaseSearched: false,
  neteaseLoggedIn: false,
  neteaseNickname: "",
  neteaseCache: {},

  // ---------- 初始化 ----------

  async init() {
    if (unbinds.length) return;

    unbinds.push(
      await listenEvent<PlayState>("player://state", (p) => {
        const liked =
          p.kind === "track" && p.id != null
            ? get().tracks.find((t) => t.id === p.id)?.liked ?? false
            : false;
        set({
          current: {
            id: p.id ?? null,
            kind: p.kind,
            path: p.path,
            title: p.title,
            artist: p.artist,
            album: p.album,
            cover: p.cover,
            durationMs: p.durationMs,
            liked,
          },
          playing: p.playing,
          dur: p.durationMs,
          pos: 0,
        });
        if (!p.playing) set({ pos: get().pos });
      })
    );

    unbinds.push(
      await listenEvent<{ pos: number; dur: number }>("player://pos", (p) => {
        set({ pos: p.pos, dur: p.dur > 0 ? p.dur : get().dur });
      })
    );

    unbinds.push(await listenEvent("player://ended", () => get().next(true)));

    unbinds.push(
      await listenEvent<{ action: string; value?: number }>("media://control", (p) =>
        get().applyMediaControl(p.action, p.value)
      )
    );

    unbinds.push(
      await listenEvent<ScanState>("scan://progress", (p) => {
        const wasActive = get().scan.active;
        set({ scan: p });
        if (wasActive && !p.active) {
          get().refreshTracks();
          get().refreshFolders();
        }
      })
    );

    unbinds.push(
      await listenEvent<{
        url: string;
        pct?: number;
        done?: boolean;
        error?: string;
        received?: number;
        total?: number;
      }>("download://progress", (p) => {
        if (p.error) {
          set({ download: null });
          get().toast(`音源下载失败：${p.error}`, "error");
          return;
        }
        if (p.done) {
          set({ download: null });
          return;
        }
        set({
          download: {
            title: p.url.split("/").pop() ?? p.url,
            pct: p.pct ?? 0,
          },
        });
      })
    );

    try {
      const [settings, tracks, folders, playlists, sources, neteaseStatus] = await Promise.all([
        api.getSettings(),
        api.listTracks(),
        api.listFolders(),
        api.listPlaylists(),
        api.listSources(),
        api.neteaseStatus(),
      ]);
      set({
        volume: settings.volume,
        speed: settings.speed,
        eqGains: settings.eqGains,
        eqEnabled: settings.eqEnabled,
        tracks,
        folders,
        playlists,
        sources,
        neteaseLoggedIn: neteaseStatus.loggedIn,
        neteaseNickname: neteaseStatus.nickname,
        ready: true,
      });
    } catch (e) {
      set({ ready: true });
      get().toast(`初始化失败：${e}`, "error");
    }
  },

  // ---------- 提示 ----------

  toast(msg, type = "info") {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts, { id, msg, type }] }));
    setTimeout(() => get().dismissToast(id), 3600);
  },

  dismissToast(id) {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
  },

  // ---------- 视图 ----------

  setView(v, param = 0) {
    set({ view: v, viewParam: param, search: "" });
  },

  setSearch(s) {
    set({ search: s });
  },

  setNowPlayingOpen(v) {
    set({ nowPlayingOpen: v });
  },

  setQueueOpen(v) {
    set({ queueOpen: v });
  },

  // ---------- 数据刷新 ----------

  async refreshTracks() {
    try {
      set({ tracks: await api.listTracks() });
    } catch {}
  },

  async refreshFolders() {
    try {
      set({ folders: await api.listFolders() });
    } catch {}
  },

  async refreshPlaylists() {
    try {
      set({ playlists: await api.listPlaylists() });
    } catch {}
  },

  async refreshSources() {
    try {
      set({ sources: await api.listSources() });
    } catch {}
  },

  // ---------- 播放 ----------

  playTracks(tracks, idx) {
    if (!tracks.length) return;
    const queue: QueueItem[] = tracks.map((t) => ({ kind: "track", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
    }));
    get().playQueueIndex(target);
  },

  playNetease(list: NeteaseTrack[], idx: number) {
    if (!list.length) return;
    const cache = { ...get().neteaseCache };
    for (const t of list) cache[t.id] = t;
    const queue: QueueItem[] = list.map((t) => ({ kind: "netease", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      neteaseCache: cache,
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
    }));
    get().playQueueIndex(target);
  },

  playSourceItem(s) {
    const queue: QueueItem[] = [{ kind: "url", id: s.id }];
    set((st) => ({ queue, qIndex: 0, history: [...st.history.slice(-50), st.qIndex] }));
    api
      .playSource(s.id)
      .catch((e) => get().toast(`播放音源失败：${e}`, "error"));
  },

  playQueueIndex(i) {
    const { queue } = get();
    const item = queue[i];
    if (!item) return;
    if (item.kind === "track") {
      api.playTrack(item.id).catch((e) => get().toast(`播放失败：${e}`, "error"));
    } else if (item.kind === "netease") {
      const t = get().neteaseCache[item.id];
      if (!t) {
        get().toast("该在线曲目信息已失效，请重新搜索", "error");
        return;
      }
      api
        .neteasePlay({
          id: t.id,
          title: t.name,
          artist: t.ar.map((a) => a.name).join(" / "),
          album: t.al?.name ?? "",
          cover: t.al?.picUrl ?? "",
          durationMs: t.dt,
        })
        .catch((e) => get().toast(`播放失败：${e}`, "error"));
    } else {
      api
        .playSource(item.id)
        .catch((e) => get().toast(`播放音源失败：${e}`, "error"));
    }
  },

  togglePlay() {
    const { current, queue, qIndex, tracks } = get();
    if (!current) {
      if (queue.length) {
        get().playQueueIndex(qIndex);
      } else if (tracks.length) {
        get().playTracks(tracks, 0);
      }
      return;
    }
    api.playPause().catch((e) => get().toast(String(e), "error"));
  },

  next(auto = false) {
    const { queue, qIndex, repeat, shuffle, current } = get();
    if (!queue.length) return;

    if (auto && repeat === "one" && current) {
      // 单曲循环：重新播放当前曲目
      const item = queue[qIndex];
      if (item.kind === "track") api.playTrack(item.id).catch(() => {});
      else api.playSource(item.id).catch(() => {});
      return;
    }

    let idx: number;
    if (shuffle && queue.length > 1) {
      do {
        idx = Math.floor(Math.random() * queue.length);
      } while (idx === qIndex);
    } else {
      idx = qIndex + 1;
    }
    if (idx >= queue.length) {
      if (repeat === "all") {
        idx = 0;
      } else {
        set({ playing: false, pos: 0 });
        return;
      }
    }
    set((s) => ({ qIndex: idx, history: [...s.history.slice(-50), s.qIndex] }));
    get().playQueueIndex(idx);
  },

  prev() {
    const { queue, qIndex, history, pos } = get();
    if (!queue.length) return;
    if (pos > 3000) {
      get().seek(0);
      return;
    }
    if (history.length) {
      const idx = history[history.length - 1];
      set({ history: history.slice(0, -1), qIndex: Math.min(idx, queue.length - 1) });
      get().playQueueIndex(get().qIndex);
    } else if (qIndex > 0) {
      set({ qIndex: qIndex - 1 });
      get().playQueueIndex(qIndex - 1);
    } else {
      get().seek(0);
    }
  },

  seek(ms) {
    set({ pos: ms });
    api.seek(Math.round(ms)).catch(() => {});
  },

  setVolume(v) {
    const vol = Math.max(0, Math.min(1, v));
    set({ volume: vol });
    if (volumeTimer) clearTimeout(volumeTimer);
    volumeTimer = setTimeout(() => {
      api.setVolume(vol).catch(() => {});
    }, 300);
  },

  setSpeed(v) {
    set({ speed: v });
    api.setSpeed(v).catch(() => {});
  },

  setRepeat(m) {
    set({ repeat: m });
  },

  toggleShuffle() {
    set((s) => ({ shuffle: !s.shuffle }));
  },

  // ---------- 喜欢 / 队列 ----------

  toggleLike(id) {
    const t = get().tracks.find((x) => x.id === id);
    if (!t) return;
    const liked = !t.liked;
    set((s) => ({
      tracks: s.tracks.map((x) => (x.id === id ? { ...x, liked } : x)),
      current:
        s.current && s.current.id === id ? { ...s.current, liked } : s.current,
    }));
    api.likeTrack(id, liked).catch(() => {});
  },

  addToQueue(item) {
    set((s) => ({ queue: [...s.queue, item] }));
    get().toast("已加入播放队列", "success");
  },

  playNext(item) {
    set((s) => {
      const q = [...s.queue];
      q.splice(s.qIndex + 1, 0, item);
      return { queue: q };
    });
    get().toast("将在当前曲目后播放", "success");
  },

  removeQueueItem(i) {
    set((s) => {
      const q = s.queue.filter((_, idx) => idx !== i);
      let qIndex = s.qIndex;
      if (i < s.qIndex) qIndex -= 1;
      return { queue: q, qIndex };
    });
  },

  clearQueue() {
    const { current } = get();
    const keep = current ? [get().queue[get().qIndex]] : [];
    set({
      queue: keep.filter(Boolean),
      qIndex: 0,
    });
  },

  jumpTo(i) {
    set({ qIndex: i, history: [...get().history.slice(-50), get().qIndex] });
    get().playQueueIndex(i);
  },

  // ---------- 播放列表 ----------

  async createPlaylist(name) {
    try {
      await api.createPlaylist(name);
      await get().refreshPlaylists();
      get().toast(`已创建播放列表「${name}」`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async deletePlaylist(id) {
    try {
      await api.deletePlaylist(id);
      await get().refreshPlaylists();
      if (get().view === "playlist" && get().viewParam === id) {
        set({ view: "library" });
      }
      get().toast("播放列表已删除", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async renamePlaylist(id, name) {
    try {
      await api.renamePlaylist(id, name);
      await get().refreshPlaylists();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async addToPlaylist(pid, tid) {
    try {
      await api.addToPlaylist(pid, tid);
      await get().refreshPlaylists();
      const pl = get().playlists.find((p) => p.id === pid);
      get().toast(`已添加到「${pl?.name ?? "播放列表"}」`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async removeFromPlaylist(pid, tid) {
    try {
      await api.removeFromPlaylist(pid, tid);
      await get().refreshPlaylists();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  // ---------- 媒体库 ----------

  async addFolderByDialog() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false, title: "选择音乐文件夹" });
      if (!selected || typeof selected !== "string") return;
      await api.addFolder(selected);
      get().toast("文件夹已添加，正在扫描…", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async removeFolder(id) {
    try {
      await api.removeFolder(id);
      await get().refreshFolders();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  rescan() {
    api.rescan().catch((e) => get().toast(String(e), "error"));
  },

  // ---------- 在线音源 ----------

  async addSource(url, title) {
    try {
      await api.addSource(url, title);
      await get().refreshSources();
      get().toast("音源已添加", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async deleteSource(id) {
    try {
      await api.deleteSource(id);
      await get().refreshSources();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  // ---------- 均衡器 / 缓存 ----------

  setEq(gains, enabled) {
    set({ eqGains: [...gains], eqEnabled: enabled });
    api.setEq(gains, enabled).catch(() => {});
  },

  async clearCache() {
    try {
      const n = await api.clearCache();
      get().toast(`已清理 ${n} 个缓存文件`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  // ---------- 网易云 ----------

  async neteaseSearch(kw) {
    const keyword = kw.trim();
    if (!keyword) return;
    set({ neteaseSearching: true, neteaseSearched: true });
    try {
      const r = await api.neteaseSearch(keyword);
      const cache = { ...get().neteaseCache };
      for (const t of r.songs) cache[t.id] = t;
      set({
        neteaseResults: r.songs,
        neteaseTotal: r.total,
        neteaseSearching: false,
        neteaseCache: cache,
      });
    } catch (e) {
      set({ neteaseSearching: false });
      get().toast(String(e), "error");
    }
  },

  async neteaseRefreshStatus() {
    try {
      const s = await api.neteaseStatus();
      set({ neteaseLoggedIn: s.loggedIn, neteaseNickname: s.nickname });
    } catch {}
  },

  neteaseSetLogin(loggedIn, nickname) {
    set({ neteaseLoggedIn: loggedIn, neteaseNickname: nickname });
  },

  async neteaseLogout() {
    try {
      await api.neteaseLogout();
      set({ neteaseLoggedIn: false, neteaseNickname: "" });
      get().toast("已退出网易云登录", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  // ---------- 歌词 ----------

  async loadLyrics(trackId) {
    if (get().lyricsFor === trackId) return;
    set({ lyricsLoading: true, lyricsFor: trackId, lyrics: null });
    try {
      const payload = await api.getLyrics(trackId);
      if (get().lyricsFor === trackId) {
        set({ lyrics: payload, lyricsLoading: false });
      }
    } catch {
      if (get().lyricsFor === trackId) set({ lyricsLoading: false });
    }
  },

  applyMediaControl(action, value) {
    const s = get();
    switch (action) {
      case "play":
        if (!s.playing) api.resume().catch(() => {});
        break;
      case "pause":
        if (s.playing) api.pause().catch(() => {});
        break;
      case "toggle":
        s.togglePlay();
        break;
      case "next":
        s.next(false);
        break;
      case "prev":
        s.prev();
        break;
      case "stop":
        api.stop().catch(() => {});
        break;
      case "seek_fwd":
        s.seek(Math.min(s.pos + 10000, s.dur || s.pos + 10000));
        break;
      case "seek_back":
        s.seek(Math.max(0, s.pos - 10000));
        break;
      case "set_pos":
        if (value != null) s.seek(value);
        break;
      case "show":
        break;
    }
  },
}));
