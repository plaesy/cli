---
spec: plaesy-agent-platform
topic: feature-implement-visual-flow-ir-core-in-rust
title: Implement visual flow IR core in Rust
phase: implement
status: backlog
---

# Implement visual flow IR core in Rust

## Goal
Implement the versioned flow IR (`flow-ir/1`) in Rust: node and edge model, stable node IDs, schema version, validation with typed errors, and serialization with no target-language fields (spec R16, ADR 0002).

## Scope
- Flow IR types and serialization in `src/visual_flow/ir/`.
- Validation that returns typed errors without panic in `src/visual_flow/validate/`.
- Fixtures in `tests/fixtures/flows/`: one valid flow per node kind in ADR 0002, plus malformed inputs.

## Non-goals
- Editor UI and any codegen target.

## Deliverables
- Flow IR types and serialization in `src/visual_flow/ir/`.
- Validation that returns typed errors without panic in `src/visual_flow/validate/`.
- Fixtures in `tests/fixtures/flows/`: one valid flow per node kind in ADR 0002, plus malformed inputs.

## Dependencies
- `high_chore-establish-rust-security-and-release-quality-gates` (pinned toolchain, fmt, clippy)
- `high_chore-decide-visual-flow-ir-and-codegen-adr` (accepted, done)

## Acceptance criteria
- [ ] Flow IR round-trips without loss for every node kind fixture.
- [ ] Malformed input returns a typed error, never a panic.
- [ ] Every node kind in ADR 0002 has at least one passing fixture.
- [ ] `cargo test --locked visual_flow_ir` passes with a non-zero test count.

## Parallel execution

- Lane: H (visual flow IR, codegen and editor (R16))
- Owned paths (exclusive write): `src/visual_flow/ir/`, `src/visual_flow/validate/`, `tests/fixtures/flows/*.json` (inputs)
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked visual_flow_ir` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
