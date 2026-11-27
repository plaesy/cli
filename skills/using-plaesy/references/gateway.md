# plaesy gateway

A long-running local listener for one agent name. It polls the project's relay database and prints each new message to stdout. It never acknowledges or acts on a message, and it opens no network port.

An older installed binary may lack this command. If `plaesy --help` does not list `gateway`, use a build from source.

## Syntax

```sh
plaesy gateway serve --agent <NAME> [--path <REPO>] [--interval <SECONDS>]
```

| Option | Default | Meaning |
|---|---|---|
| `--agent` | required | Agent name whose inbox is polled |
| `--path` | `.` | Repository whose `.plaesy/relay.sqlite` is read |
| `--interval` | `5` | Seconds between polls (minimum 1) |

## Example

```sh
plaesy gateway serve --agent reviewer --interval 2
```

Status goes to stderr; each message goes to stdout. Stop it with Ctrl+C. Run it in a background terminal, not in a foreground tool call that waits for it to exit.

## Verify

Send a test message with `plaesy relay send --id t1 --from me --to reviewer "ping"` and see it printed. The message stays pending: acknowledge it with `plaesy relay ack` when handled.

`plaesy gateway serve --help` is authoritative.
