//! 播放列表域：歌单增删改、条目管理（本地 + 在线）、手动排序持久化。
import type { StateCreator } from "zustand";
import { api } from "../api";
import { tr } from "../i18n";
import type { Store } from "./contract";
import { jsonEq } from "./util";

type Fields = Pick<
  Store,
  | "playlists"
  | "manualOrder"
  | "refreshPlaylists"
  | "createPlaylist"
  | "deletePlaylist"
  | "deletePlaylists"
  | "renamePlaylist"
  | "addToPlaylist"
  | "addTracksToPlaylist"
  | "exportPlaylistM3u"
  | "importPlaylistM3u"
  | "removeFromPlaylist"
  | "addOnlineToPlaylist"
  | "removePlaylistEntryRow"
  | "removePlaylistEntryRows"
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
      get().toast(tr(`已创建播放列表「${name}」`, `Created playlist "${name}"`), "success");
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
      get().toast(tr("播放列表已删除", "Playlist deleted"), "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async deletePlaylists(ids) {
    try {
      await api.deletePlaylists(ids);
      await get().refreshPlaylists();
      if (
        get().view === "playlist" &&
        get().viewParam != null &&
        ids.includes(get().viewParam as number)
      ) {
        set({ view: "library" });
      }
      get().toast(
        tr(`已删除 ${ids.length} 个播放列表`, `Deleted ${ids.length} playlist${ids.length === 1 ? "" : "s"}`),
        "success"
      );
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
      const plName = pl?.name ?? tr("播放列表", "Playlist");
      get().toast(tr(`已添加到「${plName}」`, `Added to "${plName}"`), "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async addTracksToPlaylist(pid, tids) {
    if (!tids.length) return;
    try {
      const added = await api.addTracksToPlaylist(pid, tids);
      await get().refreshPlaylists();
      const pl = get().playlists.find((p) => p.id === pid);
      const plName = pl?.name ?? tr("播放列表", "Playlist");
      get().toast(
        added > 0
          ? tr(`已添加 ${added} 首到「${plName}」`, `Added ${added} track${added === 1 ? "" : "s"} to "${plName}"`)
          : tr(`所选歌曲都已在「${plName}」中`, `Selected tracks are already in "${plName}"`),
        added > 0 ? "success" : "info"
      );
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async exportPlaylistM3u(pid, path) {
    try {
      const n = await api.exportPlaylistM3u(pid, path);
      get().toast(
        tr(`已导出 ${n} 条到 ${path}`, `Exported ${n} track${n === 1 ? "" : "s"} to ${path}`),
        "success"
      );
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async importPlaylistM3u(name, path) {
    try {
      const r = await api.importPlaylistM3u(name, path);
      await get().refreshPlaylists();
      get().toast(
        tr(
          `已导入「${name}」：${r.added} 首${r.skipped > 0 ? `（跳过 ${r.skipped} 条无效条目）` : ""}`,
          `Imported "${name}": ${r.added} track${r.added === 1 ? "" : "s"}${r.skipped > 0 ? ` (${r.skipped} invalid skipped)` : ""}`
        ),
        "success"
      );
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
      const plName = pl?.name ?? tr("播放列表", "Playlist");
      get().toast(tr(`已添加到「${plName}」`, `Added to "${plName}"`), "success");
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

  async removePlaylistEntryRows(rowids) {
    try {
      await api.removePlaylistEntries(rowids);
      await get().refreshPlaylists();
      get().toast(
        tr(`已从歌单中移除 ${rowids.length} 首歌曲`, `Removed ${rowids.length} track${rowids.length === 1 ? "" : "s"} from playlist`),
        "success"
      );
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
      get().toast(tr("顺序保存失败", "Failed to save order"), "error");
    }
  },

  async reorderPlaylist(pid, rowids) {
    // 乐观更新：useDragList 的提交契约要求 onSubmit 同步重渲染，
    // 否则松手清空 transform 的瞬间 DOM 会闪回旧顺序（SQLite 往返期间）。
    // 本地按 rowid 序列重排 entries，持久化失败回滚
    const prev = get().playlists;
    const next = prev.map((p) => {
      if (p.id !== pid) return p;
      const byRowid = new Map(p.entries.map((e) => [e.rowid, e] as const));
      const ordered = rowids
        .map((rid) => byRowid.get(rid))
        .filter((e): e is (typeof p.entries)[number] => e != null);
      if (ordered.length !== p.entries.length) {
        for (const e of p.entries) {
          if (!rowids.includes(e.rowid)) ordered.push(e);
        }
      }
      return { ...p, entries: ordered };
    });
    set({ playlists: next });
    try {
      await api.reorderPlaylist(pid, rowids);
    } catch (e) {
      set({ playlists: prev });
      get().toast(tr("顺序保存失败", "Failed to save order"), "error");
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
      get().toast(tr("顺序保存失败", "Failed to save order"), "error");
    }
  },
});
