# Removes OpenController: quits it the clean way, shows again any controller it hid, removes it
# from HidHide's program list, "Start with Windows", the Start menu, its files and its settings.
# The drivers (ViGEmBus, HidHide) stay; uninstall them from Windows Settings if you want.
$ErrorActionPreference = 'Stop'
$dest = Join-Path $env:LOCALAPPDATA 'Programs\open-controller'
$exe = Join-Path $dest 'open-controller.exe'
$data = Join-Path $env:APPDATA 'io.github.brunovncs.open-controller'

# A native program's exit code does not stop the script, so --restore's is checked: when it fails,
# the settings folder, which holds the list of what was hidden, stays for the next install.
$restored = -not (Test-Path (Join-Path $data 'hidhide-journal.json'))
if (Test-Path $exe) {
    & $exe --quit | Out-Null
    # Only this session's copies: as administrator this would end other users' copies too.
    $session = (Get-Process -Id $PID).SessionId
    Get-Process open-controller, open-controller-ui -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session |
        ForEach-Object { $_.Kill(); $_.WaitForExit(3000) | Out-Null }
    & $exe --restore | Out-Null
    $restored = $LASTEXITCODE -eq 0
}

foreach ($name in 'OpenController', 'Open Controller') {
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name $name -ErrorAction SilentlyContinue
}
foreach ($name in 'OpenController.lnk', 'Open Controller.lnk') {
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) $name
    if (Test-Path $shortcut) { Remove-Item -LiteralPath $shortcut }
}
if (Test-Path $dest) { Remove-Item -LiteralPath $dest -Recurse -Force }
if (-not $restored) {
    Write-Warning "OpenController could not show your controllers to games again: HidHide was busy or did not answer. Its settings were kept in $data, with the list of what it hid, so installing OpenController again brings them back. You can also take them off the list in HidHide Configuration Client."
    exit 1
}
if (Test-Path $data) { Remove-Item -LiteralPath $data -Recurse -Force }
Write-Host 'OpenController removed'
