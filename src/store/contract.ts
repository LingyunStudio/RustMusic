//! Store 完整契约：字段与方法集中声明，各 slice 文件以 Pick<Store, ...> 认领自己的领域。
import type {
  CurrentTrack,
  NdSong,
  OnlineNavSnapshot,
  OnlineRecState,
  OnlineSource,
  PlaylistEntryMeta,
  BiliTrack,
  BiliFollow,
  DownloadState,
  Folder,
  LyricsPayload,
  NeteaseTrack,
  QqSong,
  KgSong,
  Playlist,
  QueueItem,
  RepeatMode,
  ScanState,
  SourceItem,
  Toast,
  TrackMeta,
  ViewName,
} from "../types";
import type { DesktopLyricsColors } from "../theme";

export interface Store {
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
  /** 歌手/专辑详情页的名称（view = artist/album 时有效） */
  detailName: string;
  /** 详情页回退栈：openDetailPage 压入来源视图，popDetailPage 弹出恢复 */
  viewHistory: { view: ViewName; viewParam: number; detailName: string }[];
  /** 在线曲库推荐视图（随便听听/榜单/每日推荐/私人FM），按源存放：
   *  放 store 里保证跳转歌手/专辑页后返回时还原当时的推荐列表 */
  onlineRec: Record<OnlineSource, OnlineRecState | null>;
  /** 在线曲库视图内导航栈（搜索/推荐切换前快照），跳转详情页往返后仍可逐步返回 */
  onlineNav: Record<OnlineSource, OnlineNavSnapshot[]>;
  /** 各源最近一次搜索词（返回在线曲库时还原搜索框） */
  lastKw: Record<OnlineSource, string>;
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

  /** 酷狗在线曲库（搜索/播放匿名；VIP 曲目需扫码登录） */
  kugouResults: KgSong[];
  kugouSearching: boolean;
  kugouSearched: boolean;
  kugouPage: number;
  kugouCache: Record<string, KgSong>;
  kugouLoggedIn: boolean;
  kugouNickname: string;

  /** B 站曲目元数据缓存（键 = rid "BVxxx-cid"，我喜欢/播放列表/队列条目用） */
  biliCache: Record<string, BiliTrack>;
  /** Navidrome 元数据缓存（键 = 歌曲 id，最近播放/队列恢复用） */
  ndCache: Record<
    string,
    { title: string; artist: string; album: string; cover: string; durationMs: number }
  >;

  /** B 站登录态（扫码；字幕功能依赖登录） */
  biliLoggedIn: boolean;
  biliNickname: string;
  /** 收藏的 UP 主（localStorage 持久化，在线音源页横排展示） */
  biliFollows: BiliFollow[];
  /** 收藏/取消收藏 UP 主 */
  biliToggleFollow(up: BiliFollow): void;
  /** 拖拽排序收藏的 UP 主（from/to 为数组下标；松手落位并持久化） */
  biliReorderFollows(from: number, to: number): void;

  /** 在线音源页“当前结果”（切页保留，换解析目标才替换） */
  sourcesResult: import("../types").SourcesResult | null;
  setSourcesResult(
    r:
      | import("../types").SourcesResult
      | null
      | ((cur: import("../types").SourcesResult | null) => import("../types").SourcesResult | null)
  ): void;
  /** 在线音源页当前音源页签（切页保留） */
  sourcesTab: "bilibili" | "navidrome" | "other";
  setSourcesTab(t: "bilibili" | "navidrome" | "other"): void;

  quality: string;
  /** 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用 */
  closeAction: "tray" | "exit";
  /** 启动时自动检查 GitHub 更新（默认开启） */
  autoUpdate: boolean;
  /** WASAPI 独占模式（默认关闭；切换后下一首生效） */
  wasapiExclusive: boolean;
  /** 音源缓存上限（字节），0 = 不限制 */
  cacheLimit: number;
  /** 当前缓存占用（字节），null = 尚未查询 */
  cacheBytes: number | null;
  /** 播放失败的在线曲目（键 kind:id，值失败原因）：列表置灰 + 自动跳过 */
  unavailable: Record<string, string>;
  /** 连续播放失败计数（成功开播清零；达队列长度停止自动跳过） */
  failStreak: number;
  /** 当前失败提示的 toast id：连跳期间只更新这一条，不堆叠刷屏 */
  failToastId: number | null;
  /** 当前播放的自定义在线音源 id（SourcesView 高亮用；path 是缓存文件名，无法从 URL 判断） */
  playingSourceId: number | null;
  scrubbing: boolean;
  theme: "dark" | "light";
  accent: string;
  savedOnline: Record<string, boolean>;
  likedOnline: import("../types").PlaylistEntryMeta[];
  /** “最近播放”的在线曲目部分（含 lastPlayed 用于合并排序） */
  recentOnline: import("../types").PlaylistEntryMeta[];
  loadMoreLock: boolean;
  neteaseLiked: Record<number, boolean>;

