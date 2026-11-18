# Spec: Realtime `plaesy files` agent collaboration

Status: draft
Topic: files-realtime-collaboration
Tasks: none

## Purpose

Agents using `plaesy files` need to learn promptly when another cooperating agent claims or releases a file and need a shared channel for coordination. Current commands expose claim state only when queried; they do not deliver events to running agents.

## Requirements

| ID | Requirement | Acceptance criterion (Given / When / Then) | Status |
| --- | --- | --- | --- |
| R1 | Provide a machine-readable watch interface that streams file-claim lifecycle events to a running agent without requiring repeated manual `files list` calls. | Given two Plaesy processes watching the same project, when one claims or releases a file, then the other receives a structured event containing type, path, agent label, optional task, lease ID, and timestamp. | open |
| R2 | Provide project-scoped targeted and broadcast agent messaging, with bounded message size and explicit delivery/reconnect semantics. | Given two agents connected to the same project's collaboration channel, when one sends a valid targeted message, then only the intended recipient receives it; when sent as a broadcast, all other subscribed agents receive it. | open |
| R3 | Keep existing `claim`, `who`, `list`, and `release` commands compatible and make clear that claims are advisory, not filesystem write locks. | Given an existing CLI invocation, when the upgraded CLI runs it, then the command retains its existing behavior and documentation says non-cooperating writers are not fenced. | open |
| R4 | Isolate event subscriptions and messages by resolved project root and protect local transport/state from other users where supported. | Given agents connected to different project roots, when one project emits a claim event or message, then subscribers in the other project receive no such event. | open |
| R5 | Bound slow-subscriber resource use and define behavior for disconnects and reconnects. | Given a subscriber that disconnects or cannot keep up, when other agents continue publishing events, then publishers remain responsive and memory/queues remain bounded; reconnect behavior matches the documented delivery guarantee. | open |
| R6 | Validate project-relative paths and agent labels and test independent processes, lifecycle events, message routing, isolation, disconnects, malformed input, and slow consumers. | Given the focused collaboration test suite, when tests execute against separate processes and project fixtures, then each security, routing, lifecycle, and bounded-resource assertion passes deterministically. | open |

## Non-goals

- Preventing non-cooperating processes from writing a claimed file.
- Collaborative simultaneous editing, merge resolution, or filesystem write fencing.
- Treating a self-declared agent label as authenticated identity.

## Constraints

- Preserve local-first/offline operation and current CLI compatibility.
- State transport/platform guarantees explicitly; do not assume local advisory file locks are reliable on network filesystems.
- Do not expose file contents in claim lifecycle events.

## Open questions

- DECISION (proposed for owner approval): Reuse the existing durable `plaesy relay` for inter-agent text messages; keep claim events as a separate event stream, since claim changes are structured state transitions and relay messages already have explicit acknowledgement semantics.
- DECISION (proposed): Persist claim events and expose a cursor-based watch so subscribers can recover after disconnect; document retention/compaction before shipping to bound storage.
- DECISION (proposed): Prefer a project-local SQLite event journal polled by watch clients initially; this preserves offline operation and avoids an always-running daemon or a cross-platform socket service. Treat polling latency as near-real-time, not push.
- UNKNOWN: Should claims support heartbeat renewal and automatic release on graceful agent shutdown? Owner: project owner.

These proposals are design findings, not approved product decisions; implement only after review against actual code and tests.

## Research notes

- `notify` is already a project dependency, but its filesystem watcher observes filesystem activity, not semantic `claim`/`release` transitions. It is not sufficient for authoritative claim notifications.
- The `notify` crate documentation recommends a platform-selected watcher but documents event-loss limitations for large directories and polling as a workaround for some filesystems (https://docs.rs/notify/8.2.2/notify/, reviewed 2026-11-16).
- `plaesy relay` already implements durable SQLite-backed direct messages and explicit acknowledgement (`src/relay.rs`); reusing it for messaging avoids creating a second message protocol, but does not itself publish claim lifecycle events.
- The current claim store serializes updates under a project-local advisory lock and writes `claims.json` atomically (`src/files.rs`). An event journal must be updated within the same critical section as the corresponding claim change to avoid lost/misordered notifications.
- No realtime event stream is currently implemented; `ClaimEvent` was removed as unused. Existing broad working-tree modifications are unrelated and must be preserved.

## Next implementation slice

- Snapshot polling (`plaesy files watch`) is implemented as an interim CLI; it diffs claim snapshots every 500 ms by default and can miss short-lived transitions. The CLI build and a separate-process claim-to-watcher smoke check passed.
- Remaining: durable cursor/replay event journal, automated process-level regression coverage, relay message delivery integration, and owner decision on claim renewal policy.

1. Add a focused library test proving claim/release generate replayable events with increasing cursors and failed operations emit none.
2. Confirm the test fails before implementing the event journal.
3. Write the journal under the same claim lock, with bounded retention and explicit expired-cursor behavior.
4. Replace the interim snapshot watcher with cursor-based polling after the library contract passes.
5. Test independent processes, project isolation, concurrency, and relay integration before claiming realtime coordination.

## References

- `docs/commands/files.md` — current claim command behavior and limitations.
- `src/files.rs` — claim storage and concurrency behavior.
- `src/relay.rs` — existing durable agent messaging implementation.
