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
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
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
OutputBaseFilename=open-controller-{#Version}-windows-x64-setup
LicenseFile={#Root}\LICENSE

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
en.RemoveData=Do you also want to delete your OpenController settings and controller profiles?%n%nChoose No to keep them for a future installation.
pt.RemoveData=Deseja também apagar suas configurações e os perfis de controle do OpenController?%n%nEscolha Não para mantê-los para uma instalação futura.

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

[UninstallRun]
Filename: "{app}\open-controller.exe"; Parameters: "--quit"; Flags: runhidden waituntilterminated; RunOnceId: "Quit"
; A copy still starting up cannot hear "quit" yet; --restore then shows again anything it hid.
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM open-controller.exe /IM open-controller-ui.exe"; Flags: runhidden waituntilterminated; RunOnceId: "Kill"
Filename: "{app}\open-controller.exe"; Parameters: "--restore"; Flags: runhidden waituntilterminated; RunOnceId: "Restore"

[Code]
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
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM open-controller.exe /IM open-controller-ui.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;

// The settings stay unless the user asks to delete them; a silent uninstall keeps them.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Data: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    Data := ExpandConstant('{userappdata}\io.github.brunovncs.open-controller');
    if DirExists(Data) and not UninstallSilent and
      (MsgBox(CustomMessage('RemoveData'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
      DelTree(Data, True, True, True);
  end;
end;