  init(): Promise<void>;
  toast(msg: string, type?: Toast["type"]): number;
  dismissToast(id: number): void;
  setView(v: ViewName, param?: number): void;
  setOnlineRec(source: OnlineSource, rec: OnlineRecState | null): void;
  pushOnlineNav(source: OnlineSource, snap: OnlineNavSnapshot): void;
  /** 弹出栈顶快照；栈空返回 null */
  popOnlineNav(source: OnlineSource): OnlineNavSnapshot | null;
  clearOnlineNav(source: OnlineSource): void;
  setLastKw(source: OnlineSource, kw: string): void;
  /** 打开歌手/专辑详情页（聚合本地曲目与在线搜索结果），自带返回栈 */
  openDetailPage(kind: "artist" | "album", name: string): void;
  popDetailPage(): void;
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
  /** Navidrome 全部歌曲列表播放（整表入队，支持自动续页） */
  playNdList(songs: NdSong[], idx: number): void;
  playQq(list: QqSong[], idx: number): void;
  playKugou(list: KgSong[], idx: number): void;
  playEntries(entries: PlaylistEntryMeta[], idx: number): void;
  playQueueIndex(i: number): void;
  /** 队列自动续页：列表视图注册"加载下一页并返回新增队列项"的回调；
      播到队列最后一条时 store 自动触发并追加（无更多/上下文失效返回 null） */
  setQueueExtender(fn: (() => Promise<QueueItem[] | null>) | null): void;
  /** 在线条目（网易云/QQ/酷狗）转可播放的队列项；无元数据时返回 null */
  entryToQueueItem(e: PlaylistEntryMeta): QueueItem | null;
  togglePlay(): void;
  next(auto?: boolean, ended?: boolean, bypassRepeatOne?: boolean): void;
  prev(): void;
  seek(ms: number): void;
  setScrubbing(v: boolean): void;
  setVolume(v: number): void;
  /** 定时停止播放：min=null 取消；到点停止引擎并复位 UI */
  sleepAt: number | null;
  setSleepTimer(min: number | null): void;
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

  toggleLike(id: number): void;
  addToQueue(item: QueueItem): void;
  playNext(item: QueueItem): void;
  removeQueueItem(i: number): void;
  clearQueue(): void;
  jumpTo(i: number): void;
  queueExtender: (() => Promise<QueueItem[] | null>) | null;

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
  /** 解析 B 站视频加入在线音源（多P全部加入） */
  addBilibili(input: string): Promise<void>;
  biliSetLogin(loggedIn: boolean, nickname: string): void;
  biliLogout(): Promise<void>;
  /** 播放一条 B 站曲目（空间/单视频结果区）：rid 可为纯 bvid */
  playBilibili(row: {
    rid: string;
    title: string;
    artist: string;
    cover: string;
    durationMs: number;
  }): void;
  /** 整个结果列表进队列、从第 idx 首开始播（播完自动下一首） */
  playBilibiliList(
    rows: {
      rid: string;
      title: string;
      artist: string;
      cover: string;
      durationMs: number;
    }[],
    idx: number
  ): void;
  /** 按 id 播放直链音源（结果区展示用） */
  playSourceId(id: number): void;
  /** UP 主空间：信息 + 投稿第一页 + 合集列表 */
  biliSpace(
    input: string,
    order: string
  ): Promise<{
    mid: string;
    name: string;
    face: string;
    fans: string;
    total: number;
    hasMore: boolean;
    items: import("../types").BiliSpaceItem[];
    collections: { id: number; kind: string; title: string; total: number }[];
  }>;
  biliSpaceMore(
    mid: string,
    order: string,
    pn: number
  ): Promise<{ total: number; hasMore: boolean; items: import("../types").BiliSpaceItem[] }>;
  biliSpaceCollection(
    mid: string,
    id: number,
    kind: string
  ): Promise<{ total: number; hasMore: boolean; items: import("../types").BiliSpaceItem[] }>;
  biliSpaceCollectionMore(
    mid: string,
    id: number,
    kind: string,
    pn: number
  ): Promise<{ total: number; hasMore: boolean; items: import("../types").BiliSpaceItem[] }>;
  biliFavFolders(): Promise<{
    folders: { id: number; title: string; total: number }[];
    name: string;
    face: string;
  }>;
  biliVideoInfo(input: string): Promise<import("../types").BiliSpaceItem[]>;
  setEq(gains: number[], enabled: boolean): void;
  clearCache(): Promise<void>;

