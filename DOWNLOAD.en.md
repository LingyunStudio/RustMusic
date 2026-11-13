**Language / 语言**: [简体中文](DOWNLOAD.md) | [English](DOWNLOAD.en.md)

# RustMusic Download Guide

Visit the [Releases page](https://github.com/LingyunStudio/RustMusic/releases/latest) and look under **Assets** at the bottom of the release. Choose the single installer matching your operating system and CPU architecture.

## Quick Reference

| Operating System | Device / Architecture | File to Download | Notes |
| --- | --- | --- | --- |
| **Windows** | Windows 10 / 11 (64-bit x64, most PCs) | `RustMusic_<version>_x64-setup.exe` | Standard setup wizard. Double-click and follow prompts (requires WebView2 Runtime, preinstalled on Win10/11). |
| **macOS** (Experimental) | Apple Silicon (M1 / M2 / M3 / M4, macOS 11.0+) | `RustMusic_<version>_aarch64.dmg` | Open the `.dmg` and drag `RustMusic.app` into **Applications**. |
| **macOS** (Experimental) | Intel Mac (macOS 11.0+) | `RustMusic_<version>_x64.dmg` | Open the `.dmg` and drag `RustMusic.app` into **Applications**. |
| **Linux** (x64) | Ubuntu / Debian / Linux Mint / Deepin (`x86_64` / `amd64`) | `RustMusic_<version>_amd64.deb` | System package, e.g. `sudo apt install ./RustMusic_<version>_amd64.deb`. |
| **Linux** (x64) | Arch / Fedora / openSUSE or portable use | `RustMusic_<version>_amd64.AppImage` | Portable single executable (`chmod +x` and run); supports in-app auto-updates. |
| **Linux** (arm64) | Raspberry Pi / ARM SBC / ARM Cloud VM running Debian/Ubuntu (`aarch64` / `arm64`) | `RustMusic_<version>_arm64.deb` | CI-built ARM64 system package. |
| **Linux** (arm64) | General ARM64 distros or portable use | `RustMusic_<version>_aarch64.AppImage` | Portable single executable (`chmod +x` and run). |

## Files You Do Not Need to Download Manually

- **`*.sig` files** (e.g. `RustMusic_<version>_x64-setup.exe.sig`, `*.AppImage.sig`, `*.deb.sig`)  
  Ed25519 digital signatures used automatically by the **in-app auto-updater** to verify integrity and authenticity. You do **not** need them when installing manually.
- **`Source code (zip)` / `Source code (tar.gz)`**  
  Source archives generated automatically by GitHub for developers building from source; they are not runnable installers.

## How to Check Your System Architecture

- **macOS**: Click `` in the top-left menu bar → **About This Mac**:
  - Chip shows **Apple M1 / M2 / M3 / M4** → download `aarch64.dmg`
  - Processor shows **Intel** → download `x64.dmg`
  - Or run `uname -m` in Terminal: `arm64` → `aarch64.dmg`; `x86_64` → `x64.dmg`.
- **Linux**: Run `uname -m` in a terminal:
  - `x86_64` → choose `amd64.deb` or `amd64.AppImage`
  - `aarch64` or `arm64` → choose `arm64.deb` or `aarch64.AppImage`

## First Launch & Update Notes

- **Windows**: The installer does not currently use a commercial code-signing certificate. If Windows SmartScreen displays *"Windows protected your PC"*, click **More info → Run anyway**. Once installed, in-app update checking and silent installation are supported.
- **macOS (Experimental)**: macOS builds are compiled on GitHub Actions without Apple notarization and do not currently support in-app auto-updates (download newer `.dmg` releases manually to upgrade). If Gatekeeper blocks the first launch:
  1. Open **Finder → Applications**, right-click `RustMusic`, and choose **Open**;
  2. On newer macOS versions (Sequoia 15+) where that option may not appear, dismiss the prompt, open **System Settings → Privacy & Security**, and click **Open Anyway** at the bottom;
  3. Alternatively, run `xattr -cr /Applications/RustMusic.app` in Terminal before launching.
- **Linux**:
  - Grant execute permission before running an `.AppImage`:
    ```bash
    chmod +x RustMusic_*.AppImage
    ./RustMusic_*.AppImage
    ```
    On newer distros (such as Ubuntu 24.04), ensure `libfuse2` is installed if the AppImage fails to start.
  - `.AppImage` supports in-app auto-updates when a matching `.sig` file is present in the release; `.deb` users should download newer `.deb` packages from the Releases page.
