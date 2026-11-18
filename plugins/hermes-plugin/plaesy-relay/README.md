# Plaesy Relay for Hermes

This directory contains an opt-in Hermes plugin that bridges Plaesy Relay's local
SQLite queue into Hermes turns. It is a local adapter, not a network transport.

## Configure

Set these environment variables for the Hermes process:

- `PLAESY_CLI_BIN`: absolute path to the Plaesy CLI (optional if `plaesy` is on
  `PATH`).
- `PLAESY_RELAY_PROJECT`: project root containing `.plaesy/relay.sqlite`.
- `PLAESY_RELAY_AGENT_ID`: stable Relay recipient/sender ID for this Hermes
  agent. Configure a distinct ID for each agent.
- `PLAESY_RELAY_HOOK_LEASE_SECONDS`: claim lease duration for hook delivery
  (default `3600`; allowed range `1`–`86400`). Increase it for long turns; there
  is no lease-renewal hook during generation.

Relay identities are labels supplied by local configuration, not authenticated
process identities. Keep the database private and do not share it across
untrusted users or hosts.

## Install locally

Hermes' plugin installer accepts catalog and Git sources, not a local directory.
Copy this directory to `~/.hermes/plugins/plaesy-relay/` and explicitly enable it:

```sh
mkdir -p ~/.hermes/plugins/plaesy-relay
cp -a plugins/hermes-plugin/plaesy-relay/. ~/.hermes/plugins/plaesy-relay/
hermes plugins validate ~/.hermes/plugins/plaesy-relay
hermes plugins enable plaesy-relay --no-allow-tool-override
```

Restart the Hermes gateway/process after enabling. Keep `PLAESY_RELAY_PROJECT`
pointed at the same project for every cooperating local agent.

## Behavior and limits

- `plaesy_relay_send` enqueues a message for the configured sender.
- The `pre_llm_call` hook atomically claims at most one message for the configured
  agent and adds it as untrusted user context at the next turn boundary. Its
  lease defaults to 3,600 seconds and is configurable as described above.
- Messages are acknowledged by the `post_llm_call` hook with the lease token after
  the receiving session completes a turn. If Hermes exits before that callback,
  the lease expires and the durable queue allows retry (at-least-once delivery).
  Acks are scoped to the session that received each message.
- Plugin hooks and the optional supervisor share SQLite claims: only one can hold
  a message's active lease. A claimed message may wait for its turn to complete or
  its lease to expire before another consumer can retry it.
- If a turn exceeds the hook's 300-second lease, another consumer may reclaim the
  message while the original turn is still running; the hook does not renew leases.
  This remains a bounded mitigation, not exactly-once execution.

---

The `pre_llm_call` hook polls only at turn boundaries. It cannot interrupt a
model generation or wake an idle Hermes process; it is not mid-turn real-time
push. Inbox/claim CLI failures or malformed output are logged without message
contents and are not treated as an empty inbox or acknowledged.

Multiple Hermes sessions using the same agent ID compete for messages through
atomic claims. Successful claim grants a temporary exclusive lease, but a crash
after an external tool action and before ack can still cause the action to be
retried after lease expiry.

---

The hook polls at turn boundaries, so it cannot interrupt generation or wake an
idle Hermes process. CLI failures and malformed results are logged without
payload contents. Multiple sessions sharing an agent ID compete through SQLite
leases; a turn crash or expiry can cause at-least-once redelivery.

### Optional one-shot supervisor

`plugins/hermes-plugin/relay-supervisor.py` polls the local Relay queue and
starts a Hermes one-shot turn per message. It requires an explicit Hermes
profile and project path; it does not start or modify the default gateway or
profile. It holds a nonblocking per-project/per-agent OS file lock, so a second
supervisor for the same inbox on this host exits instead of racing to process
the same pending message. This is local filesystem coordination only; it does
not coordinate other hosts or network filesystems with incompatible lock
semantics.

```sh
python3 plugins/hermes-plugin/relay-supervisor.py \\
  --plaesy /path/to/plaesy --hermes /path/to/hermes \\
  --profile relay-worker --project /path/to/project --agent worker-b --once
```

Remove `--once` to keep polling. The supervisor atomically claims each message
with an expiring SQLite lease and acknowledges it with the returned ownership
token only after Hermes exits successfully. `--lease-seconds` defaults to 900
and must exceed `--hermes-timeout` (default 300 seconds); lease duration is
bounded to 24 hours. On timeout or other failure, the claim expires and another
consumer can retry. This is cross-process only for processes sharing the same
SQLite database, not a distributed lock across remote hosts. Retries use
a stable Hermes session name derived from the Relay message ID, and the prompt
asks the agent to avoid repeating completed actions. This preserves retry
context but does not guarantee tool/action deduplication. A crash between turn
completion and acknowledgment can also cause a retry, so delivery is
at-least-once. Successful CLI exit confirms completion
of a one-shot turn, not any external action by the agent. This does not wake an
already-running session or provide cross-machine transport.

Messages remain untrusted coordination data. Do not treat their contents as
system or developer instructions.
