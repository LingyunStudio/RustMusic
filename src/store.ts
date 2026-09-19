import { create } from "zustand";
import { api, listenEvent, type ListenerUnbind } from "./api";
import {
  ACCENTS,
  applyAccent,
  applyTheme,
  loadAccent,
  loadTheme,
  saveAccent,
  saveTheme,
  loadDesktopLyricsColors,
  saveDesktopLyricsColors,
  type DesktopLyricsColors,
  loadLyricsColors,
  saveLyricsColors,
  applyLyricsColors,
  type LyricPageColors,
} from "./theme";
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
  /** 后端“开播代次”，用于区分换曲开播与暂停/恢复 */
  playSeq: number;
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
  /** 播放页无边框全屏（隐藏系统任务栏；播放条隐藏、hover 唤起） */
  fullscreen: boolean;
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
  /** 搜索分页页码（QQ 按页码翻页，不按结果数推算） */
  qqPage: number;
  qqLoggedIn: boolean;
  qqNickname: string;
  qqCache: Record<string, QqSong>;

  quality: string;
  /** 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用 */
  closeAction: "tray" | "exit";
  /** 启动时自动检查 GitHub 更新（默认开启） */
  autoUpdate: boolean;
  /** 音源缓存上限（字节），0 = 不限制 */
  cacheLimit: number;
  /** 当前缓存占用（字节），null = 尚未查询 */
  cacheBytes: number | null;
  /** 播放失败的在线曲目（键 kind:id，值失败原因）：列表置灰 + 自动跳过 */
  unavailable: Record<string, string>;
  /** 连续播放失败计数（成功开播清零；达队列长度停止自动跳过） */
  failStreak: number;
  /** 当前播放的自定义在线音源 id（SourcesView 高亮用；path 是缓存文件名，无法从 URL 判断） */
  playingSourceId: number | null;
  scrubbing: boolean;
  theme: "dark" | "light";
  accent: string;
  savedOnline: Record<string, boolean>;
  likedOnline: import("./types").PlaylistEntryMeta[];
  /** “最近播放”的在线曲目部分（含 lastPlayed 用于合并排序） */
  recentOnline: import("./types").PlaylistEntryMeta[];
  loadMoreLock: boolean;
  neteaseLiked: Record<number, boolean>;

  init(): Promise<void>;
  toast(msg: string, type?: Toast["type"]): void;
  dismissToast(id: number): void;
  setView(v: ViewName, param?: number): void;
  setSearch(s: string): void;
  setNowPlayingOpen(v: boolean): void;
  /** 切换无边框全屏（退出时同时收起播放页） */
  toggleFullscreen(v?: boolean): void;
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
  /** 在线条目（网易云/QQ）转可播放的队列项；无元数据时返回 null */
  entryToQueueItem(e: PlaylistEntryMeta): QueueItem | null;
  togglePlay(): void;
  next(auto?: boolean): void;
  prev(): void;
  seek(ms: number): void;
  setScrubbing(v: boolean): void;
  setVolume(v: number): void;
  setSpeed(v: number): void;
  setRepeat(m: RepeatMode): void;
  toggleShuffle(): void;

  /** 桌面歌词：开关状态 + 打开/关闭/解锁动作 */
  desktopLyricsOn: boolean;
  desktopLyricsLock: boolean;
  openDesktopLyrics(): Promise<void>;
  closeDesktopLyrics(): Promise<void>;
  unlockDesktopLyrics(): Promise<void>;
  /** 桌面歌词三色（已唱/未唱/下一句） */
  dlyricsColors: DesktopLyricsColors;
  setDlyricsColors(c: DesktopLyricsColors): void;
  /** 播放页歌词三色（已唱/未唱/下一句，空 = 跟随主题） */
  lyricsColors: LyricPageColors;
  setLyricsColors(c: LyricPageColors): void;

  toggleLike(id: number): void;
  addToQueue(item: QueueItem): void;
  playNext(item: QueueItem): void;
  removeQueueItem(i: number): void;
  clearQueue(): void;
  jumpTo(i: number): void;

  createPlaylist(name: string): Promise<number>;
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
  setCloseAction(a: "tray" | "exit"): void;
  setAutoUpdate(enabled: boolean): void;
  setCacheLimit(bytes: number): void;
  refreshCacheBytes(): Promise<void>;
  setTheme(t: "dark" | "light"): void;
  setAccent(key: string): void;
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
  refreshRecentOnline(): Promise<void>;
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
  importQqPlaylist(remotePid: number, name: string): Promise<void>;

  /** 行 key（unavailable 同款：track:<id> / netease:<rid> / qq:<rid>） */
  rowKeyOf(e: { kind: string; trackId?: number | null; onlineId?: string | null }): string;
  /** “资料库/我喜欢”手动排序（整份顺序全量覆盖，乐观更新本地 state） */
  saveManualOrder(list: "library" | "liked", keys: string[]): Promise<void>;
  /** 手动序号缓存（list → row_key → pos；排序时用，0 = 无记录排最后） */
  manualOrder: Record<string, Record<string, number>>;
  loadManualOrder(list: "library" | "liked"): Promise<void>;
  /** 播放列表条目手动排序（按 rowid 序列重写 position） */
  reorderPlaylist(pid: number, rowids: number[]): Promise<void>;

  loadLyricsByKey(key: string): Promise<void>;
  loadLyrics(trackId: number): Promise<void>;
  applyMediaControl(action: string, value?: number): void;
}

