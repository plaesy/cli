# plaesy version

Prints the running version, the running binary's path, and the installed binary's path (or that none is installed). No network access. `plaesy --version` prints only the version.

## Syntax

```sh
plaesy version
```

## Use

Compare the two paths. When the installed binary is older than the source you are working on, run a source build directly (`target/debug/plaesy.exe` after `cargo build -p plaesy --locked`) or replace the install with `upgrade`.

`plaesy version --help` is authoritative.
