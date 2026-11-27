# plaesy sessions

Self-reported agent session state in each project's `.plaesy/sessions.json`. `list` also covers projects registered with `plaesy projects register`. Records expire by TTL and contain no transcript.

## Syntax

```sh
plaesy sessions status --id <ID> --agent <AGENT> [--state working] [--task <TASK>] [--file <PATH>]... [--ttl 1800]
plaesy sessions heartbeat --id <ID> [--ttl 1800]
plaesy sessions list [--all] [--project <ID-or-PATH>]
plaesy sessions stop --id <ID>
```

- `status` creates or replaces a record; `--file` is repeatable.
- `heartbeat` refreshes the TTL of an unexpired record without changing its state.
- `list --all` also shows expired records as `stale`.
- TTL is 1 to 86,400 seconds.

## Example

```sh
plaesy sessions status --id claude-1 --agent claude --state working --task high_feature-relay-lease --file crates/plaesy/adapter/relay/src/relay.rs
plaesy sessions heartbeat --id claude-1
plaesy sessions stop --id claude-1
```

## Safety

State is advisory and may be stale. It does not discover processes, authenticate labels, or prevent writes. Use a distinct `--id` per concurrent session.

## Verify

`plaesy sessions list` shows the record after `status` and not after `stop`.

`plaesy sessions <sub> --help` is authoritative.
