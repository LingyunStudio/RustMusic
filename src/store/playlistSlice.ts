//! 播放列表域：歌单增删改、条目管理（本地 + 在线）、手动排序持久化。
import type { StateCreator } from "zustand";
import { api } from "../api";
import type { Store } from "./contract";
import { jsonEq } from "./util";

type Fields = Pick<
  Store,
  | "playlists"
  | "manualOrder"
  | "refreshPlaylists"
  | "createPlaylist"
  | "deletePlaylist"
  | "renamePlaylist"
  | "addToPlaylist"
  | "removeFromPlaylist"
  | "addOnlineToPlaylist"
  | "removePlaylistEntryRow"
  | "rowKeyOf"
  | "loadManualOrder"
  | "saveManualOrder"
  | "reorderPlaylist"
  | "reorderPlaylists"
>;

export const createPlaylistSlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  playlists: [],
  /** 手动排序序号缓存：{ list → row_key → pos } */
  manualOrder: {},
  async refreshPlaylists() {
    try {
      const playlists = await api.listPlaylists();
      if (!jsonEq(playlists, get().playlists)) set({ playlists });
    } catch {}
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

  async reorderPlaylists(ids) {
    // 乐观更新：拖拽结束本地顺序立即生效（useDragList 依赖提交触发的
    // 同步重渲染），持久化失败回滚旧顺序
    const prev = get().playlists;
    const byId = new Map(prev.map((p) => [p.id, p] as const));
    const next = ids
      .map((id) => byId.get(id))
      .filter((p): p is (typeof prev)[number] => p != null);
    // 竞态下未出现在 ids 里的列表（拖拽中新建等）按原顺序补在末尾
    if (next.length !== prev.length) {
      for (const p of prev) if (!ids.includes(p.id)) next.push(p);
    }
    set({ playlists: next });
    try {
      await api.reorderPlaylists(ids);
    } catch {
      set({ playlists: prev });
      get().toast("顺序保存失败", "error");
    }
  },
});
