# plaesy install

Copies the running binary to the per-user bin directory and puts that directory on the user PATH. No flags.

| Platform | Install path |
|---|---|
| Windows | `%LOCALAPPDATA%/Plaesy/bin/plaesy.exe`; moved to the front of user PATH |
| Unix | `$HOME/.local/bin/plaesy`; the active shell startup file (including Nushell `config.nu`) is updated |

Machine-wide PATH is never edited. If the PATH update fails, the binary stays installed and manual steps are printed.

## Syntax

```sh
plaesy install
```

From a source checkout: `cargo build -p plaesy --locked`, then `target/debug/plaesy.exe install` (or `make reload`).

## Safety

On Windows, "Access is denied" means a running `plaesy mcp` or `plaesy memory serve` holds the file. Stop it first.

## Verify

Open a new terminal and run `plaesy version`: the installed path and version match the build you ran.

`plaesy install --help` is authoritative.
