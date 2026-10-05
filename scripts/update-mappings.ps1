# Refreshes crates/core/assets/gamecontrollerdb.txt: the Windows entries of the community
# SDL_GameControllerDB, which SDL loads on top of its own built-in mappings.
$ErrorActionPreference = 'Stop'
$repo = 'https://github.com/mdqinc/SDL_GameControllerDB'
$raw = (Invoke-WebRequest 'https://raw.githubusercontent.com/mdqinc/SDL_GameControllerDB/master/gamecontrollerdb.txt' -UseBasicParsing).Content
$commit = (git ls-remote $repo HEAD).Split()[0]
$lines = $raw -split "`n" | ForEach-Object { $_.TrimEnd("`r") } | Where-Object { $_ -match 'platform:Windows,' }
$header = "# Windows entries of SDL_GameControllerDB ($repo),`n# commit $commit, zlib license. Refresh with scripts/update-mappings.ps1.`n"
$out = Join-Path $PSScriptRoot '..\crates\core\assets\gamecontrollerdb.txt'
[IO.File]::WriteAllText($out, $header + ($lines -join "`n") + "`n")
Write-Host "$($lines.Count) mappings from $commit"
