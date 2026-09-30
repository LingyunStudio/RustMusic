import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  Folder,
  LyricsPayload,
  NeteaseTrack,
  Playlist,
  QqSong,
  UpdateInfo,
  UserPlaylistMeta,
  PlayState,
  PlayStateSnapshot,
  ScanState,
  SettingsPayload,
  SourceItem,
  TrackMeta,
  BiliSpaceItem,
} from "./types";

export const api = {
  listTracks: () => invoke<TrackMeta[]>("list_tracks"),
  listFolders: () => invoke<Folder[]>("list_folders"),
  addFolder: (path: string) => invoke<void>("add_folder", { path }),
  removeFolder: (id: number) => invoke<void>("remove_folder", { id }),
  rescan: () => invoke<void>("rescan"),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  listOutputDevices: () =>
    invoke<{
      devices: { name: string; isDefault: boolean }[];
      current: string;
      preference: string | null;
    }>("list_output_devices"),
  setOutputDevice: (name: string | null) =>
    invoke<void>("set_output_device", { name }),
  dropPaths: (paths: string[]) => invoke<number>("drop_paths", { paths }),
  getLyrics: (trackId: number) => invoke<LyricsPayload>("get_lyrics", { trackId }),
  likeTrack: (id: number, liked: boolean) =>
    invoke<void>("like_track", { id, liked }),
  listPlaylists: () => invoke<Playlist[]>("list_playlists"),
  createPlaylist: (name: string) => invoke<number>("create_playlist", { name }),
  deletePlaylist: (id: number) => invoke<void>("delete_playlist", { id }),
  renamePlaylist: (id: number, name: string) =>
    invoke<void>("rename_playlist", { id, name }),
  addToPlaylist: (playlistId: number, trackId: number) =>
    invoke<void>("add_to_playlist", { playlistId, trackId }),
  removeFromPlaylist: (playlistId: number, trackId: number) =>
    invoke<void>("remove_from_playlist", { playlistId, trackId }),
  listSources: () => invoke<SourceItem[]>("list_sources"),
  addSource: (url: string, title?: string) =>
    invoke<number>("add_source", { url, title: title ?? "" }),
  deleteSource: (id: number) => invoke<void>("delete_source", { id }),
  /** 解析 B 站视频（链接/BV号/av号/短链），多P每个分P各加一条，返回新增条数 */
  bilibiliAdd: (input: string) => invoke<number>("bilibili_add", { input }),
  /** 播放 B 站曲目（我喜欢/播放列表/最近播放入口；rid = "BVxxx-cid"） */
  bilibiliPlay: (track: {
    rid: string;
    title: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
  }) => invoke<void>("bilibili_play", { track }),
  biliQrCreate: () => invoke<{ key: string; qr: string }>("bilibili_qr_create"),
  biliQrCheck: (key: string) =>
    invoke<{ status: string; nickname?: string }>("bilibili_qr_check", { key }),
  biliStatus: () => invoke<{ loggedIn: boolean; nickname: string }>("bilibili_status"),
  biliLogout: () => invoke<void>("bilibili_logout"),
  /** B 站字幕歌词（rid = "BVxxx-cid"；需 B 站登录，无字幕返回空行列表） */
  biliLyric: (rid: string) => invoke<LyricsPayload>("bilibili_lyric", { rid }),
  /** UP 主空间：信息 + 投稿列表第一页 + 合集（order: pubdate|click|stow） */
  biliSpace: (input: string, order: string) =>
    invoke<{
      mid: string;
      name: string;
      face: string;
      fans: string;
      total: number;
      hasMore: boolean;
      items: BiliSpaceItem[];
      collections: { id: number; kind: string; title: string; total: number }[];
    }>("bilibili_space", { input, order }),
  biliSpaceMore: (mid: string, order: string, pn: number) =>
    invoke<{ total: number; hasMore: boolean; items: BiliSpaceItem[] }>(
      "bilibili_space_more",
      { mid, order, pn }
    ),
  biliSpaceCollection: (mid: string, id: number, kind: string) =>
    invoke<{ total: number; hasMore: boolean; items: BiliSpaceItem[] }>(
      "bilibili_space_collection",
      { mid, id, kind }
    ),
  biliSpaceCollectionMore: (mid: string, id: number, kind: string, pn: number) =>
    invoke<{ total: number; hasMore: boolean; items: BiliSpaceItem[] }>(
      "bilibili_space_collection_more",
      { mid, id, kind, pn }
    ),
  /** 登录用户创建的收藏夹列表（含昵称/头像；kind "fav" 复用合集内容接口） */
  biliFavFolders: () =>
    invoke<{
      folders: { id: number; title: string; total: number }[];
      name: string;
      face: string;
    }>("bilibili_fav_folders"),
  /** 解析单个视频（当前结果展示，不落库），每分P一条 */
  biliVideoInfo: (input: string) =>
    invoke<BiliSpaceItem[]>("bilibili_video_info", { input }),
  neteaseSearch: (keyword: string, offset: number) =>
    invoke<{ total: number; songs: NeteaseTrack[] }>("netease_search", {
      keyword,
      offset,
    }),
  neteasePlay: (track: {
    id: number;
    title: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
  }) => invoke<void>("netease_play", { track }),
  neteaseStatus: () => invoke<{ loggedIn: boolean; nickname: string }>("netease_status"),
  neteaseQrCreate: () => invoke<{ key: string; qr: string }>("netease_qr_create"),
  neteaseQrCheck: (key: string) =>
    invoke<{ status: string; nickname?: string }>("netease_qr_check", { key }),
  neteaseLyric: (id: number) => invoke<LyricsPayload>("netease_lyric", { id }),
  qqSearch: (keyword: string, page: number) =>
    invoke<{ songs: QqSong[] }>("qq_search", { keyword, page }),
  qqPlay: (track: {
    songmid: string;
    title: string;
    artist: string;
    album: string;
    albumMid: string;
    mediaMid: string;
    durationMs: number;
    vip: boolean;
  }) => invoke<void>("qq_play", { track }),
  qqLyric: (songmid: string) => invoke<LyricsPayload>("qq_lyric", { songmid }),
  kugouSearch: (keyword: string, page: number) =>
    invoke<{ songs: import("./types").KgSong[] }>("kugou_search", { keyword, page }),
  kugouPlay: (track: {
    hash: string;
    title: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
    vip: boolean;
    albumAudioId?: number;
    albumId?: number;
    hqHash?: string;
    sqHash?: string;
    superHash?: string;
  }) => invoke<void>("kugou_play", { track }),
  kugouLyric: (hash: string) => invoke<LyricsPayload>("kugou_lyric", { hash }),
  kugouQrCreate: () => invoke<{ key: string; qr: string }>("kugou_qr_create"),
  kugouQrCheck: (key: string) =>
    invoke<{ status: string; nickname?: string }>("kugou_qr_check", { key }),
  kugouStatus: () => invoke<{ loggedIn: boolean; nickname: string }>("kugou_status"),
  kugouLogout: () => invoke<void>("kugou_logout"),
  kugouToplists: () =>
    invoke<{ toplists: import("./types").KgToplist[] }>("kugou_toplists"),
  kugouToplistTracks: (topId: number) =>
    invoke<{ songs: import("./types").KgSong[] }>("kugou_toplist_tracks", { topId }),
  kugouRandomPlaylist: () =>
    invoke<import("./types").KgPublicPlaylist>("kugou_random_playlist"),
  kugouPlaylistTracks: (id: string) =>
    invoke<import("./types").KgPublicPlaylist>("kugou_playlist_tracks", { id }),
  kugouUserPlaylists: () =>
    invoke<{ id: string; name: string; trackCount: number }[]>(
      "kugou_user_playlists"
    ),
  kugouImportPlaylist: (remotePid: string, name: string) =>
    invoke<[number, number]>("kugou_import_playlist", { remotePid, name }),
  qqQrCreate: () => invoke<{ qrsig: string; qr: string }>("qq_qr_create"),
  qqQrCheck: (qrsig: string) =>
    invoke<{ status: string; nickname?: string }>("qq_qr_check", { qrsig }),
  qqStatus: () => invoke<{ loggedIn: boolean; nickname: string }>("qq_status"),
  qqLogout: () => invoke<void>("qq_logout"),
  neteaseLikeList: () => invoke<number[]>("netease_like_list"),
  neteaseLike: (id: number, like: boolean) =>
    invoke<void>("netease_like", { id, like }),
  neteaseLogout: () => invoke<void>("netease_logout"),
  likeOnline: (req: {
    kind: string;
    rid: string;
    title: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
    mediaMid: string;
    vip: boolean;
    like: boolean;
  }) => invoke<void>("like_online", req),
  downloadOnline: (req: {
    kind: string;
    id: string;
    title: string;
    artist: string;
    album: string;
    coverUrl: string;
    durationMs: number;
    mediaMid: string;
    albumAudioId?: number;
    hqHash?: string;
    sqHash?: string;
    superHash?: string;
  }) => invoke<string>("download_online", { req }),
  likedOnlineList: () =>
    invoke<import("./types").PlaylistEntryMeta[]>("liked_online_list"),
  recentOnlineList: () =>
    invoke<import("./types").PlaylistEntryMeta[]>("recent_online_list"),
  saveDirGet: () =>
    invoke<{ dir: string; default: string }>("save_dir_get"),
  saveDirSet: (dir: string) => invoke<void>("save_dir_set", { dir }),
  addOnlineToPlaylist: (req: {
    playlistId: number;
    kind: string;
    rid: string;
    title: string;
    artist: string;
    album: string;
    cover: string;
    durationMs: number;
    mediaMid: string;
    vip: boolean;
  }) => invoke<void>("add_online_to_playlist", req),
  removePlaylistEntry: (rowid: number) =>
    invoke<void>("remove_playlist_entry", { rowid }),
  saveManualOrder: (list: string, keys: string[]) =>
    invoke<void>("save_manual_order", { list, keys }),
  getManualOrder: (list: string) =>
    invoke<Record<string, number>>("get_manual_order", { list }),
  reorderPlaylist: (playlistId: number, rowids: number[]) =>
    invoke<void>("reorder_playlist", { playlistId, rowids }),
  reorderPlaylists: (ids: number[]) =>
    invoke<void>("reorder_playlists", { ids }),
  neteaseUserPlaylists: () =>
    invoke<UserPlaylistMeta[]>("netease_user_playlists"),
  neteaseImportPlaylist: (remotePid: number, name: string) =>
    invoke<[number, number]>("netease_import_playlist", { remotePid, name }),
  qqUserPlaylists: () =>
    invoke<import("./types").UserPlaylistMeta[]>("qq_user_playlists"),
  qqImportPlaylist: (remotePid: number, name: string) =>
    invoke<[number, number]>("qq_import_playlist", { remotePid, name }),
  qqToplists: () =>
    invoke<{ toplists: import("./types").QqToplist[] }>("qq_toplists"),
  qqToplistTracks: (topId: number) =>
    invoke<{ songs: QqSong[] }>("qq_toplist_tracks", { topId }),
  qqRandomPlaylist: () =>
    invoke<import("./types").QqRandomPlaylist>("qq_random_playlist"),
  neteaseToplists: () =>
    invoke<{ toplists: import("./types").NetToplist[] }>("netease_toplists"),
  neteaseToplistTracks: (topId: number) =>
    invoke<{ songs: NeteaseTrack[] }>("netease_toplist_tracks", { topId }),
  neteaseRandomPlaylist: () =>
    invoke<import("./types").NetRandomPlaylist>("netease_random_playlist"),
  neteaseDailyRecommend: () =>
    invoke<{ songs: NeteaseTrack[] }>("netease_daily_recommend"),
  neteasePersonalFm: () =>
    invoke<{ songs: NeteaseTrack[] }>("netease_personal_fm"),
  setPlayQuality: (quality: string) => invoke<void>("set_play_quality", { quality }),
  setWasapiExclusive: (enabled: boolean) =>
    invoke<void>("set_wasapi_exclusive", { enabled }),
  setCloseAction: (action: string) => invoke<void>("set_close_action", { action }),
  setAutoUpdate: (enabled: boolean) => invoke<void>("set_auto_update", { enabled }),
  // ---------- 自动更新（GitHub Release） ----------
  /** 前端就绪后触发一次启动自动检查（结果通过 update://available 事件推送） */
  autoCheckUpdate: () => invoke<void>("auto_check_update"),
  /** 手动检查：有更新返回信息，已是最新返回 null */
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  /** 下载安装包到临时目录，进度走 update://progress 事件，返回文件路径 */
  downloadUpdate: (req: { url: string; name: string; size: number; digest: string | null }) =>
    invoke<string>("download_update", req),
  cancelUpdateDownload: () => invoke<void>("cancel_update_download"),
  /** 安装并重启应用（安装位置不变，安装完成后自动重启） */
  installUpdate: (path: string) => invoke<void>("install_update", { path }),
  /** 用系统浏览器打开链接（release notes 内跳转用） */
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  extractCoverPalette: (url: string) => invoke<string[]>("extract_cover_palette", { url }),
  assetScopeAllow: (path: string) => invoke<void>("asset_scope_allow", { path }),
  /** 皮肤图片的展示用压缩副本（≤1920px；动图/小图原样返回，失败回退原路径） */
  prepareSkinImage: (path: string) => invoke<string>("prepare_skin_image", { path }),
  navidromeSave: (server: string, username: string, password: string) =>
    invoke<void>("navidrome_save", { req: { server, username, password } }),
  navidromeAlbums: (server: string, username: string) =>
    invoke<import("./types").NdAlbum[]>("navidrome_albums", { server, username }),
  navidromeSearch: (server: string, username: string, query: string) =>
    invoke<import("./types").NdSong[]>("navidrome_search", { server, username, query }),
  navidromeAllSongs: (server: string, username: string, offset: number) =>
    invoke<{ songs: import("./types").NdSong[]; total: number }>("navidrome_all_songs", {
      server,
      username,
      offset,
    }),
  navidromeAlbumSongs: (server: string, username: string, id: string) =>
    invoke<{ name: string; artist: string; songs: import("./types").NdSong[] }>(
      "navidrome_album_songs",
      { server, username, id }
    ),
  navidromePlay: (
    server: string,
    username: string,
    track: {
      id: string;
      title: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
    }
  ) => invoke<void>("navidrome_play", { server, username, track }),
  navidromeForget: (server: string, username: string) =>
    invoke<void>("navidrome_forget", { server, username }),
  navidromeLyric: (id: string) =>
    invoke<import("./types").LyricsPayload>("navidrome_lyric", { id }),
  playTrack: (id: number) => invoke<void>("play_track", { id }),
  playSource: (id: number) => invoke<void>("play_source", { id }),
  playPause: () => invoke<void>("play_pause"),
  getPlayState: () => invoke<PlayStateSnapshot | null>("get_play_state"),
  pause: () => invoke<void>("pause"),
  resume: () => invoke<void>("resume"),
  stop: () => invoke<void>("stop"),
  seek: (ms: number) => invoke<void>("seek", { ms }),
  setVolume: (v: number) => invoke<void>("set_volume", { v }),
  setSpeed: (v: number) => invoke<void>("set_speed", { v }),
  setEq: (gains: number[], enabled: boolean) =>
    invoke<void>("set_eq", { gains, enabled }),
  getSettings: () => invoke<SettingsPayload>("get_settings"),
  /** 最近一次扫描进度快照（WebView 挂起恢复后补发用） */
  getScanState: () => invoke<ScanState>("get_scan_state"),
  clearCache: () => invoke<number>("clear_cache"),
  cacheStats: () =>
    invoke<{ bytes: number; files: number }>("cache_stats"),
  setCacheLimit: (bytes: number) =>
    invoke<void>("set_cache_limit", { bytes }),
  getAppInfo: () => invoke<{ version: string; dataDir: string }>("get_app_info"),
  desktopLyricsOpen: () => invoke<void>("desktop_lyrics_open"),
  desktopLyricsClose: () => invoke<void>("desktop_lyrics_close"),
  desktopLyricsUnlock: () => invoke<void>("desktop_lyrics_unlock"),
  desktopLyricsIsOpen: () => invoke<boolean>("desktop_lyrics_is_open"),
};

export { convertFileSrc };

export function coverSrc(path: string): string {
  if (!path) return "";
  // http(s) 封面直接用原 URL；本地文件路径才转 asset 协议
  if (path.startsWith("http://") || path.startsWith("https://")) return path;
  return convertFileSrc(path);
}

export type ListenerUnbind = () => void;

export async function listenEvent<T>(
  event: string,
  handler: (payload: T) => void
): Promise<ListenerUnbind> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => handler(e.payload));
}

export type {
  Folder,
  LyricsPayload,
  Playlist,
  PlayState,
  ScanState,
  SettingsPayload,
  SourceItem,
  TrackMeta,
  UpdateInfo,
};
