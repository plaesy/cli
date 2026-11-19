---
spec: plaesy-agent-platform
topic: feature-implement-bounded-supervised-runtime-execution-and-usage-reporting
title: Implement bounded supervised runtime execution and usage reporting
phase: implement
status: backlog
---

# Implement bounded supervised runtime execution and usage reporting

## Goal

Implementasikan eksekusi backend yang supervised, bounded, cancellable dan melaporkan usage/error secara truthful.

## Scope

- Tidak memilih provider/backend atau menjalankan tool eksternal tanpa workflow dan approval owner.

## Non-goals

- Tidak memilih provider/backend atau menjalankan tool eksternal tanpa workflow dan approval owner.

- Do not infer parity or mark work complete without execution evidence.

## Deliverables

- Rust runtime adapter, timeout/resource bounds, cancellation, usage/error mapping, fake adapter tests.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` where decisions affect scope; document UNKNOWN blockers.

- Relevant audit/baseline tasks and approved spec/contract; do not begin dependent implementation before these are satisfied.

## Acceptance criteria

- [ ] Adapter hanya mengiklankan capability yang teruji; backend failure tidak dilaporkan sebagai sukses.

- [ ] Timeout, cancellation, crash, backpressure, output limit dan shutdown diuji dengan fake deterministic.
- [ ] Usage/error fields provenance dan unknown values jelas; secret tidak bocor ke output/audit.

- [ ] Concurrency dibatasi, operasi asynchronous bounded dan cleanup berlangsung saat cancel/failure.
- [ ] Fake conformance asserts availability-check failure behavior, enabled/disabled toolset filtering and exposed-schema parity, dispatch errors, single-vs-multi execution, interactive-tool serialization, serta result order independen dari waktu selesai.
- [ ] Approval deny has zero execution side effects; timeout/unanswerable contexts, session-vs-permanent grant, hardline-vs-user deny layers, and authorized-vs-unauthorized approvers have distinct tested outcomes. Keep secret-reference/audit requirements covered by their owning approval-controls task.
- [ ] Runtime acceptance is scoped to the owner-approved workflow and selected adapters; documentation-only incumbent candidates are not parity evidence.
- [ ] R14 toolchain/lockfile/fmt/clippy/focused tests/security gates lulus.

## Parallel execution

- Lane: A (adapter and runtime contract)
- Owned paths (exclusive write): `src/runtime/` (new: supervisor, timeout, cancellation, usage mapping). Reads `src/adapter/` only.
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked bounded_runtime` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.

## Progress (2026-11-19)

Implemented: step budget, cancellation, external-call gate (VS-06, VS-07, VS-08). Remaining: usage and cost reporting. Status stays backlog until every acceptance criterion passes.
