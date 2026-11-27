# plaesy doctor

Health checks for the local install, the memory vault, reach platforms, and delegate providers.

## Syntax

```sh
plaesy doctor [PLATFORM] [--verbose]
```

Without a platform, the local section runs offline and checks:

- the binary version is well formed;
- the configured vault exists and is readable;
- no stale `.plaesy-memory-daemon.lock` (not refreshed for more than 15 seconds);
- at least 1 GiB free on the vault's volume.

It then checks enabled reach platforms and shows delegate provider status (informational). Any failed check exits with status 1. `--verbose` adds rate limits and cache statistics.

## Examples

```sh
plaesy doctor
plaesy doctor github --verbose
```

`plaesy doctor --help` is authoritative.
