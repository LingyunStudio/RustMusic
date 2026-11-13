**Language / 语言**: [简体中文](README.md) | [English](README.en.md)

# RustMusic 🎵

A high-performance, crafted desktop music player built with **Rust + Tauri 2 + React**.

![Rust](https://img.shields.io/badge/Rust-1.87%2B-DEA584?logo=rust&logoColor=white) ![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white) ![React](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=black) ![Node](https://img.shields.io/badge/Node-20%2B-5FA04E?logo=nodedotjs&logoColor=white) ![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0078D6)

![image-20261023115804740](https://cdn.jsdelivr.net/gh/LingyunStudio/LingyunImg@master/2026/10/upgit_20261023_1792727886.png)

## ✨ Features

### Playback Engine (Pure Rust)

- **Full-Format Decoding**: MP3 / FLAC / WAV / OGG (Vorbis) / M4A (AAC) / AAC / ALAC (Apple Lossless), plus MP4 / M4B containers (detected by file header with automatic selection of decodable audio tracks), decoded in pure Rust via `symphonia` with no FFmpeg dependency.
- **Playback Controls**: Play / Pause / Previous / Next, arbitrary timeline seeking, ±5s skip, volume, playback speed (5 presets from 0.75x to 2x), and sleep timer.
- **Play Modes**: Sequential / Repeat All / Repeat One / Shuffle.
- **Audio Output Device**: Select a specific output device or follow the system default device automatically (switches seamlessly when headphones are plugged/unplugged).
- **WASAPI Exclusive Mode** (optional, disabled by default, Windows only): Bypasses the Windows system mixer for direct hardware output at the source file's native sample rate without system resampling or effects, preserving original fidelity. Supported uniformly for both local and online tracks (including Bilibili DASH streams); seek, playback speed, and device switching take effect immediately inside the exclusive session. Requires audio driver support (Bluetooth / network speakers typically do not support exclusive mode and fall back to shared mode with an explanation). Disabling exclusive mode immediately releases the device back to other apps. Automatically hidden on Linux (where PipeWire/PulseAudio manages mixing).
- **10-Band Equalizer**: Real-time DSP (RBJ biquad filters) with built-in Pop / Rock / Classical / Jazz / Electronic / Vocal presets and full custom tuning.
- **Play Queue**: Play Next, Add to Queue, jump to track, remove single tracks, clear queue; automatically loads and appends the next page when reaching the end of paginated NetEase / QQ / Kugou, Bilibili, or Navidrome lists.
- **Shortcuts**: `Space` Play / Pause, `←` / `→` Seek ±5s (hold to step continuously), `L` Toggle / Unlock Desktop Lyrics, `Esc` Exit Now Playing view.

### Music Sources

- **NetEase Cloud Music**: Built-in NetEase Cloud API (`weapi` protocol). Instant search and playback; scan QR code to sign in with your own account and stream according to your account entitlements (standard quality for free tracks / VIP quality for subscribed accounts; credentials stored locally only). Supports Top Charts, Playlists ("Discover"), Daily Mix, Personal FM, playlist import, and "Favorites" sync.
- **QQ Music**: Search / Playback / Top Charts / Playlists ("Discover") / Playlist Import / QR Sign-In. Free tracks play anonymously; VIP tracks resolve according to your signed-in account entitlements.
- **Kugou Music**: Search / Top Charts (TOP500, etc.) / Playlists ("Discover") / Playlist Import; QR sign-in for VIP entitlements, supporting Lossless (FLAC) and Hi-Res audio quality.
- **Navidrome (Subsonic API)**: Connect to self-hosted servers (primarily targeting Navidrome) for Album browsing / All Songs / Search / Lyrics (prefers OpenSubsonic synchronized lyrics with automatic fallback to legacy endpoints) / Download; passwords are stored securely in the OS credential store; view state is preserved across tab switches without reconnecting.
- **Bilibili**: Paste a video URL / BV ID / `b23.tv` short link to extract audio; browse creator spaces (sort uploads, collections & series) and your signed-in favorite folders; load video subtitles as lyrics; download tracks to your library; QR sign-in.
- **Local Library**: Multi-folder parallel scanning, SQLite storage, incremental updates (skips unchanged files by `mtime`/`size`), and automatic cleanup of missing files; supports deleting files directly from disk (with confirmation).
- **Metadata Tags**: Parses ID3v2 / FLAC / MP4 tags (title, artist, album, year, track number) with smart filename fallback when tags are missing.
- **Cover Art**: Extracts embedded cover art with compressed caching (uses compact thumbnails in lists to reduce memory footprint), showing dynamic gradient placeholders when unavailable.
- **Online Sources (Direct URL)**: Add any direct HTTP/HTTPS audio stream URL for playback (`m3u8` / HLS not yet supported).
- **Unified Playback Pipeline**: All online sources share the same stream-while-downloading cache feeding into the playback engine (supporting WASAPI Exclusive Mode / Equalizer / Playback Speed). Uncached tracks pre-buffer ~512 KB for instant start while downloading in the background, and completed tracks are cached locally with LRU eviction (one-click clear in Settings). Supports Play Next, Add to Queue, Favorite, Add to Playlist, and Batch Download.
- **Streaming Quality**: Configurable in Settings; actual quality depends on account entitlements, with clear notifications showing the delivered quality and reason if downgraded.

### Lyrics

- Supports sidecar `.lrc` files and embedded lyrics tags.
- **Word-by-Word Lyrics**: QQ QRC (encrypted & decrypted), NetEase `yrc`, and Kugou `KRC` are converted into word-level timestamps that highlight syllable-by-syllable during playback; malformed lines are skipped cleanly without discarding the whole track.
- **Lyrics Translation**: Displays official translations beneath each line for NetEase tracks (QQ / Kugou not yet supported).
- **Online Lyrics Cache**: Caches NetEase / QQ / Kugou lyrics (including word-level timing and translations) in the local SQLite database for instant display on replay.
- Full-screen Now Playing view with smooth scrolling, active-line highlight, and click-to-seek; adapts colors dynamically from cover art and displays the active lyrics source badge (Word-by-Word KRC / YRC / QRC / Line LRC / Subtitles).
- **Desktop Lyrics**: Standalone transparent floating overlay with customizable sung / unsung / next-line colors, dragging, resizing, always-on-top, and click-through lock (`L` in main window or tray menu to toggle / unlock).

### Desktop Integration

- **System Media Controls**: Windows uses SMTC (media keys and OS media overlay with title / artist / cover / progress); Linux uses MPRIS (GNOME/KDE media widgets and `playerctl`); macOS integrates with Now Playing and media keys.
- **System Tray**: Tray menu lets you show main window, toggle / unlock desktop lyrics, Play / Pause / Previous / Next, and Quit; left-click restores the main window. Close button can be configured in Settings to minimize to tray or quit immediately.
- **Auto-Updater** (Windows / Linux AppImage; macOS requires manual download): Checks GitHub Releases for updates, selects the matching installer for your platform and CPU architecture (`*setup*.exe` on Windows / `.AppImage` on Linux), downloads in-app with progress and SHA-256 verification, and installs silently; Linux `.deb` installs or missing platform assets guide you to the Releases page.
- **Credential Management**: Login tokens and Navidrome passwords are stored in the OS credential manager (Windows Credential Manager / macOS Keychain / Linux Secret Service via `keyring`).
- **Window**: Borderless custom-drawn titlebar and drag-and-drop file/folder import into the library.

### Interface

- Light / Dark themes + 15 accent colors (including custom picker), frosted glassmorphism in dark mode with dynamic cover-extracted ambient glow; 13 built-in themes/skins (including an audio-reactive Vinyl skin driven by real-time engine FFT spectrum bars, beat ripples, ambient glow, spinning disc, and floating notes) + custom background image skins.
- Sidebar: Library / Favorites / Recently Played / Playlists / NetEase / QQ Music / Kugou / Sources (Bilibili · Navidrome · Direct URL) / Settings (with Simplified Chinese / English UI and automatic system language detection).
- Library search (tracks / artists / albums), per-source search, multi-column sorting, context menus, batch multi-select actions, Play Queue drawer, and full-screen Now Playing view.
- Playlists support batch management (multi-select, select all, batch delete playlists / batch remove tracks).
- Opening the Play Queue automatically scrolls to the current track and follows track changes; a "Locate Current" floating pill appears whenever the playing track scrolls out of view in playlists or online lists.

## 🏗️ Architecture

```
├── index.html               # Main window entry
├── desktop-lyrics.html      # Desktop lyrics overlay entry (Vite multi-page build)
├── .github/workflows/       # CI (3-platform test) and Release (multi-platform packaging)
├── src/                     # React frontend
│   ├── components/          # PlayerBar, NowPlaying, QueuePanel, AudioFxLayer, shared UI
│   ├── views/               # Library / PlaylistDetail / Settings / Sources, etc.
│   │   └── online/          # Online source components (QR login, modals, row menu, track row)
│   ├── desktop-lyrics/      # Desktop lyrics window frontend
│   ├── hooks/               # Shared hooks (drag-to-reorder, virtual window, locate pill)
│   ├── store/               # Zustand store: full contract + domain slices
│   │   └──                  # (player / queue / online / playlist / settings / ui)
│   ├── api.ts               # Tauri invoke / event wrappers
│   ├── fx.ts                # Audio reactive bridge: fx://spectrum listener & backend gate
│   ├── i18n.ts              # Language state & zh/en translation helpers
│   ├── types.ts / utils.ts  # Shared TypeScript types and utilities
│   ├── theme.ts             # Themes and accent colors
│   └── skins.ts             # Built-in skins and custom image skins
├── scripts/icon-tool/       # Icon generator tool (SVG source → master → multi-platform icons)
├── installer/               # Windows packaging: Inno Setup script + build script + release notes
└── src-tauri/               # Rust backend
    ├── examples/            # Debug examples (WASAPI probe, Bilibili audio decode)
    └── src/
        ├── main.rs          # Application wiring (window events, DWM styling, plugins)
        ├── models.rs        # Shared backend/frontend data models
        ├── i18n.rs          # Backend zh/en translation helper & system locale detection
        ├── engine/          # Audio engine
        │   ├── mod.rs       #   Core playback (rodio sink, seek rebuild, state events)
        │   ├── output.rs    #   Output device management (open / hot-swap / enumerate)
        │   ├── exclusive.rs #   WASAPI exclusive session coordinator
        │   └── cache.rs     #   Online stream download cache (stream-on-download, LRU eviction)
        ├── commands/        # Tauri command handlers split by domain (app / library / player /
        │   └──              #   playlist / lyrics / netease / qq / kugou / bilibili / navidrome / source / download / settings / updater)
        ├── webview_suspend.rs # Main WebView2 suspend/resume (TrySuspend memory reclamation)
        ├── tray.rs          # System tray + unified media command forwarding
        ├── monitor.rs       # Playback progress event emitter & output device hot-plug monitor
        ├── eq.rs            # 10-band equalizer (biquad) + position tracking + FX sample tap
        ├── fx.rs            # Audio-reactive skin pipeline: ring buffer → FFT → 36 bands/beat → fx://spectrum
        ├── wasapi_out.rs    # WASAPI exclusive output (format negotiation, alignment recovery, event-driven feed)
        ├── symdec.rs        # Direct symphonia decoder source (MP4/M4A/M4B, Bilibili DASH/fMP4, packet seek)
        ├── streaming.rs     # Progressive streaming cache reader (pre-buffer start while downloading)
        ├── library.rs       # Library scanner (rayon parallel scan, batched transactions) & lofty tags/covers
        ├── lyrics.rs        # LRC / yrc / QRC / KRC → word-synced LRC & merged translations
        ├── db.rs            # SQLite (rusqlite, WAL) persistence + migration tests
        ├── netease.rs       # NetEase API client (weapi, search, stream URL, charts, Daily/FM, QR login)
        ├── qq.rs            # QQ Music API client (search, stream URL, lyrics, charts, playlists, QR login)
        ├── kugou.rs         # Kugou API client (search, charts, playlists, lossless URLs, QR login)
        ├── navidrome.rs     # Navidrome / Subsonic API client (keyring password storage)
        ├── bilibili.rs      # Bilibili API client (video audio, creator space/collections/favorites, subtitles, QR login)
        ├── qrc.rs           # QQ QRC encrypted lyrics decoder → word-synced LRC (from MIT-licensed upstream)
        ├── smtc.rs          # System media controls (souvlaki: Windows SMTC / Linux MPRIS / macOS Control Center)
        └── updater.rs       # GitHub Release auto-updater (download + SHA-256 verification + silent install)
```

**Why Tauri 2**: Rust backend performance + Web-rendered polished UI; cross-platform builds target Windows, macOS (Apple Silicon / Intel), and Linux (x64 / arm64).

### 💡 Key Design Highlight: WebView2 TrySuspend Deep Sleep & Event-Driven Wake (Windows)

> Non-Windows note: WebKit (Linux WebKitGTK / macOS WKWebView) does not expose WebView2's `TrySuspend` API. On Linux and macOS, minimizing to tray simply hides the window while the WebView continues running in the background (all playback and tray controls work identically); only the WebView memory suspension optimization is Windows-specific.

Desktop apps built on hybrid frameworks (Tauri / Electron) commonly face a tradeoff between background playback and memory consumption:

- **Keeping the WebView always active**: Chromium / WebView2 renderer processes stay resident at 100+ MB of RAM;
- **Destroying the WebView**: Recreating the window incurs visible white-flash startup latency and loses frontend state (play queue, history stack, shuffle state) managed in Zustand.

RustMusic implements an **event-driven state wake + delayed re-suspend** architecture:

1. **Deep Freeze & Instant Memory Reclamation**: When the main window minimizes to the system tray, the backend invokes Windows' native `ICoreWebView2_3::TrySuspend` to freeze the renderer process and calls `EmptyWorkingSet` to reclaim physical memory down to single-digit MBs (while pure Rust audio threads continue decoding and playing with zero glitches).
2. **Precision Event-Driven Wake on Track Switch**: Queue ordering and play mode logic live in frontend Zustand. When the Rust monitor detects track completion (or receives a tray / SMTC skip command), `resume_main_webview` wakes the WebView in milliseconds and safely delivers `player://ended` or the media action once the JS runtime unfreezes.
3. **Automatic Delayed Re-Suspend**: Once Zustand dispatches the next track to the backend engine, `schedule_resuspend` quietly waits ~8 seconds of inactivity before re-suspending the WebView, delivering seamless continuous background playback with minimal memory footprint.
4. **Flicker-Free Restore**: When clicking the tray icon to restore the window, WebView2 wakes off-screen and allows ~500 ms for backdrop blur and cover art re-rendering to settle before presenting the window, eliminating white flashes and transition artifacts.
5. **Low-Memory Foreground Footprint**: WebView2 launches with `--memory-usage-target-level=low`; visual skin animations and FX layers run strictly on compositor properties (`transform`/`opacity`, 60 fps with zero layout thrashing), while FFT analysis runs in Rust (2048-point FFT @ 30 Hz, ~0.3% single-core CPU), keeping JS heap resident around ~10 MB.

## 📦 Download & Install

Head to [Releases](https://github.com/LingyunStudio/RustMusic/releases/latest) to download the installer for your platform (see [DOWNLOAD.en.md](DOWNLOAD.en.md) for a detailed guide):

- **Windows** (10/11 x64): `RustMusic_<version>_x64-setup.exe` (Inno Setup installer)
- **macOS** (Apple Silicon / Intel, experimental): `RustMusic_<version>_aarch64.dmg` / `RustMusic_<version>_x64.dmg`
- **Linux** x64: `RustMusic_<version>_amd64.deb` (Debian/Ubuntu) or `RustMusic_<version>_amd64.AppImage` (portable, `chmod +x` to run)
- **Linux** arm64: `RustMusic_<version>_arm64.deb` or `RustMusic_<version>_aarch64.AppImage` (CI builds)

Existing installations receive in-app update prompts (automatically matching your platform/architecture, verifying SHA-256, and silently restarting on Windows and AppImage; Linux `.deb` and macOS link to the Releases page).

> **macOS Note (Experimental / Community Testing)**: macOS bundles are built via GitHub Actions without full local Mac regression testing or Apple code signing / notarization. If Gatekeeper blocks first launch: right-click `RustMusic.app` in Finder and choose **Open**; on newer macOS versions (such as Sequoia 15+) without that option, dismiss the dialog, open **System Settings → Privacy & Security**, and click **Open Anyway** at the bottom (or run `xattr -cr /Applications/RustMusic.app` in Terminal). Issue reports on macOS testing are welcome.

## 🚀 Development & Build

```bash
npm install          # Install frontend dependencies
npm run tauri dev    # Start dev mode (with HMR)
npm run tauri build  # Build release artifacts (Windows: exe; macOS: dmg; Linux: deb / AppImage)
                     # Windows Inno Setup installer uses installer/build.ps1 separately
```

Requirements: Rust **1.87+** (minimum required by `rodio 0.22`) and Node **20+**; Windows requires WebView2 Runtime (preinstalled on Win10/11). macOS requires Xcode Command Line Tools (`xcode-select --install`); `.dmg` targets macOS 11.0+ (`src-tauri/tauri.macos.conf.json`).

Linux also requires system build libraries (Debian / Ubuntu):

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
                 librsvg2-dev libxdo-dev libdbus-1-dev libasound2-dev \
                 pkg-config build-essential
```

(Other Rust D-Bus usage — MPRIS media controls — goes through pure-Rust `zbus`; audio output uses `cpal` via ALSA/PipeWire, hence `libasound2-dev`. WebKitGTK's media element depends on GStreamer plugins, so installing `sudo apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good` is also recommended.)

Tests (matching CI):

```bash
npm run build                                          # Build frontend first (Tauri context embeds dist)
cd src-tauri && cargo test --bin rustmusic --locked
```

Tests marked `#[ignore]` require network access or local user app data and are skipped by default.

Additional notes:

- **Updating the app icon**: Edit `scripts/icon-tool/app-icon-new.svg` and regenerate from **inside the tool directory** (it has its own `package.json` and `render.js` uses relative paths):

  ```bash
  cd scripts/icon-tool
  npm install
  node render.js
  cd ../..
  npm run tauri icon scripts/icon-tool/icon-1024.png   # Regenerate all platform icons
  ```

- **Custom update repository**: Defaults to `LingyunStudio/RustMusic`; override with `RUSTMUSIC_UPDATE_REPO` (`owner/name`) for staging or internal repos.
- **Windows installer**: Install [Inno Setup 6](https://jrsoftware.org/isinfo.php), point `$Iscc` in `installer/build.ps1` to your local `ISCC.exe` (default `D:\InnoSetup6\ISCC.exe`), and run `powershell -ExecutionPolicy Bypass -File installer\build.ps1` (outputs to `installer/output/`; pass `-SkipBuild` to repackage existing binaries).
- **Release workflow**: Windows x64 and Linux x64 builds are packaged locally/in VMs and signed offline (`installer/signing.key` never leaves the maintainer machine). Pushing a `v<version>` tag triggers `release.yml` to build macOS (Apple Silicon / Intel) and Linux arm64 artifacts as a draft Release, to which local packages (and optional offline arm64 `.sig` files) are uploaded before publishing.

## ⚖️ Sources & Copyright

NetEase Cloud Music, QQ Music, and Kugou integrations access each platform solely using your own account according to your existing account entitlements, and **contain no features to bypass VIP or copyright restrictions**. Custom online sources rely on direct URLs added by the user that you have the right to stream. Please support licensed music.

## 📌 Roadmap

- [ ] HLS / m3u8 streaming support
- [ ] Lyrics translations for QQ Music / Kugou / Navidrome (NetEase already supported)
- [ ] Audio transcoding / batch tag editor
- [x] Linux builds (x64 / arm64)
- [x] macOS builds (experimental: Apple Silicon / Intel)

## 📄 License

Released under the [Apache-2.0](LICENSE) license.

`src-tauri/src/qrc.rs` (QQ QRC encrypted lyrics decoder) is adapted from the open-source [navidrome-lyrics-plugin](https://github.com/J0R6IT0/navidrome-lyrics-plugin) (MIT), relicensed/integrated under Apache-2.0 while preserving the original copyright header.
