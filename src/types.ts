export interface TrackMeta {
  id: number;
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  trackNo: number;
  disc: number;
  year: number;
  duration: number;
  format: string;
  bitrate: number;
  sampleRate: number;
  bitDepth: number;
  cover: string;
  hasLrc: boolean;
  size: number;
  mtime: number;
  liked: boolean;
  playCount: number;
  lastPlayed: number;
  /** 首次喜欢的时间（unix 秒，0 = 未喜欢过） */
  likedAt: number;
  /** 文件已不在任何监控目录（软删除：资料库隐藏，喜欢/最近播放保留记录） */
  missing: boolean;
}

export interface Folder {
  id: number;
  path: string;
}

export interface PlaylistEntryMeta {
  rowid: number;
  kind: "local" | "netease" | "qq" | "kugou" | "bilibili" | "navidrome";
  trackId: number | null;
  onlineId: string | null;
  title: string;
  artist: string;
  album: string;
  cover: string;
  duration: number;
  /** QQ 在线条目的媒体 mid（播放/下载取链接需要） */
  mediaMid?: string;
  /** 在线条目是否 VIP */
  vip?: boolean;
  /** 最近播放时间（unix 秒，0 = 无记录） */
  lastPlayed?: number;
  /** 收藏时间（unix 秒，0 = 无记录） */
  likedAt?: number;
}

export interface Playlist {
  id: number;
  name: string;
  trackIds: number[];
  entries: PlaylistEntryMeta[];
  cover: string;
  createdAt: number;
  /** 来源远程歌单标识（"netease"/"qq" + 远程歌单 id；空 = 普通本地列表）。
   *  重复导入时按它合并进已有列表 */
  remoteKind?: string;
  remotePid?: string;
  /** 原始导入名：改名后仍能认出来源（重导入合并 + 界面提示用） */
  originName?: string;
}

export interface SourceItem {
  id: number;
  url: string;
  title: string;
  createdAt: number;
}

export interface LyricWord {
  startMs: number;
  endMs: number;
  text: string;
}

export interface LyricLine {
  timeMs: number | null;
  text: string;
  /** 逐字时间戳（yrc/QRC/增强 LRC）；缺省时按文字长度加权推进 */
  words?: LyricWord[];
  /** 行级翻译（网易 tlyric），随当前行小字展示 */
  trans?: string;
}

export interface LyricsPayload {
  synced: boolean;
  lines: LyricLine[];
  /** 歌词来源标记（诊断用："逐字 KRC"/"逐字 YRC"/"行级 LRC"/"字幕 B站"…） */
  source?: string;
}

export interface TrackInfo {
  id: number | null;
  kind: "track" | "url" | "netease" | "qq" | "kugou" | "bilibili" | "navidrome";
  path: string;
  title: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
  nid?: number | null;
  qid?: string | null;
  kgid?: string | null;
  quality?: string | null;
}

export interface QqSong {
  id: string;
  name: string;
  singer: string;
  album: string;
  albumMid: string;
  mediaMid: string;
  durationMs: number;
  vip: boolean;
}

export interface KgSong {
  /** 歌曲 hash（酷狗唯一曲目标识，128k 档） */
  id: string;
  name: string;
  singer: string;
  album: string;
  durationMs: number;
  cover: string;
  vip: boolean;
  /** 专辑音频 ID / 专辑 ID：登录后按音质取链接用 */
  albumAudioId?: number;
  albumId?: number;
  /** 高音质文件 hash（搜索/榜单通道附带） */
  hqHash?: string;
  sqHash?: string;
  superHash?: string;
}

/** 酷狗官方榜单 */
export interface KgToplist {
  id: number;
  name: string;
  pic: string;
}

/** 酷狗公开歌单（global_collection_id） */
export interface KgPublicPlaylist {
  id: string;
  name: string;
  cover: string;
  playCount: number;
  creator: string;
  songs: KgSong[];
}

/** Navidrome（Subsonic）歌曲条目 */
export interface NdSong {
  id: string;
  title: string;
  artist: string;
  album: string;
  /** 秒 */
  duration: number;
  coverUrl: string;
}

/** Navidrome 专辑条目 */
export interface NdAlbum {
  id: string;
  name: string;
  artist: string;
  coverUrl: string;
  songCount: number;
  duration: number;
}

/** B 站视频音频条目（rid = "BVxxx-cid"，与后端 qid/缓存键一致） */
export interface BiliTrack {
  rid: string;
  title: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
}

/** 空间/单视频解析结果条目（rid 可为纯 bvid，播放时按需解析 cid） */
export interface BiliSpaceItem {
  rid: string;
  title: string;
  artist: string;
  cover: string;
  durationMs: number;
  play: number;
  created: number;
}

export interface BiliCollection {
  id: number;
  kind: string;
  title: string;
  total: number;
}

/** 收藏的 UP 主（localStorage 持久化，在线音源页横排展示） */
export interface BiliFollow {
  mid: string;
  name: string;
  face: string;
}

