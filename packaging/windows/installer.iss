; Inno Setup script for Meeting Assistant (Windows).
;
; Built by packaging/windows/build-installer.ps1, which stages the
; two-binary bundle + the windeployqt Qt runtime into one folder and passes:
;   /DMyAppVersion=<ver>  /DStageDir=<abs path to staged payload>
;
; v1 is UNSIGNED (no code-signing cert — fast-follow). On a clean machine
; SmartScreen shows a one-click "More info -> Run anyway". This is documented,
; expected, and reputation-based: it fades as downloads accrue, and an OV/EV
; cert later removes it outright. See SMARTSCREEN.md.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#ifndef StageDir
  #define StageDir "..\..\dist\windows\stage"
#endif

[Setup]
AppId={{8E1F2C7A-3B6D-4E2A-9C1F-MEETINGASSIST}}
AppName=Meeting Assistant
AppVersion={#MyAppVersion}
AppPublisher=Dmitry Medvedev
AppPublisherURL=https://github.com/codemedvedev/meeting-assistant
DefaultDirName={autopf}\Meeting Assistant
DefaultGroupName=Meeting Assistant
UninstallDisplayIcon={app}\meeting-assistant-qt.exe
OutputDir=.
OutputBaseFilename=MeetingAssistant-Setup-{#MyAppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Per-user install so no UAC elevation / admin cert assumptions for v1.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; Flags: unchecked

[Files]
; The entire windeployqt-completed payload: meeting-assistant-qt.exe,
; meeting-server.exe (sibling — the GUI locates it via its own exe dir),
; Qt6*.dll, plugins\, qml\, licenses\.
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\Meeting Assistant"; Filename: "{app}\meeting-assistant-qt.exe"
Name: "{group}\Uninstall Meeting Assistant"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Meeting Assistant"; Filename: "{app}\meeting-assistant-qt.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\meeting-assistant-qt.exe"; Description: "Launch Meeting Assistant"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Inno removes everything it installed; this also clears windeployqt's
; generated caches so uninstall leaves no residue under {app}.
Type: filesandordirs; Name: "{app}"

[Code]
// Clean uninstall: offer to also remove the user's recordings/settings/db.
// The Rust core uses an XDG-style layout under the user profile
// (rust/crates/app/src/container.rs + adapters/src/settings_store.rs).
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Base: string;
begin
  if CurUninstallStep = usUninstall then
  begin
    if MsgBox('Also delete your Meeting Assistant data (recordings, ' +
              'transcripts, settings, downloaded model)?',
              mbConfirmation, MB_YESNO) = IDYES then
    begin
      Base := GetEnv('USERPROFILE');
      DelTree(Base + '\.local\share\meeting-assistant', True, True, True);
      DelTree(Base + '\.cache\meeting-assistant', True, True, True);
      DelTree(Base + '\.config\meeting-assistant', True, True, True);
    end;
  end;
end;
