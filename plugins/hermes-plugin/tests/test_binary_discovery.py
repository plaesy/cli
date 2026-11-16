"""Regression tests for the un-packaged Plaesy Hermes plugin bootstrap."""

from __future__ import annotations

import importlib.util
import os
import stat
from pathlib import Path
from types import ModuleType

PLUGIN = Path(__file__).parents[1] / "plaesy-memory" / "__init__.py"


def load_plugin(monkeypatch, **environment: str) -> ModuleType:
    """Load the plugin while applying a controlled process environment."""
    for key in ("PLAESY_CLI_BIN", "PATH"):
        monkeypatch.delenv(key, raising=False)
    for key, value in environment.items():
        monkeypatch.setenv(key, value)

    spec = importlib.util.spec_from_file_location("plaesy_hermes_plugin", PLUGIN)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_find_binary_prefers_configured_cli_path(tmp_path, monkeypatch):
    binary = tmp_path / "plaesy"
    binary.touch()

    module = load_plugin(monkeypatch, PLAESY_CLI_BIN=str(binary))

    assert module.BIN == str(binary)
    assert module.BIN_ARGS == ["memory", "serve"]


def test_find_binary_falls_back_to_path(tmp_path, monkeypatch):
    binary = tmp_path / "plaesy"
    binary.touch()
    binary.chmod(binary.stat().st_mode | stat.S_IXUSR)

    module = load_plugin(monkeypatch, PATH=str(tmp_path))

    assert module.BIN == str(binary)
    assert module.BIN_ARGS == ["memory", "serve"]


def test_find_binary_rejects_missing_configured_path(tmp_path, monkeypatch):
    missing = tmp_path / "missing-plaesy"

    try:
        load_plugin(monkeypatch, PLAESY_CLI_BIN=str(missing))
    except FileNotFoundError as error:
        assert str(missing) in str(error)
    else:
        raise AssertionError("missing PLAESY_CLI_BIN should fail")
