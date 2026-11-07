// 开发预览专用：在纯浏览器里模拟 Tauri IPC，让 UI 能脱离后端渲染。
// 仅用于界面设计预览（vite 打开 preview.html），不参与应用构建/打包。

const TRACK_TITLES: [string, string, string, number][] = [
  ["Midnight Harbor", "Aria Chen", "City Lights", 214],
  ["Glass Cathedral", "Northern Lights", "Refraction", 257],
  ["Velvet Rain", "The Blue Hour", "Nocturne", 198],
  ["Paper Planes Home", "Ivy & Oak", "Small Hours", 236],
  ["Golden Hour Drive", "Sunset Express", "Highway Songs", 189],
  ["Static Bloom", "Neon Garden", "Chlorophyll", 244],
  ["Slow Tide", "Marina Vault", "Undertow", 301],
  ["Amber Static", "Field Notes", "Longwave", 175],
  ["Cirrus", "Altitude", "Weather Systems", 268],
  ["Low Light Quarter", "The Blue Hour", "Nocturne", 223],
  ["Neon Monsoon", "City Lights", "Chlorophyll", 210],
  ["Salt & Cedar", "Harbor Voices", "Driftwood", 256],
  ["Winter Bells", "Yuki Tanaka", "First Snow", 187],
  ["Echo Park '89", "Synth Drive", "Retrograde", 232],
  ["Copper Sky", "Sunset Express", "Highway Songs", 245],
];

const tracks = TRACK_TITLES.map(([title, artist, album, dur], i) => ({
  id: i + 1,
  path: `C:/Music/${artist}/${album}/${String(i + 1).padStart(2, "0")} ${title}.flac`,
  title,
  artist,
  album,
  albumArtist: artist,
  trackNo: i + 1,
  disc: 1,
  year: 2019 + (i % 6),
  duration: dur,
  format: "flac",
  bitrate: 986,
  sampleRate: 44100,
  bitDepth: 16,
  cover: "",
  hasLrc: true,
  size: dur * 120_000,
  mtime: Math.floor(Date.now() / 1000) - i * 86_400,
  liked: i % 4 === 0,
  playCount: (i * 7) % 31,
  lastPlayed: Math.floor(Date.now() / 1000) - i * 3_600,
  likedAt: i % 4 === 0 ? Math.floor(Date.now() / 1000) - i * 86_400 : 0,
  missing: false,
}));

const playlists = [
  { id: 1, name: "深夜专注", trackIds: [1, 2, 3, 4], entries: [], cover: "", createdAt: Date.now() },
  { id: 2, name: "通勤快歌", trackIds: [5, 6, 7], entries: [], cover: "", createdAt: Date.now() },
  { id: 3, name: "雨天爵士", trackIds: [8, 9], entries: [], cover: "", createdAt: Date.now() },
];

const settings = {
  volume: 0.72,
  speed: 1,
  eqGains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  eqEnabled: false,
  quality: "lossless",
  cacheLimit: 2 * 1024 * 1024 * 1024,
  closeAction: "tray" as const,
  autoUpdate: false,
  wasapiExclusive: false,
};

const nowPlaying = {
  id: 1,
  kind: "track",
  path: tracks[0].path,
  title: tracks[0].title,
  artist: tracks[0].artist,
  album: tracks[0].album,
  cover: "",
  durationMs: tracks[0].duration * 1000,
  nid: null,
  qid: null,
  kgid: null,
  quality: "FLAC",
  playing: true,
  pos: 47_000,
  seq: 1,
};

export function installMockTauri() {
  const w = window as unknown as Record<string, unknown>;

  w.__TAURI_INTERNALS__ = {
    transformCallback: (cb: unknown) => {
      const id = Math.floor(Math.random() * 1e9);
      w[`_${id}`] = cb;
      return id;
    },
    convertFileSrc: (filePath: string) => filePath,
    metadata: { currentWindow: { label: "main" }, currentWebview: {} },
    plugins: {},
    async invoke(cmd: string, args?: Record<string, unknown>) {
      if (cmd === "plugin:event|listen") {
        const handler = (args as { handler?: number } | undefined)?.handler;
        if (handler != null) {
          setTimeout(() => {
            const cb = w[`_${handler}`] as
              | ((p: unknown) => void)
              | undefined;
            cb?.({ event: "webview://resumed", id: 0, payload: null });
          }, 60);
        }
        return 0;
      }
      switch (cmd) {
        case "get_settings":
          return settings;
        case "list_tracks":
          return tracks;
        case "list_folders":
          return [{ id: 1, path: "C:/Music" }];
        case "list_playlists":
          return playlists;
        case "list_sources":
          return [];
        case "netease_status":
        case "qq_status":
        case "bilibili_status":
        case "kugou_status":
          return { loggedIn: false, nickname: "" };
        case "get_play_state":
          return nowPlaying;
        case "get_scan_state":
          return { active: false, done: 0, total: 0 };
        case "desktop_lyrics_is_open":
          return false;
        case "liked_online_list":
        case "recent_online_list":
        case "downloaded_online_list":
          return [];
        case "cache_stats":
          return { bytes: 0, files: 0 };
        case "auto_check_update":
        case "check_update":
          return null;
        case "get_app_info":
          return { version: "0.7.1", dataDir: "C:/dev-preview", platform: "windows" };
        case "list_output_devices":
          return { devices: [{ name: "Speakers (Mock)", isDefault: true }], current: "Speakers (Mock)", preference: null };
        case "taskbar_lyrics_list_displays":
          return [{ id: "MONITOR-1", name: "Display 1", primary: true, has_taskbar: true }];
        case "get_lyrics":
          return {
            synced: false,
            lines: [],
          };
        default:
          // 其余命令一律安全空返回：预览环境不产生副作用
          return null;
      }
    },
  };
}