let toastSeq = 1;
let unbinds: ListenerUnbind[] = [];
let volumeTimer: ReturnType<typeof setTimeout> | null = null;
/** init 单例：React StrictMode 双挂载 / 并发调用时只注册一次事件监听 */
let initPromise: Promise<void> | null = null;

/** 队列项显示名（失败提示用；取不到返回占位） */
function titleOfQueueItem(
  item: { kind: string; id: number | string },
  caches: Pick<Store, "tracks" | "neteaseCache" | "qqCache" | "sources">
): string {
  if (item.kind === "track") {
    return caches.tracks.find((t) => t.id === item.id)?.title ?? `曲目 #${item.id}`;
  }
  if (item.kind === "netease") {
    return caches.neteaseCache[item.id as number]?.name ?? `网易云 #${item.id}`;
  }
  if (item.kind === "qq") {
    return caches.qqCache[item.id as string]?.name ?? `QQ音乐 #${item.id}`;
  }
  return caches.sources.find((s) => s.id === item.id)?.title ?? `音源 #${item.id}`;
}

/** 推一帧歌词/进度给桌面歌词窗口（事件式，窗口不存在时 emit 静默无副作用）。
 *  歌词行数组只在变化时随帧携带（逐字歌词可达几十 KB，250ms 一帧全量序列化
 *  是持续的 GC 压力）；歌词窗口对缺失的 lines 字段沿用上一次的值。 */
