# RustMusic Windows 安装包一键打包（Inno Setup）
# 用法：
#   powershell -ExecutionPolicy Bypass -File installer\build.ps1              # 构建产物 + 打包
#   powershell -ExecutionPolicy Bypass -File installer\build.ps1 -SkipBuild   # 跳过构建，只用现有产物打包
# 输出：installer\output\RustMusic_<版本>_x64-setup.exe

param([switch]$SkipBuild)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Iscc = "D:\InnoSetup6\ISCC.exe"

if (-not (Test-Path $Iscc)) {
    throw "未找到 Inno Setup 编译器：$Iscc"
}

if (-not $SkipBuild) {
    Set-Location $Root
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "tauri build 失败" }
}

$Exe = Join-Path $Root "src-tauri\target\release\rustmusic.exe"
if (-not (Test-Path $Exe)) {
    throw "未找到编译产物：$Exe（请先运行 tauri build）"
}

& $Iscc (Join-Path $PSScriptRoot "RustMusic.iss")
if ($LASTEXITCODE -ne 0) { throw "ISCC 打包失败" }

$Installer = Get-ChildItem (Join-Path $Root "installer\output\*.exe") |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

# 更新包签名（ed25519）：私钥本机保管（signing.key，不入库），签名随附件
# 上传（<安装包>.sig），客户端内置公钥验签——发布账号被盗也无法推送可安装
# 的恶意更新包。密钥缺失时首次运行自动生成（公钥需与 updater.rs 一致）
$KeyFile = Join-Path $PSScriptRoot "signing.key"
if (-not (Test-Path $KeyFile)) {
    cargo run --quiet --example asset_signer -- genkey $KeyFile
    if ($LASTEXITCODE -ne 0) { throw "签名密钥生成失败" }
    Write-Host "已生成签名密钥：$KeyFile（请离线备份；公钥须与 updater.rs 的 UPDATE_PUBKEY 一致）" -ForegroundColor Yellow
}
cargo run --quiet --example asset_signer -- sign $Installer.FullName $KeyFile
if ($LASTEXITCODE -ne 0) { throw "安装包签名失败" }

Write-Host ""
Write-Host "安装包已生成：$($Installer.FullName)  $([math]::Round($Installer.Length / 1MB, 2)) MB" -ForegroundColor Green
