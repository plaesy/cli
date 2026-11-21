"""End-to-end tests for the Hermes adapter against the Plaesy CLI."""
from __future__ import annotations

import importlib.util
import os
import subprocess
from pathlib import Path
from types import ModuleType
from typing import Any

import pytest

PLUGIN = Path(__file__).parents[1] / "plaesy" / "relay.py"
BINARY = Path(__file__).parents[3] / "target" / "debug" / "plaesy"


def load_plugin(monkeypatch: Any, project: Path, agent_id: str = "worker-b") -> ModuleType:
    monkeypatch.setenv("PLAESY_CLI_BIN", str(BINARY.resolve()))
    monkeypatch.setenv("PLAESY_RELAY_AGENT_ID", agent_id)
    monkeypatch.setenv("PLAESY_RELAY_PROJECT", str(project))
    spec = importlib.util.spec_from_file_location("plaesy_relay_e2e", PLUGIN)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FakeContext:
    def __init__(self) -> None:
        self.tools: dict[str, Any] = {}
        self.hooks: dict[str, Any] = {}

    def register_tool(self, *, name: str, **kwargs: Any) -> None:
        self.tools[name] = kwargs["handler"]

    def register_hook(self, name: str, handler: Any) -> None:
        self.hooks[name] = handler


def run_cli(project: Path, *args: str) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        [str(BINARY.resolve()), "relay", "--db", str(project / ".plaesy" / "relay.sqlite"), *args],
        cwd=project,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    return result


@pytest.mark.skipif(not BINARY.is_file(), reason="build Plaesy CLI first")
def test_two_agent_plugin_send_receive_and_ack(monkeypatch, tmp_path):
    project = tmp_path / "project"
    project.mkdir()

    sender_module = load_plugin(monkeypatch, project, "worker-a")
    sender_ctx = FakeContext()
    sender_module.register(sender_ctx)
    sent = sender_ctx.tools["plaesy_relay_send"](
        {"recipient": "worker-b", "message": "handoff from A to B"}
    )
    assert '"success": true' in sent

    receiver_module = load_plugin(monkeypatch, project, "worker-b")
    receiver_ctx = FakeContext()
    receiver_module.register(receiver_ctx)
    delivery = receiver_ctx.hooks["pre_llm_call"](session_id="session-b")
    assert delivery is not None
    assert "handoff from A to B" in delivery["context"]
    receiver_ctx.hooks["post_llm_call"](session_id="session-b")

    inbox = run_cli(project, "inbox", "--agent", "worker-b").stdout
    assert inbox.strip() == "[]"


@pytest.mark.skipif(not BINARY.is_file(), reason="build Plaesy CLI first")
def test_restart_cannot_reclaim_active_lease_then_original_acknowledges(monkeypatch, tmp_path):
    project = tmp_path / "project"
    project.mkdir()
    body = "coordinate a test"
    run_cli(project, "send", "--id", "restart-1", "--from", "worker-a", "--to", "worker-b", body)

    first_module = load_plugin(monkeypatch, project)
    first_ctx = FakeContext()
    first_module.register(first_ctx)
    delivered = first_ctx.hooks["pre_llm_call"](session_id="session-b")
    assert delivered is not None
    assert body in delivered["context"]

    # A new plugin instance cannot claim a message while its lease is active.
    second_module = load_plugin(monkeypatch, project)
    second_ctx = FakeContext()
    second_module.register(second_ctx)
    assert second_ctx.hooks["pre_llm_call"](session_id="session-b") is None

    # The original completed turn acknowledges with its lease capability.
    first_ctx.hooks["post_llm_call"](session_id="session-b")
    inbox = run_cli(project, "inbox", "--agent", "worker-b").stdout
    assert inbox.strip() == "[]"
