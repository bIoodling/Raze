#define AppName "Raze Macro"
#define AppVersion "1.0.0-dev"
[Setup]
AppId={{D641A1E7-5918-4D22-BA18-63B1544D7D95}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Raze
AppComments=Offline development build
DefaultDirName={localappdata}\Programs\Raze Macro Development
DefaultGroupName=Raze Macro
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
OutputDir=..\dist
OutputBaseFilename=Raze-Core-Setup
SetupIconFile=assets\raze.ico
UninstallDisplayIcon={app}\raze.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
WizardImageFile=assets\wizard.bmp
WizardSmallImageFile=assets\small.bmp
WizardImageStretch=no
WizardSizePercent=100
DisableWelcomePage=no
DisableDirPage=yes
DisableReadyPage=yes
SetupLogging=yes
UninstallLogging=yes
CloseApplications=yes
RestartApplications=no
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
[Files]
Source: "..\RazeMacro\target\release\Raze-Macro.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\raze.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
[Icons]
Name: "{group}\Raze Macro"; Filename: "{app}\Raze-Macro.exe"; IconFilename: "{app}\raze.ico"
Name: "{group}\Uninstall Raze Macro"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Raze Macro"; Filename: "{app}\Raze-Macro.exe"; IconFilename: "{app}\raze.ico"; Tasks: desktopicon
[Run]
Filename: "{app}\Raze-Macro.exe"; Description: "Launch Raze Macro"; Flags: nowait postinstall skipifsilent
[Code]
procedure InitializeWizard();
begin
  WizardForm.Color := $00100908;
  WizardForm.Font.Name := 'Consolas';
  WizardForm.Font.Color := $00F4EFF8;
  WizardForm.MainPanel.Color := $00100908;
  WizardForm.InnerPage.Color := $00100908;
  WizardForm.WelcomePage.Color := $00100908;
  WizardForm.FinishedPage.Color := $00100908;
  WizardForm.WelcomeLabel1.Caption := 'R A Z E';
  WizardForm.WelcomeLabel2.Caption := 'Raze Macro  v{#AppVersion}' + #13#10 + #13#10 +
    'Development build' + #13#10 + 'Offline activation only';
  WizardForm.FinishedHeadingLabel.Caption := 'Installation Complete';
  WizardForm.FinishedLabel.Caption := 'Raze Macro is installed.' + #13#10 + 'Development activation: RAZE-DEV-2026';
  WizardForm.NextButton.Caption := 'Install';
end;
procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpWelcome then WizardForm.NextButton.Caption := 'Install';
  if CurPageID = wpInstalling then WizardForm.PageNameLabel.Caption := 'Installing...';
end;
