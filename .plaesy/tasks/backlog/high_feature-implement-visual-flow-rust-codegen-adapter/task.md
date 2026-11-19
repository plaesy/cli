---
spec: plaesy-agent-platform
topic: feature-implement-visual-flow-rust-codegen-adapter
title: Implement visual flow Rust codegen adapter
phase: implement
status: backlog
---

# Implement visual flow Rust codegen adapter

## Goal

Generate Rust code from the flow IR through the first codegen adapter, and report unsupported node kinds as explicit losses (spec R16, ADR 0002).

## Scope

- Rust codegen adapter in `src/visual_flow/codegen/`.
- Expected generated outputs in `tests/fixtures/flows/expected/`.
- Loss report for unsupported node kinds.

## Non-goals

- Go and JavaScript adapters (separate later tasks). Editor UI.

## Deliverables

- Rust codegen adapter in `src/visual_flow/codegen/`.
- Expected generated outputs in `tests/fixtures/flows/expected/`.
- Loss report for unsupported node kinds.

## Dependencies

- `high_feature-implement-visual-flow-ir-core-in-rust`

## Acceptance criteria

- [ ] Generated Rust for each fixture compiles and produces the result the IR fixture expects.
- [ ] Unsupported node kinds appear in the loss report, never dropped.
- [ ] `cargo test --locked visual_flow_codegen` passes with a non-zero test count.

## Parallel execution

- Lane: H (visual flow IR, codegen and editor (R16))
- Owned paths (exclusive write): `src/visual_flow/codegen/`, `tests/fixtures/flows/expected/` (new)
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked visual_flow_codegen` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
