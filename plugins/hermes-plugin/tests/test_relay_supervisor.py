from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT = Path(__file__).parents[1] / "relay-supervisor.py"
spec = importlib.util.spec_from_file_location("relay_supervisor", SCRIPT)
assert spec is not None and spec.loader is not None
supervisor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(supervisor)


def test_message_is_acked_only_after_successful_hermes_turn(tmp_path: Path) -> None:
    calls: list[list[str]] = []
    timeouts: list[int | None] = []

    def run(command: list[str], *, cwd: Path, input_text: str | None = None, timeout: int | None = None):
        timeouts.append(timeout)
        del cwd, input_text
        calls.append(command)
        if "claim" in command:
            return subprocess.CompletedProcess(command, 0, json.dumps({
                "id": "m-1", "sender": "a", "recipient": "b", "body": "hello",
                "lease_owner": "opaque-token", "lease_expiry_epoch_ms": 123456789
            }), "")
        return subprocess.CompletedProcess(command, 0, "done", "")

    processed = supervisor.process_one(
        plaesy="plaesy", hermes="hermes", profile="isolated", project=tmp_path,
        agent="b", timeout=1, poll_interval=10, run=run,
    )

    assert processed is True
    assert calls[1][0:5] == ["hermes", "-p", "isolated", "--in", str(tmp_path)]
    assert "-z" in calls[1]
    assert "untrusted-relay-message" in calls[1][-1]
    assert "--continue" in calls[1]
    session_name = calls[1][calls[1].index("--continue") + 1]
    assert session_name == supervisor._session_name("m-1")
    assert "--create-if-missing" in calls[1]
    assert "Message ID: m-1" in calls[1][-1]
    assert calls[2][-7:] == ["ack", "--id", "m-1", "--agent", "b", "--lease-token", "opaque-token"]
    assert calls[2][0:4] == ["plaesy", "relay", "--db", str(tmp_path / ".plaesy" / "relay.sqlite")]
    assert timeouts == [None, 300, None]


def test_zero_timeout_waits_indefinitely_until_a_message_is_claimed(tmp_path: Path) -> None:
    calls: list[list[str]] = []
    claim_count = 0

    def run(command: list[str], *, cwd: Path, input_text: str | None = None, timeout: int | None = None):
        nonlocal claim_count
        del cwd, input_text, timeout
        calls.append(command)
        if "claim" in command:
            claim_count += 1
            payload = None if claim_count == 1 else {
                "id": "m-infinite", "sender": "a", "recipient": "b", "body": "hello",
                "lease_owner": "opaque-token", "lease_expiry_epoch_ms": 123456789
            }
            return subprocess.CompletedProcess(command, 0, json.dumps(payload), "")
        if "wait" in command:
            assert command[command.index("--timeout") + 1] == "0"
            return subprocess.CompletedProcess(command, 0, json.dumps({"id": "observed"}), "")
        return subprocess.CompletedProcess(command, 0, "done", "")

    assert supervisor.process_one(
        plaesy="plaesy", hermes="hermes", profile="isolated", project=tmp_path,
        agent="b", timeout=0, poll_interval=10, run=run,
    ) is True
    assert ["wait" in call for call in calls].count(True) == 1


def test_retries_claim_after_competing_consumer_wins_wake_race(tmp_path: Path) -> None:
    calls: list[list[str]] = []
    claim_count = 0

    def run(command: list[str], *, cwd: Path, input_text: str | None = None, timeout: int | None = None):
        nonlocal claim_count
        del cwd, input_text, timeout
        calls.append(command)
        if "claim" in command:
            claim_count += 1
            payload = None if claim_count < 3 else {
                "id": "m-race", "sender": "a", "recipient": "b", "body": "hello",
                "lease_owner": "opaque-token", "lease_expiry_epoch_ms": 123456789
            }
            return subprocess.CompletedProcess(command, 0, json.dumps(payload), "")
        if "wait" in command:
            # The message returned by wait can already be claimed by another consumer.
            return subprocess.CompletedProcess(command, 0, json.dumps({"id": "observed"}), "")
        return subprocess.CompletedProcess(command, 0, "done", "")

    processed = supervisor.process_one(
        plaesy="plaesy", hermes="hermes", profile="isolated", project=tmp_path,
        agent="b", timeout=1, poll_interval=10, run=run,
    )
    assert processed is True
    assert ["claim" in call for call in calls].count(True) == 3
    assert ["wait" in call for call in calls].count(True) == 2
    assert any("m-race" in call for call in calls if "ack" in call)


