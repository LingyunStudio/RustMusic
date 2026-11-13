# RustMusic 下载文件选择指南

前往 [Releases 页面](https://github.com/LingyunStudio/RustMusic/releases/latest) 后，底部的 **Assets** 会列出各平台安装包及附加文件。请按自己的操作系统和芯片架构挑选对应的一个安装包下载即可。

## 快速对照表

| 操作系统 | 适用设备 / 架构 | 应下载的文件 | 说明 |
| --- | --- | --- | --- |
| **Windows** | Windows 10 / 11（64 位 x64，绝大多数 PC） | `RustMusic_<版本>_x64-setup.exe` | 安装向导程序，双击按提示安装即可（需 WebView2 运行时，Win10/11 通常已自带） |
| **macOS**（实验性支持） | Apple Silicon（M1 / M2 / M3 / M4 等 M 系列芯片，macOS 11.0+） | `RustMusic_<版本>_aarch64.dmg` | 打开后将 `RustMusic.app` 拖入「应用程序」 |
| **macOS**（实验性支持） | Intel 处理器 Mac（macOS 11.0+） | `RustMusic_<版本>_x64.dmg` | 打开后将 `RustMusic.app` 拖入「应用程序」 |
| **Linux**（x64） | Ubuntu / Debian / Linux Mint / Deepin 等 Debian 系（`x86_64` / `amd64`） | `RustMusic_<版本>_amd64.deb` | 系统安装包，如 `sudo apt install ./RustMusic_<版本>_amd64.deb` |
| **Linux**（x64） | Arch / Fedora / openSUSE 等发行版，或不想安装到系统目录 | `RustMusic_<版本>_amd64.AppImage` | 便携单文件，赋可执行权限后直接运行，支持应用内自动更新 |
| **Linux**（arm64） | 树莓派、ARM 开发板、ARM 云主机等 Debian / Ubuntu 系（`aarch64` / `arm64`） | `RustMusic_<版本>_arm64.deb` | CI 构建的 ARM64 系统安装包 |
| **Linux**（arm64） | ARM64 通用发行版或便携运行 | `RustMusic_<版本>_aarch64.AppImage` | 便携单文件，赋可执行权限后直接运行 |

## 这些文件不用手动下载

- **`*.sig` 文件**（如 `RustMusic_<版本>_x64-setup.exe.sig`、`*.AppImage.sig`、`*.deb.sig`）  
  这是**应用内自动更新器**用来校验安装包完整性和发布者身份的 Ed25519 数字签名文件。手动下载安装时**不需要**下载 `.sig` 文件，只下主安装包即可。
- **`Source code (zip)` / `Source code (tar.gz)`**  
  GitHub 自动生成的源代码压缩包，仅供开发者阅读源码或自行编译使用，不是可安装运行的软件。

## 如何查看自己的系统架构？

- **macOS**：点击屏幕左上角 `` → **关于本机**：
  - 芯片显示 **Apple M1 / M2 / M3 / M4** 等 → 下载 `aarch64.dmg`
  - 处理器显示 **Intel** → 下载 `x64.dmg`
  - 也可以在终端运行 `uname -m`：`arm64` 选 `aarch64.dmg`，`x86_64` 选 `x64.dmg`。
- **Linux**：在终端运行 `uname -m`：
  - 输出 `x86_64` → 选 `amd64.deb` 或 `amd64.AppImage`
  - 输出 `aarch64` 或 `arm64` → 选 `arm64.deb` 或 `aarch64.AppImage`

## 首次打开与更新说明

- **Windows**：安装包暂未购买商业代码签名证书，如首次运行遇到 SmartScreen「Windows 已保护你的电脑」提示，点击 **更多信息 → 仍要运行** 即可。安装后支持在应用内直接检查并自动静默更新。
- **macOS（实验性支持）**：macOS 安装包由 GitHub Actions 自动构建且未经过 Apple 公证，暂不支持应用内自动更新（有新版时需到 Releases 页面手动下载覆盖安装）。首次打开如被系统拦截：
  1. 可在「访达 → 应用程序」中右键点击 `RustMusic` 选「打开」；
  2. macOS Sequoia（15+）若无该选项，请先取消拦截弹窗，再打开 **系统设置 → 隐私与安全性**，在底部点击 **仍要打开**；
  3. 也可以在终端执行 `xattr -cr /Applications/RustMusic.app` 后再打开。
- **Linux**：
  - 使用 `.AppImage` 时需先赋予执行权限：
    ```bash
    chmod +x RustMusic_*.AppImage
    ./RustMusic_*.AppImage
    ```
    若部分较新发行版（如 Ubuntu 24.04）无法启动 AppImage，请先确认系统已安装 `libfuse2`。
  - `.AppImage` 在当前版本附带对应 `.sig` 文件时支持应用内自动更新；通过 `.deb` 安装的用户需前往 Releases 页面手动下载新版安装包升级。
