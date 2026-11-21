$ErrorActionPreference = 'Stop'
# Runs only on a disposable GitHub-hosted Windows runner.
$root = [IO.Path]::GetFullPath((Join-Path $env:RUNNER_TEMP 'Raze-Install-Test'))
$runnerRoot = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (!$root.StartsWith($runnerRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Test directory escaped runner temp' }
$setup = (Resolve-Path 'dist/Raze-Macro-Setup.exe').Path
$install = Start-Process -FilePath $setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', ('/DIR="' + $root + '"'), '/TASKS=desktopicon' -PassThru -Wait -WindowStyle Hidden
if ($install.ExitCode -ne 0) { throw "Install failed: $($install.ExitCode)" }
if (!(Test-Path -LiteralPath (Join-Path $root 'Raze-Macro.exe'))) { throw 'Missing application payload' }
if (!(Test-Path -LiteralPath (Join-Path $root 'unins000.exe'))) { throw 'Missing uninstaller' }
$desktop = [Environment]::GetFolderPath('Desktop')
if (!(Test-Path -LiteralPath (Join-Path $desktop 'Raze Macro.lnk'))) { throw 'Missing desktop shortcut' }
$start = Join-Path ([Environment]::GetFolderPath('Programs')) 'Raze Macro\Raze Macro.lnk'
if (!(Test-Path -LiteralPath $start)) { throw 'Missing Start Menu shortcut' }
$uninstall = Start-Process -FilePath (Join-Path $root 'unins000.exe') -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -PassThru -Wait -WindowStyle Hidden
if ($uninstall.ExitCode -ne 0) { throw "Uninstall failed: $($uninstall.ExitCode)" }
if (Test-Path -LiteralPath (Join-Path $root 'Raze-Macro.exe')) { throw 'Uninstall left application payload' }
if (Test-Path -LiteralPath $start) { throw 'Uninstall left Start Menu shortcut' }
if (Test-Path -LiteralPath (Join-Path $desktop 'Raze Macro.lnk')) { throw 'Uninstall left desktop shortcut' }
'PASS: silent install, payload, Start Menu shortcut, optional desktop shortcut, uninstaller, removal' | Set-Content dist/install-test.txt
(Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash + '  Raze-Macro-Setup.exe' | Set-Content dist/SHA256SUMS.txt
