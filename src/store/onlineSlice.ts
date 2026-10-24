//! 在线音源域：网易/QQ/酷狗/B 站/Navidrome 的搜索、登录态、元数据缓存、
//! 收藏/下载/最近播放、歌单导入，以及各平台的入队播放入口。
import type { StateCreator } from "zustand";
import { api } from "../api";
import type { BiliFollow, KgSong, NeteaseTrack, NdSong, QqSong, QueueItem } from "../types";
import type { Store } from "./contract";
import { capOnlineCache, pinnedIdsOf } from "./onlineCache";
import { resetVipFailStreak } from "./playerSlice";
import { jsonEq } from "./util";

type Fields = Pick<
  Store,
  | "neteaseResults"
  | "neteaseTotal"
  | "neteaseSearching"
  | "neteaseSearched"
  | "neteaseLoggedIn"
  | "neteaseNickname"
  | "neteaseCache"
  | "neteaseLiked"
  | "qqResults"
  | "qqSearching"
  | "qqSearched"
  | "qqPage"
  | "qqLoggedIn"
  | "qqNickname"
  | "qqCache"
  | "kugouResults"
  | "kugouSearching"
  | "kugouSearched"
  | "kugouPage"
  | "kugouCache"
  | "kugouLoggedIn"
  | "kugouNickname"
  | "biliCache"
  | "ndCache"
  | "biliLoggedIn"
  | "biliNickname"
  | "biliFollows"
  | "savedOnline"
  | "likedOnline"
  | "recentOnline"
  | "loadMoreLock"
  | "biliToggleFollow"
  | "biliReorderFollows"
  | "biliSetLogin"
  | "biliLogout"
  | "playBilibili"
  | "playBilibiliList"
  | "biliSpace"
  | "biliSpaceMore"
  | "biliSpaceCollection"
  | "biliSpaceCollectionMore"
  | "biliFavFolders"
  | "biliVideoInfo"
  | "neteaseSearch"
  | "neteaseRefreshStatus"
  | "neteaseSetLogin"
  | "neteaseSyncLikes"
  | "neteaseToggleLike"
  | "neteaseLogout"
  | "qqSearch"
  | "qqRefreshStatus"
  | "qqSetLogin"
  | "qqLogout"
  | "kugouSearch"
  | "kugouRefreshStatus"
  | "kugouSetLogin"
  | "kugouLogout"
  | "playNetease"
  | "playNdList"
  | "playQq"
  | "playKugou"
  | "playSourceId"
  | "toggleLikeOnline"
  | "downloadOnline"
  | "cancelDownloadQueue"
  | "refreshLikedOnline"
  | "refreshRecentOnline"
  | "importKugouPlaylist"
  | "importNeteasePlaylist"
  | "importQqPlaylist"
  | "importOnline"
  | "importAllPlaylists"
>;

/** 搜索请求代次（按源）：新请求使同源在途旧响应作废，防止旧响应后到覆盖新结果 */
const searchGen: Record<"netease" | "qq" | "kugou", number> = {
  netease: 0,
  qq: 0,
  kugou: 0,
};
/** 网易云搜索已请求条数（去重后 results.length 会小于服务端已消费数，
 *  直接用 length 当 offset 会基准漂移、反复拉同一批重复条目） */
let neteaseReqCount = 0;

/** 分页追加按 id 去重：服务端结果集在两次请求间变动会返回重叠条目 */
function dedupeAppend<T extends { id: string | number }>(old: T[], add: T[]): T[] {
  const seen = new Set(old.map((t) => String(t.id)));
  const fresh = add.filter((t) => !seen.has(String(t.id)));
  return fresh.length ? [...old, ...fresh] : old;
}

/** 加载更多进行中标记（按源隔离：三源共享一把锁会让别源的 append 互相误清/重入） */
const loadMoreBusy: Record<"netease" | "qq" | "kugou", boolean> = {
  netease: false,
  qq: false,
  kugou: false,
};
/** 下载串行队列：download://progress 是单通道事件、download 是单槽状态，
 *  并发下载会互抢进度条，且同名文件并发写会损坏——排成队依次执行。
 *  显式队列（而非 promise 链）：排队中的任务可整队取消（批量误选后不必逐个等）。
 *  每项带 abort：取消时把等待方立即 settle（false），Promise 不悬空挂起 */
