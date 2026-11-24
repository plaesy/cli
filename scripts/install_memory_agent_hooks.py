"""Install Plaesy memory review hooks for all Codex and Claude projects."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(data, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def install_handler(config_path: Path, command: str) -> bool:
    config = json.loads(config_path.read_text(encoding="utf-8")) if config_path.exists() else {}
    if not isinstance(config, dict):
        raise ValueError(f"expected JSON object in {config_path}")
    hooks = config.setdefault("hooks", {})
    if not isinstance(hooks, dict):
        raise ValueError(f"expected hooks object in {config_path}")
    changed = False
    for event in ("SessionStart", "Stop"):
        groups = hooks.setdefault(event, [])
        if not isinstance(groups, list):
            raise ValueError(f"expected {event} array in {config_path}")
        installed = any(
            isinstance(group, dict)
            and isinstance(group.get("hooks"), list)
            and any(
                isinstance(handler, dict) and handler.get("command") == command
                for handler in group["hooks"]
            )
            for group in groups
        )
        if not installed:
            groups.append({"hooks": [{"type": "command", "command": command, "timeout": 10}]})
            changed = True
    if changed:
        write_json(config_path, config)
    return changed


def install(home: Path, python: Path) -> list[Path]:
    shared = home / ".agents"
    hook_path = shared / "hooks" / "memory_agent_hook.py"
    skill_path = shared / "skills" / "learning-loop" / "SKILL.md"
    claude_skill = home / ".claude" / "skills" / "learning-loop" / "SKILL.md"
    for destination, source in (
        (hook_path, ROOT / "scripts" / "memory_agent_hook.py"),
        (skill_path, ROOT / "skills" / "learning-loop" / "SKILL.md"),
        (claude_skill, ROOT / "skills" / "learning-loop" / "SKILL.md"),
    ):
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    command = f'"{python.as_posix()}" "{hook_path.as_posix()}"'
    configs = (home / ".codex" / "hooks.json", home / ".claude" / "settings.json")
    for config_path in configs:
        install_handler(config_path, command)
    return [hook_path, skill_path, claude_skill, *configs]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--home", type=Path, default=Path.home())
    args = parser.parse_args()
    for path in install(args.home, Path(sys.executable)):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
