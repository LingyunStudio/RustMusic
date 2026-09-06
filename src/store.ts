import { create } from "zustand";
import { api, listenEvent, type ListenerUnbind } from "./api";
import type {
  CurrentTrack,
  PlaylistEntryMeta,
  DownloadState,
  Folder,
  LyricsPayload,
  NeteaseTrack,
  PlayState,
  QqSong,
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
  lyricsFor: string | null;

  // 网易云在线曲库
  neteaseResults: NeteaseTrack[];
  neteaseTotal: number;
  neteaseSearching: boolean;
  neteaseSearched: boolean;
  neteaseLoggedIn: boolean;
  neteaseNickname: string;
  neteaseCache: Record<number, NeteaseTrack>;

  qqResults: QqSong[];
  qqSearching: boolean;
  qqSearched: boolean;
  qqLoggedIn: boolean;
  qqNickname: string;
  qqCache: Record<string, QqSong>;

  quality: string;
  savedOnline: Record<string, boolean>;
  likedOnline: import("./types").PlaylistEntryMeta[];
  loadMoreLock: boolean;
  neteaseLiked: Record<number, boolean>;

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
  playQq(list: QqSong[], idx: number): void;
  playEntries(entries: PlaylistEntryMeta[], idx: number): void;
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

  neteaseSearch(kw: string, append?: boolean): Promise<void>;
  neteaseRefreshStatus(): Promise<void>;
  neteaseSetLogin(loggedIn: boolean, nickname: string): void;
  neteaseSyncLikes(): Promise<void>;
  neteaseToggleLike(id: number): void;
  neteaseLogout(): Promise<void>;

  qqSearch(kw: string, append?: boolean): Promise<void>;
  qqRefreshStatus(): Promise<void>;
  qqSetLogin(loggedIn: boolean, nickname: string): void;
  qqLogout(): Promise<void>;
  playQq(list: QqSong[], idx: number): void;

  setQuality(q: string): void;
  toggleLikeOnline(row: {
    kind: string;
    id: string | number;
    name: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
    mediaMid?: string;
    vip?: boolean;
  }): Promise<void>;
  downloadOnline(row: {
    kind: string;
    id: string | number;
    name: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
    mediaMid?: string;
  }): Promise<void>;
  refreshLikedOnline(): Promise<void>;
  addOnlineToPlaylist(
    pid: number,
    row: {
      kind: string;
      id: string | number;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      mediaMid?: string;
      vip?: boolean;
    }
  ): Promise<void>;
  removePlaylistEntryRow(rowid: number): Promise<void>;
  importNeteasePlaylist(remotePid: number, name: string): Promise<void>;

  loadLyricsByKey(key: string): Promise<void>;
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

  qqResults: [],
  qqSearching: false,
  qqSearched: false,
  qqLoggedIn: false,
  qqNickname: "",
  qqCache: {},

  quality: "high",
  savedOnline: {},
  likedOnline: [],
  loadMoreLock: false,
  neteaseLiked: {},

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
            nid: p.nid ?? null,
            qid: p.qid ?? null,
            quality: p.quality ?? null,
            liked,
          },
          playing: p.playing,
          dur: p.durationMs,
          pos: 0,
        });
        if (!p.playing) set({ pos: get().pos });
        // 自动加载当前曲目的歌词（播放栏滚动展示用）
        const key =
          p.kind === "track" && p.id != null
            ? `track-${p.id}`
            : p.kind === "netease" && p.nid != null
              ? `net-${p.nid}`
              : p.kind === "qq" && p.qid != null
                ? `qq-${p.qid}`
                : null;
        if (key) get().loadLyricsByKey(key);
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
      const [settings, tracks, folders, playlists, sources, neteaseStatus, qqStatus] = await Promise.all([
        api.getSettings(),
        api.listTracks(),
        api.listFolders(),
        api.listPlaylists(),
        api.listSources(),
        api.neteaseStatus(),
        api.qqStatus(),
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
        qqLoggedIn: qqStatus.loggedIn,
        qqNickname: qqStatus.nickname,
        quality: settings.quality,
        ready: true,
      });
      if (neteaseStatus.loggedIn) get().neteaseSyncLikes();
      get().refreshLikedOnline();
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

  playEntries(entries: PlaylistEntryMeta[], idx: number) {
    const queue: QueueItem[] = [];
    const neteaseCache = { ...get().neteaseCache };
    const qqCache = { ...get().qqCache };
    for (const e of entries) {
      if (e.kind === "local" && e.trackId != null) {
        queue.push({ kind: "track", id: e.trackId });
      } else if (e.kind === "netease" && e.onlineId) {
        const idNum = Number(e.onlineId);
        neteaseCache[idNum] = {
          id: idNum,
          name: e.title,
          ar: [{ name: e.artist }],
          al: { name: e.album, picUrl: e.cover },
          dt: Math.round(e.duration * 1000),
          fee: 0,
        };
        queue.push({ kind: "netease", id: idNum });
      } else if (e.kind === "qq" && e.onlineId) {
        qqCache[e.onlineId] = {
          id: e.onlineId,
          name: e.title,
          singer: e.artist,
          album: e.album,
          albumMid: "",
          mediaMid: "",
          durationMs: Math.round(e.duration * 1000),
          vip: false,
        };
        queue.push({ kind: "qq", id: e.onlineId });
      }
    }
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      neteaseCache,
      qqCache,
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
    }));
    if (queue.length) get().playQueueIndex(target);
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
    } else if (item.kind === "qq") {
      const t = get().qqCache[item.id];
      if (!t) {
        get().toast("该在线曲目信息已失效，请重新搜索", "error");
        return;
      }
      api
        .qqPlay({
          songmid: t.id,
          title: t.name,
          artist: t.singer,
          album: t.album,
          albumMid: t.albumMid,
          mediaMid: t.mediaMid,
          durationMs: t.durationMs,
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
      get().playQueueIndex(qIndex);
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
    const { queue, qIndex } = get();
    if (!queue.length) return;
    // 直接切到上一曲（到列表头则回绕到最后一首）
    const idx = qIndex > 0 ? qIndex - 1 : queue.length - 1;
    set((s) => ({ qIndex: idx, history: [...s.history.slice(-50), s.qIndex] }));
    get().playQueueIndex(idx);
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

  playQq(list, idx) {
    if (!list.length) return;
    const cache = { ...get().qqCache };
    for (const t of list) cache[t.id] = t;
    const queue: QueueItem[] = list.map((t) => ({ kind: "qq", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      qqCache: cache,
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
    }));
    get().playQueueIndex(target);
  },

  async qqSearch(kw, append = false) {
    const keyword = kw.trim();
    if (!keyword) return;
    if (append && get().loadMoreLock) return;
    set({ qqSearching: true, qqSearched: true });
    try {
      const page = append ? Math.floor(get().qqResults.length / 30) + 1 : 1;
      const r = await api.qqSearch(keyword, page);
      const cache = { ...get().qqCache };
      for (const t of r.songs) cache[t.id] = t;
      set((s) => ({
        qqResults: append ? [...s.qqResults, ...r.songs] : r.songs,
        qqSearching: false,
        qqCache: cache,
      }));
    } catch (e) {
      set({ qqSearching: false });
      get().toast(String(e), "error");
    }
  },

  async qqRefreshStatus() {
    try {
      const s = await api.qqStatus();
      set({ qqLoggedIn: s.loggedIn, qqNickname: s.nickname });
    } catch {}
  },

  qqSetLogin(loggedIn, nickname) {
    set({ qqLoggedIn: loggedIn, qqNickname: nickname });
  },

  async qqLogout() {
    try {
      await api.qqLogout();
      set({ qqLoggedIn: false, qqNickname: "" });
      get().toast("已退出 QQ 音乐登录", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  setQuality(q) {
    set({ quality: q });
    api.setPlayQuality(q).catch((e) => get().toast(String(e), "error"));
  },

  async toggleLikeOnline(row) {
    const key = `${row.kind}-${row.id}`;
    const next = !get().savedOnline[key];
    // 乐观更新
    set((s) => ({ savedOnline: { ...s.savedOnline, [key]: next } }));
    try {
      await api.likeOnline({
        kind: row.kind,
        rid: String(row.id),
        title: row.name,
        artist: row.artist,
        album: row.album,
        cover: row.cover,
        durationMs: row.durationMs,
        mediaMid: row.mediaMid ?? "",
        vip: row.vip ?? false,
        like: next,
      });
      await get().refreshLikedOnline();
    } catch (e) {
      // 回滚
      set((s) => ({ savedOnline: { ...s.savedOnline, [key]: !next } }));
      get().toast(String(e), "error");
    }
  },

  async downloadOnline(row) {
    get().toast("开始下载…", "info");
    try {
      const name = await api.downloadOnline({
        kind: row.kind,
        id: String(row.id),
        title: row.name,
        artist: row.artist,
        album: row.album,
        coverUrl: row.cover,
        durationMs: row.durationMs,
        mediaMid: row.mediaMid ?? "",
      });
      await get().refreshTracks();
      get().toast(`已下载到资料库：${name}`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async refreshLikedOnline() {
    try {
      const list = await api.likedOnlineList();
      const saved: Record<string, boolean> = {};
      for (const e of list) {
        if (e.onlineId) saved[`${e.kind}-${e.onlineId}`] = true;
      }
      set({ likedOnline: list, savedOnline: { ...saved } });
    } catch {}
  },

  async addOnlineToPlaylist(pid, row) {
    try {
      await api.addOnlineToPlaylist({
        playlistId: pid,
        kind: row.kind,
        rid: String(row.id),
        title: row.name,
        artist: row.artist,
        album: row.album,
        cover: row.cover,
        durationMs: row.durationMs,
        mediaMid: row.mediaMid ?? "",
        vip: row.vip ?? false,
      });
      await get().refreshPlaylists();
      const pl = get().playlists.find((p) => p.id === pid);
      get().toast(`已添加到「${pl?.name ?? "播放列表"}」`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async removePlaylistEntryRow(rowid) {
    try {
      await api.removePlaylistEntry(rowid);
      await get().refreshPlaylists();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async importNeteasePlaylist(remotePid, name) {
    try {
      await get().createPlaylist(name);
      const pls = get().playlists;
      const created = pls[pls.length - 1];
      if (!created) throw new Error("创建播放列表失败");
      const n = await api.neteaseImportPlaylist(remotePid, created.id);
      await get().refreshPlaylists();
      get().toast(`已导入「${name}」${n} 首（在线播放，按账号权益）`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async neteaseSearch(kw, append = false) {
    const keyword = kw.trim();
    if (!keyword) return;
    set({ neteaseSearching: true, neteaseSearched: true });
    try {
      const offset = append ? get().neteaseResults.length : 0;
      const r = await api.neteaseSearch(keyword, offset);
      const cache = { ...get().neteaseCache };
      for (const t of r.songs) cache[t.id] = t;
      set((s) => ({
        neteaseResults: append ? [...s.neteaseResults, ...r.songs] : r.songs,
        neteaseTotal: r.total,
        neteaseSearching: false,
        neteaseCache: cache,
      }));
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

  async neteaseSyncLikes() {
    if (!get().neteaseLoggedIn) return;
    try {
      const ids = await api.neteaseLikeList();
      const liked: Record<number, boolean> = {};
      for (const id of ids) liked[id] = true;
      set({ neteaseLiked: liked });
    } catch {
      /* 静默：下次启动再同步 */
    }
  },

  neteaseToggleLike(id) {
    const likedState = get().neteaseLiked;
    const next = !likedState[id];
    set({ neteaseLiked: { ...likedState, [id]: next } });
    api
      .neteaseLike(id, next)
      .then(() =>
        get().toast(next ? "已收藏到账号的“我喜欢”" : "已取消收藏", "success")
      )
      .catch((e) => {
        // 回滚
        const cur = { ...get().neteaseLiked };
        if (next) delete cur[id];
        else cur[id] = true;
        set({ neteaseLiked: cur });
        get().toast(String(e), "error");
      });
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

  async loadLyricsByKey(key) {
    if (get().lyricsFor === key) return;
    set({ lyricsLoading: true, lyricsFor: key, lyrics: null });
    const [kind, ...rest] = key.split("-");
    const id = rest.join("-");
    try {
      const payload =
        kind === "net"
          ? await api.neteaseLyric(Number(id))
          : kind === "qq"
            ? await api.qqLyric(id)
            : await api.getLyrics(Number(id));
      if (get().lyricsFor === key) {
        set({ lyrics: payload, lyricsLoading: false });
      }
    } catch {
      if (get().lyricsFor === key) set({ lyricsLoading: false });
    }
  },

  async loadLyrics(trackId) {
    get().loadLyricsByKey(`track-${trackId}`);
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
