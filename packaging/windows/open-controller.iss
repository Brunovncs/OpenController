; The Windows installer, built with Inno Setup 6:
;   iscc /DVersion=0.3.0 /DBuild=target\release packaging\windows\open-controller.iss
; It installs for the current user, without administrator rights, into the folder install.ps1
; used too, so it updates an existing installation in place whichever way it got there. Running
; copies are asked to quit first, which unplugs their virtual controllers and shows the hidden
; ones again. Settings stay in %APPDATA%\io.github.brunovncs.open-controller across updates and
; uninstalls.

#ifndef Version
  #error Pass the version: /DVersion=x.y.z
#endif
#ifndef Build
  #define Build "target\release"
#endif
#define Root "..\.."

[Setup]
AppId={{6F3C2A1E-8B4D-4E7A-9C51-2D0B7A4F9E13}
AppName=Open Controller
AppVersion={#Version}
AppVerName=Open Controller {#Version}
AppPublisher=Brunovncs
AppPublisherURL=https://open-controller-site.vercel.app
AppSupportURL=https://github.com/Brunovncs/OpenController/issues
AppUpdatesURL=https://github.com/Brunovncs/OpenController/releases/latest
VersionInfoVersion={#Version}.0
DefaultDirName={localappdata}\Programs\open-controller
DisableDirPage=auto
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile={#Root}\assets\icon.ico
UninstallDisplayIcon={app}\open-controller.exe
UninstallDisplayName=Open Controller
CloseApplications=yes
RestartApplications=no
Compression=lzma2/max
SolidCompression=yes
OutputDir={#Root}\dist
OutputBaseFilename=open-controller-{#Version}-windows-x64-setup
LicenseFile={#Root}\LICENSE

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Root}\{#Build}\open-controller.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\{#Build}\open-controller-ui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Open Controller"; Filename: "{app}\open-controller.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\Open Controller"; Filename: "{app}\open-controller.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; "Start with Windows" is the app's own setting; the value it writes goes with the app.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Open Controller"; Flags: uninsdeletevalue dontcreatekey

[Run]
; Also after a silent update, so the app the user had open comes back.
Filename: "{app}\open-controller.exe"; Description: "{cm:LaunchProgram,Open Controller}"; Flags: nowait postinstall

[UninstallRun]
Filename: "{app}\open-controller.exe"; Parameters: "--quit"; Flags: runhidden waituntilterminated; RunOnceId: "Quit"
Filename: "{app}\open-controller.exe"; Parameters: "--restore"; Flags: runhidden waituntilterminated; RunOnceId: "Restore"

[Code]
// A running copy quits the clean way before its files are replaced; the window, which holds no
// state, is closed if it is still open.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
  Exe: String;
begin
  Exe := ExpandConstant('{app}\open-controller.exe');
  if FileExists(Exe) then
    Exec(Exe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM open-controller-ui.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;