/** 在线音源页“当前结果”（存 store：切页返回不丢） */
export type SourcesResult =
  | { type: "video"; rows: BiliSpaceItem[] }
  | {
      type: "space";
      mid: string;
      name: string;
      face: string;
      fans: string;
      total: number;
      order: "pubdate" | "click" | "stow";
      rows: BiliSpaceItem[];
      pn: number;
      hasMore: boolean;
      loadingMore: boolean;
      collections: BiliCollection[];
      activeCollection: (BiliCollection & {
        rows: BiliSpaceItem[];
        pn: number;
        hasMore: boolean;
        loadingMore: boolean;
      }) | null;
    }
  | { type: "direct"; rows: (BiliSpaceItem & { sourceId: number })[] };

export interface NeteaseTrack {
  id: number;
  name: string;
  ar: { name: string }[];
  al: { name: string; picUrl?: string | null };
  dt: number;
  fee: number;
}

/** QQ 官方榜单（musicToplist.Toplist GetAll） */
export interface QqToplist {
  id: number;
  title: string;
  pic: string;
  updateTime: string;
}

/** 网易云官方榜单（榜单 ID 即歌单 ID） */
export interface NetToplist {
  id: number;
  name: string;
  cover: string;
  updateFrequency: string;
  trackCount: number;
}

/** QQ 随机公开歌单（歌单广场随机抽取） */
export interface QqRandomPlaylist {
  id: number;
  name: string;
  cover: string;
  listenNum: number;
  creator: string;
  songs: QqSong[];
}

/** 网易云随机推荐歌单（个性化推荐池随机抽取，榜单兜底） */
export interface NetRandomPlaylist {
  id: number;
  name: string;
  cover: string;
  playCount: number;
  creator: string;
  songs: NeteaseTrack[];
}

export interface PlayState extends TrackInfo {
  playing: boolean;
  /** 后端“开播代次”（每次换曲 +1），用于区分开播与暂停/恢复 */
  seq?: number;
}

/** 播放状态快照（含进度）：WebView 挂起恢复后前端主动拉取用 */
export interface PlayStateSnapshot extends PlayState {
  pos: number;
}

export interface CurrentTrack extends TrackInfo {
  liked: boolean;
  quality?: string | null;
}

export type RepeatMode = "off" | "all" | "one";

export type QueueItemKind = "track" | "url" | "netease" | "qq" | "kugou" | "bilibili" | "navidrome";

export type QueueItem =
  | { kind: "track" | "netease" | "url"; id: number }
  | { kind: "qq" | "kugou"; id: string }
  | { kind: "bilibili"; id: string }
  | { kind: "navidrome"; id: string };

export type ViewName =
  | "library"
  | "liked"
  | "recent"
  | "sources"
  | "netease"
  | "qq"
  | "kugou"
  | "settings"
  | "playlist"
  | "artist"
  | "album";

/** 在线曲库"推荐视图"状态（随便听听/榜单/每日推荐/私人FM），按源存放：
 *  存在 store 里保证跳转歌手/专辑页再返回时能还原当时的推荐列表 */
export interface OnlineRecState {
  /** 来源：换一批按钮按来源刷新 */
  origin: "random" | "top" | "daily" | "fm";
  title: string;
  cover: string;
  subtitle: string;
  /** 榜单分页：榜单 ID + 已加载页码 + 是否还有更多（列表底部"加载更多"） */
  topId?: number;
  recPage?: number;
  recHasMore?: boolean;
  /** 可整单收藏时：远程歌单 ID（netease 榜单/个性化歌单、QQ 公开歌单、
   *  酷狗 global_collection_id 字符串） */
  playlistId?: number | string;
  netease?: NeteaseTrack[];
  qq?: QqSong[];
  kugou?: KgSong[];
}

export type OnlineSource = "netease" | "qq" | "kugou";

/** 在线曲库视图内导航快照（搜索/推荐切换前的完整状态），按源存 store，
 *  跳转歌手/专辑页往返后依然可以逐步返回 */
export interface OnlineNavSnapshot {
  rec: OnlineRecState | null;
  kw: string;
  neteaseResults: NeteaseTrack[];
  neteaseTotal: number;
  neteaseSearched: boolean;
  qqResults: QqSong[];
  qqSearched: boolean;
  qqPage: number;
  kugouResults: KgSong[];
  kugouSearched: boolean;
  kugouPage: number;
}

export interface ScanState {
  active: boolean;
  done: number;
  total: number;
}

export interface DownloadState {
  title: string;
  pct: number;
}

export interface SettingsPayload {
  volume: number;
  speed: number;
  eqGains: number[];
  eqEnabled: boolean;
  quality: string;
  cacheLimit: number;
  /** 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用 */
  closeAction: "tray" | "exit";
  /** 启动时自动检查 GitHub 更新（默认开启） */
  autoUpdate: boolean;
  /** WASAPI 独占模式（默认关闭；切换后下一首生效） */
  wasapiExclusive: boolean;
}

/** GitHub 最新 release 的可安装更新信息 */
export interface UpdateInfo {
  /** 最新版本号（不含 v 前缀） */
  version: string;
  /** release notes（markdown 原文） */
  notes: string;
  assetName: string;
  assetUrl: string;
  assetSize: number;
  /** 附件 SHA-256（GitHub 返回 "sha256:<hex>"；缺省 = 无法校验） */
  assetDigest: string | null;
  publishedAt: string;
}

export interface UserPlaylistMeta {
  id: number;
  name: string;
  trackCount: number;
}

export interface Toast {
  id: number;
  msg: string;
  type: "info" | "error" | "success";
}
