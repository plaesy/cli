$ErrorActionPreference = 'Stop'
$Owner = if ($env:PLAESY_GITHUB_OWNER) { $env:PLAESY_GITHUB_OWNER } else { 'plaesy' }
$CliRepo = if ($env:PLAESY_CLI_REPO) { $env:PLAESY_CLI_REPO } else { 'cli' }
$MemoryRepo = if ($env:PLAESY_MEMORY_REPO) { $env:PLAESY_MEMORY_REPO } else { 'memory' }
$Version = if ($env:PLAESY_VERSION) { $env:PLAESY_VERSION } else { 'latest' }
$InstallDir = if ($env:PLAESY_INSTALL_DIR) { $env:PLAESY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Plaesy\bin' }
$Arch = if ([Environment]::Is64BitOperatingSystem) { 'x86_64' } else { throw '32-bit Windows is not supported' }
$Temp = Join-Path ([IO.Path]::GetTempPath()) ("plaesy-{0}" -f [guid]::NewGuid())
New-Item -ItemType Directory -Force $InstallDir, $Temp | Out-Null
try {
  function Download-Asset($repo, $name, $out) {
    $url = if ($Version -eq 'latest') { "https://github.com/$Owner/$repo/releases/latest/download/$name" } else { "https://github.com/$Owner/$repo/releases/download/$Version/$name" }
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $out
  }
  Write-Host "Installing Plaesy for windows/$Arch..."
  Download-Asset $CliRepo "plaesy-windows-$Arch.exe" (Join-Path $Temp 'plaesy.exe')
  Download-Asset $MemoryRepo "memory-windows-$Arch.exe" (Join-Path $Temp 'memory.exe')
  Copy-Item (Join-Path $Temp 'plaesy.exe') (Join-Path $InstallDir 'plaesy.exe') -Force
  Copy-Item (Join-Path $Temp 'memory.exe') (Join-Path $InstallDir 'memory.exe') -Force
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (($userPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable('Path', (($userPath.TrimEnd(';') + ';' + $InstallDir).Trim(';')), 'User')
    Write-Host "Added $InstallDir to your user PATH. Open a new terminal."
  }
  & (Join-Path $InstallDir 'plaesy.exe') status
  Write-Host "Plaesy installed successfully. Open a new terminal, then run: plaesy --help"
} finally { Remove-Item $Temp -Recurse -Force -ErrorAction SilentlyContinue }
