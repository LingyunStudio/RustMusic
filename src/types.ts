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
}

export interface Folder {
  id: number;
  path: string;
}

export interface PlaylistEntryMeta {
  rowid: number;
  kind: "local" | "netease" | "qq";
  trackId: number | null;
  onlineId: string | null;
  title: string;
  artist: string;
  album: string;
  cover: string;
  duration: number;
}

export interface Playlist {
  id: number;
  name: string;
  trackIds: number[];
  entries: PlaylistEntryMeta[];
  cover: string;
  createdAt: number;
}

export interface SourceItem {
  id: number;
  url: string;
  title: string;
  createdAt: number;
}

export interface LyricLine {
  timeMs: number | null;
  text: string;
}

export interface LyricsPayload {
  synced: boolean;
  lines: LyricLine[];
}

export interface TrackInfo {
  id: number | null;
  kind: "track" | "url" | "netease" | "qq";
  path: string;
  title: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
  nid?: number | null;
  qid?: string | null;
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

export interface NeteaseTrack {
  id: number;
  name: string;
  ar: { name: string }[];
  al: { name: string; picUrl?: string | null };
  dt: number;
  fee: number;
}

export interface PlayState extends TrackInfo {
  playing: boolean;
}

export interface CurrentTrack extends TrackInfo {
  liked: boolean;
  quality?: string | null;
}

export type RepeatMode = "off" | "all" | "one";

export type QueueItemKind = "track" | "url" | "netease" | "qq";

export type QueueItem =
  | { kind: "track" | "netease" | "url"; id: number }
  | { kind: "qq"; id: string };

export type ViewName =
  | "library"
  | "liked"
  | "recent"
  | "sources"
  | "netease"
  | "qq"
  | "settings"
  | "playlist";

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
