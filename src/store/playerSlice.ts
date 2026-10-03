//! 播放器域：播放队列、切歌/上一首/自动跳过失败项、进度与音量倍速、均衡器、歌词加载。
import type { StateCreator } from "zustand";
import { api } from "../api";
import type { PlaylistEntryMeta, QueueItem } from "../types";
import type { Store } from "./contract";
import { capOnlineCache } from "./onlineCache";
import { pushDesktopLyrics } from "./util";

type Fields = Pick<
  Store,
  | "current"
  | "playing"
  | "pos"
  | "dur"
  | "playSeq"
  | "queue"
  | "qIndex"
  | "history"
  | "volume"
  | "speed"
  | "repeat"
  | "shuffle"
  | "eqGains"
  | "eqEnabled"
  | "unavailable"
  | "failStreak"
  | "failToastId"
  | "playingSourceId"
  | "scrubbing"
  | "sleepAt"
  | "queueExtender"
  | "lyrics"
  | "lyricsLoading"
  | "lyricsFor"
  | "playTracks"
  | "playSourceItem"
  | "playEntries"
  | "playQueueIndex"
  | "setQueueExtender"
  | "entryToQueueItem"
  | "togglePlay"
  | "next"
  | "prev"
  | "seek"
  | "setScrubbing"
  | "setVolume"
  | "setSleepTimer"
  | "setSpeed"
  | "setRepeat"
  | "toggleShuffle"
  | "setEq"
  | "addToQueue"
  | "playNext"
  | "removeQueueItem"
  | "clearQueue"
  | "jumpTo"
  | "loadLyricsByKey"
  | "loadLyrics"
  | "applyMediaControl"
>;

let volumeTimer: ReturnType<typeof setTimeout> | null = null;
let sleepTimerRef: ReturnType<typeof setTimeout> | null = null;
/** 队列自动续页进行中：防止最后一条上的重复触发并发拉页 */
let queueExtendBusy = false;
/** set_eq IPC 防抖定时器 */
let eqTimer: ReturnType<typeof setTimeout> | null = null;

/** 队列项显示名（失败提示用；取不到返回占位） */
function titleOfQueueItem(
  item: { kind: string; id: number | string },
  caches: Pick<
    Store,
    "tracks" | "neteaseCache" | "qqCache" | "kugouCache" | "biliCache" | "ndCache" | "sources"
  >
): string {
  if (item.kind === "track") {
    return caches.tracks.find((t) => t.id === item.id)?.title ?? `曲目 #${item.id}`;
  }
  if (item.kind === "netease") {
    return caches.neteaseCache[item.id as number]?.name ?? `网易云 #${item.id}`;
  }
  if (item.kind === "qq") {
    return caches.qqCache[item.id as string]?.name ?? `QQ音乐 #${item.id}`;
  }
  if (item.kind === "kugou") {
    return caches.kugouCache[item.id as string]?.name ?? `酷狗 #${item.id}`;
  }
  if (item.kind === "navidrome") {
    return caches.ndCache[item.id as string]?.title ?? `Navidrome #${item.id}`;
  }
  if (item.kind === "bilibili") {
    return caches.biliCache[item.id as string]?.title ?? `B站 #${item.id}`;
  }
  return caches.sources.find((s) => s.id === item.id)?.title ?? `音源 #${item.id}`;
}

