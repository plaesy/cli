# plaesy audit

Reads the audit trail of authorization decisions (approvals, tool calls) as JSON Lines, newest first.

## Syntax

```sh
plaesy audit list [--log <FILE>] [--limit 20] [--action <LABEL>] [--since <RFC3339>] [--until <RFC3339>]
```

| Option | Meaning |
|---|---|
| `--log` | Audit file; default `.plaesy/audit.log` in the project root |
| `--action` | Only entries with this label, for example `submit` or `tool:shell` |
| `--since`, `--until` | Time bounds such as `2026-11-01T00:00:00Z` |

## Example

```sh
plaesy audit list --action tool:shell --since 2026-11-27T00:00:00Z --limit 50
```

Treat the log as private project data.

`plaesy audit list --help` is authoritative.
