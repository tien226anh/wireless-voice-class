; Build with scripts/build-windows-installer.ps1 after staging the Windows package.
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef ReleaseTag
  #error ReleaseTag is required
#endif
#ifndef PackageDir
  #error PackageDir is required
#endif
#ifndef InstallerOutput
  #error InstallerOutput is required
#endif

[Setup]
; Keep this ID stable so future versions upgrade the same installation.
AppId={{5871AA42-5399-4C16-B6BF-8A946B6AA938}
AppName=Wireless PA
AppVersion={#AppVersion}
AppPublisher=tien226anh
AppPublisherURL=https://github.com/tien226anh/wireless-voice-class
AppSupportURL=https://github.com/tien226anh/wireless-voice-class/issues
AppUpdatesURL=https://github.com/tien226anh/wireless-voice-class/releases
DefaultDirName={localappdata}\Programs\Wireless PA
DefaultGroupName=Wireless PA
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
LicenseFile={#PackageDir}\LICENSE
SetupIconFile={#PackageDir}\assets\icons\wireless-pa.ico
UninstallDisplayIcon={app}\wireless-pa.exe
UninstallDisplayName=Wireless PA
VersionInfoVersion={#AppVersion}.0
VersionInfoProductName=Wireless PA
OutputDir={#InstallerOutput}
OutputBaseFilename=wireless-pa-{#ReleaseTag}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Wireless PA"; Filename: "{app}\wireless-pa.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Wireless PA"; Filename: "{app}\wireless-pa.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\wireless-pa.exe"; Description: "{cm:LaunchProgram,Wireless PA}"; Flags: nowait postinstall skipifsilent
