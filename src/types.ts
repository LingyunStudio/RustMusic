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

export interface Playlist {
  id: number;
  name: string;
  trackIds: number[];
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
  kind: "track" | "url" | "netease";
  path: string;
  title: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
  nid?: number | null;
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
}

export type RepeatMode = "off" | "all" | "one";

export type QueueItemKind = "track" | "url" | "netease";

export interface QueueItem {
  kind: QueueItemKind;
  id: number;
}

export type ViewName =
  | "library"
  | "liked"
  | "recent"
  | "sources"
  | "netease"
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
}

export interface Toast {
  id: number;
  msg: string;
  type: "info" | "error" | "success";
}
