; Build with scripts/build-installer.ps1 (Inno Setup 6.3 or newer).
#ifndef AppVersion
  #define AppVersion "0.2.3"
#endif
#ifndef SourceDir
  #define SourceDir "..\target\release"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif
#ifndef BootstrapperPath
  #define BootstrapperPath "..\artifacts\installer\MicrosoftEdgeWebview2Setup.exe"
#endif
#ifndef RuntimeDir
  #define RuntimeDir "..\artifacts\installer\crt"
#endif

[Setup]
AppId={{F78D1490-8623-462E-A75C-F8E022A94284}
AppName=Anni
AppVersion={#AppVersion}
AppVerName=Anni {#AppVersion}
AppPublisher=Anni contributors
DefaultDirName={localappdata}\Programs\Anni
DefaultGroupName=Anni
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=Anni-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\crates\inna-app\assets\anni.ico
UninstallDisplayIcon={app}\anni.exe
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
VersionInfoVersion={#AppVersion}

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
Name: startup; Description: "Start Anni and check for updates when I sign in to Windows"; GroupDescription: "Startup:"; Flags: checkedonce

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "anni"; ValueData: """{app}\anni.exe"" --desktop"; Tasks: startup; Check: not IsAutomaticUpdate

[Files]
Source: "{#SourceDir}\anni.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\inna-auth.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RuntimeDir}\vcruntime140.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RuntimeDir}\vcruntime140_1.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BootstrapperPath}"; Flags: dontcopy

[Icons]
Name: "{group}\Anni"; Filename: "{app}\anni.exe"
Name: "{group}\Anni desktop widgets"; Filename: "{app}\anni.exe"; Parameters: "--desktop"
Name: "{autodesktop}\Anni"; Filename: "{app}\anni.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\anni.exe"; Description: "Open Anni"; Flags: nowait postinstall skipifsilent

[Code]
const
  WebViewKey = 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function IsAutomaticUpdate: Boolean;
begin
  Result := ExpandConstant('{param:AUTOMATICUPDATE|0}') = '1';
end;

function HasRuntime(Root: Integer): Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(Root, WebViewKey, 'pv', Version) and
    (Version <> '') and (Version <> '0.0.0.0');
end;

function WebViewInstalled: Boolean;
begin
  Result := HasRuntime(HKLM32) or HasRuntime(HKCU32) or HasRuntime(HKCU64);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ExitCode: Integer;
begin
  Result := '';
  if WebViewInstalled then
    Exit;
  WizardForm.StatusLabel.Caption := 'Installing Microsoft Edge WebView2 for INNA sign-in...';
  ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
  if not Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'),
    '/silent /install', '', SW_HIDE, ewWaitUntilTerminated, ExitCode) then
    Result := 'Could not start Microsoft Edge WebView2 setup. Please retry Anni setup.'
  else if not WebViewInstalled then
    Result := 'Microsoft Edge WebView2 could not be installed (code ' +
      IntToStr(ExitCode) + '). Connect to the internet and retry Anni setup.';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command: String;
begin
  if CurUninstallStep = usUninstall then
    if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run',
      'anni', Command) then
      if CompareText(Command, ExpandConstant('"{app}\anni.exe" --desktop')) = 0 then
        RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'anni');
end;
