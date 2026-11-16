# `plaesy sessions`

Track self-reported agent/session state in the current project's `.plaesy/sessions.json`. This is a local project registry, not a cross-device service or live session transport. It does not discover OS processes, verify identity, inject messages, lock files, or prevent writes. Use distinct `--id` values for concurrent sessions. Session state is advisory and may be stale; entries expire by TTL.

```sh
plaesy sessions status --id hermes-1 --agent hermes --state working --task TASK-123 --file src/main.rs --ttl 1800
plaesy sessions list
plaesy sessions heartbeat --id hermes-1 --ttl 1800
plaesy sessions list --all
plaesy sessions stop --id hermes-1
```

- `status` creates or replaces a session record and accepts repeatable `--file` options.
- `heartbeat` refreshes the TTL for an unexpired record without changing its status.
- `list` shows unexpired sessions; `list --all` also shows expired records as `stale`.
- `stop` removes a record explicitly.
- TTL is 1800 seconds by default and must be between 1 and 86400 seconds.

Records live in the project-local `.plaesy/sessions.json`, guarded by a local advisory lock. Network filesystem semantics are not verified. Heartbeat expiry only means no refresh was observed before the deadline; it does not prove the session stopped. This facility coordinates only callers that voluntarily report state.
