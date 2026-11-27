# plaesy chat-schedule

Installs per-user OS timers (Task Scheduler, systemd user timers, or LaunchAgents) that rerun a saved chat session's goal at an interval.

## Syntax

```sh
plaesy chat-schedule install <SESSION> [--every 30m] [--path .] [--allow-writes] [--supervised]
plaesy chat-schedule list [--path .]
plaesy chat-schedule uninstall --schedule <ABSOLUTE-SPEC-PATH>
```

- `<SESSION>` is a saved file name under `<path>/.plaesy/chats/`.
- `--every` accepts values such as `15m`, `1h`, or `1d`.
- `install` writes a specification under `.plaesy/goal-schedules/`; `uninstall` takes that absolute path, which `list` prints.
- `--allow-writes` keeps the chat's ability to mutate the repository; add `--supervised` to require persisted approvals.

## Example

```sh
plaesy chat-schedule install 2026-11-27-release-goal.json --every 1h --supervised
plaesy chat-schedule list
```

## Safety

This creates an OS-level scheduled job that runs without a person present. Ask the user before installing one, and remove it with `uninstall` when done.

`plaesy chat-schedule <sub> --help` is authoritative.
