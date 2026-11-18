# `plaesy relay`

Durable direct messaging between agent identities within the selected local project. The current implementation stores messages in a local SQLite database; it is not a network relay or authenticated multi-agent service.

```sh
plaesy relay --help
plaesy relay send --help
plaesy relay inbox --help
plaesy relay ack --help
```

## Send

```sh
plaesy relay send --id <stable-message-id> --from <sender-agent> --to <recipient-agent> <body>
```

Use the same ID to retry safely. Reusing an ID with conflicting message content is an error.

## Inbox and acknowledgment

```sh
plaesy relay inbox --agent <recipient-agent> [--limit <n>]
plaesy relay ack --id <message-id> --agent <recipient-agent>
```

Inbox reads are non-destructive; a message stays pending until its addressed recipient acknowledges it. Acknowledgment is explicit and only valid for the recipient.

## Wait for a message

```sh
plaesy relay wait --agent <recipient-agent> [--timeout <seconds>] [--poll-interval <milliseconds>]
```

`wait` blocks while checking the local SQLite inbox and emits the first pending message as one JSON object without acknowledging it. A finite timeout emits `null`; `--timeout 0` waits indefinitely. The default timeout is 60 seconds and the default polling interval is 500 ms. Set a positive polling interval. The caller remains responsible for acknowledging the message after successful delivery to its agent session.

This provides a bounded blocking primitive for a local supervisor or integration; it is polling, not push. It does not itself wake or inject into an idle agent session. Ctrl-C terminates the waiting CLI process; any pending message remains in the queue.

- `plaesy relay wait --agent worker-b --timeout 30`
- `plaesy relay wait --agent worker-b --timeout 0 --poll-interval 250`

## Claim and lease-checked acknowledgment

```sh
plaesy relay claim --agent <recipient-agent> [--lease-seconds <seconds>]
plaesy relay ack --id <message-id> --agent <recipient-agent> --lease-token <lease-token>
```

`claim` atomically reserves the oldest pending message without an active lease and returns it as JSON, or `null` when none is available. Its response contains an opaque `lease_owner` token and `lease_expiry_epoch_ms`; pass that exact token to `ack --lease-token`. The CLI's `send` (including idempotent retries), `inbox`, and `wait` outputs omit `lease_owner` so they do not disclose an active acknowledgment capability. The JSON field name is retained for compatibility; the value is a capability token, not an owner identity. A stale or expired token cannot acknowledge the message. Legacy `ack` without a token is allowed only for messages that have never been claimed, including after a lease expires. If a consumer crashes or does not acknowledge before expiry, another consumer can claim the message. The lease duration is 1–86,400 seconds; it is not renewed, so keep it longer than processing time. The generated lease token is an acknowledgment capability, not an authenticated identity. This remains a local SQLite coordination feature, not multi-host locking or authentication.

The optional supervisor uses claims and token-checked acknowledgment; its lease must outlast the configured Hermes turn timeout.

This does not provide exactly-once side effects: a consumer may perform an action and crash before acknowledgment, causing redelivery after lease expiry.

The messages remain local; claims do not add push delivery or wake an idle Hermes session.

The current design trusts processes with access to the database and CLI; lease tokens are not a substitute for agent authentication.

For this reason, don't expose the database or CLI as a network service without a separate authenticated transport design.

The supervisor's local OS lock is still useful to avoid redundant polling on the same project and host; the database lease provides cross-process exclusion only when processes share the same SQLite file and SQLite's locking semantics.

A successful `send` still means only that the queue accepted the message, not that `wait` is running or the recipient is online.

## Storage and scope

By default, messages are stored in `.plaesy/relay.sqlite`; `--db <path>` selects another database file. Treat the database as private project data. When opening a previous database explicitly, Relay migrates its legacy table to the Relay schema while preserving queued messages. Sender and recipient IDs are caller-supplied, not authenticated; anyone who can invoke the CLI can claim another agent's ID, and anyone with access to the database can read or alter queued messages. The local prototype trusts processes under the same OS user and must not be exposed as a network service. It does not provide shared identity/authentication, project ACLs, remote delivery, live session injection, agent discovery, task assignment/acceptance, retention/dead-letter management, or multi-device synchronization. Do not interpret a successful send as proof that the recipient is online or has read the message.

See `docs/platform/agent-collaboration.md` for the broader design and limitations. The installed `plaesy relay ... --help` is authoritative for the current syntax.
