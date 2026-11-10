# `install`

```bash

plaesy install
```

Install the `plaesy` CLI to a well-known bin directory. Copies the currently
running binary to `%LOCALAPPDATA%\Plaesy\bin\plaesy.exe` (Windows) or
`$HOME/.local/bin/plaesy` (Unix), then adds that directory to the current user's
PATH when possible. On Windows, it moves the managed directory to the start of user PATH so an
older same-name executable cannot shadow this install. The command never edits machine-wide PATH.
On Unix, it updates the active shell startup file, including Nushell's
`config.nu`; on Windows, it updates the user PATH. Open a new terminal after
installation. If the PATH update fails, it prints manual instructions and
leaves the installed binary in place.

**No flags.** Detects the running executable and copies it into the managed install directory.

## Clean-install validation

The CI workflow starts with a clean OS account on Linux, macOS, and Windows and uses each runner's
actual home, user configuration, and PATH. It runs the platform installer against the bundle and
checksum produced in that job, then checks PATH discovery from a new shell, commands from the
installed binary, repeat installation, repair, upgrade, uninstall, PATH cleanup, and preservation of
user data. The release workflow repeats this against every OS/architecture bundle before it can
create a draft release.

Both installer scripts accept `PLAESY_BUNDLE_PATH` and `PLAESY_SHA256SUMS_PATH` together to install
a local bundle while retaining the same checksum, extraction, and CLI install steps. CI uses this
path so it can test an unpublished build without a network release.

The opt-in post-install test is `cargo test --locked --test install_e2e -- --ignored --nocapture`.
It writes the current user's persistent PATH and briefly creates a uniquely named sentinel in
Plaesy's user data directory, so run it only on a disposable CI runner or when intentionally testing
that account. It requires `PLAESY_INSTALL_E2E=1` and `PLAESY_E2E_BINARY` to point to the extracted
bundle binary; the platform installer must have run first. Ordinary `cargo test --locked` skips this
test.

For local development, `make reload` builds the debug binary and runs its `install` command. This
replaces the binary in the normal per-user install directory and ensures that directory is on the
user PATH; it does not create a separate development install path.

See [CLI Reference](../reference.md).
