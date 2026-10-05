# Removes Open Controller: quits it the clean way, shows again any controller it hid, removes it
# from HidHide's program list, "Start with Windows", the Start menu, its files and its settings.
# The drivers (ViGEmBus, HidHide) stay; uninstall them from Windows Settings if you want.
$ErrorActionPreference = 'Stop'
$dest = Join-Path $env:LOCALAPPDATA 'Programs\open-controller'
$exe = Join-Path $dest 'open-controller.exe'
$data = Join-Path $env:APPDATA 'io.github.brunovncs.open-controller'

if (Test-Path $exe) {
    & $exe --quit | Out-Null
    Get-Process open-controller, open-controller-ui -ErrorAction SilentlyContinue | ForEach-Object { $_.Kill(); $_.WaitForExit(3000) | Out-Null }
    & $exe --restore | Out-Null
}

Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'Open Controller' -ErrorAction SilentlyContinue
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Open Controller.lnk'
if (Test-Path $shortcut) { Remove-Item -LiteralPath $shortcut }
if (Test-Path $dest) { Remove-Item -LiteralPath $dest -Recurse -Force }
if (Test-Path $data) { Remove-Item -LiteralPath $data -Recurse -Force }
Write-Host 'Open Controller removed'
