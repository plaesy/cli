# `doctor`

```bash

plaesy doctor [platform] [--verbose]
```

Check configured reach backends for availability and credentials, and show
the configured status of delegate providers. Delegate provider status is
informational; a disabled provider does not make the doctor command fail.

| Flag | Default | Description |
|---|---|---|
| `[platform]` | `all enabled platforms` | Check one platform (`web`, `github`, `youtube`, etc.) |
| `--verbose` | `false` | Show rate limit and cache stats |

**Examples**:

- `plaesy doctor` — full system health check
- `plaesy doctor web` — check one reach platform
- `plaesy doctor --verbose` — include rate limits and cache stats

See [CLI Reference](../reference.md).
