---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Enforce isolated concurrent agent changes
phase: implement
status: backlog
---

# Enforce isolated concurrent agent changes

## Goal

Prevent unsafe overlapping file changes for the selected workflow using isolated workspaces or a verified enforced conflict boundary.

## Scope

- Follow child spec and current `spec/files-realtime-collaboration/spec.md`. Prefer disjoint worktrees if sufficient; if shared write coordination is required, define fencing/CAS and out-of-band writer behavior. Advisory claims alone are not write locks.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Selected isolation/conflict mechanism and process-level concurrent edit tests.

## Dependencies

- Child spec; owner-selected deployment profile.

## Acceptance criteria

[ ] Two agents editing overlapping files cannot silently overwrite each other.

- [ ] Out-of-band edit/stale base is detected and original changes preserved.

- [ ] Coordination failure has specified fail-safe behavior.
- [ ] Worktree/branch/index side effects are isolated or serialized.

- [ ] If concurrency is outside first workflow, defer with rationale.

## Parallel execution

- Lane: G (workspace isolation (R8))
- Owned paths (exclusive write): `src/workspace/` (new: isolated workspace and conflict detection)
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked isolation` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
