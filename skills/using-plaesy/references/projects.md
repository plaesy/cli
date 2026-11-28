# plaesy projects

Per-user registry of local Plaesy projects, used by `sessions list` and `workspace`. Registration is explicit; Plaesy does not scan the disk. The registry lives in the per-user Plaesy data directory, not in the project.

## Syntax

```sh
plaesy projects register [PATH]   # default: the current project root; must contain .plaesy/
plaesy projects list              # registered projects that still exist
```

Each path is canonicalized and gets a deterministic id derived from it. A moved project disappears from `list`; register the new root.

## Example

```sh
plaesy init
plaesy projects register
plaesy projects list
```

## Verify

`plaesy projects list` shows the root and its id.

`plaesy projects <sub> --help` is authoritative.
