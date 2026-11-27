# plaesy update

Compares the installed version with the `version` field of a local JSON manifest. It never contacts a release server and never downloads anything.

## Syntax

```sh
plaesy update --check --manifest <PATH>
```

`--check` is required; `plaesy update` without it fails because downloading is not implemented. The command exits 0 whether the install is up to date, behind, or ahead, and exits 1 only on an error such as a missing or invalid manifest.

## Example

```sh
printf '{"version": "0.0.2"}\n' > manifest.json
plaesy update --check --manifest manifest.json
```

To apply a newer build, build or fetch it and run its `upgrade`.

`plaesy update --help` is authoritative.