  neteaseSearch(kw: string, append?: boolean): Promise<void>;
  neteaseRefreshStatus(): Promise<void>;
  neteaseSetLogin(loggedIn: boolean, nickname: string): void;
  neteaseSyncLikes(): Promise<void>;
  neteaseToggleLike(id: number): void;
  neteaseLogout(): Promise<void>;

  qqSearch(kw: string, append?: boolean): Promise<void>;
  kugouSearch(kw: string, append?: boolean): Promise<void>;
  kugouRefreshStatus(): Promise<void>;
  kugouSetLogin(loggedIn: boolean, nickname: string): void;
  kugouLogout(): Promise<void>;
  /** 收藏在线发现的酷狗歌单为本地播放列表（global_collection_id） */
  importKugouPlaylist(remotePid: string, name: string, opts?: { quiet?: boolean }): Promise<number | null>;
  qqRefreshStatus(): Promise<void>;
  qqSetLogin(loggedIn: boolean, nickname: string): void;
  qqLogout(): Promise<void>;
  playQq(list: QqSong[], idx: number): void;

  setQuality(q: string): void;
  setCloseAction(a: "tray" | "exit"): void;
  setAutoUpdate(enabled: boolean): void;
  setWasapiExclusive(enabled: boolean): void;
  setCacheLimit(bytes: number): void;
  refreshCacheBytes(): Promise<void>;
  setTheme(t: "dark" | "light"): void;
  setAccent(key: string): void;
  /** 当前皮肤 key（"default" = 内置氛围背景） */
  skin: string;
  setSkin(key: string): void;
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
  }): Promise<boolean>;
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
  importNeteasePlaylist(remotePid: number, name: string, opts?: { quiet?: boolean }): Promise<number | null>;
  importQqPlaylist(remotePid: number, name: string, opts?: { quiet?: boolean }): Promise<number | null>;
  /** 三个平台共用的导入实现：返回本次新增数，失败返回 null。
   *  酷狗用 global_collection_id（字符串）作为远程歌单 ID */
  importOnline(
    source: "netease" | "qq" | "kugou",
    remotePid: number | string,
    name: string,
    opts?: { quiet?: boolean }
  ): Promise<number | null>;
  /** 依次导入账号下全部歌单（quiet 逐个导入，结束时统一刷新 + 汇总提示） */
  importAllPlaylists(
    source: "netease" | "qq" | "kugou",
    list: { id: number | string; name: string; trackCount: number }[],
    onProgress?: (p: { done: number; total: number; name: string }) => void
  ): Promise<{ added: number; failed: number }>;

  /** 行 key（unavailable 同款：track:<id> / netease:<rid> / qq:<rid>） */
  rowKeyOf(e: { kind: string; trackId?: number | null; onlineId?: string | null }): string;
  /** “资料库/我喜欢”手动排序（整份顺序全量覆盖，乐观更新本地 state） */
  saveManualOrder(list: "library" | "liked", keys: string[]): Promise<void>;
  /** 手动序号缓存（list → row_key → pos；排序时用，0 = 无记录排最后） */
  manualOrder: Record<string, Record<string, number>>;
  loadManualOrder(list: "library" | "liked"): Promise<void>;
  /** 播放列表条目手动排序（按 rowid 序列重写 position） */
  reorderPlaylist(pid: number, rowids: number[]): Promise<void>;
  /** 侧边栏播放列表手动排序（按 id 序列重写 sort_pos，乐观更新本地 state） */
  reorderPlaylists(ids: number[]): Promise<void>;

  loadLyricsByKey(key: string): Promise<void>;
  loadLyrics(trackId: number): Promise<void>;
  applyMediaControl(action: string, value?: number): void;
}
