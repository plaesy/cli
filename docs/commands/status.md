# `status`

```bash

plaesy status
```

Show Plaesy CLI version, environment, and installation state. Reports:

- git availability
- resolved git repository root when run inside a Git checkout
- whether the managed binary is installed, its path, and whether its directory is on the current process PATH
- the path of the running executable when the operating system provides it

The PATH check reports the current process environment. After changing the persistent user PATH,
open a new terminal before relying on this check.

See [CLI Reference](../reference.md).
