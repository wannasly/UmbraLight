#define AppName "UmbraLight"
#define AppVersion "1.0.0"
#define ProjectRoot ".."
#define SingBoxDir ProjectRoot + "\target\third-party\sing-box-1.13.14\sing-box-1.13.14-windows-amd64"
#define WintunDir ProjectRoot + "\target\third-party\wintun-0.14.1\wintun"

[Setup]
AppId={{A8717981-E49D-41B0-A4BC-470E1C1638D4}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=UmbraLight
AppPublisherURL=https://github.com/wannasly/UmbraLight
AppSupportURL=https://github.com/wannasly/UmbraLight/issues
AppUpdatesURL=https://github.com/wannasly/UmbraLight/releases
DefaultDirName={localappdata}\Programs\UmbraLight
DefaultGroupName=UmbraLight
UninstallDisplayIcon={app}\UmbraLight.exe
OutputDir=..\target\installer
OutputBaseFilename=UmbraLight-1.0.0-windows-x64-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Создать ярлык на рабочем столе"; GroupDescription: "Дополнительные задачи:"; Flags: unchecked

[Files]
Source: "..\target\release\UmbraLight.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\UmbraLight-settings.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SingBoxDir}\sing-box.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SingBoxDir}\libcronet.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SingBoxDir}\LICENSE"; DestDir: "{app}\licenses"; DestName: "sing-box-LICENSE.txt"; Flags: ignoreversion
Source: "{#WintunDir}\bin\amd64\wintun.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#WintunDir}\LICENSE.txt"; DestDir: "{app}\licenses"; DestName: "Wintun-LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\UmbraLight"; Filename: "{app}\UmbraLight.exe"
Name: "{group}\Удалить UmbraLight"; Filename: "{uninstallexe}"
Name: "{autodesktop}\UmbraLight"; Filename: "{app}\UmbraLight.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\UmbraLight.exe"; Description: "Запустить UmbraLight"; Flags: nowait postinstall skipifsilent
