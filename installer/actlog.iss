; Inno Setup Script for actlog
; PrevilegesRequired=lowest enables installing without admin rights.

[Setup]
AppName=actlog
AppVersion=0.0.2-beta
WizardStyle=modern
DefaultDirName={localappdata}\Programs\actlog
DefaultGroupName=actlog
UninstallDisplayIcon={app}\actlog.exe
Compression=lzma2
SolidCompression=yes
OutputDir=..\dist
OutputBaseFilename=actlog-v0.0.2-beta-installer
PrivilegesRequired=lowest
DisableProgramGroupPage=yes

[Files]
Source: "..\target\release\actlog.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userstartup}\actlog"; Filename: "{app}\actlog.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\actlog.exe"; Description: "Launch actlog"; Flags: nowait postinstall skipifsilent
