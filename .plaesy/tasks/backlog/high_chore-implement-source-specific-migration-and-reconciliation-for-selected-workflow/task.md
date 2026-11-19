---
spec: plaesy-agent-platform
topic: chore-implement-source-specific-migration-and-reconciliation-for-selected-workflow
title: Implement source-specific migration and reconciliation for selected workflow
phase: implement
status: backlog
---

# Implement source-specific migration and reconciliation for selected workflow

## Goal

Bangun migrasi dan rekonsiliasi sumber hanya setelah workflow, sumber, dan cutover disetujui.

## Scope

- Tidak mengubah sumber

- tidak menjalankan migrasi terhadap data nyata tanpa izin eksplisit dan backup teruji.

## Non-goals

- tidak menjalankan migrasi terhadap data nyata tanpa izin eksplisit dan backup teruji.

- Do not infer parity or mark work complete without execution evidence.

## Deliverables

- Rust migrator/importer, mapping versioned, dry-run report, reconciliation checks, rollback procedure.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` where decisions affect scope; document UNKNOWN blockers.

- Relevant audit/baseline tasks and approved spec/contract; do not begin dependent implementation before these are satisfied.

## Acceptance criteria

- [ ] Sumber/release dan format input dipin; fixture bebas rahasia tersedia dan mapping tiap field beralasan.

- [ ] Dry-run melaporkan created/updated/skipped/conflict/loss counts serta checksum/referential integrity.
- [ ] Uji duplicate, partial failure, restart, rollback dan source immutability lulus di disposable fixture.

- [ ] Cutover dilarang sampai parity/backup/rollback gates dan persetujuan owner terpenuhi.
- [ ] R14 gates lulus.

## Parallel execution

- Lane: D (run and task persistence, recovery)
- Owned paths (exclusive write): `DEFERRED`: out of the first replacement slice (child spec section 3, approved 2026-11-19). The owner reactivates this task to assign owned paths. Until then no agent may claim it.
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked migration` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