export const createPlayerSlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  current: null,
  playing: false,
  pos: 0,
  dur: 0,
  playSeq: 0,
  scrubbing: false,
  queue: [],
  queueExtender: null,
  qIndex: 0,
  history: [],
  volume: 0.8,
  sleepAt: null,
  speed: 1,
  repeat: "off",
  shuffle: false,
  eqGains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  eqEnabled: false,
  lyrics: null,
  lyricsLoading: false,
  lyricsFor: null,
  unavailable: {},
  failStreak: 0,
  failToastId: null,
  playingSourceId: null,
  // ---------- 播放 ----------

  playTracks(tracks, idx) {
    if (!tracks.length) return;
    const queue: QueueItem[] = tracks.map((t) => ({ kind: "track", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
    }));
    get().playQueueIndex(target);
  },
  playSourceItem(s) {
    const queue: QueueItem[] = [{ kind: "url", id: s.id }];
    set((st) => ({
      playingSourceId: s.id,
      queue,
      qIndex: 0,
      history: [...st.history.slice(-50), st.qIndex],
      failStreak: 0,
      failToastId: null,
    }));
    api
      .playSource(s.id)
      .catch((e) => get().toast(`播放音源失败：${e}`, "error"));
  },
  playEntries(entries: PlaylistEntryMeta[], idx: number) {
    const queue: QueueItem[] = [];
    const neteaseCache = { ...get().neteaseCache };
    const qqCache = { ...get().qqCache };
    const kugouCache = { ...get().kugouCache };
    const biliCache = { ...get().biliCache };
    const ndCache = { ...get().ndCache };
    for (const e of entries) {
      if (e.kind === "local" && e.trackId != null) {
        queue.push({ kind: "track", id: e.trackId });
      } else if (e.kind === "netease" && e.onlineId) {
        const idNum = Number(e.onlineId);
        neteaseCache[idNum] = {
          id: idNum,
          name: e.title,
          ar: [{ name: e.artist }],
          al: { name: e.album, picUrl: e.cover },
          dt: Math.round(e.duration * 1000),
          fee: e.vip ? 1 : 0,
        };
        queue.push({ kind: "netease", id: idNum });
      } else if (e.kind === "qq" && e.onlineId) {
        // e.cover 为完整封面 URL，最后一段即 albumMid
        const albumMid = e.cover.match(/M000([0-9A-Za-z]+)\.jpg?/)?.[1] ?? "";
        qqCache[e.onlineId] = {
          id: e.onlineId,
          name: e.title,
          singer: e.artist,
          album: e.album,
          albumMid,
          mediaMid: e.mediaMid ?? "",
          durationMs: Math.round(e.duration * 1000),
          vip: e.vip ?? false,
        };
        queue.push({ kind: "qq", id: e.onlineId });
      } else if (e.kind === "kugou" && e.onlineId) {
        kugouCache[e.onlineId] = {
          id: e.onlineId,
          name: e.title,
          singer: e.artist,
          album: e.album,
          durationMs: Math.round(e.duration * 1000),
          cover: e.cover,
          vip: e.vip ?? false,
          // media_mid 列存专辑音频 ID（播放/导入时写入），恢复播放可用
          albumAudioId: Number(e.mediaMid) || 0,
        };
        queue.push({ kind: "kugou", id: e.onlineId });
      } else if (e.kind === "bilibili" && e.onlineId) {
        biliCache[e.onlineId] = {
          rid: e.onlineId,
          title: e.title,
          artist: e.artist,
          album: e.album,
          cover: e.cover,
          durationMs: Math.round(e.duration * 1000),
        };
        queue.push({ kind: "bilibili", id: e.onlineId });
      } else if (e.kind === "navidrome" && e.onlineId) {
        ndCache[e.onlineId] = {
          title: e.title,
          artist: e.artist,
          album: e.album,
          cover: e.cover,
          durationMs: Math.round(e.duration * 1000),
        };
        queue.push({ kind: "navidrome", id: e.onlineId });
      }
    }
    let target = Math.max(0, Math.min(idx, queue.length - 1));
    // 双击已知失败的曲目：从点击处向后找第一个可播项（全是坏项则停在原地提示）
    const avail = queue.findIndex(
      (q, i) => i >= target && get().unavailable[`${q.kind}:${q.id}`] == null
    );
    if (avail >= 0) target = avail;
    set((s) => ({
      neteaseCache: capOnlineCache("netease", neteaseCache, get().neteaseCache),
      qqCache: capOnlineCache("qq", qqCache, get().qqCache),
      kugouCache: capOnlineCache("kugou", kugouCache, get().kugouCache),
      ndCache: capOnlineCache("nd", ndCache, get().ndCache),
      biliCache: capOnlineCache("bili", biliCache, get().biliCache),
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
    }));
    if (queue.length) get().playQueueIndex(target);
  },

  entryToQueueItem(e) {
    if (e.kind === "local" && e.trackId != null) {
      return { kind: "track", id: e.trackId };
    }
    if (e.kind === "netease" && e.onlineId) {
      const idNum = Number(e.onlineId);
      if (!Number.isFinite(idNum) || idNum <= 0) return null;
      const neteaseCache = { ...get().neteaseCache };
      neteaseCache[idNum] = {
        id: idNum,
        name: e.title,
        ar: [{ name: e.artist }],
        al: { name: e.album, picUrl: e.cover },
        dt: Math.round(e.duration * 1000),
        fee: e.vip ? 1 : 0,
      };
      set({ neteaseCache: capOnlineCache("netease", neteaseCache, get().neteaseCache) });
      return { kind: "netease", id: idNum };
    }
    if (e.kind === "qq" && e.onlineId) {
      const albumMid = e.cover.match(/M000([0-9A-Za-z]+)\.jpg?/)?.[1] ?? "";
      const qqCache = { ...get().qqCache };
      qqCache[e.onlineId] = {
        id: e.onlineId,
        name: e.title,
        singer: e.artist,
        album: e.album,
        albumMid,
        mediaMid: e.mediaMid ?? "",
        durationMs: Math.round(e.duration * 1000),
        vip: e.vip ?? false,
      };
      set({ qqCache: capOnlineCache("qq", qqCache, get().qqCache) });
      return { kind: "qq", id: e.onlineId };
    }
    if (e.kind === "kugou" && e.onlineId) {
      const kugouCache = { ...get().kugouCache };
      kugouCache[e.onlineId] = {
        id: e.onlineId,
        name: e.title,
        singer: e.artist,
        album: e.album,
        durationMs: Math.round(e.duration * 1000),
        cover: e.cover,
        vip: e.vip ?? false,
        albumAudioId: Number(e.mediaMid) || 0,
      };
      set({ kugouCache: capOnlineCache("kugou", kugouCache, get().kugouCache) });
      return { kind: "kugou", id: e.onlineId };
    }
    if (e.kind === "bilibili" && e.onlineId) {
      const biliCache = { ...get().biliCache };
      biliCache[e.onlineId] = {
        rid: e.onlineId,
        title: e.title,
        artist: e.artist,
        album: e.album,
        cover: e.cover,
        durationMs: Math.round(e.duration * 1000),
      };
      set({ biliCache: capOnlineCache("bili", biliCache, get().biliCache) });
      return { kind: "bilibili", id: e.onlineId };
    }
    if (e.kind === "navidrome" && e.onlineId) {
      const ndCache = { ...get().ndCache };
      ndCache[e.onlineId] = {
        title: e.title,
        artist: e.artist,
        album: e.album,
        cover: e.cover,
        durationMs: Math.round(e.duration * 1000),
      };
      set({ ndCache: capOnlineCache("nd", ndCache, get().ndCache) });
      return { kind: "navidrome", id: e.onlineId };
    }
    return null;
  },

  setQueueExtender(fn) {
    set({ queueExtender: fn });
  },
  playQueueIndex(i) {
    const { queue } = get();
    const item = queue[i];
    if (!item) return;

    // 自动续页：播到队列最后一条时，若来源（搜索/在线榜单）还有下一页，
    // 后台加载并追加进队列——最后一首自然播完后无缝继续，代替人工点
    // "加载更多"。extender 由列表视图注册；返回 null（无更多/上下文失效）
    // 即注销。busy 防同一首上的重复触发并发拉页。
    if (i === queue.length - 1 && !queueExtendBusy) {
      const extender = get().queueExtender;
      if (extender) {
        queueExtendBusy = true;
        extender()
          .then((items) => {
            if (items && items.length) {
              set((s) => ({ queue: [...s.queue, ...items] }));
            } else {
              set({ queueExtender: null });
            }
          })
          .catch(() => {})
          .finally(() => {
            queueExtendBusy = false;
          });
      }
    }
    // 维护“正在播放的自定义音源 id”（供 SourcesView 高亮；非 url 播放时清除）
    if (item.kind === "url") set({ playingSourceId: item.id });
    else if (get().playingSourceId != null) set({ playingSourceId: null });

    /** 播放失败统一处理：提示 + 按需标记不可用（列表置灰）+ 自动跳下一首。
     *  只有真永久失败（无版权/下架/信息失效）才置灰；
     *  VIP/权益不足随登录与会员状态可恢复，网络错误是瞬时的——都不标记，
     *  下次仍会尝试。登录过期则整个队列都会失败：立即停止并提示重新登录，
     *  不再连跳刷屏。 */
    const fail = (msg: string) => {
      const needRelogin = /登录已过期|请重新登录|未登录/.test(msg);
      const permanent = !needRelogin && /无版权|下架|已失效|信息失效/.test(msg);
      const key = `${item.kind}:${item.id}`;
      set((s) => ({
        unavailable: permanent
          ? { ...s.unavailable, [key]: msg }
          : s.unavailable,
        failStreak: s.failStreak + 1,
      }));
      // 失败提示只保留一条、原地更新（连跳多少首都只占一个位置，不刷屏）：
      // 首次失败报具体原因，连跳时滚动显示累计数与最近一首；登录过期
      // 则整条提示就是原因本身，手动再点别的歌也只更新这一条
      const streak = get().failStreak;
      const title = titleOfQueueItem(item, get());
      const text = needRelogin
        ? msg
        : streak > 1
          ? `已连续跳过 ${streak} 首无法播放的歌曲（最近：「${title}」${msg}）`
          : `跳过「${title}」：${msg}`;
      const prev = get().failToastId;
      if (prev != null) get().dismissToast(prev);
      set({ failToastId: get().toast(text, "error") });
      // 登录过期：整个队列都会失败，停止继续尝试即可。
      // 注意：失败的只是"切歌尝试"，引擎里可能仍在放换队列前的歌
      // （如在线曲目失败回落的场景），绝不能动 playing——按钮和进度
      // 一律以引擎的 player://nowplaying 事件为准。
      if (needRelogin) return;
      if (streak < queue.length) {
        // 失败自动跳歌必须绕过单曲循环：repeat-one 下 next(true) 会重播
        // 刚失败的同一首，坏歌被反复重试直到 failStreak 追平队列长度
        get().next(true, false, true);
      }
    };
    // 开播成功则清零连跳计数，并自愈清除本曲历史置灰标记
    //（元数据后补/状态恢复后同一曲目仍可正常播放，kugou 等无登录态
    // 刷新路径的来源也由此恢复；登录刷新的前缀清理只作兜底）
    const ok = () =>
      set((s) => {
        const key = `${item.kind}:${item.id}`;
        if (s.unavailable[key] == null)
          return { failStreak: 0, failToastId: null };
        const { [key]: _cleared, ...rest } = s.unavailable;
        return { unavailable: rest, failStreak: 0, failToastId: null };
      });

    if (item.kind === "track") {
      api.playTrack(item.id).then(ok).catch((e) => fail(String(e)));
    } else if (item.kind === "netease") {
      const t = get().neteaseCache[item.id];
      if (!t) {
        fail("曲目信息缺失，请重试");
        return;
      }
      api
        .neteasePlay({
          id: t.id,
          title: t.name,
          artist: t.ar.map((a) => a.name).join(" / "),
          album: t.al?.name ?? "",
          cover: t.al?.picUrl ?? "",
          durationMs: t.dt,
        })
        .then(ok)
        .catch((e) => fail(String(e)));
    } else if (item.kind === "qq") {
      const t = get().qqCache[item.id];
      if (!t) {
        fail("曲目信息缺失，请重试");
        return;
      }
      api
        .qqPlay({
          songmid: t.id,
          title: t.name,
          artist: t.singer,
          album: t.album,
          albumMid: t.albumMid,
          mediaMid: t.mediaMid,
          durationMs: t.durationMs,
          vip: t.vip ?? false,
        })
        .then(ok)
        .catch((e) => fail(String(e)));
    } else if (item.kind === "kugou") {
      const t = get().kugouCache[item.id];
      if (!t) {
        fail("曲目信息缺失，请重试");
        return;
      }
      api
        .kugouPlay({
          hash: t.id,
          title: t.name,
          artist: t.singer,
          album: t.album,
          cover: t.cover,
          durationMs: t.durationMs,
          vip: t.vip ?? false,
          albumAudioId: t.albumAudioId ?? 0,
          albumId: t.albumId ?? 0,
          hqHash: t.hqHash ?? "",
          sqHash: t.sqHash ?? "",
          superHash: t.superHash ?? "",
        })
        .then(ok)
        .catch((e) => fail(String(e)));
    } else if (item.kind === "navidrome") {
      const t = get().ndCache[item.id as string];
      if (!t) {
        fail("曲目信息缺失，请重试");
        return;
      }
      const server = localStorage.getItem("navidrome.ui.server") ?? "";
      const username = localStorage.getItem("navidrome.ui.username") ?? "";
      api
        .navidromePlay(server, username, {
          id: item.id as string,
          title: t.title,
          artist: t.artist,
          album: t.album,
          cover: t.cover,
          durationMs: t.durationMs,
        })
        .then(ok)
        .catch((e) => fail(String(e)));
    } else if (item.kind === "bilibili") {
      const t = get().biliCache[item.id as string];
      if (!t) {
        fail("曲目信息缺失，请重试");
        return;
      }
      api
        .bilibiliPlay({
          rid: t.rid,
          title: t.title,
          artist: t.artist,
          album: t.album,
          cover: t.cover,
          durationMs: t.durationMs,
        })
        .then(ok)
        .catch((e) => fail(String(e)));
    } else if (item.kind === "url") {
      api.playSource(item.id).then(ok).catch((e) => fail(String(e)));
    }
  },
  togglePlay() {
    const { current, queue, qIndex, tracks } = get();
    if (!current) {
      if (queue.length) {
        // qIndex=-1（当前播放项刚被删除）时 playQueueIndex(-1) 会静默
        // 返回，播放按钮彻底无响应——夹回 0 从队首开播
        get().playQueueIndex(Math.max(0, qIndex));
      } else if (tracks.length) {
        get().playTracks(tracks, 0);
      }
      return;
    }
    api.playPause().catch((e) => get().toast(String(e), "error"));
  },

  next(auto = false, ended = false, bypassRepeatOne = false) {
    const { queue, qIndex, repeat, shuffle, current } = get();
    if (!queue.length) return;

    if (auto && repeat === "one" && !bypassRepeatOne && current) {
      // 单曲循环：重新播放当前曲目（qIndex 可能为 -1——当前项刚被删除，取 0）
      get().playQueueIndex(Math.max(0, qIndex));
      return;
    }

    let idx: number;
    if (shuffle && queue.length > 1) {
      do {
        idx = Math.floor(Math.random() * queue.length);
      } while (idx === qIndex);
    } else {
      idx = qIndex + 1;
    }
    // 跳过已知失败的在线曲目（无版权/下架），最多检查一整圈防止死循环
    let guard = queue.length;
    while (guard-- > 0) {
      if (idx >= queue.length) {
        if (repeat === "all") idx = 0;
        else break;
      }
      const q = queue[idx];
      if (get().unavailable[`${q.kind}:${q.id}`] == null) break;
      idx++;
    }
    if (idx >= queue.length) {
      if (repeat === "all" && get().failStreak === 0) {
        idx = 0;
      } else {
        // 引擎可能仍在放换队列前的歌（在线播放失败场景），此时不能
        // 把 playing/pos 一把清掉；引擎空闲时该分支本就是幂等复位。
        // ended=true 表示由"自然播完"事件驱动、引擎已空闲：立即复位，
        // 不必等看门狗数秒后纠正（否则按钮卡"播放中"、进度冻结）
        if (ended || !get().playing) set({ playing: false, pos: 0 });
        return;
      }
    }
    set((s) => ({ qIndex: idx, history: [...s.history.slice(-50), s.qIndex] }));
    get().playQueueIndex(idx);
  },

  prev() {
    const { queue, qIndex, shuffle, history } = get();
    if (!queue.length) return;
    // shuffle：按播放历史回跳——history 是每次切歌记录的"来时的位置"，
    // 逐个弹出直到找到可播项；历史耗尽再走常规回绕（此后 prev 继续回绕）
    if (shuffle && history.length > 0) {
      let h = [...history];
      let guard = h.length;
      while (guard-- > 0) {
        const last = h[h.length - 1];
        h = h.slice(0, -1);
        const item = last >= 0 ? queue[last] : undefined;
        if (
          item &&
          get().unavailable[`${item.kind}:${item.id}`] == null
        ) {
          set({ qIndex: last, history: h });
          get().playQueueIndex(last);
          return;
        }
      }
    }
    // 直接切到上一曲（到列表头则回绕到最后一首），跳过已知失败项
    let idx = qIndex > 0 ? qIndex - 1 : queue.length - 1;
    let guard = queue.length;
    while (guard-- > 0) {
      const q = queue[idx];
      if (get().unavailable[`${q.kind}:${q.id}`] == null) break;
      idx = idx > 0 ? idx - 1 : queue.length - 1;
    }
    if (get().unavailable[`${queue[idx].kind}:${queue[idx].id}`] != null) return;
    set((s) => ({ qIndex: idx, history: [...s.history.slice(-50), s.qIndex] }));
    get().playQueueIndex(idx);
  },

  setScrubbing(v) {
    set({ scrubbing: v });
  },

  seek(ms) {
    // scrubbing 保持锁定直到后端 seek 完成（FLAC 重建耗时数百 ms）
    set({ pos: ms });
    api
      .seek(Math.round(ms))
      .catch((e) => get().toast(`跳转失败：${e}`, "error"))
      .finally(() => set({ scrubbing: false }));
  },

  setVolume(v) {
    const vol = Math.max(0, Math.min(1, v));
    set({ volume: vol });
    if (volumeTimer) clearTimeout(volumeTimer);
    volumeTimer = setTimeout(() => {
      api.setVolume(vol).catch(() => {});
    }, 300);
  },

  setSleepTimer(min) {
    if (sleepTimerRef) {
      clearTimeout(sleepTimerRef);
      sleepTimerRef = null;
    }
    if (min == null) {
      set({ sleepAt: null });
      return;
    }
    set({ sleepAt: Date.now() + min * 60000 });
    sleepTimerRef = setTimeout(() => {
      sleepTimerRef = null;
      set({ sleepAt: null, playing: false, pos: 0 });
      // 停引擎（后端会同步 SMTC 等）；失败也无害——UI 已复位
      api.stop().catch(() => {});
      get().toast("定时时间到，已停止播放", "info");
    }, min * 60000);
  },

  setSpeed(v) {
    set({ speed: v });
    api.setSpeed(v).catch(() => {});
  },

  setRepeat(m) {
    set({ repeat: m });
  },

  toggleShuffle() {
    set((s) => ({ shuffle: !s.shuffle }));
  },
  addToQueue(item) {
    set((s) => ({ queue: [...s.queue, item] }));
    get().toast("已加入播放队列", "success");
  },

  playNext(item) {
    set((s) => {
      const q = [...s.queue];
      q.splice(s.qIndex + 1, 0, item);
      return { queue: q };
    });
    get().toast("将在当前曲目后播放", "success");
  },

  removeQueueItem(i) {
    set((s) => {
      const q = s.queue.filter((_, idx) => idx !== i);
      // 删除当前播放项（i === qIndex）时 qIndex 回退一位（可为 -1），
      // 使播完后的 next() 恰好落在原下一首上，避免跳歌；UI 无高亮项符合语义
      let qIndex = s.qIndex;
      if (i <= s.qIndex) qIndex = Math.max(-1, qIndex - 1);
      return { queue: q, qIndex };
    });
  },

  clearQueue() {
    const { current } = get();
    const keep = current ? [get().queue[get().qIndex]] : [];
    set({
      queue: keep.filter(Boolean),
      qIndex: 0,
    });
  },

  jumpTo(i) {
    set({ qIndex: i, history: [...get().history.slice(-50), get().qIndex] });
    get().playQueueIndex(i);
  },
  setEq(gains, enabled) {
    set({ eqGains: [...gains], eqEnabled: enabled });
    // 拖动滑块每个中间值都会调到这里：防抖 150ms 只发最后一帧，
    // 避免一次拖动发几十次 set_eq IPC（后端每帧都要重算滤波器）
    if (eqTimer) clearTimeout(eqTimer);
    eqTimer = setTimeout(() => {
      eqTimer = null;
      api.setEq(gains, enabled).catch(() => {});
    }, 150);
  },
  // ---------- 歌词 ----------

  async loadLyricsByKey(key) {
    // 同 key 且已在加载/已加载：跳过（防止 pos 事件高频触发重复请求）；
    // 不同 key 之间的来回切换由 lyricsFor 标识丢弃过期响应
    if (get().lyricsFor === key) return;
    set({ lyricsLoading: true, lyricsFor: key, lyrics: null });
    const [kind, ...rest] = key.split("-");
    const id = rest.join("-");
    try {
      const payload =
        kind === "net"
          ? await api.neteaseLyric(Number(id))
          : kind === "qq"
            ? await api.qqLyric(id)
            : kind === "kug"
              ? await api.kugouLyric(id)
              : kind === "nd"
                ? await api.navidromeLyric(id)
                : kind === "bili"
                  ? await api.biliLyric(id)
                  : await api.getLyrics(Number(id));
      if (get().lyricsFor === key) {
        set({ lyrics: payload, lyricsLoading: false });
        pushDesktopLyrics(get());
      }
    } catch {
      // lyricsFor 一并清空：留着 key 会把后续所有重试拦在
      // "if (get().lyricsFor === key) return" 上——一次网络抖动，
      // 这首歌的歌词直到切歌都出不来
      if (get().lyricsFor === key)
        set({ lyricsLoading: false, lyrics: null, lyricsFor: null });
    }
  },

  async loadLyrics(trackId) {
    get().loadLyricsByKey(`track-${trackId}`);
  },

  applyMediaControl(action, value) {
    const s = get();
    switch (action) {
      case "play":
        if (!s.playing) api.resume().catch(() => {});
        break;
      case "pause":
        if (s.playing) api.pause().catch(() => {});
        break;
      case "toggle":
        s.togglePlay();
        break;
      case "next":
        s.next(false);
        break;
      case "prev":
        s.prev();
        break;
      case "stop":
        api.stop().catch(() => {});
        break;
      case "seek_fwd":
        s.seek(Math.min(s.pos + 10000, s.dur || s.pos + 10000));
        break;
      case "seek_back":
        s.seek(Math.max(0, s.pos - 10000));
        break;
      case "set_pos":
        if (value != null) s.seek(value);
        break;
      case "show":
        break;
    }
  },
});
