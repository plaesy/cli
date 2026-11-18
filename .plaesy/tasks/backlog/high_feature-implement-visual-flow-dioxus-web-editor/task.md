---
spec: plaesy-agent-platform
topic: feature-implement-visual-flow-dioxus-web-editor
title: Implement visual flow Dioxus web editor
phase: implement
status: backlog
---

# Implement visual flow Dioxus web editor

## Goal
Build the web visual flow editor in Dioxus on top of the Rust IR core, using a surface that desktop and mobile can reuse later (spec R16, owner choice 2026-11-18).

## Scope
- Editor crate `crates/flow-editor/`.
- Save and load of flows as versioned IR through the core API only.

## Non-goals
- Desktop and mobile surfaces. Codegen. Any business logic outside the Rust core.

## Deliverables
- Editor crate `crates/flow-editor/`.
- Save and load of flows as versioned IR through the core API only.

## Dependencies
- `high_feature-implement-visual-flow-ir-core-in-rust`
- Integrator adds the crate to the workspace before claim (see Parallel execution).

## Acceptance criteria
- [ ] A flow saved from the editor loads back as the same IR.
- [ ] The editor reaches the core only through the documented API.
- [ ] `cargo test --locked -p flow-editor` passes with a non-zero test count.

## Parallel execution

- Lane: H (visual flow IR, codegen and editor (R16))
- Owned paths (exclusive write): `crates/flow-editor/` (new crate). Integrator first adds it to `[workspace] members` in `Cargo.toml` and refreshes `Cargo.lock`.
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked -p flow-editor` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
