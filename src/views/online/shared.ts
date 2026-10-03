import type { NeteaseTrack, QqSong } from "../../types";

export type Source = "netease" | "qq" | "kugou";

export interface OnlineRow {
  kind: Source;
  id: number | string;
  name: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
  vip: boolean;
  mediaMid: string;
}

export const qqCover = (albumMid: string) =>
  albumMid
    ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${albumMid}.jpg`
    : "";

/** 在线搜索历史（localStorage，跨会话保留，最多 12 条） */
export const SEARCH_HISTORY_KEY = "rustmusic_search_history";
export const loadSearchHistory = (): string[] => {
  try {
    const raw = localStorage.getItem(SEARCH_HISTORY_KEY);
    const list = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(list)
      ? list.filter((x): x is string => typeof x === "string")
      : [];
  } catch {
    return [];
  }
};
