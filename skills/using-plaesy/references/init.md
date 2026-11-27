# plaesy init

Creates the `.plaesy/` marker directory that task, relay, session, and MCP commands look for.

## Syntax

```sh
plaesy init [PATH] [--dry-run]
```

- `PATH` defaults to the current directory. `--dry-run` reports without writing.
- Safe to repeat; existing `.plaesy/` contents are kept.
- Refuses to replace a non-directory `.plaesy` and refuses to nest inside another Plaesy project (a nested git repository counts as its own boundary).
- Creates only `.plaesy/`: no task folders, no `.gitignore` edits, no memory settings, no agent instruction files.

## Next steps

Decide whether `.plaesy/tasks/` is shared through git before committing it. Then `plaesy projects register` to make the project discoverable.

`plaesy init --help` is authoritative.
