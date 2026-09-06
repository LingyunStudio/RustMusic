import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  Folder,
  LyricsPayload,
  NeteaseTrack,
  Playlist,
  QqSong,
  UserPlaylistMeta,
  PlayState,
  ScanState,
  SettingsPayload,
  SourceItem,
  TrackMeta,
} from "./types";

export const api = {
  listTracks: () => invoke<TrackMeta[]>("list_tracks"),
  listFolders: () => invoke<Folder[]>("list_folders"),
  addFolder: (path: string) => invoke<void>("add_folder", { path }),
  removeFolder: (id: number) => invoke<void>("remove_folder", { id }),
  rescan: () => invoke<void>("rescan"),
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
  }) => invoke<void>("qq_play", { track }),
  qqLyric: (songmid: string) => invoke<LyricsPayload>("qq_lyric", { songmid }),
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
  }) => invoke<string>("download_online", { req }),
  likedOnlineList: () =>
    invoke<import("./types").PlaylistEntryMeta[]>("liked_online_list"),
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
  neteaseUserPlaylists: () =>
    invoke<UserPlaylistMeta[]>("netease_user_playlists"),
  neteaseImportPlaylist: (remotePid: number, localPid: number) =>
    invoke<number>("netease_import_playlist", { remotePid, localPid }),
  qqUserPlaylists: () =>
    invoke<import("./types").UserPlaylistMeta[]>("qq_user_playlists"),
  qqImportPlaylist: (remotePid: number, localPid: number) =>
    invoke<number>("qq_import_playlist", { remotePid, localPid }),
  setPlayQuality: (quality: string) => invoke<void>("set_play_quality", { quality }),
  playTrack: (id: number) => invoke<void>("play_track", { id }),
  playSource: (id: number) => invoke<void>("play_source", { id }),
  playPause: () => invoke<void>("play_pause"),
  pause: () => invoke<void>("pause"),
  resume: () => invoke<void>("resume"),
  stop: () => invoke<void>("stop"),
  seek: (ms: number) => invoke<void>("seek", { ms }),
  setVolume: (v: number) => invoke<void>("set_volume", { v }),
  setSpeed: (v: number) => invoke<void>("set_speed", { v }),
  setEq: (gains: number[], enabled: boolean) =>
    invoke<void>("set_eq", { gains, enabled }),
  getSettings: () => invoke<SettingsPayload>("get_settings"),
  clearCache: () => invoke<number>("clear_cache"),
  getAppInfo: () => invoke<{ version: string; dataDir: string }>("get_app_info"),
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
};