let lastPushedLines: unknown = undefined; // undefined = 尚未推过（下次必带 lines）
async function pushDesktopLyrics(
  s: Pick<
    Store,
    "lyrics" | "pos" | "dur" | "playing" | "current" | "desktopLyricsOn"
  > & { dlyricsColors?: DesktopLyricsColors }
) {
  if (!s.desktopLyricsOn) return;
  try {
    const lines = s.lyrics?.lines ?? null;
    const includeLines = lines !== lastPushedLines;
    lastPushedLines = lines;
    const { emit } = await import("@tauri-apps/api/event");
    emit(
      "dlyrics://push",
      {
        ...(includeLines ? { lines } : {}),
        synced: s.lyrics?.synced ?? false,
        pos: s.pos,
        dur: s.dur,
        playing: s.playing,
        title: s.current?.title ?? "",
        artist: s.current?.artist ?? "",
        colors: s.dlyricsColors ?? loadDesktopLyricsColors(),
      } as Record<string, unknown>
    );
  } catch {
    // 桌面歌词窗口未开/已关：忽略
  }
}

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
  playSeq: 0,
  scrubbing: false,
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
  fullscreen: false,
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
  qqPage: 1,
  qqLoggedIn: false,
  qqNickname: "",
  qqCache: {},

  quality: "high",
  closeAction: "tray",
  autoUpdate: true,
  cacheLimit: 2 * 1024 * 1024 * 1024,
  cacheBytes: null,
  unavailable: {},
  failStreak: 0,
  desktopLyricsOn: false,
  desktopLyricsLock: false,
  dlyricsColors: loadDesktopLyricsColors(),
  lyricsColors: loadLyricsColors(),
  playingSourceId: null,
  theme: "light",
  accent: "amber",
  savedOnline: {},
  likedOnline: [],
  recentOnline: [],
  loadMoreLock: false,
  neteaseLiked: {},
  /** 手动排序序号缓存：{ list → row_key → pos } */
  manualOrder: {},

  // ---------- 初始化 ----------

  async init() {
    if (initPromise) return initPromise;
    initPromise = (async () => {
    // 播放页歌词自定义配色（CSS 变量），启动即恢复
    applyLyricsColors(get().lyricsColors);
    unbinds.push(
      await listenEvent<PlayState>("player://state", (p) => {
        const liked =
          p.kind === "track" && p.id != null
            ? get().tracks.find((t) => t.id === p.id)?.liked ?? false
            : false;
        // seq 增加 = 换曲开播，进度归零；seq 不变 = 暂停/恢复，保留进度
        const isFreshStart =
          p.seq != null ? p.seq > get().playSeq : p.playing && !get().playing;
        if (p.seq != null) set({ playSeq: p.seq });
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
          pos: isFreshStart ? 0 : get().pos,
        });
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
        // 换曲开播：在线曲目更新“最近播放”；本地曲目只在本地更新单条的
        // lastPlayed/playCount（后端 record_play 已在开播时落库）——
        // 不再全量拉取曲目列表（大曲库下每首歌一次全量 IPC + 整表重渲染）
        if (isFreshStart && p.kind !== "track") get().refreshRecentOnline();
        if (isFreshStart && p.kind === "track" && p.id != null) {
          const now = Math.floor(Date.now() / 1000);
          set((s) => ({
            tracks: s.tracks.map((t) =>
              t.id === p.id ? { ...t, lastPlayed: now, playCount: t.playCount + 1 } : t
            ),
          }));
        }
        // 播放/暂停/停止的即时同步：暂停后 pos 事件停发，
        // 不在这里推一帧的话桌面歌词会一直按旧 playing 状态外推
        pushDesktopLyrics(get());
      })
    );

    unbinds.push(
      await listenEvent<{ pos: number; dur: number }>("player://pos", (p) => {
        // 拖动进度条期间不回写事件进度，避免位置抖动
        if (get().scrubbing) return;
        // 自愈：引擎只在播放中发 pos 事件。UI 的 playing 若与此不符
        // （在线切歌失败等路径误改），以引擎为准纠正
        if (!get().playing) set({ playing: true });
        set({ pos: p.pos, dur: p.dur > 0 ? p.dur : get().dur });
        // 桌面歌词跟随（250ms 一帧，歌词窗口自行插值当前行）
        pushDesktopLyrics(get());
      })
    );

    unbinds.push(await listenEvent("player://ended", () => get().next(true)));

    // 主窗口隐藏到托盘时 WebView 会被挂起（后端 TrySuspend 回收渲染内存），
    // 挂起期间发往前端的事件全部丢失：恢复后刷新一遍数据收敛状态
    unbinds.push(
      await listenEvent("webview://resumed", () => {
        const s = get();
        s.refreshTracks();
        s.refreshFolders();
        s.refreshPlaylists();
        s.refreshRecentOnline();
        s.refreshCacheBytes();
        // 扫描进度以快照为准；下载完成事件若丢失则复位下载条
        api
          .getScanState()
          .then((scan) => set({ scan }))
          .catch(() => {});
        set({ download: null });
      })
    );

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
        theme: loadTheme(),
        accent: loadAccent(),
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
        closeAction: settings.closeAction ?? "tray",
        autoUpdate: settings.autoUpdate ?? true,
        cacheLimit: settings.cacheLimit ?? 2 * 1024 * 1024 * 1024,
        ready: true,
      });
      get().refreshCacheBytes();
      if (neteaseStatus.loggedIn) get().neteaseSyncLikes();
      get().refreshLikedOnline();
      get().refreshRecentOnline();
      get().loadManualOrder("library");
      get().loadManualOrder("liked");
    } catch (e) {
      set({ ready: true });
      get().toast(`初始化失败：${e}`, "error");
    }
    })();
    return initPromise;
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

  toggleFullscreen(v) {
    const next = v ?? !get().fullscreen;
    set({ fullscreen: next });
    // 真全屏（占据整个显示器、隐藏任务栏）。最大化状态下 Windows 会拒绝
    // 切全屏，先还原窗口；错误显式提示而非静默吞掉
    import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        try {
          if (next && await win.isMaximized()) {
            await win.unmaximize();
          }
          await win.setFullscreen(next);
        } catch (e) {
          get().toast(`全屏切换失败：${e}`, "error");
        }
      })
      .catch((e) => get().toast(`全屏切换失败：${e}`, "error"));
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
    set((st) => ({
      playingSourceId: s.id,
      queue,
      qIndex: 0,
      history: [...st.history.slice(-50), st.qIndex],
    }));
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
          fee: e.vip ? 1 : 0,
        };
        queue.push({ kind: "netease", id: idNum });
      } else if (e.kind === "qq" && e.onlineId) {
        // e.cover 为完整封面 URL，最后一段即 albumMid
        const albumMid = e.cover.match(/M000([0-9A-Za-z]+)\.jpg?/)?.[1] ?? "";
        qqCache[e.onlineId] = {
          id: e.onlineId,
          name: e.title,
          singer: e.artist,
          album: e.album,
          albumMid,
          mediaMid: e.mediaMid ?? "",
          durationMs: Math.round(e.duration * 1000),
          vip: e.vip ?? false,
        };
        queue.push({ kind: "qq", id: e.onlineId });
      }
    }
    let target = Math.max(0, Math.min(idx, queue.length - 1));
    // 双击已知失败的曲目：从点击处向后找第一个可播项（全是坏项则停在原地提示）
    const avail = queue.findIndex(
      (q, i) => i >= target && get().unavailable[`${q.kind}:${q.id}`] == null
    );
    if (avail >= 0) target = avail;
    set((s) => ({
      neteaseCache,
      qqCache,
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
    }));
    if (queue.length) get().playQueueIndex(target);
  },

  entryToQueueItem(e) {
    if (e.kind === "local" && e.trackId != null) {
      return { kind: "track", id: e.trackId };
    }
    if (e.kind === "netease" && e.onlineId) {
      const idNum = Number(e.onlineId);
      if (!Number.isFinite(idNum) || idNum <= 0) return null;
      const neteaseCache = { ...get().neteaseCache };
      neteaseCache[idNum] = {
        id: idNum,
        name: e.title,
        ar: [{ name: e.artist }],
        al: { name: e.album, picUrl: e.cover },
        dt: Math.round(e.duration * 1000),
        fee: e.vip ? 1 : 0,
      };
      set({ neteaseCache });
      return { kind: "netease", id: idNum };
    }
    if (e.kind === "qq" && e.onlineId) {
      const albumMid = e.cover.match(/M000([0-9A-Za-z]+)\.jpg?/)?.[1] ?? "";
      const qqCache = { ...get().qqCache };
      qqCache[e.onlineId] = {
        id: e.onlineId,
        name: e.title,
        singer: e.artist,
        album: e.album,
        albumMid,
        mediaMid: e.mediaMid ?? "",
        durationMs: Math.round(e.duration * 1000),
        vip: e.vip ?? false,
      };
      set({ qqCache });
      return { kind: "qq", id: e.onlineId };
    }
    return null;
  },

  playQueueIndex(i) {
    const { queue } = get();
    const item = queue[i];
    if (!item) return;
    // 维护“正在播放的自定义音源 id”（供 SourcesView 高亮；非 url 播放时清除）
    if (item.kind === "url") set({ playingSourceId: item.id });
    else if (get().playingSourceId != null) set({ playingSourceId: null });

    /** 播放失败统一处理：提示 + 按需标记不可用（列表置灰）+ 自动跳下一首。
     *  只有真永久失败（无版权/下架/信息失效）才置灰；
     *  VIP/权益不足随登录与会员状态可恢复，网络错误是瞬时的——都不标记，
     *  下次仍会尝试。登录过期则整个队列都会失败：立即停止并提示重新登录，
     *  不再连跳刷屏。 */
    const fail = (msg: string) => {
      const needRelogin = /登录已过期|请重新登录|未登录/.test(msg);
      const permanent = !needRelogin && /无版权|下架|已失效|信息失效/.test(msg);
      const key = `${item.kind}:${item.id}`;
      set((s) => ({
        unavailable: permanent
          ? { ...s.unavailable, [key]: msg }
          : s.unavailable,
        failStreak: s.failStreak + 1,
      }));
      get().toast(
        `跳过「${titleOfQueueItem(item, get())}」：${msg}`,
        "error"
      );
      // 登录过期：整个队列都会失败，停止继续尝试即可。
      // 注意：失败的只是"切歌尝试"，引擎里可能仍在放换队列前的歌
      // （如在线曲目失败回落的场景），绝不能动 playing——按钮和进度
      // 一律以引擎的 player://nowplaying 事件为准。
      if (needRelogin) return;
      if (get().failStreak < queue.length) {
        get().next(true);
      }
    };
    // 开播成功则清零连跳计数
    const ok = () => set({ failStreak: 0 });

    if (item.kind === "track") {
      api.playTrack(item.id).then(ok).catch((e) => fail(String(e)));
    } else if (item.kind === "netease") {
      const t = get().neteaseCache[item.id];
      if (!t) {
        fail("曲目信息已失效");
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
        .then(ok)
        .catch((e) => fail(String(e)));
    } else if (item.kind === "qq") {
      const t = get().qqCache[item.id];
      if (!t) {
        fail("曲目信息已失效");
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
          vip: t.vip ?? false,
        })
        .then(ok)
        .catch((e) => fail(String(e)));
    } else {
      api.playSource(item.id).then(ok).catch((e) => fail(String(e)));
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
      // 单曲循环：重新播放当前曲目（qIndex 可能为 -1——当前项刚被删除，取 0）
      get().playQueueIndex(Math.max(0, qIndex));
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
    // 跳过已知失败的在线曲目（无版权/下架），最多检查一整圈防止死循环
    let guard = queue.length;
    while (guard-- > 0) {
      if (idx >= queue.length) {
        if (repeat === "all") idx = 0;
        else break;
      }
      const q = queue[idx];
      if (get().unavailable[`${q.kind}:${q.id}`] == null) break;
      idx++;
    }
    if (idx >= queue.length) {
      if (repeat === "all" && get().failStreak === 0) {
        idx = 0;
      } else {
        // 引擎可能仍在放换队列前的歌（在线播放失败场景），此时不能
        // 把 playing/pos 一把清掉；引擎空闲时该分支本就是幂等复位
        if (!get().playing) set({ playing: false, pos: 0 });
        return;
      }
    }
    set((s) => ({ qIndex: idx, history: [...s.history.slice(-50), s.qIndex] }));
    get().playQueueIndex(idx);
  },

  prev() {
    const { queue, qIndex } = get();
    if (!queue.length) return;
    // 直接切到上一曲（到列表头则回绕到最后一首），跳过已知失败项
    let idx = qIndex > 0 ? qIndex - 1 : queue.length - 1;
    let guard = queue.length;
    while (guard-- > 0) {
      const q = queue[idx];
      if (get().unavailable[`${q.kind}:${q.id}`] == null) break;
      idx = idx > 0 ? idx - 1 : queue.length - 1;
    }
    if (get().unavailable[`${queue[idx].kind}:${queue[idx].id}`] != null) return;
    set((s) => ({ qIndex: idx, history: [...s.history.slice(-50), s.qIndex] }));
    get().playQueueIndex(idx);
  },

  setScrubbing(v) {
    set({ scrubbing: v });
  },

  seek(ms) {
    // scrubbing 保持锁定直到后端 seek 完成（FLAC 重建耗时数百 ms）
    set({ pos: ms });
    api
      .seek(Math.round(ms))
      .catch((e) => get().toast(`跳转失败：${e}`, "error"))
      .finally(() => set({ scrubbing: false }));
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

  // ---------- 桌面歌词 ----------

  async openDesktopLyrics() {
    try {
      await api.desktopLyricsOpen();
      set({ desktopLyricsOn: true, desktopLyricsLock: false });
      // 窗口是全新的（没有历史帧可沿用）：强制下一帧携带完整歌词
      lastPushedLines = undefined;
      // 立即推一帧当前状态（窗口加载完成可能晚于这次推送，靠后续 pos 事件补）
      pushDesktopLyrics(get());
      get().toast("桌面歌词已开启（L 键切换）", "info");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async closeDesktopLyrics() {
    try {
      await api.desktopLyricsClose();
      set({ desktopLyricsOn: false, desktopLyricsLock: false });
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async unlockDesktopLyrics() {
    try {
      await api.desktopLyricsUnlock();
      set({ desktopLyricsLock: false });
    } catch {
      // 窗口可能已关闭：静默
    }
  },

  setDlyricsColors(c) {
    set({ dlyricsColors: c });
    saveDesktopLyricsColors(c);
    // 即时生效：推一帧新颜色给悬浮窗（窗口没开时 push 内部自跳过）
    pushDesktopLyrics(get());
  },

  setLyricsColors(c) {
    set({ lyricsColors: c });
    saveLyricsColors(c);
    // 播放页歌词通过 CSS 变量取色，写入后即时生效（rAF/染色层都引用变量）
    applyLyricsColors(c);
  },

  // ---------- 喜欢 / 队列 ----------

  toggleLike(id) {
    const t = get().tracks.find((x) => x.id === id);
    if (!t) return;
    const liked = !t.liked;
    const apply = (v: boolean) =>
      set((s) => ({
        tracks: s.tracks.map((x) => (x.id === id ? { ...x, liked: v } : x)),
        current:
          s.current && s.current.id === id ? { ...s.current, liked: v } : s.current,
      }));
    apply(liked);
    api.likeTrack(id, liked).catch(() => {
      // 失败回滚，避免 UI 与数据库状态不一致（重启后状态跳回）
      apply(!liked);
      get().toast("喜欢状态同步失败", "error");
    });
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
      // 删除当前播放项（i === qIndex）时 qIndex 回退一位（可为 -1），
      // 使播完后的 next() 恰好落在原下一首上，避免跳歌；UI 无高亮项符合语义
      let qIndex = s.qIndex;
      if (i <= s.qIndex) qIndex = Math.max(-1, qIndex - 1);
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
      const id = await api.createPlaylist(name);
      await get().refreshPlaylists();
      get().toast(`已创建播放列表「${name}」`, "success");
      return id;
    } catch (e) {
      get().toast(String(e), "error");
      return -1;
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
    await get().refreshCacheBytes();
  },

  setCacheLimit(bytes) {
    set({ cacheLimit: bytes });
    api
      .setCacheLimit(bytes)
      .then(() => get().refreshCacheBytes())
      .catch((e) => get().toast(String(e), "error"));
  },

  async refreshCacheBytes() {
    try {
      const s = await api.cacheStats();
      set({ cacheBytes: s.bytes });
    } catch {
      // 非致命：设置页进入时会再查一次
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
      if (append) set({ loadMoreLock: true });
      const page = append ? get().qqPage + 1 : 1;
      const r = await api.qqSearch(keyword, page);
      const cache = { ...get().qqCache };
      for (const t of r.songs) cache[t.id] = t;
      set((s) => ({
        qqResults: append ? [...s.qqResults, ...r.songs] : r.songs,
        qqSearching: false,
        qqPage: page,
        qqCache: cache,
      }));
    } catch (e) {
      set({ qqSearching: false });
      get().toast(String(e), "error");
    } finally {
      set({ loadMoreLock: false });
    }
  },

  async qqRefreshStatus() {
    try {
      const s = await api.qqStatus();
      set({ qqLoggedIn: s.loggedIn, qqNickname: s.nickname });
    } catch {}
  },

  qqSetLogin(loggedIn, nickname) {
    set((s) => {
      // 登录/退出都会改变曲目可用性：清除之前会话状态下做出的置灰标记，
      // 让重新登录后的 VIP 曲目得以重试（修复"过期误标后永远不能播"）
      const unavailable = Object.fromEntries(
        Object.entries(s.unavailable).filter(([k]) => !k.startsWith("qq:"))
      );
      return { qqLoggedIn: loggedIn, qqNickname: nickname, unavailable };
    });
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

  setCloseAction(a) {
    set({ closeAction: a });
    api.setCloseAction(a).catch((e) => get().toast(String(e), "error"));
  },

  setAutoUpdate(enabled) {
    set({ autoUpdate: enabled });
    api.setAutoUpdate(enabled).catch((e) => get().toast(String(e), "error"));
  },

  setTheme(t) {
    set({ theme: t });
    saveTheme(t);
    applyTheme(t);
    applyAccent(get().accent); // 强调色需按模式重算
  },

  setAccent(key) {
    set({ accent: key });
    saveAccent(key);
    applyAccent(key);
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

  async refreshRecentOnline() {
    try {
      set({ recentOnline: await api.recentOnlineList() });
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

  // ---------- 手动排序（拖拽调序） ----------

  rowKeyOf(e) {
    if (e.kind === "local") return `track:${e.trackId ?? 0}`;
    return `${e.kind}:${e.onlineId ?? ""}`;
  },

  async loadManualOrder(list) {
    try {
      const map = await api.getManualOrder(list);
      set((s) => ({ manualOrder: { ...s.manualOrder, [list]: map } }));
    } catch {
      // 静默：无排序记录时按默认排序展示
    }
  },

  async saveManualOrder(list, keys) {
    // 乐观更新：立即写入本地缓存，失败时回滚旧值
    const prev = get().manualOrder[list] ?? {};
    try {
      const next: Record<string, number> = {};
      keys.forEach((k, i) => (next[k] = i + 1));
      set((s) => ({ manualOrder: { ...s.manualOrder, [list]: next } }));
      await api.saveManualOrder(list, keys);
    } catch (e) {
      set((s) => ({ manualOrder: { ...s.manualOrder, [list]: prev } }));
      get().toast("顺序保存失败", "error");
    }
  },

  async reorderPlaylist(pid, rowids) {
    try {
      await api.reorderPlaylist(pid, rowids);
      await get().refreshPlaylists();
    } catch (e) {
      get().toast("顺序保存失败", "error");
    }
  },

  async importNeteasePlaylist(remotePid, name) {
    try {
      // 合并语义：同名/同远程 id 的已有列表直接补新歌（去重、不动顺序），
      // 没有才新建——由后端统一判断
      const [, added] = await api.neteaseImportPlaylist(remotePid, name);
      await get().refreshPlaylists();
      get().toast(
        added > 0
          ? `已同步「${name}」：新增 ${added} 首`
          : `「${name}」没有新歌需要同步`,
        "success"
      );
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async importQqPlaylist(remotePid, name) {
    try {
      const [, added] = await api.qqImportPlaylist(remotePid, name);
      await get().refreshPlaylists();
      get().toast(
        added > 0
          ? `已同步「${name}」：新增 ${added} 首`
          : `「${name}」没有新歌需要同步`,
        "success"
      );
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async neteaseSearch(kw, append = false) {
    const keyword = kw.trim();
    if (!keyword) return;
    if (append && get().loadMoreLock) return;
    set({ neteaseSearching: true, neteaseSearched: true });
    try {
      if (append) set({ loadMoreLock: true });
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
    } finally {
      set({ loadMoreLock: false });
    }
  },

  async neteaseRefreshStatus() {
    try {
      const s = await api.neteaseStatus();
      set({ neteaseLoggedIn: s.loggedIn, neteaseNickname: s.nickname });
    } catch {}
  },

  neteaseSetLogin(loggedIn, nickname) {
    set((s) => {
      const unavailable = Object.fromEntries(
        Object.entries(s.unavailable).filter(([k]) => !k.startsWith("netease:"))
      );
      return { neteaseLoggedIn: loggedIn, neteaseNickname: nickname, unavailable };
    });
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
    // 同 key 且已在加载/已加载：跳过（防止 pos 事件高频触发重复请求）；
    // 不同 key 之间的来回切换由 lyricsFor 标识丢弃过期响应
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
        pushDesktopLyrics(get());
      }
    } catch {
      if (get().lyricsFor === key) set({ lyricsLoading: false, lyrics: null });
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
