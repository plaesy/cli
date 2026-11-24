# `doctor`

```bash

plaesy doctor [platform] [--verbose]
```

Check the local install and memory vault, configured reach backends for
availability and credentials, and show the configured status of delegate
providers. Delegate provider status is informational; a disabled provider does
not make the doctor command fail.

The local section runs when no platform is given and works offline. It checks:

- **binary version**: the version is a well-formed `MAJOR.MINOR.PATCH`. It does
  not compare against the installed binary.
- **memory vault**: the configured vault folder exists and can be read.
- **daemon lock**: no `.plaesy-memory-daemon.lock` left behind by a crashed
  daemon. A lock not refreshed for more than 15 seconds fails.
- **free disk space**: at least 1 GiB free on the vault's volume (or the
  working directory's volume when no vault is usable).

Any failed check makes the command exit with status 1.

| Flag | Default | Description |
|---|---|---|
| `[platform]` | `all enabled platforms` | Check one platform (`web`, `github`, `youtube`, etc.) |
| `--verbose` | `false` | Show rate limit and cache stats |

**Examples**:

- `plaesy doctor` — full system health check
- `plaesy doctor web` — check one reach platform
- `plaesy doctor --verbose` — include rate limits and cache stats

See [command index](README.md).
