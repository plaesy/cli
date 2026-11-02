# Plaesy

Plaesy is an umbrella CLI for the independent Plaesy products:

```text
plaesy memory ...
plaesy spec-kit ...
plaesy desktop ...
```

## Install for normal users

The installer downloads one verified release bundle containing both `plaesy` and the standalone
`memory` engine. Rust, Cargo, and Git are not required.

### macOS / Linux

```sh
curl -fsSL https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.sh | sh
```

For a pinned version:

```sh
export PLAESY_VERSION=v0.1.0
curl -fsSL https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.sh | sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.ps1 | iex
```

The installer verifies `SHA256SUMS` before installing. It installs into `~/.local/bin` on macOS/Linux
or `%LOCALAPPDATA%\Plaesy\bin` on Windows and updates the user PATH.

## Verify

```sh
plaesy status
plaesy --help
plaesy memory --help
```

## Developer installation

```sh
cargo install --git https://github.com/plaesy/cli.git --locked --bin plaesy
cargo install --git https://github.com/plaesy/memory.git --locked --bin memory
```

## Manual release without CI

Build from a maintainer machine that has the CLI and memory repositories checked out:

```text
projects/
├── cli/
└── memory/
```

Build and verify one platform:

```sh
cd projects/cli
./release/build-and-publish.sh v0.1.0 ../memory
./release/verify.sh dist/v0.1.0
```

For another Rust target:

```sh
./release/build-and-publish.sh v0.1.0 ../memory aarch64-apple-darwin
```

After all platform builds are collected under the same `dist/v0.1.0/` directory, publish them:

```sh
./release/publish.sh v0.1.0
```

The publish script creates or reuses one draft GitHub Release, uploads every verified platform
bundle, and stops on any upload failure. Inspect the draft, then publish it explicitly:

```sh
gh release edit v0.1.0 --repo plaesy/cli --draft=false
```

The build script produces a platform archive, `SHA256SUMS`, and `manifest.json`. It does not
silently ignore build failures. Build each platform separately, collect the artifacts, verify them,
then publish once.

Release assets:

```text
plaesy-linux-x86_64.tar.gz
plaesy-linux-arm64.tar.gz
plaesy-macos-x86_64.tar.gz
plaesy-macos-arm64.tar.gz
plaesy-windows-x86_64.zip
SHA256SUMS
manifest.json
```

The CLI and memory source repositories remain independent. Distribution combines their binaries
only at release packaging time.