const downloadQueue: Array<{ task: () => Promise<void>; abort: () => void }> = [];
let downloadRunning = false;
function pumpDownloadQueue() {
  if (downloadRunning) return;
  const next = downloadQueue.shift();
  if (!next) return;
  downloadRunning = true;
  void next.task().finally(() => {
    downloadRunning = false;
    pumpDownloadQueue();
  });
}
/** refreshLikedOnline 合并器（批量收藏并发触发时防旧快照覆盖新状态） */
let likedRefreshInFlight = false;
let likedRefreshDirty = false;

/** 收藏 UP 主的 localStorage 键 */
const BILI_FOLLOWS_KEY = "rustmusic.biliFollows";

function loadBiliFollows(): BiliFollow[] {
  try {
    const raw = localStorage.getItem(BILI_FOLLOWS_KEY);
    const arr = raw ? JSON.parse(raw) : [];
    return Array.isArray(arr)
      ? arr.filter(
          (x): x is BiliFollow =>
            x != null && typeof x.mid === "string" && typeof x.name === "string"
        )
      : [];
  } catch {
    return [];
  }
}

function saveBiliFollows(list: BiliFollow[]) {
  try {
    localStorage.setItem(BILI_FOLLOWS_KEY, JSON.stringify(list));
  } catch {
    // 存储失败仅影响下次启动的记忆，运行时状态不受影响
  }
}

