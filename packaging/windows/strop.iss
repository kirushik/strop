; Per-user LocalAppData avoids UAC and keeps self-update writable.
#define MyVersion GetEnv("STROP_VERSION")
#define MySource GetEnv("STROP_EXE")
#define MyOutput GetEnv("STROP_OUTPUT")

[Setup]
AppId={{A2CD8091-B899-48DB-B96E-FEA4A18F256F}
AppName=Strop
AppVersion={#MyVersion}
AppPublisher=Kirill Pimenov
DefaultDirName={localappdata}\Programs\Strop
DefaultGroupName=Strop
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ChangesAssociations=yes
SetupIconFile=..\generated\strop.ico
UninstallDisplayIcon={app}\strop.exe
OutputDir={#MyOutput}
OutputBaseFilename=strop-{#MyVersion}-x86_64-windows-installer
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; CI may inject: SignTool=signtool $f (or an Azure/SignPath wrapper).
; SignTool signs Setup and the generated uninstaller; the inner EXE is
; signed before ISCC runs.

[Files]
Source: "{#MySource}"; DestDir: "{app}"; DestName: "strop.exe"; Flags: ignoreversion
Source: "..\..\assets\fonts\coldread\URWBookman-Light.otf"; DestDir: "{app}\assets\fonts\coldread"; Flags: ignoreversion
Source: "..\..\assets\fonts\coldread\URWBookman-LightItalic.otf"; DestDir: "{app}\assets\fonts\coldread"; Flags: ignoreversion
Source: "..\..\assets\fonts\coldread\URWBookman-Demi.otf"; DestDir: "{app}\assets\fonts\coldread"; Flags: ignoreversion
Source: "..\..\assets\fonts\coldread\LICENSE"; DestDir: "{app}\assets\fonts\coldread"; Flags: ignoreversion
Source: "..\..\assets\hyphenation\en-us.standard.bincode"; DestDir: "{app}\assets\hyphenation"; Flags: ignoreversion
Source: "..\..\assets\hyphenation\ru.standard.bincode"; DestDir: "{app}\assets\hyphenation"; Flags: ignoreversion
Source: "..\..\assets\hyphenation\ATTRIBUTION.txt"; DestDir: "{app}\assets\hyphenation"; Flags: ignoreversion
Source: "..\..\assets\paper-noise-256.png"; DestDir: "{app}\assets"; Flags: ignoreversion

[Icons]
Name: "{group}\Strop"; Filename: "{app}\strop.exe"; AppUserModelID: "cc.pimenov.strop"

[Registry]
Root: HKCU; Subkey: "Software\Classes\.strop"; ValueType: string; ValueData: "Strop.Document"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\Strop.Document"; ValueType: string; ValueData: "Strop document"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Strop.Document"; ValueType: string; ValueName: "AppUserModelID"; ValueData: "cc.pimenov.strop"
Root: HKCU; Subkey: "Software\Classes\Strop.Document\DefaultIcon"; ValueType: string; ValueData: "{app}\strop.exe,0"
Root: HKCU; Subkey: "Software\Classes\Strop.Document\shell\open\command"; ValueType: string; ValueData: """{app}\strop.exe"" ""%1"""

[Run]
Filename: "{app}\strop.exe"; Description: "Launch Strop"; Flags: nowait postinstall skipifsilent
