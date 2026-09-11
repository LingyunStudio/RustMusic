# RustMusic 🎵

一款用 **Rust + Tauri 2 + React** 打造的高性能、精美界面的桌面音乐播放器。

![tech](https://img.shields.io/badge/Rust-1.77+-DEA584) ![tech](https://img.shields.io/badge/Tauri-2-24C8D8) ![tech](https://img.shields.io/badge/React-18-61DAFB) ![platform](https://img.shields.io/badge/platform-Windows-blue)

## ✨ 功能特性

### 播放引擎（纯 Rust）
- **全格式解码**：MP3 / FLAC / WAV / OGG(Vorbis) / M4A(AAC) / AAC，基于 `symphonia` 纯 Rust 解码，无 FFmpeg 依赖
- **播放控制**：播放 / 暂停 / 上一首 / 下一首、任意拖动定位（seek）、音量、播放速度（0.5x–2x）
- **播放模式**：顺序 / 列表循环 / 单曲循环 / 随机播放
- **输出设备**：可选指定输出设备，或跟随系统默认设备自动切换（耳机插拔即切）
- **10 段均衡器**：实时 DSP（RBJ 双二阶滤波器），内置流行 / 摇滚 / 古典 / 爵士 / 电子 / 人声预设
- **播放队列**：下一首播放、加入队列、拖动跳转、清空

### 音源
- **网易云在线曲库**：内置网易云音乐接口（官方客户端同款 weapi 协议），搜索即点即播（自动下载缓存）；扫码登录自己的账号后按账号权益播放（免费曲目标准音质 / 会员曲目按会员权益），登录凭证仅存本机
- **QQ 音乐在线曲库**：搜索 / 播放 / 扫码登录，免费曲目匿名可播，VIP 曲目登录后按会员权益获取播放链接
- **本地媒体库**：多文件夹扫描（并行解析）、SQLite 存储、增量更新（按 mtime/size 跳过未变文件）、自动清理失效条目
- **标签元数据**：ID3v2 / FLAC / MP4 等标签解析（标题、艺术家、专辑、年份、音轨号），无标签时按文件名智能回退
- **封面提取**：自动提取内嵌封面并压缩缓存，未嵌入的显示动态渐变占位图
- **在线音源**：添加任意音频文件直链 URL（http/https），自动下载缓存并播放，带实时进度条；缓存可在设置中一键清理

### 歌词
- 支持 `.lrc` 同名歌词文件与内嵌歌词标签
- **QQ QRC 逐字歌词**：解密 QRC 加密歌词并转为逐字时间轴，跟随播放逐字点亮
- 全屏播放页逐行滚动、当前行高亮、点击歌词跳转播放位置
- **桌面歌词**：独立透明悬浮窗口，逐字 / 逐行着色（已唱 / 未唱 / 下一句可自定义配色），支持拖动、置顶、鼠标穿透锁定

### 桌面集成（Windows）
- **系统媒体控制（SMTC）**：键盘媒体键、系统媒体浮窗显示曲名 / 艺术家 / 封面 / 进度
- **系统托盘**：托盘菜单播放 / 暂停 / 切歌、左键回到主界面
- **窗口**：无边框自绘标题栏、拖拽文件 / 文件夹直接导入资料库

### 界面
- 暗色玻璃拟态风格，封面主色动态氛围光（实时提取封面主色）
- 资料库 / 我喜欢 / 最近播放 / 播放列表管理 / 网易云 / QQ 音乐 / 在线音源 / 设置
- 全局搜索、多列排序、右键菜单、播放队列侧栏、全屏歌词页

## 🏗️ 架构

```
├── index.html               # 主窗口入口
├── desktop-lyrics.html      # 桌面歌词窗口入口（Vite 多页构建）
├── src/                     # React 前端
│   ├── components/          # 播放栏、歌词页、队列、通用组件
│   ├── views/               # 资料库 / 播放列表 / 在线曲库（网易 & QQ）/ 音源 / 设置
│   ├── desktop-lyrics/      # 桌面歌词窗口前端
│   ├── hooks/               # 通用 hooks（如列表拖拽 useDragList）
│   ├── store.ts             # Zustand 状态（队列与播放逻辑在前端编排）
│   └── api.ts               # Tauri invoke / 事件封装
├── scripts/icon-tool/       # 图标生成工具（SVG 源文件 → 母图 → 全平台图标）
├── installer/               # Windows 打包：Inno Setup 脚本 + 一键打包脚本
└── src-tauri/               # Rust 后端
    └── src/
        ├── engine.rs        # 音频引擎：rodio Sink、解码、在线源下载缓存
        ├── eq.rs            # 10 段均衡器（biquad）+ 播放位置统计
        ├── library.rs       # 媒体库扫描（rayon 并行）、lofty 标签/封面
        ├── lyrics.rs        # LRC 解析
        ├── db.rs            # SQLite（rusqlite）持久化
        ├── netease.rs       # 网易云音乐接口客户端（weapi 加密、搜索、取链接、扫码登录）
        ├── qq.rs            # QQ 音乐接口客户端（搜索、取链接、歌词、扫码登录）
        ├── qrc.rs           # QQ QRC 加密歌词解密 → 逐字增强 LRC（来自开源实现，MIT）
        ├── smtc.rs          # Windows 系统媒体控制（souvlaki）
        └── commands.rs      # Tauri 命令层
```

**选型说明**：Tauri 2 = Rust 高性能后端 + Web 渲染的精美界面；框架天然支持 Windows / macOS / Linux（当前只做 Windows 构建），未来可低成本扩展。

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
