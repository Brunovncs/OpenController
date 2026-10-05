# Builds Open Controller and installs it for the current user in %LOCALAPPDATA%\Programs\open-controller,
# with a Start menu shortcut, then starts it. No administrator rights are needed. The drivers it
# uses, ViGEmBus and HidHide, are installed separately (see the README).
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

# GPUI's release build precompiles its shaders with fxc.exe from the Windows SDK. Without the SDK,
# the `local` profile builds the same optimized program and compiles them at start-up instead.
$fxc = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter fxc.exe -ErrorAction SilentlyContinue | Select-Object -First 1
$profile = if ($fxc) { 'release' } else { 'local' }
cargo build --profile $profile
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$dest = Join-Path $env:LOCALAPPDATA 'Programs\open-controller'
New-Item -ItemType Directory -Force $dest | Out-Null

# A running copy is asked to quit first, so it unplugs its virtual controllers and shows the
# hidden ones again before its files are replaced.
$exe = Join-Path $dest 'open-controller.exe'
if (Test-Path $exe) { & $exe --quit | Out-Null }
Get-Process open-controller-ui -ErrorAction SilentlyContinue | ForEach-Object { $_.Kill(); $_.WaitForExit(3000) | Out-Null }

Copy-Item "target\$profile\open-controller.exe", "target\$profile\open-controller-ui.exe" $dest -Force

$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Open Controller.lnk'
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = $exe
$link.WorkingDirectory = $dest
$link.Description = 'Any controller, any connection, as an Xbox controller'
$link.Save()

Start-Process $exe
Write-Host "Open Controller installed to $dest"
