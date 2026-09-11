; RustMusic Windows 安装包脚本（Inno Setup 6）
; 一键打包: powershell -ExecutionPolicy Bypass -File installer\build.ps1
;          （等价于: npm run tauri build → 本脚本编译）
; 输出: installer\output\RustMusic_<版本>_x64-setup.exe

#define MyAppName "RustMusic"
#define MyAppExeName "rustmusic.exe"
; 版本号直接取自编译产物的文件版本，与 tauri.conf.json 保持一致
#define MyAppVersion GetFileVersion("..\src-tauri\target\release\rustmusic.exe")

[Setup]
AppId={{B7DA4657-0D10-4718-A6D5-A02863D39352}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppName}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
OutputDir=output
OutputBaseFilename=RustMusic_{#MyAppVersion}_x64-setup
SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; 与 Tauri NSIS 默认一致：优先按当前用户安装（{autopf} 解析为 %LOCALAPPDATA%\Programs），
; 用户可在向导中改为为所有用户安装
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ShowLanguageDialog=no

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; 覆盖前先结束正在运行的实例
Source: "..\src-tauri\target\release\rustmusic.exe"; DestDir: "{app}"; Flags: ignoreversion; BeforeInstall: KillRunningApp

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[Code]
procedure KillRunningApp;
var
  R: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/f /im {#MyAppExeName}', '', SW_HIDE, ewWaitUntilTerminated, R);
end;

function IsWebView2Installed: Boolean;
var
  Pv: string;
begin
  Result := False;
  if RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) and (Pv <> '') then
    Result := True
  else if RegQueryStringValue(HKLM32, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) and (Pv <> '') then
    Result := True
  else if RegQueryStringValue(HKCU, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) and (Pv <> '') then
    Result := True;
end;

procedure OpenWebView2Download;
var
  R: Integer;
begin
  ShellExec('open', 'https://go.microsoft.com/fwlink/?linkid=2124701', '', '', SW_SHOWNORMAL, ewNoWait, R);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and (not IsWebView2Installed) then
  begin
    if MsgBox(
        '系统未检测到 Microsoft Edge WebView2 运行时，缺少它 RustMusic 将无法启动。' + #13#10 + #13#10 +
        '是否现在打开官方下载页面？下载安装完成后，再重新运行本安装程序即可。',
        mbConfirmation, MB_YESNO) = IDYES then
      OpenWebView2Download;
    Result := False;
  end;
end;
