"""Codex and Claude lifecycle hook for automatic Plaesy memory review.

The hook never copies conversation text into the vault. It asks the active agent
to retrieve project memory on entry and to classify durable outcomes before its
first stop, so the agent can verify claims against the project's source files.
"""

from __future__ import annotations

import json
import sys
from typing import Any


START_CONTEXT = (
    "Plaesy shared memory is enabled for this user. Before substantive work, "
    "identify this project and retrieve its current project, decision, and context "
    "notes with Plaesy Memory. Verify consequential claims against the project's "
    "own files. At task closeout, apply the global learning-loop skill; preserve "
    "durable project status, context, lessons, knowledge, and decisions without "
    "asking the user to run memory commands."
)

STOP_CONTEXT = (
    "Complete the learning-loop closeout before your final response. Search the "
    "project's existing documentation and Plaesy Memory. Update the project's "
    "source of truth first. For a substantive reusable outcome, persist the "
    "appropriate project context, lesson, knowledge, or decision in the user's "
    "configured Plaesy vault, with a source reference. For routine work with no "
    "durable outcome, record nothing. Do this yourself; do not ask the user to "
    "run commands. Then finish the response."
)


def response(event: dict[str, Any]) -> dict[str, Any]:
    name = event.get("hook_event_name")
    if name == "SessionStart":
        return {
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": START_CONTEXT,
            }
        }
    if name == "Stop" and not event.get("stop_hook_active"):
        message = event.get("last_assistant_message")
        if isinstance(message, str) and message.strip():
            return {"decision": "block", "reason": STOP_CONTEXT}
    return {}


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except (ValueError, UnicodeError):
        payload = {}
    result = response(payload if isinstance(payload, dict) else {})
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