def test_supervisor_lock_rejects_second_process(tmp_path: Path) -> None:
    lock = supervisor._SupervisorLock(tmp_path, "worker/b")
    with lock:
        assert lock.path.exists()
        code = (
            "import importlib.util, pathlib; "
            f"spec=importlib.util.spec_from_file_location('relay_supervisor', {str(SCRIPT)!r}); "
            "module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); "
            f"lock=module._SupervisorLock(pathlib.Path({str(tmp_path)!r}), 'worker/b'); "
            "lock.__enter__()"
        )
        result = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True)
        assert result.returncode != 0
        assert "already running" in result.stderr
    with supervisor._SupervisorLock(tmp_path, "worker/b"):
        pass


def test_session_isolation_lock_name_is_safe(tmp_path: Path) -> None:
    first = supervisor._lock_path(tmp_path, "worker/b")
    second = supervisor._lock_path(tmp_path, "worker b")
    assert first != second
    assert first.parent == tmp_path / ".plaesy"


def test_session_name_is_stable_and_sanitized() -> None:
    first = supervisor._session_name("unsafe/id with spaces")
    assert first == supervisor._session_name("unsafe/id with spaces")
    assert first.startswith("plaesy-relay-unsafe-id-with-spaces-")
    assert "/" not in first
    assert " " not in first


def test_timed_out_hermes_turn_leaves_message_pending(tmp_path: Path) -> None:
    calls: list[list[str]] = []

    def run(command: list[str], *, cwd: Path, input_text: str | None = None, timeout: int | None = None):
        del cwd, input_text
        calls.append(command)
        if "claim" in command:
            return subprocess.CompletedProcess(command, 0, json.dumps({
                "id": "m-timeout", "sender": "a", "recipient": "b", "body": "hello",
                "lease_owner": "opaque-token", "lease_expiry_epoch_ms": 123456789
            }), "")
        assert timeout == 5
        raise subprocess.TimeoutExpired(command, timeout=timeout)

    try:
        supervisor.process_one(
            plaesy="plaesy", hermes="hermes", profile="isolated", project=tmp_path,
            agent="b", timeout=1, poll_interval=10, hermes_timeout=5, run=run,
        )
    except subprocess.TimeoutExpired:
        pass
    else:
        raise AssertionError("timed-out Hermes turn must propagate")

    assert len(calls) == 2
    assert "ack" not in calls[-1]


def test_run_enforces_process_timeout(monkeypatch) -> None:
    seen = {}

    def fake_run(command, **kwargs):
        seen.update(kwargs)
        raise subprocess.TimeoutExpired(command, timeout=kwargs["timeout"])

    monkeypatch.setattr(supervisor.subprocess, "run", fake_run)
    try:
        supervisor._run(["hermes", "-z", "prompt"], cwd=Path("."), timeout=5)
    except subprocess.TimeoutExpired:
        pass
    else:
        raise AssertionError("subprocess timeout should propagate")

    assert seen["timeout"] == 5


def test_failed_hermes_turn_leaves_message_pending(tmp_path: Path) -> None:
    calls: list[list[str]] = []

    def run(command: list[str], *, cwd: Path, input_text: str | None = None, timeout: int | None = None):
        assert timeout in (None, 300)
        del cwd, input_text
        calls.append(command)
        if "claim" in command:
            return subprocess.CompletedProcess(command, 0, json.dumps({
                "id": "m-2", "sender": "a", "recipient": "b", "body": "hello",
                "lease_owner": "opaque-token", "lease_expiry_epoch_ms": 123456789
            }), "")
        return subprocess.CompletedProcess(command, 1, "", "failed")

    try:
        supervisor.process_one(
            plaesy="plaesy", hermes="hermes", profile="isolated", project=tmp_path,
            agent="b", timeout=1, poll_interval=10, run=run,
        )
    except RuntimeError as exc:
        assert "message remains pending" in str(exc)
    else:
        raise AssertionError("failed Hermes turn must not be acknowledged")

    assert len(calls) == 2
    assert "ack" not in calls[-1]
