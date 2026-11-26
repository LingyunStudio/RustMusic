/**
 * 音频律动桥：Rust 侧 fx://spectrum 事件（30Hz，36 频段 + 节拍）→
 * 皮肤律动层。30Hz 帧流不走 React 状态（每秒 30 次重渲染整个组件树
 * 毫无意义）：订阅回调直供律动层的 rAF 消费。
 *
 * 后端开关有可见性门控：窗口隐藏/挂起时关闭，避免向冻结的 WebView
 * 空发事件（挂起期间 JS 不执行，visibilitychange 收不到，重开窗口
 * 后由 visible 状态重同步兜底）。
 */
import { api, listenEvent } from "./api";

export interface SpectrumFrame {
  bands: number[];
  level: number;
  beat: boolean;
}

const PREF_KEY = "rustmusic.fxAudio";

/** 音频律动偏好（默认开；仅影响带律动层的皮肤，当前为黑胶） */
export const loadFxPref = () => localStorage.getItem(PREF_KEY) !== "0";
export const saveFxPref = (on: boolean) =>
  localStorage.setItem(PREF_KEY, on ? "1" : "0");

let latest: SpectrumFrame = { bands: [], level: 0, beat: false };
let lastAt = 0;
const fxOwners = new Set<string>(); // 业务期望（引用计数，多消费方互不覆盖）
let backendOn = false; // 后端实际状态（fxOwners.size > 0 && 可见）
let visible = !document.hidden;
let inited = false;

function syncBackend() {
  const on = fxOwners.size > 0 && visible;
  if (on === backendOn) return;
  backendOn = on;
  api.fxSetEnabled(on).catch(() => {
    /* 开关失败不致命：律动层会按数据陈旧自行回落 */
  });
}

/** App 挂载时调用一次：事件订阅 + 可见性门控 */
export function initFx() {
  if (inited) return;
  inited = true;
  listenEvent<SpectrumFrame>("fx://spectrum", (f) => {
    latest = f;
    lastAt = performance.now();
  });
  document.addEventListener("visibilitychange", () => {
    visible = !document.hidden;
    syncBackend();
  });
}

/** 消费方挂载/卸载时登记或撤销后端频谱分析需求（按 owner 去重引用计数） */
export function setFxWanted(on: boolean, owner = "default") {
  if (on) fxOwners.add(owner);
  else fxOwners.delete(owner);
  syncBackend();
}

/** 最近一帧 + 距今毫秒数（陈旧 = 暂停/无声，消费方按零值回落） */
export function getFxFrame(): { frame: SpectrumFrame; age: number } {
  return { frame: latest, age: performance.now() - lastAt };
}
