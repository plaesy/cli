# `analyze`

```bash

plaesy analyze [project_path] [flags]
```

Generate AI-optimized project analysis, reports, and a dependency/symbol graph
under `.plaesy/analysis/`. Query flags read the existing graph and do not
trigger a rebuild; run `plaesy analyze` first to create or refresh it.

| Flag | Default | Description |
|---|---|---|
| `--no-graph` | `false` | Skip dependency graph build entirely |
| `--force` | `false` | Force full regeneration (bypass fingerprint check) |
| `--watch` | `false` | Re-run analyze automatically when source files change |
| `--watch-interval` | `3` | Poll interval (seconds) for `--watch` |

**Example**: `plaesy analyze` — analyzes the current directory.

See [CLI Reference](../reference.md).
