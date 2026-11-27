# plaesy repair

Copies the running binary back to the install directory when the installed copy is missing or different, and updates the user PATH only when needed.

## Syntax

```sh
plaesy repair
```

## Use

Run it from a known-good build when `plaesy status` reports a missing install or a PATH problem. On Windows, stop processes holding `plaesy.exe` first. Verify with `plaesy version` in a new terminal.

`plaesy repair --help` is authoritative.
