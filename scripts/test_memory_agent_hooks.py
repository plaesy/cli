"""Focused checks for the shared memory hook installation."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPTS = Path(__file__).parent


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


hook = load("memory_agent_hook", SCRIPTS / "memory_agent_hook.py")
installer = load("install_memory_agent_hooks", SCRIPTS / "install_memory_agent_hooks.py")


class MemoryAgentHooksTest(unittest.TestCase):
    def test_stop_continues_only_once(self):
        initial = hook.response({"hook_event_name": "Stop", "last_assistant_message": "Done"})
        self.assertEqual(initial["decision"], "block")
        repeated = hook.response(
            {"hook_event_name": "Stop", "last_assistant_message": "Done", "stop_hook_active": True}
        )
        self.assertEqual(repeated, {})
        self.assertEqual(hook.response({"hook_event_name": "Stop"}), {})

    def test_install_preserves_other_hooks_and_is_idempotent(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            codex = home / ".codex" / "hooks.json"
            codex.parent.mkdir()
            codex.write_text(
                json.dumps({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "other"}]}]}}),
                encoding="utf-8",
            )
            first = installer.install(home, Path("python"))
            self.assertTrue(all(path.exists() for path in first))
            data = json.loads(codex.read_text(encoding="utf-8"))
            self.assertEqual(data["hooks"]["Stop"][0]["hooks"][0]["command"], "other")
            self.assertEqual(len(data["hooks"]["Stop"]), 2)
            installer.install(home, Path("python"))
            data = json.loads(codex.read_text(encoding="utf-8"))
            self.assertEqual(len(data["hooks"]["Stop"]), 2)


if __name__ == "__main__":
    unittest.main()
