//! 设置域：音质 / 关闭行为 / 自动更新 / WASAPI 独占 / 缓存上限与占用。
import type { StateCreator } from "zustand";
import { api } from "../api";
import type { Store } from "./contract";

type Fields = Pick<
  Store,
  | "quality"
  | "closeAction"
  | "autoUpdate"
  | "wasapiExclusive"
  | "cacheLimit"
  | "cacheBytes"
  | "setQuality"
  | "setCloseAction"
  | "setAutoUpdate"
  | "setWasapiExclusive"
  | "setCacheLimit"
  | "refreshCacheBytes"
  | "clearCache"
>;

export const createSettingsSlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  quality: "high",
  closeAction: "tray",
  autoUpdate: true,
  wasapiExclusive: false,
  cacheLimit: 2 * 1024 * 1024 * 1024,
  cacheBytes: null,
  async clearCache() {
    try {
      const n = await api.clearCache();
      get().toast(`已清理 ${n} 个缓存文件`, "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
    await get().refreshCacheBytes();
  },

  setCacheLimit(bytes) {
    set({ cacheLimit: bytes });
    api
      .setCacheLimit(bytes)
      .then(() => get().refreshCacheBytes())
      .catch((e) => get().toast(String(e), "error"));
  },

  async refreshCacheBytes() {
    try {
      const s = await api.cacheStats();
      set({ cacheBytes: s.bytes });
    } catch {
      // 非致命：设置页进入时会再查一次
    }
  },
  setQuality(q) {
    set({ quality: q });
    api.setPlayQuality(q).catch((e) => get().toast(String(e), "error"));
  },

  setCloseAction(a) {
    set({ closeAction: a });
    api.setCloseAction(a).catch((e) => get().toast(String(e), "error"));
  },

  setAutoUpdate(enabled) {
    set({ autoUpdate: enabled });
    api.setAutoUpdate(enabled).catch((e) => get().toast(String(e), "error"));
  },

  setWasapiExclusive(enabled) {
    set({ wasapiExclusive: enabled });
    api.setWasapiExclusive(enabled).catch((e) => get().toast(String(e), "error"));
  },
});
