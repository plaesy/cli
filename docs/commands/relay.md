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

## Storage and scope

By default, messages are stored in `.plaesy/relay.sqlite`; `--db <path>` selects another database file. Treat the database as private project data. When opening a previous database explicitly, Relay migrates its legacy table to the Relay schema while preserving queued messages. Sender and recipient IDs are caller-supplied, not authenticated; anyone who can invoke the CLI can claim another agent's ID, and anyone with access to the database can read or alter queued messages. The local prototype trusts processes under the same OS user and must not be exposed as a network service. It does not provide shared identity/authentication, project ACLs, remote delivery, live session injection, agent discovery, task assignment/acceptance, retention/dead-letter management, or multi-device synchronization. Do not interpret a successful send as proof that the recipient is online or has read the message.

See `docs/platform/agent-collaboration.md` for the broader design and limitations. The installed `plaesy relay ... --help` is authoritative for the current syntax.
