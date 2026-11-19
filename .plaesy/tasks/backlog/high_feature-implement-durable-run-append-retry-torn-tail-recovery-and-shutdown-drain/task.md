---
spec: plaesy-agent-platform
topic: feature-implement-durable-run-append-retry-torn-tail-recovery-and-shutdown-drain
title: Implement durable run append, retry, torn-tail recovery, and shutdown drain
phase: implement
status: backlog
---

# Implement durable run append, retry, torn-tail recovery, and shutdown drain

## Goal

Implementasikan durability dan recovery run store sesuai kontrak yang disetujui; jangan mengasumsikan format incumbent.

## Scope

- Perubahan dibatasi ke persistence Rust dan kontrak terpilih

- hindari migrasi produksi.

## Non-goals

- hindari migrasi produksi.

- Do not infer parity or mark work complete without execution evidence.

## Deliverables

- Store/event append semantics, retry queue, recovery, shutdown drain dan fault tests.

## Dependencies

- `high_chore-specify-versioned-project-task-run-artifact-and-event-contracts` — approve the persistence/event contract before implementation.
- `high_chore-establish-rust-security-and-release-quality-gates` — pinned toolchain and Rust quality gates.
- `high_chore-owner-decisions-for-first-replacement-slice` where decisions affect scope; document UNKNOWN blockers.
- Relevant audit/baseline tasks and approved spec/contract; do not begin dependent implementation before these are satisfied.

## Acceptance criteria

- [ ] Append-vs-flush durability is specified in the approved contract and proved by focused fault-injection tests at each documented boundary.
- [ ] Retry preserves ordering and batch data; duplicate/replay is idempotent according to the approved contract, verified by deterministic tests.
- [ ] Torn-tail recovery preserves valid records and reports corruption without false success; tests cover partial final records and corruption before the tail.
- [ ] Shutdown drain/cancellation is bounded and tested; restart restores persistent state consistently, with interruption/restart tests.
- [ ] Schema migration compatibility policy is specified; migration tests cover supported versions without production data.
- [ ] Focused locked Rust tests and R14 format/Clippy gates pass; attach exact commands and results. No completion based only on task status or checklist presence.

## Parallel execution

- Lane: D (run and task persistence, recovery)
- Owned paths (exclusive write): `src/runstore/` (new module: append, retry, torn-tail recovery, shutdown drain)
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked runstore` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.

## Progress (2026-11-19)

Implemented: checksummed append-only log with torn-tail report and corruption error (VS-10), shutdown drain that refuses new records, ordered retry queue with dead letters. Remaining: crash tests against a real process kill. Status stays backlog until every acceptance criterion passes.
