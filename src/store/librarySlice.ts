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
      get().toast("文件夹已添加，正在扫描…", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async removeFolder(id) {
    try {
      await api.removeFolder(id);
      await get().refreshFolders();
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  rescan() {
    api.rescan().catch((e) => get().toast(String(e), "error"));
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
