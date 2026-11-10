$ErrorActionPreference = 'Stop'
$Owner = if ($env:PLAESY_GITHUB_OWNER) { $env:PLAESY_GITHUB_OWNER } else { 'plaesy' }
$Repo = if ($env:PLAESY_RELEASE_REPO) { $env:PLAESY_RELEASE_REPO } else { 'cli' }
$Version = if ($env:PLAESY_VERSION) { $env:PLAESY_VERSION } else { 'latest' }
$InstallDir = Join-Path $env:LOCALAPPDATA 'Plaesy\bin'
$Tag = if ($Version -eq 'latest') { 'latest' } elseif ($Version.StartsWith('v')) { $Version } else { "v$Version" }
$Base = "https://github.com/$Owner/$Repo/releases"
$Suffix = "windows-x86_64"
$Name = "plaesy-$Suffix.zip"
$Url = if ($Tag -eq 'latest') { "$Base/latest/download/$Name" } else { "$Base/download/$Tag/$Name" }
$SumsUrl = if ($Tag -eq 'latest') { "$Base/latest/download/SHA256SUMS" } else { "$Base/download/$Tag/SHA256SUMS" }
$Temp = Join-Path ([IO.Path]::GetTempPath()) ("plaesy-{0}" -f [guid]::NewGuid())
New-Item -ItemType Directory -Force $Temp | Out-Null
try {
  Write-Host "Installing Plaesy for windows/x86_64..."
  $HasLocalBundle = -not [string]::IsNullOrWhiteSpace($env:PLAESY_BUNDLE_PATH)
  $HasLocalSums = -not [string]::IsNullOrWhiteSpace($env:PLAESY_SHA256SUMS_PATH)
  if ($HasLocalBundle -or $HasLocalSums) {
    if (-not ($HasLocalBundle -and $HasLocalSums)) {
      throw 'PLAESY_BUNDLE_PATH and PLAESY_SHA256SUMS_PATH must be provided together'
    }
    if (-not (Test-Path -LiteralPath $env:PLAESY_BUNDLE_PATH -PathType Leaf)) {
      throw "bundle file not found: $env:PLAESY_BUNDLE_PATH"
    }
    if (-not (Test-Path -LiteralPath $env:PLAESY_SHA256SUMS_PATH -PathType Leaf)) {
      throw "checksum file not found: $env:PLAESY_SHA256SUMS_PATH"
    }
    Copy-Item -LiteralPath $env:PLAESY_BUNDLE_PATH -Destination (Join-Path $Temp $Name)
    Copy-Item -LiteralPath $env:PLAESY_SHA256SUMS_PATH -Destination (Join-Path $Temp 'SHA256SUMS')
  } else {
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile (Join-Path $Temp $Name)
    Invoke-WebRequest -UseBasicParsing -Uri $SumsUrl -OutFile (Join-Path $Temp 'SHA256SUMS')
  }
  $line = Get-Content (Join-Path $Temp 'SHA256SUMS') | Where-Object { $_ -match [regex]::Escape($Name) } | Select-Object -First 1
  if (-not $line) { throw "checksum entry is missing for $Name" }
  $expected = ($line -split '\s+')[0].ToLowerInvariant()
  $actual = (Get-FileHash (Join-Path $Temp $Name) -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($expected -ne $actual) { throw 'checksum mismatch' }
  Expand-Archive -Path (Join-Path $Temp $Name) -DestinationPath $Temp -Force
  if (-not (Test-Path (Join-Path $Temp 'plaesy.exe'))) { throw 'bundle must contain plaesy.exe' }
  $env:Path = "$InstallDir;$env:Path"
  & (Join-Path $Temp 'plaesy.exe') install
  if ($LASTEXITCODE -ne 0) { throw "Plaesy install failed with exit code $LASTEXITCODE" }
  & (Join-Path $InstallDir 'plaesy.exe') status
  if ($LASTEXITCODE -ne 0) { throw "Plaesy status failed with exit code $LASTEXITCODE" }
  Write-Host 'Plaesy installed successfully. Open a new terminal, then run: plaesy --help'
} finally { Remove-Item $Temp -Recurse -Force -ErrorAction SilentlyContinue }
