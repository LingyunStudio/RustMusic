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
Write-Host ""
Write-Host "安装包已生成：$($Installer.FullName)  $([math]::Round($Installer.Length / 1MB, 2)) MB" -ForegroundColor Green
