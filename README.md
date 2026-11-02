# Plaesy

Plaesy is an umbrella CLI for the Plaesy product family:

```text
plaesy memory ...
plaesy spec-kit ...
plaesy desktop ...
```

## Install for normal users

The installers download prebuilt release binaries. Rust, Cargo, and Git are not required.

### macOS / Linux

```sh
curl -fsSL https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.sh | sh
```

The installer installs `plaesy` and the standalone `memory` engine into `~/.local/bin`.
If the directory is not already on PATH, it prints the one-line fix and the user only needs to
open a new terminal.

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.ps1 | iex
```

The installer uses `%LOCALAPPDATA%\Plaesy\bin`, updates the user PATH, and installs both
`plaesy.exe` and `memory.exe`.

## Verify

```sh
plaesy status
plaesy --help
plaesy memory --help
```

## Developer installation

Developers can build from source:

```sh
cargo install --git https://github.com/plaesy/cli.git --locked --bin plaesy
cargo install --git https://github.com/plaesy/memory.git --locked --bin memory
```

## Release contract

Before publishing the installer URL, each matching GitHub release must contain these assets:

```text
plaesy-linux-x86_64
plaesy-linux-arm64
plaesy-macos-x86_64
plaesy-macos-arm64
plaesy-windows-x86_64.exe

memory-linux-x86_64
memory-linux-arm64
memory-macos-x86_64
memory-macos-arm64
memory-windows-x86_64.exe
```

The CLI and memory repositories remain independent; the installer is the single-product user
experience that combines their release artifacts.
