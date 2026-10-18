; The Windows installer, built with Inno Setup 6:
;   iscc /DVersion=0.3.0 /DBuild=target\release packaging\windows\open-controller.iss
; It installs for the current user, without administrator rights, into the folder install.ps1
; used too, so it updates an existing installation in place whichever way it got there. Running
; copies are asked to quit first, which unplugs their virtual controllers and shows the hidden
; ones again. Settings stay in %APPDATA%\io.github.brunovncs.open-controller across updates, and
; the uninstaller asks whether to delete them.

#ifndef Version
  #error Pass the version: /DVersion=x.y.z
#endif
#ifndef Build
  #define Build "target\release"
#endif
; x64 or arm64, the build's architecture. The x64 installer also runs on ARM64 Windows, which is
; what versions up to 0.8.8 download as their update; from then on they take the arm64 one.
#ifndef Arch
  #define Arch "x64"
#endif
#define Root "..\.."

[Setup]
AppId={{6F3C2A1E-8B4D-4E7A-9C51-2D0B7A4F9E13}
AppName=OpenController
AppVersion={#Version}
AppVerName=OpenController {#Version}
AppPublisher=Brunovncs
AppPublisherURL=https://opencontroller.com.br
AppSupportURL=https://github.com/Brunovncs/OpenController/issues
AppUpdatesURL=https://github.com/Brunovncs/OpenController/releases/latest
VersionInfoVersion={#Version}.0
DefaultDirName={localappdata}\Programs\open-controller
DisableDirPage=auto
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0
WizardStyle=modern
SetupIconFile={#Root}\assets\icon.ico
UninstallDisplayIcon={app}\open-controller.exe
UninstallDisplayName=OpenController
CloseApplications=yes
RestartApplications=no
Compression=lzma2/max
SolidCompression=yes
OutputDir={#Root}\dist
OutputBaseFilename=open-controller-{#Version}-windows-{#Arch}-setup
LicenseFile={#Root}\LICENSE

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
en.RemoveData=Do you also want to delete your OpenController settings and controller profiles?%n%nChoose No to keep them for a future installation.
pt.RemoveData=Deseja também apagar suas configurações e os perfis de controle do OpenController?%n%nEscolha Não para mantê-los para uma instalação futura.
en.StillHidden=OpenController could not show your controllers to games again: HidHide was busy or did not answer.%n%nIts settings folder was kept, with the list of what it hid, so installing OpenController again brings them back. You can also take them off the list in HidHide Configuration Client.
pt.StillHidden=O OpenController não conseguiu mostrar seus controles aos jogos de novo: o HidHide estava ocupado ou não respondeu.%n%nA pasta de configurações foi mantida, com a lista do que ele escondeu, e instalar o OpenController de novo traz os controles de volta. Você também pode tirá-los da lista no HidHide Configuration Client.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Root}\{#Build}\open-controller.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\{#Build}\open-controller-ui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[InstallDelete]
; The shortcuts up to 0.4.1, named "Open Controller".
Type: files; Name: "{userprograms}\Open Controller.lnk"
Type: files; Name: "{userdesktop}\Open Controller.lnk"

[UninstallDelete]
; VIIPER's server, unpacked here by the app when the user installs it from Requirements.
Type: filesandordirs; Name: "{app}\viiper"

[Icons]
Name: "{userprograms}\OpenController"; Filename: "{app}\open-controller.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\OpenController"; Filename: "{app}\open-controller.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; "Start with Windows" is the app's own setting; the value it writes goes with the app.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "OpenController"; Flags: uninsdeletevalue dontcreatekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Open Controller"; Flags: uninsdeletevalue dontcreatekey

[Run]
; Also after a silent update, so the app the user had open comes back.
Filename: "{app}\open-controller.exe"; Description: "{cm:LaunchProgram,OpenController}"; Flags: nowait postinstall

[Code]
function GetCurrentProcessId: Cardinal;
  external 'GetCurrentProcessId@kernel32.dll stdcall';
function ProcessIdToSessionId(ProcessId: Cardinal; var SessionId: Cardinal): Boolean;
  external 'ProcessIdToSessionId@kernel32.dll stdcall';

var
  // Whether --restore showed again everything a run left hidden, so the settings folder, which
  // holds HidHide's journal, may go.
  Restored: Boolean;

// Ends what is left of OpenController in this Windows session only. Run as administrator,
// taskkill would otherwise end other signed-in users' copies too.
procedure CloseLeftovers;
var
  Code: Integer;
  Session: Cardinal;
begin
  if ProcessIdToSessionId(GetCurrentProcessId, Session) then
    Exec(ExpandConstant('{sys}\taskkill.exe'),
      '/F /FI "SESSION eq ' + IntToStr(Session) + '" /IM open-controller.exe /IM open-controller-ui.exe',
      '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

// A running copy quits the clean way before its files are replaced. Whatever is left is closed:
// the window holds no state, and a copy still starting up cannot hear "quit" yet; the new copy
// shows again anything it hid when it starts.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
  Exe: String;
begin
  Exe := ExpandConstant('{app}\open-controller.exe');
  if FileExists(Exe) then
    Exec(Exe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
  CloseLeftovers;
  Result := '';
end;

// Before the files go, the running copy quits the clean way, a copy still starting up is closed,
// and --restore shows again anything a run left hidden. The settings then stay unless the user
// asks to delete them; a silent uninstall keeps them, and so does a failed restore.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Code: Integer;
  Data, Exe: String;
begin
  Data := ExpandConstant('{userappdata}\io.github.brunovncs.open-controller');
  if CurUninstallStep = usUninstall then
  begin
    Exe := ExpandConstant('{app}\open-controller.exe');
    if FileExists(Exe) then
    begin
      Exec(Exe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
      CloseLeftovers;
      Restored := Exec(Exe, '--restore', '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0);
    end
    else
      Restored := not FileExists(Data + '\hidhide-journal.json');
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    if not Restored then
      SuppressibleMsgBox(CustomMessage('StillHidden'), mbError, MB_OK, IDOK)
    else if DirExists(Data) and not UninstallSilent and
      (MsgBox(CustomMessage('RemoveData'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
      DelTree(Data, True, True, True);
  end;
end;
