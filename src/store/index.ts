//! 全局状态入口：按领域组合各 slice（行为与拆分前的单文件 store 完全一致）。
import { create } from "zustand";
import type { Store } from "./contract";
import { createLibrarySlice } from "./librarySlice";
import { createOnlineSlice } from "./onlineSlice";
import { createPlayerSlice } from "./playerSlice";
import { createPlaylistSlice } from "./playlistSlice";
import { createSettingsSlice } from "./settingsSlice";
import { createUiSlice } from "./uiSlice";

export type { Store } from "./contract";

export const useStore = create<Store>()((...a) => ({
  ...createUiSlice(...a),
  ...createLibrarySlice(...a),
  ...createPlaylistSlice(...a),
  ...createPlayerSlice(...a),
  ...createOnlineSlice(...a),
  ...createSettingsSlice(...a),
}));
