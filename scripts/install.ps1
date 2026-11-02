$ErrorActionPreference = 'Stop'
$Owner = if ($env:PLAESY_GITHUB_OWNER) { $env:PLAESY_GITHUB_OWNER } else { 'plaesy' }
$Repo = if ($env:PLAESY_RELEASE_REPO) { $env:PLAESY_RELEASE_REPO } else { 'cli' }
$Version = if ($env:PLAESY_VERSION) { $env:PLAESY_VERSION } else { 'latest' }
$InstallDir = if ($env:PLAESY_INSTALL_DIR) { $env:PLAESY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Plaesy\bin' }
$Tag = if ($Version -eq 'latest') { 'latest' } elseif ($Version.StartsWith('v')) { $Version } else { "v$Version" }
$Base = "https://github.com/$Owner/$Repo/releases"
$Suffix = "windows-x86_64"
$Name = "plaesy-$Suffix.zip"
$Url = if ($Tag -eq 'latest') { "$Base/latest/download/$Name" } else { "$Base/download/$Tag/$Name" }
$SumsUrl = if ($Tag -eq 'latest') { "$Base/latest/download/SHA256SUMS" } else { "$Base/download/$Tag/SHA256SUMS" }
$Temp = Join-Path ([IO.Path]::GetTempPath()) ("plaesy-{0}" -f [guid]::NewGuid())
New-Item -ItemType Directory -Force $InstallDir, $Temp | Out-Null
try {
  Write-Host "Installing Plaesy for windows/x86_64..."
  Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile (Join-Path $Temp $Name)
  Invoke-WebRequest -UseBasicParsing -Uri $SumsUrl -OutFile (Join-Path $Temp 'SHA256SUMS')
  $line = Get-Content (Join-Path $Temp 'SHA256SUMS') | Where-Object { $_ -match [regex]::Escape($Name) } | Select-Object -First 1
  if (-not $line) { throw "checksum entry is missing for $Name" }
  $expected = ($line -split '\s+')[0].ToLowerInvariant()
  $actual = (Get-FileHash (Join-Path $Temp $Name) -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($expected -ne $actual) { throw 'checksum mismatch' }
  Expand-Archive -Path (Join-Path $Temp $Name) -DestinationPath $Temp -Force
  if (-not (Test-Path (Join-Path $Temp 'plaesy.exe')) -or -not (Test-Path (Join-Path $Temp 'memory.exe'))) { throw 'bundle must contain plaesy.exe and memory.exe' }
  Copy-Item (Join-Path $Temp 'plaesy.exe') (Join-Path $InstallDir 'plaesy.exe') -Force
  Copy-Item (Join-Path $Temp 'memory.exe') (Join-Path $InstallDir 'memory.exe') -Force
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (($userPath -split ';') -notcontains $InstallDir) { [Environment]::SetEnvironmentVariable('Path', (($userPath.TrimEnd(';') + ';' + $InstallDir).Trim(';')), 'User'); Write-Host "Added $InstallDir to your user PATH. Open a new terminal." }
  & (Join-Path $InstallDir 'plaesy.exe') status
  Write-Host 'Plaesy installed successfully. Open a new terminal, then run: plaesy --help'
} finally { Remove-Item $Temp -Recurse -Force -ErrorAction SilentlyContinue }