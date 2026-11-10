<div align="center" style="background-color: #f8f9fa; padding: 20px; border-radius: 10px; margin: 20px 0;">
  <img src="https://raw.githubusercontent.com/plaesy/.github/refs/heads/main/assets/img/Logo.svg" alt="Plaesy CLI" />
</div>

# Plaesy: CLI

**Version:** 0.0.1

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) [![Rust](<https://img.shields.io/badge/built%20with-Rust-00ADD8.svg>)](https://rust-lang.org/) [![Status: Early](<https://img.shields.io/badge/status-early%20(0.0.x)-orange.svg>)](VERSION)

## Install

The release workflow builds platform archives and creates a draft GitHub Release when a `v*` tag is
pushed. Publish that draft before using the `latest` installer URL. Until a release is published,
install from source using the instructions below.

### macOS / Linux

```sh
curl -fsSL https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.sh | sh
```

For a pinned version:

```sh
export PLAESY_VERSION=v0.0.1
curl -fsSL https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.sh | sh
```

### Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/plaesy/cli/main/scripts/install.ps1 | iex
```

The installers verify the downloaded release archive against `SHA256SUMS`, install the single
`plaesy` executable, and add its install directory to the current user's PATH. On macOS/Linux they
add a guarded PATH entry to the active shell startup file, including Nushell's `config.nu`; on
Windows they put the managed install directory first in the user PATH so a stale same-name
executable cannot shadow the installed CLI.
Open a new terminal after installation. The memory commands are subcommands of this same executable.
When you already have a local build, `plaesy install` performs the same managed install and PATH
setup without downloading anything.

GitHub publishes artifact provenance for each release bundle. To verify a downloaded archive, install GitHub
CLI and run `gh attestation verify <archive-path> -R plaesy/cli`; the SHA256 check in the installer
protects transfer integrity, while the attestation identifies the workflow and source that produced
the archive.

## Verify

```sh
plaesy status
plaesy --help
plaesy memory --help
```

The `memory` commands run inside the `plaesy` executable; a separate `memory` program is not
required. `plaesy memory` stores a per-vault index under the operating system's local application
data directory, separate from the Markdown vault. Each device builds its own SQLite index from the
synced notes. `plaesy uninstall` removes the binary and PATH entry but keeps local memory data.

Create a standard note or create a managed note from the selected vault's template, without a
vault-specific shell script:

```sh
plaesy memory remember "Memory scopes" "A note body" --folder knowledges/system-design
plaesy memory --vault ~/vault remember "Memory scopes" --template knowledge --folder knowledges/system-design --set ONE_SENTENCE_SUMMARY="Memory scope defines who can read a note."
```

For standard notes, omit the body to read Markdown from stdin. `--vault` selects another vault and
`--project` selects the current repository's `.plaesy/`; both override environment vault selectors.
Without those flags, vault selection checks `MEMORY_WATCH_DIR`, `PLAESY_MEMORY_VAULT`, configured
vault paths, then the shared `~/_` and repository `.plaesy/` fallbacks. A supplied `--config`
replaces the standard TOML lookup and controls the vault unless a CLI or environment selector
overrides it. Explicit database paths are honored; otherwise the CLI keeps a local index per vault.
`--type` and `--id` add Plaesy graph metadata. With `--template`, the command reads the matching
file from `<vault>/templates/`, generates the managed-note front matter and UUID, and supports
repeatable `--set KEY=VALUE` placeholder substitutions. `plaesy memory remember` is the single CLI
command for creating memory notes. See the [memory command reference](docs/commands/memory.md) for
vault selection, indexing, hygiene, and other memory commands.

## Build from source

```sh
git clone https://github.com/plaesy/cli.git
cd cli
cargo install --path . --locked --bin plaesy
```

For a normal user install, use the release installer above instead of building from source.
