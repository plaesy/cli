---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Add authenticated shared messaging and session delivery
phase: implement
status: backlog
---

# Add authenticated shared messaging and session delivery

## Goal

If the selected workflow needs inter-agent communication, implement project-authorized durable delivery with truthful acknowledgements across isolated processes/hosts.

## Scope

- Only implement if child spec requires multiple agents or hosts. Reuse existing Relay concepts; define authenticated transport, principal/project ACL, idempotency, queue/lease/ack/retry and receiver adapter semantics. Do not label polling as push/realtime or claim idle-session wake without receiver evidence.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Authenticated service/client contract, delivery implementation and multi-process/host acceptance evidence, or documented deferral if not required.

## Dependencies

- Child spec; approved identity model and runtime adapter.

## Acceptance criteria

[ ] Unauthorized project/recipient cannot read or send messages.

- [ ] Restart, duplicate send, lost ack, offline receiver and retry produce documented states.

- [ ] Queued, runtime-accepted and acknowledged are distinct and evidence-backed.
- [ ] No credentials or lease tokens leak in ordinary output/logs.

- [ ] If not in first workflow, record explicit deferment and do not expand scope.

## Parallel execution

- Lane: C (shared messaging and delivery)
- Owned paths (exclusive write): `src/relay.rs`, `src/relay_test.rs`, `src/relay/` (new service module), `tests/relay_cli.rs`. Local Relay semantics stay unchanged unless the acceptance criteria require otherwise.
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked shared_messaging` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.

## Progress (2026-11-19)

Implemented: shared inbox with idempotent delivery, lost-ack state, offline queueing, project isolation (VS-11 to VS-14), HMAC-SHA256 envelopes, JSON persistence. Remaining: network transport and key distribution. Status stays backlog until every acceptance criterion passes.
