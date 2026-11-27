# plaesy status

Shows the CLI version and environment: git availability, the git repository root when inside a checkout, whether the managed binary is installed and its path, whether its directory is on the current process PATH, and the running executable's path.

## Syntax

```sh
plaesy status
```

## Use

Run it first when a command behaves unexpectedly. The PATH check reflects the current process; after changing the persistent user PATH, open a new terminal before trusting it. For the running and installed paths side by side, use `plaesy version`.

`plaesy status --help` is authoritative.
