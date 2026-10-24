//! 媒体库域：曲目/文件夹/在线音源列表数据与刷新、扫描、喜欢（本地曲目）。
import type { StateCreator } from "zustand";
import { api } from "../api";
import type { Store } from "./contract";
import { jsonEq } from "./util";

type Fields = Pick<
  Store,
  | "tracks"
  | "folders"
  | "sources"
  | "refreshTracks"
  | "refreshFolders"
  | "refreshSources"
  | "addFolderByDialog"
  | "removeFolder"
  | "rescan"
  | "deleteTrack"
  | "deleteTracks"
  | "addSource"
  | "deleteSource"
  | "addBilibili"
  | "toggleLike"
>;

export const createLibrarySlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  tracks: [],
  folders: [],
  sources: [],
  // ---------- 数据刷新 ----------

  async refreshTracks() {
    try {
      const tracks = await api.listTracks();
      // 数据未变化时保持旧引用（挂起恢复的刷新不该引起整页重渲染闪烁）
      if (!jsonEq(tracks, get().tracks)) set({ tracks });
    } catch {}
  },

  async refreshFolders() {
    try {
      const folders = await api.listFolders();
      if (!jsonEq(folders, get().folders)) set({ folders });
    } catch {}
  },

  async refreshSources() {
    try {
      set({ sources: await api.listSources() });
    } catch {}
  },
  toggleLike(id) {
    const t = get().tracks.find((x) => x.id === id);
    if (!t) return;
    const liked = !t.liked;
    const apply = (v: boolean) =>
      set((s) => ({
        tracks: s.tracks.map((x) => (x.id === id ? { ...x, liked: v } : x)),
        current:
          s.current && s.current.id === id ? { ...s.current, liked: v } : s.current,
      }));
    apply(liked);
    api.likeTrack(id, liked).catch(() => {
      // 失败回滚，避免 UI 与数据库状态不一致（重启后状态跳回）
      apply(!liked);
      get().toast("喜欢状态同步失败", "error");
    });
  },
  // ---------- 媒体库 ----------

  async addFolderByDialog() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false, title: "选择音乐文件夹" });
      if (!selected || typeof selected !== "string") return;
      await api.addFolder(selected);
      await get().refreshFolders();
      get().toast("文件夹已添加，正在扫描…", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async removeFolder(id) {
    try {
      await api.removeFolder(id);
      await get().refreshFolders();
      get().toast("文件夹已移除，正在重新扫描…", "info");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  rescan() {
    api.rescan().catch((e) => get().toast(String(e), "error"));
  },

  async deleteTrack(id) {
    try {
      await api.deleteTrack(id);
      const s = get();
      const current = s.current;
      const isCurrentDeleted = current?.kind === "track" && current.id === id;
      const newQueue = s.queue.filter((q) => !(q.kind === "track" && q.id === id));
      const newTracks = s.tracks.filter((t) => t.id !== id);

      if (newQueue.length === 0) {
        api.stop().catch(() => {});
        set({
          queue: [],
          qIndex: 0,
          current: null,
          playing: false,
          pos: 0,
          tracks: newTracks,
        });
      } else if (isCurrentDeleted) {
        const target = Math.min(s.qIndex, newQueue.length - 1);
        set({
          queue: newQueue,
          qIndex: target,
          tracks: newTracks,
        });
        get().playQueueIndex(target);
      } else {
        const newQIndex = newQueue.findIndex((item) => {
          if (current?.kind === "track") return item.kind === "track" && item.id === current.id;
          // 在线曲目当前项的重定位：nid/qid/kgid 各源只填自己那一个，
          // 直链音源三者皆空（用 current.id 兜底）——漏掉 kgid 时酷狗当前曲
          // 恒匹配失败，qIndex 指向错误行、UI 高亮与“下一首”和实际播放脱节
          const curId =
            (current as any)?.nid ?? (current as any)?.qid ?? (current as any)?.kgid ?? current?.id;
          return item.kind === current?.kind && item.id === curId;
        });
        set({
          queue: newQueue,
          qIndex: newQIndex >= 0 ? newQIndex : Math.min(s.qIndex, newQueue.length - 1),
          tracks: newTracks,
        });
      }
      get().refreshPlaylists().catch(() => {});
      get().toast("歌曲已从硬盘删除", "success");
    } catch (e) {
      get().toast(`删除失败：${e}`, "error");
    }
  },

  async deleteTracks(ids) {
    try {
      const idSet = new Set(ids);
      const count = await api.deleteTracks(ids);
      const s = get();
      const current = s.current;
      const isCurrentDeleted =
        current?.kind === "track" && current.id != null && idSet.has(current.id);
      const newQueue = s.queue.filter(
        (q) => !(q.kind === "track" && q.id != null && idSet.has(q.id))
      );
      const newTracks = s.tracks.filter((t) => !idSet.has(t.id));

      if (newQueue.length === 0) {
        api.stop().catch(() => {});
        set({
          queue: [],
          qIndex: 0,
          current: null,
          playing: false,
          pos: 0,
          tracks: newTracks,
        });
      } else if (isCurrentDeleted) {
        const target = Math.min(s.qIndex, newQueue.length - 1);
        set({
          queue: newQueue,
          qIndex: target,
          tracks: newTracks,
        });
        get().playQueueIndex(target);
      } else {
        const newQIndex = newQueue.findIndex((item) => {
          if (current?.kind === "track") return item.kind === "track" && item.id === current.id;
          const curId =
            (current as any)?.nid ?? (current as any)?.qid ?? (current as any)?.kgid ?? current?.id;
          return item.kind === current?.kind && item.id === curId;
        });
        set({
          queue: newQueue,
          qIndex: newQIndex >= 0 ? newQIndex : Math.min(s.qIndex, newQueue.length - 1),
          tracks: newTracks,
        });
      }
      get().refreshPlaylists().catch(() => {});
      get().toast(`已从硬盘删除 ${count} 首歌曲`, "success");
    } catch (e) {
      get().toast(`删除失败：${e}`, "error");
    }
  },
  // ---------- 在线音源 ----------

  async addSource(url, title) {
    try {
      await api.addSource(url, title);
      await get().refreshSources();
      get().toast("音源已添加", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async deleteSource(id) {
    try {
      await api.deleteSource(id);
      await get().refreshSources();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  /** 解析 B 站视频加入在线音源列表（多P视频每个分P各加一条） */
  async addBilibili(input: string) {
    try {
      const n = await api.bilibiliAdd(input);
      await get().refreshSources();
      if (n > 0) {
        get().toast(
          n > 1 ? `已解析并添加 ${n} 个分P` : "已添加 B 站视频音频",
          "success"
        );
      } else {
        get().toast("该视频已在在线音源列表中", "info");
      }
    } catch (e) {
      get().toast(String(e), "error");
    }
  },
});
