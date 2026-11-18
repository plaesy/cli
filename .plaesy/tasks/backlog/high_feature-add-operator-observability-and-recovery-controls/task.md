---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Add operator observability and recovery controls
phase: implement
status: backlog
---

# Add operator observability and recovery controls

## Goal

Expose truthful run/task/queue state, failure reason, correlation and authorized recovery for the chosen workflow.

## Scope

- Use persisted lifecycle as source of truth. Add only required operator surface; avoid a UI implementation until the selected surface is decided. Include structured diagnostics and redact secrets.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- CLI/API/Studio operator journey and recovery tests.

## Dependencies

- Persisted run lifecycle; operator-surface decision.

## Acceptance criteria

[ ] Operator can find one run by stable ID and see task, state, timestamps, reason and evidence.

- [ ] Retry/cancel/recover actions are permission-checked and audited.

- [ ] Queue/adapter unavailable state is distinguishable from success.
- [ ] Synthetic secret is absent from support output.

- [ ] Focused operator acceptance test passes.

## Parallel execution

- Lane: F (operator surface)
- Owned paths (exclusive write): `src/operator/` (new), `src/cli/operator.rs` (new). Registration in `src/cli/mod.rs` is integrator-only.
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked operator_` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
