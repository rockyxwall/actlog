; Inno Setup Script for ACTLog
; PrevilegesRequired=lowest enables installing without admin rights.

[Setup]
AppName=ACTLog
AppVersion=0.0.2-beta
WizardStyle=modern
DefaultDirName={localappdata}\Programs\actlog
DefaultGroupName=ACTLog
UninstallDisplayIcon={app}\actlog.exe
Compression=lzma2
SolidCompression=yes
OutputDir=..\dist
OutputBaseFilename=ACTLog-v0.0.2-beta-installer
PrivilegesRequired=lowest
DisableProgramGroupPage=yes

[Files]
Source: "..\target\release\actlog.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userstartup}\ACTLog"; Filename: "{app}\actlog.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\actlog.exe"; Description: "Launch ACTLog"; Flags: nowait postinstall skipifsilent