export const createOnlineSlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  neteaseResults: [],
  neteaseTotal: 0,
  neteaseSearching: false,
  neteaseSearched: false,
  neteaseLoggedIn: false,
  neteaseNickname: "",
  neteaseCache: {},

  qqResults: [],
  qqSearching: false,
  qqSearched: false,
  qqPage: 1,
  qqLoggedIn: false,
  qqNickname: "",
  qqCache: {},

  kugouResults: [],
  kugouSearching: false,
  kugouSearched: false,
  kugouPage: 1,
  kugouCache: {},
  kugouLoggedIn: false,
  kugouNickname: "",

  biliCache: {},
  ndCache: {},
  biliLoggedIn: false,
  biliNickname: "",
  biliFollows: loadBiliFollows(),
  savedOnline: {},
  likedOnline: [],
  recentOnline: [],
  loadMoreLock: false,
  neteaseLiked: {},
  playNetease(list: NeteaseTrack[], idx: number) {
    if (!list.length) return;
    const cache = { ...get().neteaseCache };
    for (const t of list) cache[t.id] = t;
    const queue: QueueItem[] = list.map((t) => ({ kind: "netease", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      neteaseCache: capOnlineCache(
        "netease", cache, get().neteaseCache,
        pinnedIdsOf("netease", { queue, likedOnline: get().likedOnline })
      ),
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
      queueExtender: null,
    }));
    get().playQueueIndex(target);
  },

  playNdList(songs: NdSong[], idx: number) {
    if (!songs.length) return;
    const cache = { ...get().ndCache };
    for (const t of songs) {
      cache[t.id] = {
        title: t.title,
        artist: t.artist,
        album: t.album,
        cover: t.coverUrl,
        durationMs: t.duration * 1000,
      };
    }
    const queue: QueueItem[] = songs.map((t) => ({ kind: "navidrome", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      ndCache: capOnlineCache(
        "nd", cache, get().ndCache,
        pinnedIdsOf("navidrome", { queue, likedOnline: get().likedOnline })
      ),
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
      queueExtender: null,
    }));
    get().playQueueIndex(target);
  },
  biliSetLogin(loggedIn, nickname) {
    set({ biliLoggedIn: loggedIn, biliNickname: nickname });
  },

  biliToggleFollow(up) {
    const list = get().biliFollows;
    const exists = list.some((f) => f.mid === up.mid);
    const next = exists
      ? list.filter((f) => f.mid !== up.mid)
      : [...list, { mid: up.mid, name: up.name, face: up.face }];
    set({ biliFollows: next });
    saveBiliFollows(next);
    get().toast(
      exists ? `已取消收藏 UP主「${up.name}」` : `已收藏 UP主「${up.name}」`,
      exists ? "info" : "success"
    );
  },

  biliReorderFollows(from, to) {
    if (from === to) return;
    const list = [...get().biliFollows];
    if (from < 0 || from >= list.length) return;
    const [moved] = list.splice(from, 1);
    // to 为移除源后的最终下标（组件按被拖头像中心所在槽位折算好），直接落位
    const toIdx = Math.max(0, Math.min(list.length, to));
    list.splice(toIdx, 0, moved);
    set({ biliFollows: list });
    saveBiliFollows(list);
  },
  playBilibili(row) {
    get().playBilibiliList([row], 0);
  },

  playBilibiliList(rows, idx) {
    if (!rows.length) return;
    const cache = { ...get().biliCache };
    for (const r of rows) {
      cache[r.rid] = {
        rid: r.rid,
        title: r.title,
        artist: r.artist,
        album: "哔哩哔哩",
        cover: r.cover,
        durationMs: r.durationMs,
      };
    }
    const queue: QueueItem[] = rows.map((r) => ({ kind: "bilibili", id: r.rid }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      biliCache: capOnlineCache(
        "bili", cache, get().biliCache,
        pinnedIdsOf("bilibili", { queue, likedOnline: get().likedOnline })
      ),
      playingSourceId: null,
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
      queueExtender: null,
    }));
    get().playQueueIndex(target);
  },

  playSourceId(id) {
    set((s) => ({
      playingSourceId: id,
      queue: [{ kind: "url", id }],
      qIndex: 0,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
    }));
    api.playSource(id).catch((e) => get().toast(`播放音源失败：${e}`, "error"));
  },
  async biliSpace(input, order) {
    return api.biliSpace(input, order);
  },

  biliSpaceMore(mid, order, pn) {
    return api.biliSpaceMore(mid, order, pn);
  },

  biliSpaceCollection(mid, id, kind) {
    return api.biliSpaceCollection(mid, id, kind);
  },

  biliSpaceCollectionMore(mid, id, kind, pn) {
    return api.biliSpaceCollectionMore(mid, id, kind, pn);
  },

  biliFavFolders() {
    return api.biliFavFolders();
  },

  biliVideoInfo(input) {
    return api.biliVideoInfo(input);
  },

  async biliLogout() {
    try {
      await api.biliLogout();
      set({ biliLoggedIn: false, biliNickname: "" });
      get().toast("已退出 B 站登录", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },
  // ---------- 网易云 ----------

  playQq(list, idx) {
    if (!list.length) return;
    const cache = { ...get().qqCache };
    for (const t of list) cache[t.id] = t;
    const queue: QueueItem[] = list.map((t) => ({ kind: "qq", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      qqCache: capOnlineCache(
        "qq", cache, get().qqCache,
        pinnedIdsOf("qq", { queue, likedOnline: get().likedOnline })
      ),
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
      queueExtender: null,
    }));
    get().playQueueIndex(target);
  },

  playKugou(list: KgSong[], idx: number) {
    if (!list.length) return;
    const cache = { ...get().kugouCache };
    for (const t of list) cache[t.id] = t;
    const queue: QueueItem[] = list.map((t) => ({ kind: "kugou", id: t.id }));
    const target = Math.max(0, Math.min(idx, queue.length - 1));
    set((s) => ({
      kugouCache: capOnlineCache(
        "kugou", cache, get().kugouCache,
        pinnedIdsOf("kugou", { queue, likedOnline: get().likedOnline })
      ),
      queue,
      qIndex: target,
      history: [...s.history.slice(-50), s.qIndex],
      failStreak: 0,
      failToastId: null,
      queueExtender: null,
    }));
    get().playQueueIndex(target);
  },
  async qqSearch(kw, append = false) {
    const keyword = kw.trim();
    if (!keyword) return;
    const gen = ++searchGen.qq;
    if (append) {
      if (loadMoreBusy.qq) return;
      loadMoreBusy.qq = true;
    } else {
      // 新搜索会重置结果：同源在途 append 一并作废并解锁。
      // 页码/结果立即原子复位——否则新搜索在途时触发 append 会用旧页码发请求
      loadMoreBusy.qq = false;
    }
    set(
      append
        ? { qqSearching: true }
        : { qqSearching: true, qqSearched: true, qqResults: [], qqPage: 1 }
    );
    try {
      const page = append ? get().qqPage + 1 : 1;
      const r = await api.qqSearch(keyword, page);
      if (gen !== searchGen.qq) return; // 已有更新的请求：丢弃旧响应
      const cache = { ...get().qqCache };
      for (const t of r.songs) cache[t.id] = t;
      set((s) => ({
        // 追加按 id 去重：分页接口两次请求间数据变动会返回重叠条目
        // （同 key 双行 + React 行复用错位）
        qqResults: append ? dedupeAppend(s.qqResults, r.songs) : r.songs,
        qqSearching: false,
        qqPage: page,
        qqCache: capOnlineCache("qq", cache, get().qqCache, pinnedIdsOf("qq", get())),
      }));
    } catch (e) {
      if (gen === searchGen.qq) {
        set({ qqSearching: false });
        get().toast(String(e), "error");
      }
    } finally {
      if (append && gen === searchGen.qq) loadMoreBusy.qq = false;
    }
  },

  async kugouSearch(kw, append = false) {
    const keyword = kw.trim();
    if (!keyword) return;
    const gen = ++searchGen.kugou;
    if (append) {
      if (loadMoreBusy.kugou) return;
      loadMoreBusy.kugou = true;
    } else {
      loadMoreBusy.kugou = false;
    }
    set(
      append
        ? { kugouSearching: true }
        : { kugouSearching: true, kugouSearched: true, kugouResults: [], kugouPage: 1 }
    );
    try {
      const page = append ? get().kugouPage + 1 : 1;
      const r = await api.kugouSearch(keyword, page);
      if (gen !== searchGen.kugou) return;
      const cache = { ...get().kugouCache };
      for (const t of r.songs) cache[t.id] = t;
      set((s) => ({
        kugouResults: append ? dedupeAppend(s.kugouResults, r.songs) : r.songs,
        kugouSearching: false,
        kugouPage: page,
        kugouCache: capOnlineCache("kugou", cache, get().kugouCache, pinnedIdsOf("kugou", get())),
      }));
    } catch (e) {
      if (gen === searchGen.kugou) {
        set({ kugouSearching: false });
        get().toast(String(e), "error");
      }
    } finally {
      if (append && gen === searchGen.kugou) loadMoreBusy.kugou = false;
    }
  },

  async kugouRefreshStatus() {
    try {
      const s = await api.kugouStatus();
      set({ kugouLoggedIn: s.loggedIn, kugouNickname: s.nickname });
    } catch {}
  },

  kugouSetLogin(loggedIn, nickname) {
    // 登录/退出都意味着账号状态变化：清零 VIP 熔断计数
    resetVipFailStreak();
    set((s) => {
      const unavailable = Object.fromEntries(
        Object.entries(s.unavailable).filter(([k]) => !k.startsWith("kugou:"))
      );
      return { kugouLoggedIn: loggedIn, kugouNickname: nickname, unavailable };
    });
  },

  async kugouLogout() {
    try {
      await api.kugouLogout();
    } catch {}
    get().kugouSetLogin(false, "");
  },

  async qqRefreshStatus() {
    try {
      const s = await api.qqStatus();
      set({ qqLoggedIn: s.loggedIn, qqNickname: s.nickname });
    } catch {}
  },

  qqSetLogin(loggedIn, nickname) {
    // 登录/退出都意味着账号状态变化：清零 VIP 熔断计数
    resetVipFailStreak();
    set((s) => {
      // 登录/退出都会改变曲目可用性：清除之前会话状态下做出的置灰标记，
      // 让重新登录后的 VIP 曲目得以重试（修复"过期误标后永远不能播"）
      const unavailable = Object.fromEntries(
        Object.entries(s.unavailable).filter(([k]) => !k.startsWith("qq:"))
      );
      return { qqLoggedIn: loggedIn, qqNickname: nickname, unavailable };
    });
  },

  async qqLogout() {
    try {
      await api.qqLogout();
      // 走 SetLogin 包装：退出登录同样改变曲目可用性，需清掉 qq: 前缀的
      // 置灰标记（与 kugouLogout 行为一致；直接 set 曾让标记残留到下次登录）
      get().qqSetLogin(false, "");
      get().toast("已退出 QQ 音乐登录", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },
  async toggleLikeOnline(row) {
    const key = `${row.kind}-${row.id}`;
    const next = !get().savedOnline[key];
    // 乐观更新
    set((s) => ({ savedOnline: { ...s.savedOnline, [key]: next } }));
    try {
      await api.likeOnline({
        kind: row.kind,
        rid: String(row.id),
        title: row.name,
        artist: row.artist,
        album: row.album,
        cover: row.cover,
        durationMs: row.durationMs,
        mediaMid: row.mediaMid ?? "",
        vip: row.vip ?? false,
        like: next,
      });
      await get().refreshLikedOnline();
    } catch (e) {
      // 回滚仅当当前值仍是本次的乐观值：并发二次点击已成功时，
      // 无条件回滚会把正确状态改错且再无纠正路径
      set((s) =>
        s.savedOnline[key] === next
          ? { savedOnline: { ...s.savedOnline, [key]: !next } }
          : {}
      );
      get().toast(String(e), "error");
    }
  },

  downloadOnline(row) {
    // 返回是否成功：批量下载靠它统计失败数（错误已 toast，不 reject）
    const run = async (): Promise<boolean> => {
      get().toast(`开始下载「${row.name}」…`, "info");
      try {
        const r = await api.downloadOnline({
          kind: row.kind,
          id: String(row.id),
          title: row.name,
          artist: row.artist,
          album: row.album,
          coverUrl: row.cover,
          durationMs: row.durationMs,
          mediaMid: row.mediaMid ?? "",
          albumAudioId: (row as { albumAudioId?: number }).albumAudioId,
          hqHash: (row as { hqHash?: string }).hqHash,
          sqHash: (row as { sqHash?: string }).sqHash,
          superHash: (row as { superHash?: string }).superHash,
        });
        await get().refreshTracks();
        get().toast(
          `已下载到资料库：${r.name}${r.quality ? `（${r.quality}）` : ""}`,
          "success"
        );
        return true;
      } catch (e) {
        get().toast(String(e), "error");
        return false;
      }
    };
    // 入串行队列：批量下载逐个执行，进度条/文件写入不再互相干扰；
    // 排队中的任务可被 cancelDownloadQueue 整队取消（批量误选后不必逐个等）
    return new Promise<boolean>((resolve) => {
      const task = () => run().then((ok) => resolve(ok), () => resolve(false));
      downloadQueue.push({ task, abort: () => resolve(false) });
      pumpDownloadQueue();
    });
  },

  cancelDownloadQueue() {
    const pending = downloadQueue.splice(0, downloadQueue.length);
    for (const p of pending) p.abort();
    if (pending.length > 0) {
      get().toast(`已取消排队中的 ${pending.length} 个下载`, "info");
    }
    return pending.length;
  },
  async refreshLikedOnline() {
    // 批量收藏会并发触发 N 次本方法：并发快照乱序返回时，旧的会覆盖
    // 新的 savedOnline（已成功的红心消失）。改为单飞行 + 脏标记：
    // 飞行中的调用只标脏，循环保证最后一次刷新拿到的是最新全量快照
    if (likedRefreshInFlight) {
      likedRefreshDirty = true;
      return;
    }
    likedRefreshInFlight = true;
    try {
      do {
        likedRefreshDirty = false;
        const list = await api.likedOnlineList();
        const saved: Record<string, boolean> = {};
        for (const e of list) {
          if (e.onlineId) saved[`${e.kind}-${e.onlineId}`] = true;
        }
        set({ likedOnline: list, savedOnline: { ...saved } });
      } while (likedRefreshDirty);
    } catch {
    } finally {
      likedRefreshInFlight = false;
    }
  },

  async refreshRecentOnline() {
    try {
      const recentOnline = await api.recentOnlineList();
      if (!jsonEq(recentOnline, get().recentOnline)) set({ recentOnline });
    } catch {}
  },
  async importNeteasePlaylist(remotePid, name, opts) {
    return get().importOnline("netease", remotePid, name, opts);
  },

  async importQqPlaylist(remotePid, name, opts) {
    return get().importOnline("qq", remotePid, name, opts);
  },

  async importKugouPlaylist(remotePid: string, name: string, opts) {
    return get().importOnline("kugou", remotePid, name, opts);
  },

  async importOnline(source, remotePid, name, opts) {
    const quiet = opts?.quiet ?? false;
    try {
      // 合并语义：同名/同远程 id 的已有列表直接补新歌（去重、不动顺序），
      // 没有才新建——由后端统一判断
      const [, added] =
        source === "netease"
          ? await api.neteaseImportPlaylist(remotePid as number, name)
          : source === "qq"
            ? await api.qqImportPlaylist(remotePid as number, name)
            : await api.kugouImportPlaylist(String(remotePid), name);
      if (!quiet) {
        await get().refreshPlaylists();
        get().toast(
          added > 0
            ? `已同步「${name}」：新增 ${added} 首`
            : `「${name}」没有新歌需要同步`,
          "success"
        );
      }
      return added;
    } catch (e) {
      if (!quiet) get().toast(String(e), "error");
      return null;
    }
  },

  async importAllPlaylists(source, list, onProgress) {
    let added = 0;
    let failed = 0;
    for (let i = 0; i < list.length; i++) {
      const p = list[i];
      onProgress?.({ done: i, total: list.length, name: p.name });
      const r =
        source === "netease"
          ? await get().importNeteasePlaylist(p.id as number, p.name, { quiet: true })
          : source === "qq"
            ? await get().importQqPlaylist(p.id as number, p.name, { quiet: true })
            : await get().importKugouPlaylist(String(p.id), p.name, { quiet: true });
      if (r == null) failed++;
      else added += r;
    }
    await get().refreshPlaylists();
    if (failed === 0) {
      get().toast(
        added > 0
          ? `已导入全部 ${list.length} 个歌单：新增 ${added} 首`
          : `${list.length} 个歌单均已同步，没有新歌`,
        "success"
      );
    } else {
      get().toast(
        `已导入 ${list.length - failed}/${list.length} 个歌单（新增 ${added} 首），${failed} 个失败`,
        failed >= list.length ? "error" : "info"
      );
    }
    return { added, failed };
  },
  async neteaseSearch(kw, append = false) {
    const keyword = kw.trim();
    if (!keyword) return;
    const gen = ++searchGen.netease;
    if (append) {
      if (loadMoreBusy.netease) return;
      loadMoreBusy.netease = true;
    } else {
      loadMoreBusy.netease = false;
    }
    set(
      append
        ? { neteaseSearching: true }
        : { neteaseSearching: true, neteaseSearched: true, neteaseResults: [], neteaseTotal: 0 }
    );
    if (!append) neteaseReqCount = 0;
    try {
      const offset = append ? neteaseReqCount : 0;
      const r = await api.neteaseSearch(keyword, offset);
      if (gen !== searchGen.netease) return;
      const cache = { ...get().neteaseCache };
      for (const t of r.songs) cache[t.id] = t;
      set((s) => ({
        // 按 offset 翻页在服务端结果集变动时必出重复条目，追加按 id 去重
        neteaseResults: append ? dedupeAppend(s.neteaseResults, r.songs) : r.songs,
        neteaseTotal: r.total,
        neteaseSearching: false,
        // 计数用服务端返回的原始条数（去重前的已消费量）
        neteaseCache: capOnlineCache("netease", cache, get().neteaseCache, pinnedIdsOf("netease", get())),
      }));
      neteaseReqCount += r.songs.length;
    } catch (e) {
      if (gen === searchGen.netease) {
        set({ neteaseSearching: false });
        get().toast(String(e), "error");
      }
    } finally {
      if (append && gen === searchGen.netease) loadMoreBusy.netease = false;
    }
  },

  async neteaseRefreshStatus() {
    try {
      const s = await api.neteaseStatus();
      set({ neteaseLoggedIn: s.loggedIn, neteaseNickname: s.nickname });
    } catch {}
  },

  neteaseSetLogin(loggedIn, nickname) {
    // 登录/退出都意味着账号状态变化：清零 VIP 熔断计数
    resetVipFailStreak();
    set((s) => {
      const unavailable = Object.fromEntries(
        Object.entries(s.unavailable).filter(([k]) => !k.startsWith("netease:"))
      );
      return { neteaseLoggedIn: loggedIn, neteaseNickname: nickname, unavailable };
    });
  },

  async neteaseSyncLikes() {
    if (!get().neteaseLoggedIn) return;
    try {
      const ids = await api.neteaseLikeList();
      const liked: Record<number, boolean> = {};
      for (const id of ids) liked[id] = true;
      set({ neteaseLiked: liked });
    } catch {
      /* 静默：下次启动再同步 */
    }
  },

  neteaseToggleLike(id) {
    const likedState = get().neteaseLiked;
    const next = !likedState[id];
    set({ neteaseLiked: { ...likedState, [id]: next } });
    api
      .neteaseLike(id, next)
      .then(() =>
        get().toast(next ? "已收藏到账号的“我喜欢”" : "已取消收藏", "success")
      )
      .catch((e) => {
        // 回滚
        const cur = { ...get().neteaseLiked };
        if (next) delete cur[id];
        else cur[id] = true;
        set({ neteaseLiked: cur });
        get().toast(String(e), "error");
      });
  },

  async neteaseLogout() {
    try {
      await api.neteaseLogout();
      get().neteaseSetLogin(false, "");
      get().toast("已退出网易云登录", "success");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },
});
