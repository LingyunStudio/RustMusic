//! 设置域：音质 / 关闭行为 / 自动更新 / WASAPI 独占 / 缓存上限与占用。
import type { StateCreator } from "zustand";
import { api } from "../api";
import { detectSystemLang, setLanguagePref, tr } from "../i18n";
import type { Store } from "./contract";

type Fields = Pick<
  Store,
  | "quality"
  | "qualities"
  | "setSourceQuality"
  | "closeAction"
  | "language"
  | "autoUpdate"
  | "wasapiExclusive"
  | "cacheLimit"
  | "cacheBytes"
  | "setQuality"
  | "setCloseAction"
  | "setLanguage"
  | "setAutoUpdate"
  | "setWasapiExclusive"
  | "setCacheLimit"
  | "refreshCacheBytes"
  | "clearCache"
>;

export const createSettingsSlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  quality: "high",
  qualities: { netease: "high", qq: "high", kugou: "high" },
  closeAction: "tray",
  language: "auto",
  autoUpdate: true,
  wasapiExclusive: false,
  cacheLimit: 2 * 1024 * 1024 * 1024,
  cacheBytes: null,
  async clearCache() {
    try {
      const n = await api.clearCache();
      get().toast(
        tr(`已清理 ${n} 个缓存文件`, `Cleared ${n} cached file${n === 1 ? "" : "s"}`),
        "success"
      );
    } catch (e) {
      get().toast(String(e), "error");
    }
    await get().refreshCacheBytes();
  },

  setCacheLimit(bytes) {
    const prev = get().cacheLimit;
    set({ cacheLimit: bytes });
    api
      .setCacheLimit(bytes)
      .then(() => get().refreshCacheBytes())
      .catch((e) => {
        set({ cacheLimit: prev });
        get().toast(String(e), "error");
      });
  },

  async refreshCacheBytes() {
    try {
      const s = await api.cacheStats();
      set({ cacheBytes: s.bytes });
    } catch {
      // 非致命：设置页进入时会再查一次
    }
  },
  // 设置项全部由后端持久化：先乐观更新、失败回滚——否则 UI 显示新值、
  // 实际仍是旧值，重启后“设置自己跳回去”
  setQuality(q) {
    const prev = get().quality;
    set({ quality: q });
    // 兼容旧入口：单一音质设置不再使用，转发为三源默认（netease 为准），
    // 避免遗留调用方写脏 per-source 配置
    api.setPlayQuality("netease", q).catch((e) => {
      set({ quality: prev });
      get().toast(String(e), "error");
    });
  },

  setSourceQuality(kind, q) {
    const prev = get().qualities[kind];
    set({ qualities: { ...get().qualities, [kind]: q } });
    api.setPlayQuality(kind, q).catch((e) => {
      set({ qualities: { ...get().qualities, [kind]: prev } });
      get().toast(String(e), "error");
    });
  },

  setCloseAction(a) {
    const prev = get().closeAction;
    set({ closeAction: a });
    api.setCloseAction(a).catch((e) => {
      set({ closeAction: prev });
      get().toast(String(e), "error");
    });
  },

  setLanguage(lang) {
    const prev = get().language;
    const hint = detectSystemLang();
    set({ language: lang });
    setLanguagePref(lang);
    api.setLanguage(lang, hint).catch((e) => {
      set({ language: prev });
      setLanguagePref(prev);
      get().toast(String(e), "error");
    });
  },

  setAutoUpdate(enabled) {
    const prev = get().autoUpdate;
    set({ autoUpdate: enabled });
    api.setAutoUpdate(enabled).catch((e) => {
      set({ autoUpdate: prev });
      get().toast(String(e), "error");
    });
  },

  setWasapiExclusive(enabled) {
    const prev = get().wasapiExclusive;
    set({ wasapiExclusive: enabled });
    api.setWasapiExclusive(enabled).catch((e) => {
      set({ wasapiExclusive: prev });
      get().toast(String(e), "error");
    });
  },
});
