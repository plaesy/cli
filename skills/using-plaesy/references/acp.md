# plaesy acp

Serves an Agent Client Protocol (ACP) v1 agent over stdio. An ACP client (an editor or another agent host) sends prompts; a configured delegate provider answers them.

## Syntax

```sh
plaesy acp --provider <NAME> [--path .] [--confirm]
```

| Option | Meaning |
|---|---|
| `--provider` | Provider name from `delegate.providers` that answers prompts |
| `--path` | Project whose `.plaesy/settings.json` is read and where `documents.sqlite` is created |
| `--confirm` | Allow a provider marked billable |

## Use

Register the command in the ACP client as a stdio agent, for example `plaesy acp --provider claude-cli --path /abs/project`. Stdout carries protocol messages only; do not run it interactively and do not add output to stdout.

`plaesy acp --help` is authoritative.
