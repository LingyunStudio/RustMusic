# RustMusic 🎵

一款用 **Rust + Tauri 2 + React** 打造的高性能、精美界面的桌面音乐播放器。

![Rust](https://img.shields.io/badge/Rust-1.87%2B-DEA584?logo=rust&logoColor=white) ![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white) ![React](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=black) ![Node](https://img.shields.io/badge/Node-20%2B-5FA04E?logo=nodedotjs&logoColor=white) ![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-0078D6)



![image-20261023115804740](https://cdn.jsdelivr.net/gh/LingyunStudio/LingyunImg@master/2026/10/upgit_20261023_1792727886.png)

## ✨ 功能特性

### 播放引擎（纯 Rust）

- **全格式解码**：MP3 / FLAC / WAV / OGG(Vorbis) / M4A(AAC) / AAC / ALAC（Apple Lossless），另支持 MP4 / M4B 容器（按文件头识别，自动选取可解码的音频轨），基于 `symphonia` 纯 Rust 解码，无 FFmpeg 依赖
- **播放控制**：播放 / 暂停 / 上一首 / 下一首、任意拖动定位（seek）、快退 / 快进 5 秒、音量、播放速度（界面提供 0.75x–2x 五档）、定时停止
- **播放模式**：顺序 / 列表循环 / 单曲循环 / 随机播放
- **输出设备**：可选指定输出设备，或跟随系统默认设备自动切换（耳机插拔后自动跟随）
- **WASAPI 独占模式**（可选，默认关闭，仅 Windows）：绕过 Windows 系统混音器直连声卡，采样率按源文件直通输出，不经系统重采样与音效处理，尽可能保留原始音质；本地与全部在线音源（含 B 站 DASH 流）统一支持，独占会话内 seek / 倍速 / 切换设备即时生效；需要声卡驱动支持独占（蓝牙 / 网络音箱等驱动不支持），不支持的设备自动回退普通模式并提示原因；关闭独占立即把设备交还系统，其它应用即刻恢复出声。Linux 上该设置项自动隐藏（系统混音由 PipeWire/PulseAudio 管理，不存在独占机制）
- **10 段均衡器**：实时 DSP（RBJ 双二阶滤波器），内置流行 / 摇滚 / 古典 / 爵士 / 电子 / 人声预设，支持自定义
- **播放队列**：下一首播放、加入队列、点击跳转、移除单曲、清空；网易云 / QQ / 酷狗、B 站、Navidrome 的分页列表播到本页末尾时自动加载下一页并追加进队列
- **快捷键**：空格 播放 / 暂停，← / → 快退 / 快进 5 秒（长按连续步进），L 开关 / 解锁桌面歌词，Esc 退出全屏歌词页

### 音源

- **网易云在线曲库**：内置网易云音乐接口（官方客户端同款 weapi 协议），搜索即点即播；扫码登录自己的账号后按账号权益播放（免费曲目标准音质 / 会员曲目按会员权益），登录凭证仅存本机。支持排行榜、歌单广场「随便听听」、每日推荐、私人 FM、账号歌单导入与「我喜欢」红心同步
- **QQ 音乐在线曲库**：搜索 / 播放 / 排行榜 / 歌单广场「随便听听」 / 账号歌单导入 / 扫码登录，免费曲目匿名可播，VIP 曲目登录后按会员权益获取播放链接
- **酷狗音乐在线曲库**：搜索 / 官方排行榜（TOP500 等）/ 歌单广场「随便听听」/ 账号歌单导入；扫码登录后 VIP 曲目按会员权益播放，支持无损（FLAC）/ Hi-Res 音质
- **Navidrome（Subsonic API）**：连接自建服务器（主要面向 Navidrome），专辑库浏览 / 全曲库 / 搜索 / 歌词（优先使用 OpenSubsonic 同步歌词接口，旧版接口自动回退）/ 下载；密码存 Windows 凭据管理器；页面状态记忆——切走再回来原样恢复，不重新连接
- **B 站**：粘贴视频链接 / BV 号 / b23.tv 短链解析音频，UP 主空间浏览（投稿排序、合集与系列）、登录用户收藏夹浏览，字幕（作歌词）、下载入库；扫码登录
- **本地媒体库**：多文件夹扫描（并行解析）、SQLite 存储、增量更新（按 mtime/size 跳过未变文件）、自动清理失效条目；支持从硬盘直接删除歌曲（带二次确认）
- **标签元数据**：ID3v2 / FLAC / MP4 等标签解析（标题、艺术家、专辑、年份、音轨号），无标签时按文件名智能回退
- **封面提取**：自动提取内嵌封面并压缩缓存（列表使用小尺寸缩略图以降低内存），未嵌入的显示动态渐变占位图
- **在线音源（直链）**：添加任意音频文件直链 URL（http/https）播放（暂不支持 m3u8 / HLS）
- **统一播放链路**：所有在线音源走同一套边下边播缓存 → 播放引擎（含 WASAPI 独占 / 均衡器 / 倍速）。未缓存的歌曲预缓冲约 512 KB 即起播、后台继续下载，听过的歌完整进入缓存（LRU 清理，可在设置中一键清理）；支持"下一首播放 / 加入队列 / 收藏 / 添加到播放列表 / 批量下载"
- **在线音质**：可在设置中选择，实际音质受账号权益影响；未达成所选音质时会提示实际音质与原因

### 歌词

- 支持 `.lrc` 同名歌词文件与内嵌歌词标签
- **逐字歌词**：QQ QRC（加密解密）、网易云 yrc、酷狗 KRC 均转为逐字时间轴，跟随播放逐字点亮；单行畸形自动跳过不弃整首
- **歌词翻译**：网易云歌曲的官方翻译显示在当前行下方（QQ / 酷狗暂未支持）
- **在线歌词缓存**：网易云 / QQ / 酷狗歌词（含逐字时间轴与翻译）缓存到本地数据库，同一首歌第二次播放秒出
- 全屏播放页逐行滚动、当前行高亮、点击歌词跳转播放位置；歌词颜色按封面取色自适应，并标注当前歌词来源（逐字 KRC / 逐字 YRC / 行级 LRC / 字幕）
- **桌面歌词**：独立透明悬浮窗口，逐字 / 逐行着色（已唱 / 未唱 / 下一句可自定义配色），支持拖动、缩放、置顶、鼠标穿透锁定（主窗口 L 键或托盘菜单开关 / 解锁）

### 桌面集成

- **系统媒体控制**：Windows 走 SMTC（键盘媒体键、系统媒体浮窗显示曲名 / 艺术家 / 封面 / 进度）；Linux 走 MPRIS（GNOME/KDE 等桌面媒体控件、`playerctl` 均可控制）
- **系统托盘**：托盘菜单可显示主界面、开关 / 解锁桌面歌词、播放 / 暂停 / 上一首 / 下一首、退出，左键回到主界面；点击关闭按钮时可在设置中选择最小化到托盘或直接退出
- **自动更新**（Windows / Linux AppImage）：检查 GitHub Release 新版本，按当前平台挑选安装包（Windows `*setup*.exe` / Linux AppImage，架构自动匹配），应用内下载（带进度、SHA-256 校验）并静默安装重启；Linux AppImage 为原地原子替换，deb 安装或 release 无本平台附件时引导前往发布页手动下载
- **凭据管理**：Navidrome 密码存入系统凭据管理器（Windows 凭据管理器 / Linux Secret Service，keyring）
- **窗口**：无边框自绘标题栏、拖拽文件 / 文件夹直接导入资料库

### 界面

- 浅色 / 深色主题 + 15 种强调色（可自定义颜色），深色下为玻璃拟态风格，封面主色动态氛围光（实时提取封面主色）；13 款内置皮肤（黑胶款为音频响应动态皮肤：引擎实时 FFT 驱动频谱柱/节拍涟漪/能量光晕，唱片旋转·音符漂浮）+ 自定义图片皮肤
- 侧边栏：资料库 / 我喜欢 / 最近播放 / 播放列表 / 网易云 / QQ 音乐 / 酷狗 / 在线音源（Bilibili · Navidrome · 直链）/ 设置
- 资料库搜索（歌曲 / 艺术家 / 专辑）与各在线音源内搜索、多列排序、右键菜单、批量多选操作、播放队列侧栏、全屏歌词页
- 播放列表支持批量管理（多选、全选、批量删除歌单 / 批量移除歌曲）
- 播放队列打开时自动定位到正在播放的歌曲并随切歌跟随；歌单、在线列表中当前歌曲滚出视野时，显示「回到当前歌曲」按钮一键返回

## 🏗️ 架构

```
├── index.html               # 主窗口入口
├── desktop-lyrics.html      # 桌面歌词窗口入口（Vite 多页构建）
├── .github/workflows/       # CI（前端类型检查 + 构建、Rust 单元测试）
├── src/                     # React 前端
│   ├── components/          # 播放栏、歌词页、队列、音频律动层（AudioFxLayer）、通用组件
│   ├── views/               # 资料库 / 播放列表 / 设置等页面
│   │   └── online/          # 在线曲库子组件（扫码登录、弹窗、右键菜单、结果行）
│   ├── desktop-lyrics/      # 桌面歌词窗口前端
│   ├── hooks/               # 通用 hooks（列表拖拽、虚拟窗口、定位药丸）
│   ├── store/               # Zustand 状态：contract 全量契约 + 按领域切片
│   │   └──                  # （播放队列 / 在线音源 / 歌单 / 设置 / UI），队列与播放逻辑在前端编排
│   ├── api.ts               # Tauri invoke / 事件封装
│   ├── fx.ts                # 音频律动桥：fx://spectrum 事件订阅与后端开关门控
│   ├── types.ts / utils.ts  # 公共类型与工具函数
│   ├── theme.ts             # 主题与强调色
│   └── skins.ts             # 内置皮肤与自定义图片皮肤
├── scripts/icon-tool/       # 图标生成工具（SVG 源文件 → 母图 → 全平台图标）
├── installer/               # Windows 打包：Inno Setup 脚本 + 一键打包脚本 + 各版本发布说明
└── src-tauri/               # Rust 后端
    ├── examples/            # 调试用示例（WASAPI 探测、B 站音频解码）
    └── src/
        ├── main.rs          # 应用装配（窗口事件、DWM 样式、模块挂载）
        ├── models.rs        # 前后端共享的数据模型
        ├── engine/          # 音频引擎
        │   ├── mod.rs       #   播放核心（rodio 播放链、seek 重建、状态事件）
        │   ├── output.rs    #   输出设备管理（构建 / 热切换 / 枚举）
        │   ├── exclusive.rs #   WASAPI 独占会话调度
        │   └── cache.rs     #   在线源下载缓存（边下边播、LRU 清理）
        ├── commands/        # Tauri 命令层（按业务域拆分：应用 / 媒体库 / 播放 / 歌单 /
        │   └──              #   歌词 / 网易 / QQ / 酷狗 / B 站 / Navidrome / 直链音源 / 下载 / 设置 / 更新）
        ├── webview_suspend.rs # 主窗口 WebView 挂起/恢复（TrySuspend 回收渲染内存）
        ├── tray.rs          # 系统托盘 + 统一媒体控制转发
        ├── monitor.rs       # 播放进度事件推送 / 输出设备热切换监听
        ├── eq.rs            # 10 段均衡器（biquad）+ 播放位置统计 + 律动采样分接
        ├── fx.rs             # 皮肤音频律动：环形缓冲分接 PCM → FFT → 36 频段/节拍 → fx://spectrum 事件
        ├── wasapi_out.rs    # WASAPI 独占模式输出（格式协商、未对齐恢复、事件驱动喂采样）
        ├── symdec.rs        # symphonia 直连解码源（MP4 家族 m4a/mp4/m4b、B 站 DASH/fMP4、按包 seek）
        ├── streaming.rs     # 在线音源边下边播（流式下载供数、预缓冲起播）
        ├── library.rs       # 媒体库扫描（rayon 并行解析、批量事务入库）、lofty 标签/封面
        ├── lyrics.rs        # LRC / yrc / QRC / KRC → 逐字增强 LRC，合并行级翻译
        ├── db.rs            # SQLite（rusqlite、WAL）持久化 + 迁移测试
        ├── netease.rs       # 网易云音乐接口客户端（weapi 加密、搜索、取链接、榜单、每日推荐 / 私人 FM、扫码登录）
        ├── qq.rs            # QQ 音乐接口客户端（搜索、取链接、歌词、榜单、歌单、扫码登录）
        ├── kugou.rs         # 酷狗音乐接口客户端（搜索、榜单、歌单、无损取链、扫码登录）
        ├── navidrome.rs     # Navidrome / Subsonic 接口客户端（凭据管理器存密码）
        ├── bilibili.rs      # B 站接口客户端（视频解析、空间/合集/收藏夹、字幕、扫码登录）
        ├── qrc.rs           # QQ QRC 加密歌词解密 → 逐字增强 LRC（来自开源实现，MIT）
        ├── smtc.rs          # Windows 系统媒体控制（souvlaki）
        └── updater.rs       # GitHub Release 自动更新（下载 + SHA-256 校验 + 静默安装）
```

**选型说明**：Tauri 2 = Rust 高性能后端 + Web 渲染的精美界面；框架天然支持 Windows / macOS / Linux（当前支持 Windows 与 Linux 构建），未来可低成本扩展。

### 💡 核心设计亮点：WebView2 TrySuspend 深度休眠与事件驱动后台切歌（Windows）

> Linux 说明：WebKitGTK 没有 WebView2 的 TrySuspend 机制，Linux 构建下托盘隐藏仅隐藏窗口、WebView 保持后台运行（播放事件与托盘控制路径全部照常工作），内存优化仅在 Windows 上生效。

许多桌面开发者在处理混合架构（如 Tauri / Electron）的“后台播放与内存占用”时常面临两难：

- **常驻后台**：Chromium / WebView2 渲染进程常驻会吃掉上百 MB 内存；
- **销毁 WebView**：重新创建界面不仅有明显的白屏开销，且保存在前端状态机（Zustand）中的播放队列、历史回退栈与随机策略也会丢失。

RustMusic 设计了一套高效的**事件驱动状态唤醒 + 延迟重休眠**机制：

1. **深度冻结与内存即时回收**：当主窗口最小化至系统托盘时，后端触发 Windows 原生 `ICoreWebView2_3::TrySuspend` 冻结渲染进程，并主动执行 `EmptyWorkingSet` 将物理内存即时回收至个位数 MB（音频由纯 Rust 线程解码播放，完全独立于 UI，毫秒不乱）；
2. **切歌时精准状态唤醒**：播放队列及模式逻辑在前端 Zustand 中编排。当后端监控线程检测到曲目自然播完（或收到托盘/系统媒体键切歌指令）时，通过 `resume_main_webview` 执行毫秒级状态唤醒，在 JS 运行时解冻完成后安全投递 `player://ended` 或对应指令；
3. **完成切歌后自动重新休眠**：前端 Zustand 驱动下一首曲目加载并通知后端起播后，后台自动调度 `schedule_resuspend`（延迟约 8 秒确认无连续交互后再次静默挂起），既保证了全自动后台切歌的平滑连贯，又做到了极致的低内存驻留；
4. **无感唤醒防闪烁**：当用户从托盘点击恢复主界面时，先在离屏状态下唤醒 WebView2 并等待毛玻璃背景与封面重采样渲染稳定（约 500ms），然后再呼出窗口，彻底消除 Chromium 恢复时的渲染过渡帧与白屏闪烁；
5. **前台低内存治理**：WebView2 以 `--memory-usage-target-level=low` 运行，更积极地归还物理内存；皮肤动效与音频律动层全部走合成器路径（transform/opacity，60fps 零布局重算），频谱分析在 Rust 引擎侧完成（2048 点 FFT @30Hz，约单核 0.3%），渲染进程 JS 常驻仅约 10MB。

## 📦 下载安装

前往 [Releases](https://github.com/LingyunStudio/RustMusic/releases/latest) 下载对应平台安装包：

- **Windows**（10/11 x64）：`RustMusic_<版本>_x64-setup.exe`（Inno Setup 安装器）
- **Linux** x64：`RustMusic_<版本>_amd64.deb`（Debian/Ubuntu 系）或 `RustMusic_<版本>_amd64.AppImage`（免安装，`chmod +x` 后直接运行）

已安装的旧版本会通过应用内自动更新提示升级（按平台挑选安装包，下载带 SHA-256 校验，静默安装后自动重启；Linux deb 安装会在弹窗中引导前往发布页）。

## 🚀 开发与构建

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 开发模式（热更新）
npm run tauri build  # 构建发布版（Windows：exe；Linux：deb / AppImage）
```

要求：Rust (MSVC) **1.87+**（依赖 `rodio 0.22` 的最低版本要求）、Node **20+**；Windows 需 WebView2 Runtime（Win10/11 通常自带）。

Linux 还需要系统构建依赖（Debian / Ubuntu）：

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
                 librsvg2-dev libxdo-dev libdbus-1-dev libasound2-dev \
                 pkg-config build-essential
```

（Rust 侧的其它 D-Bus 用途——MPRIS 媒体控制——走纯 Rust 的 zbus；音频输出经 cpal 走 ALSA/PipeWire，故需要 `libasound2-dev`。WebKitGTK 的 WebView 内媒体元素依赖 GStreamer 插件，建议一并安装：`sudo apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good`。）

测试（与 CI 一致）：

```bash
npm run build                                          # 先构建前端（tauri 上下文引用 dist）
cd src-tauri && cargo test --bin rustmusic --locked
```

标记为 `#[ignore]` 的用例需要联网或本机应用数据，默认不运行。

其他：

- 修改应用图标：编辑 `scripts/icon-tool/app-icon-new.svg`，然后在**工具目录内**重新生成（图标工具有独立的 `package.json`，需先安装依赖；`render.js` 使用相对路径，必须在该目录下运行）：

  ```bash
  cd scripts/icon-tool
  npm install
  node render.js
  cd ../..
  npm run tauri icon scripts/icon-tool/icon-1024.png   # 重新生成全平台图标
  ```

- 自动更新源默认为 `LingyunStudio/RustMusic`，可用环境变量 `RUSTMUSIC_UPDATE_REPO`（`owner/name`）指向测试 / 内部仓库。
- Windows 安装包：需先安装 [Inno Setup 6](https://jrsoftware.org/isinfo.php)，并把 `installer/build.ps1` 中的 `$Iscc` 改成本机 `ISCC.exe` 的路径（默认 `D:\InnoSetup6\ISCC.exe`），然后运行 `powershell -ExecutionPolicy Bypass -File installer\build.ps1` （输出于 `installer/output/`；加 `-SkipBuild` 可跳过构建直接用现有产物打包）

## ⚖️ 音源与版权

网易云 / QQ 音乐 / 酷狗等集成仅以用户自己的账号访问平台、按账号既有权益播放，**不包含任何绕过会员 / 版权限制的功能**；自定义在线音源由用户自行添加**有权使用**的直链。请支持正版。

## 📌 Roadmap

- [ ] HLS / m3u8 流媒体支持
- [ ] QQ / 酷狗 / Navidrome 歌词翻译（网易云已支持）
- [ ] 音频转码 / 标签批量编辑
- [x] Linux 构建
- [ ] macOS 构建

## 📄 许可证

本项目基于 [Apache-2.0](LICENSE) 许可证发布。

其中 `src-tauri/src/qrc.rs`（QQ QRC 歌词解密）来自开源项目 [navidrome-lyrics-plugin](https://github.com/J0R6IT0/navidrome-lyrics-plugin)（MIT）， 按 Apache-2.0 修改后并入，文件头部保留原始版权声明。
