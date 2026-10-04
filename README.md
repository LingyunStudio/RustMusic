# RustMusic 🎵

一款用 **Rust + Tauri 2 + React** 打造的高性能、精美界面的桌面音乐播放器。

![tech](https://img.shields.io/badge/Rust-1.77+-DEA584) ![tech](https://img.shields.io/badge/Tauri-2-24C8D8) ![tech](https://img.shields.io/badge/React-18-61DAFB) ![platform](https://img.shields.io/badge/platform-Windows-blue)

<img src="https://cdn.jsdelivr.net/gh/LingyunStudio/LingyunImg@master/2026/09/upgit_20260929_1790619273.png" alt="image-20260929021433117" style="zoom:67%;" />

## ✨ 功能特性

### 播放引擎（纯 Rust）
- **全格式解码**：MP3 / FLAC / WAV / OGG(Vorbis) / M4A(AAC) / AAC，基于 `symphonia` 纯 Rust 解码，无 FFmpeg 依赖
- **播放控制**：播放 / 暂停 / 上一首 / 下一首、任意拖动定位（seek）、音量、播放速度（0.5x–2x）
- **播放模式**：顺序 / 列表循环 / 单曲循环 / 随机播放
- **输出设备**：可选指定输出设备，或跟随系统默认设备自动切换（耳机插拔即切）
- **WASAPI 独占模式**（可选，默认关闭）：绕过 Windows 系统混音器直连声卡，采样率按源文件直通输出，不经系统重采样与音效处理，尽可能保留原始音质；本地与全部在线音源（含 B 站 DASH 流）统一支持，独占会话内 seek / 倍速 / 切换设备即时生效；需要声卡驱动支持独占（蓝牙 / 网络音箱等驱动不支持），不支持的设备自动回退普通模式并提示原因；关闭独占立即把设备交还系统，其它应用即刻恢复出声
- **10 段均衡器**：实时 DSP（RBJ 双二阶滤波器），内置流行 / 摇滚 / 古典 / 爵士 / 电子 / 人声预设
- **播放队列**：下一首播放、加入队列、拖动跳转、清空

### 音源
- **网易云在线曲库**：内置网易云音乐接口（官方客户端同款 weapi 协议），搜索即点即播（自动下载缓存）；扫码登录自己的账号后按账号权益播放（免费曲目标准音质 / 会员曲目按会员权益），登录凭证仅存本机
- **QQ 音乐在线曲库**：搜索 / 播放 / 扫码登录，免费曲目匿名可播，VIP 曲目登录后按会员权益获取播放链接
- **酷狗音乐在线曲库**：搜索 / 官方排行榜（TOP500 等 56 个榜单）/ 歌单广场「随便听听」/ 账号歌单导入；扫码登录后 VIP 曲目按会员权益播放，支持无损（FLAC）/ Hi-Res 音质
- **Navidrome（Subsonic 兼容）**：连接自建服务器（兼容所有 Subsonic API 实现），专辑库浏览 / 全曲库 / 搜索 / 歌词 / 下载；密码存 Windows 凭据管理器；页面状态记忆——切走再回来原样恢复，不重新连接
- **B 站**：粘贴视频链接 / BV 号 / b23.tv 短链解析音频，UP 主空间浏览（投稿排序、合集与系列）、登录用户收藏夹浏览，字幕（作歌词）、下载入库；扫码登录
- **本地媒体库**：多文件夹扫描（并行解析）、SQLite 存储、增量更新（按 mtime/size 跳过未变文件）、自动清理失效条目
- **标签元数据**：ID3v2 / FLAC / MP4 等标签解析（标题、艺术家、专辑、年份、音轨号），无标签时按文件名智能回退
- **封面提取**：自动提取内嵌封面并压缩缓存，未嵌入的显示动态渐变占位图
- **在线音源**：添加任意音频文件直链 URL（http/https），自动下载缓存并播放，带实时进度条；缓存可在设置中一键清理
- **统一播放链路**：所有在线音源走同一套下载缓存 → 播放引擎（含 WASAPI 独占 / 均衡器 / 倍速），支持"下一首播放 / 加入队列 / 收藏 / 添加到播放列表 / 批量下载"

### 歌词
- 支持 `.lrc` 同名歌词文件与内嵌歌词标签
- **逐字歌词**：QQ QRC（加密解密）、网易云 yrc、酷狗 KRC 均转为逐字时间轴，跟随播放逐字点亮；单行畸形自动跳过不弃整首
- 全屏播放页逐行滚动、当前行高亮、点击歌词跳转播放位置
- **桌面歌词**：独立透明悬浮窗口，逐字 / 逐行着色（已唱 / 未唱 / 下一句可自定义配色），支持拖动、缩放、置顶、鼠标穿透锁定（主窗口 L 键开关 / 解锁）

### 桌面集成（Windows）
- **系统媒体控制（SMTC）**：键盘媒体键、系统媒体浮窗显示曲名 / 艺术家 / 封面 / 进度
- **系统托盘**：托盘菜单播放 / 暂停 / 切歌、左键回到主界面；关窗可最小化到托盘
- **自动更新**：检查 GitHub Release 新版本，应用内下载（带进度、SHA-256 校验）并静默安装重启
- **窗口**：无边框自绘标题栏、拖拽文件 / 文件夹直接导入资料库

### 界面
- 暗色玻璃拟态风格，封面主色动态氛围光（实时提取封面主色）；多套内置皮肤 + 自定义图片皮肤
- 资料库 / 我喜欢 / 最近播放 / 播放列表管理 / 网易云 / QQ 音乐 / 酷狗 / Navidrome / B 站 / 在线音源 / 设置
- 全局搜索、多列排序、右键菜单、批量多选操作、播放队列侧栏、全屏歌词页

## 🏗️ 架构

```
├── index.html               # 主窗口入口
├── desktop-lyrics.html      # 桌面歌词窗口入口（Vite 多页构建）
├── src/                     # React 前端
│   ├── components/          # 播放栏、歌词页、队列、通用组件
│   ├── views/               # 资料库 / 播放列表 / 设置等页面
│   │   └── online/          # 在线曲库子组件（扫码登录、弹窗、右键菜单、结果行）
│   ├── desktop-lyrics/      # 桌面歌词窗口前端
│   ├── hooks/               # 通用 hooks（列表拖拽、虚拟窗口、定位药丸）
│   ├── store/               # Zustand 状态：contract 全量契约 + 按领域切片
│   │   └──                  # （播放队列 / 在线音源 / 歌单 / 设置 / UI），队列与播放逻辑在前端编排
│   ├── api.ts               # Tauri invoke / 事件封装
│   ├── theme.ts             # 主题与强调色
│   └── skins.ts             # 内置皮肤与自定义图片皮肤
├── scripts/icon-tool/       # 图标生成工具（SVG 源文件 → 母图 → 全平台图标）
├── installer/               # Windows 打包：Inno Setup 脚本 + 一键打包脚本
└── src-tauri/               # Rust 后端
    └── src/
        ├── main.rs          # 应用装配（窗口事件、DWM 样式、模块挂载）
        ├── engine/          # 音频引擎
        │   ├── mod.rs       #   播放核心（rodio 播放链、seek 重建、状态事件）
        │   ├── output.rs    #   输出设备管理（构建 / 热切换 / 枚举）
        │   ├── exclusive.rs #   WASAPI 独占会话调度
        │   └── cache.rs     #   在线源下载缓存（边下边播、LRU 清理）
        ├── commands/        # Tauri 命令层（按业务域拆分：媒体库 / 播放 / 歌单 /
        │   └──              #   网易 / QQ / 酷狗 / B 站 / Navidrome / 下载 / 设置 / 更新）
        ├── webview_suspend.rs # 主窗口 WebView 挂起/恢复（TrySuspend 回收渲染内存）
        ├── tray.rs          # 系统托盘 + 统一媒体控制转发
        ├── monitor.rs       # 播放进度事件推送 / 输出设备热切换监听
        ├── eq.rs            # 10 段均衡器（biquad）+ 播放位置统计
        ├── wasapi_out.rs    # WASAPI 独占模式输出（格式协商、未对齐恢复、事件驱动喂采样）
        ├── symdec.rs        # symphonia 直连解码源（B 站 DASH/fMP4、按包 seek）
        ├── streaming.rs     # 在线音源边下边播（流式下载供数、预缓冲起播）
        ├── library.rs       # 媒体库扫描（rayon 并行解析、批量事务入库）、lofty 标签/封面
        ├── lyrics.rs        # LRC / yrc / QRC / KRC → 逐字增强 LRC
        ├── db.rs            # SQLite（rusqlite、WAL）持久化 + 迁移测试
        ├── netease.rs       # 网易云音乐接口客户端（weapi 加密、搜索、取链接、扫码登录）
        ├── qq.rs            # QQ 音乐接口客户端（搜索、取链接、歌词、扫码登录）
        ├── kugou.rs         # 酷狗音乐接口客户端（搜索、榜单、歌单、无损取链、扫码登录）
        ├── navidrome.rs     # Navidrome / Subsonic 接口客户端（凭据管理器存密码）
        ├── bilibili.rs      # B 站接口客户端（视频解析、空间/合集/收藏夹、字幕、扫码登录）
        ├── qrc.rs           # QQ QRC 加密歌词解密 → 逐字增强 LRC（来自开源实现，MIT）
        ├── smtc.rs          # Windows 系统媒体控制（souvlaki）
        └── updater.rs       # GitHub Release 自动更新（下载 + SHA-256 校验 + 静默安装）
```

**选型说明**：Tauri 2 = Rust 高性能后端 + Web 渲染的精美界面；框架天然支持 Windows / macOS / Linux（当前只做 Windows 构建），未来可低成本扩展。

## 📦 下载安装

前往 [Releases](https://github.com/LingyunStudio/RustMusic/releases/latest) 下载 `RustMusic_<版本>_x64-setup.exe`（Windows 10/11 x64，Inno Setup 安装器）。已安装的旧版本会通过应用内自动更新提示升级（下载带 SHA-256 校验，静默安装后自动重启）。

## 🚀 开发与构建

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 开发模式（热更新）
npm run tauri build  # 构建发布版可执行文件（前端资源内嵌进 exe，无打包步骤）
```

要求：Rust (MSVC) 1.77+、Node 18+、WebView2 Runtime（Win10/11 通常自带）。

其他：

- 修改应用图标：编辑 `scripts/icon-tool/app-icon-new.svg`，然后运行
  `node scripts/icon-tool/render.js && npm run tauri icon scripts/icon-tool/icon-1024.png`
  重新生成全平台图标
- Windows 安装包：`powershell -ExecutionPolicy Bypass -File installer\build.ps1`
  （Inno Setup 编译，输出于 `installer/output/`；加 `-SkipBuild` 可跳过构建直接用现有产物打包）

## ⚖️ 音源与版权

网易云 / QQ 音乐集成仅以用户自己的账号访问平台、按账号既有权益播放，**不包含任何绕过会员 / 版权限制的功能**；自定义在线音源由用户自行添加**有权使用**的直链。请支持正版。

## 📌 Roadmap

- [ ] HLS / m3u8 流媒体支持
- [ ] 歌词翻译 / 双语歌词
- [ ] 音频转码 / 标签批量编辑
- [ ] macOS / Linux 构建

## 📄 许可证

本项目基于 [Apache-2.0](LICENSE) 许可证发布。

其中 `src-tauri/src/qrc.rs`（QQ QRC 歌词解密）来自开源项目
[navidrome-lyrics-plugin](https://github.com/J0R6IT0/navidrome-lyrics-plugin)（MIT），
按 Apache-2.0 修改后并入，文件头部保留原始版权声明。
