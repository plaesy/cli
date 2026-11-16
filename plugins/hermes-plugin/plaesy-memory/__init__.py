"""Launch Plaesy's Hermes memory service through the current CLI binary."""

from __future__ import annotations

import os
import sys
from pathlib import Path


def _find_binary() -> Path:
    configured = os.environ.get("PLAESY_CLI_BIN")
    if configured:
        path = Path(configured).expanduser()
        if path.is_file():
            return path
        raise FileNotFoundError(f"PLAESY_CLI_BIN does not exist: {path}")

    executable = "plaesy.exe" if sys.platform == "win32" else "plaesy"
    candidates = (
        Path(__file__).parent / "bin" / executable,
        Path(sys.executable).parent / executable,
    )
    for path in candidates:
        if path.is_file():
            return path

    from shutil import which

    resolved = which(executable)
    if resolved:
        return Path(resolved)
    raise FileNotFoundError(
        f"Could not find {executable}; set PLAESY_CLI_BIN to the CLI path"
    )


# The CLI owns this protocol. Keep the executable and subcommand together so
# the plugin works with the installed Plaesy CLI rather than a separate binary.
BIN = str(_find_binary())
BIN_ARGS = ["memory", "serve"]
