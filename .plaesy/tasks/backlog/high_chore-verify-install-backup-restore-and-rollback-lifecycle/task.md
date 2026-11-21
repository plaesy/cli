---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Verify install backup restore and rollback lifecycle
phase: implement
status: backlog
---

# Verify install backup restore and rollback lifecycle

## Goal

Demonstrate documented install/upgrade/backup/restore/rollback for the first declared deployment profile.

## Scope

- After profile decision, run disposable lifecycle drills; specify unsupported platforms and boundaries. Verify integrity and reconcile persistent task/run/message data. Do not claim HA/compliance based on local tests.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Profile-specific runbook and test evidence for lifecycle/restore/rollback.

## Dependencies

- Approved shared self-hosted profile; child spec and persistence model. Keep unsupported profiles UNKNOWN.

## Acceptance criteria

[ ] Clean install/configuration/status journey passes on declared profile.

- [ ] Backup and restore preserve in-scope data with integrity checks, including checkpoint metadata/store ownership, retention/size pruning, and exclusion of the backup/checkpoint store from its own snapshots.
- [ ] Checkpoint restore preview identifies affected files and current-content conflicts; restore preserves user edits not attributable to the selected snapshot, records a pre-restore recovery point, and reports partial/failed restore outcomes without silently discarding data.
- [ ] Repeated restore, missing/corrupt snapshot metadata, interrupted restore, and pruning at/over configured size limits have reproducible recovery behavior; tests use disposable stores and assert ownership boundaries.

- [ ] Failed upgrade can roll back without loss or with documented recovery.
- [ ] Secrets are excluded or handled by documented secure process.

- [ ] Limits and untested profiles are explicit.

## Parallel execution

- Lane: E (test harness, evidence and release gates)
- Owned paths (exclusive write): `tests/install_e2e.rs`, `scripts/install.sh`, `scripts/install.ps1`
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked --test install_e2e -- --list` must list at least one test.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
