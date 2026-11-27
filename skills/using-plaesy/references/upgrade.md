# plaesy upgrade

Replaces the installed binary with the running, newer build. It refuses to replace the executable it is running from, so run it from the new build, not from the install directory.

## Syntax

```sh
<path-to-new-build>/plaesy upgrade
```

## Example

```sh
cargo build -p plaesy --locked
target/debug/plaesy.exe upgrade
plaesy version
```

## Safety

Stop running `plaesy mcp` and `plaesy memory serve` processes first on Windows; they lock the installed file.

`plaesy upgrade --help` is authoritative.
