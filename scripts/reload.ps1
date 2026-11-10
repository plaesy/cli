# Build the debug CLI and install it using its supported install command.

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$sourceBinary = Join-Path $repoRoot 'target\debug\plaesy.exe'

# Verify debug binary exists
if (-not (Test-Path $sourceBinary)) {
    Write-Error "Binary not found: $sourceBinary"
    exit 1
}

& $sourceBinary install
if ($LASTEXITCODE -ne 0) {
    throw "Plaesy install failed with exit code $LASTEXITCODE"
}
Write-Host "Plaesy CLI installed from the current debug build. Restart clients that use it."
