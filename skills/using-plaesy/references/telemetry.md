# plaesy telemetry

Summarizes local, opt-in telemetry: command runs, errors, and average durations from `.plaesy/telemetry.jsonl`. Nothing is sent over the network.

## Syntax

```sh
plaesy telemetry summary
```

## Enable or disable

Recording is off unless the project config sets `telemetry.enabled = true` in `.plaesy/config.toml` (or `"telemetry": {"enabled": true}` in the older `.plaesy/settings.json`). Only the command group name, success, and duration are recorded, never arguments. Reading the summary is not itself recorded.

`plaesy telemetry --help` is authoritative.
