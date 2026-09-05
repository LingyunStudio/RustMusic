import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  Folder,
  LyricsPayload,
  Playlist,
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
  return path ? convertFileSrc(path) : "";
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
