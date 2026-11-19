---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Define runtime adapter and fake conformance suite
phase: implement
status: backlog
---

# Define runtime adapter and fake conformance suite

## Goal

Define a minimal adapter contract and executable fake-adapter tests for the chosen workflow.

## Scope

- After child spec, define only required lifecycle operations, identity/grants, events, output artifacts, timeout/cancel, errors and idempotency. Build deterministic fake success/deny/timeout/crash/duplicate/lost-ack cases; document backend-dependent behavior. Do not modify upstream source.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Adapter contract and focused fake conformance tests.

## Dependencies

- Child vertical-slice spec.

## Acceptance criteria

- [ ] Given an approved provider set, resolve provider/model/API mode using explicit request, saved configuration, environment, and defaults in the specified precedence; return provider, mode, endpoint, credential source, and provider metadata. Unapproved source behavior remains UNKNOWN.
- [ ] Verify endpoint-scoped credential selection, including a negative test that an unrelated provider key is never sent to a custom endpoint.
- [ ] Test primary-model fallback separately from auxiliary-model fallback: documented trigger classes, credential/client re-resolution, model/provider switch, and exhausted-fallback error. Keep gateway reload timing distinct from local-session timing.
- [ ] Fake conformance tests cover registration/discovery, availability gating (including check exceptions), enabled/disabled tool filtering and matching exposed schemas, dispatch error wrapping, single vs multi-tool execution, interactive-tool serialization, and result ordering independent of completion order.
- [ ] Keep cancellation guarantees separate: define and test them only where the approved adapter/runtime contract specifies them; documentation-derived incumbent candidates are not parity evidence.
- [ ] The approved owner workflow and selected provider set are explicit; other providers/modes remain deferred or UNKNOWN.

### Evidence-derived candidates (not incumbent parity claims)

- Hermes source snapshot `40d1e95f37e409f9f618f32c170661c66b86a4f4`; official docs inspected: `website/docs/developer-guide/provider-runtime.md:9-28,47-56,91-109,118-178`, `website/docs/user-guide/features/provider-routing.md:18-30,36-105,193-223`, `website/docs/user-guide/features/fallback-providers.md:12-18,123-151,226-242,348-355`, `website/docs/developer-guide/tools-runtime.md:19-47,68-118,124-161,176-225`, `website/docs/developer-guide/agent-loop.md:126-147`.
- These docs were inspected, not tested against Hermes in this pass. Plaesy implementation acceptance must be grounded in the owner-selected workflow and executed against Plaesy code; do not claim Hermes runtime parity from documentation.

[ ] Contract maps to child-spec requirements.

- [ ] Fake tests cover success, denial, timeout, cancellation, crash/restart, duplicate call and ambiguous outcome.

- [ ] Tests prove truthful run state; failure cannot be reported as success.
- [ ] Backend-specific features are explicitly marked.

## Parallel execution

- Lane: A (adapter and runtime contract)
- Owned paths (exclusive write): `src/adapter/` (new: adapter trait and fake adapter), `tests/adapter_conformance.rs` (new)
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked adapter_conformance` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.

## Progress (2026-11-19)

Implemented: adapter contract, deterministic fake, conformance checks (VS-02, VS-03). Remaining: real backend conformance when one is approved. Status stays backlog until every acceptance criterion passes.
