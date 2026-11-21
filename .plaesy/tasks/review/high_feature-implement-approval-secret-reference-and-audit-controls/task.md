---
spec: plaesy-agent-platform
topic: feature-implement-approval-secret-reference-and-audit-controls
title: Implement approval, secret-reference, and audit controls
phase: implement
status: review
---

# Implement approval, secret-reference, and audit controls

## Goal

Implementasikan approval, secret references dan audit hanya untuk workflow dan model otorisasi yang disetujui.

## Scope

- Tidak menyimpan secret mentah pada task/log/audit

- jangan aktifkan external side effects sebelum izin tersedia.

## Non-goals

- jangan aktifkan external side effects sebelum izin tersedia.

- Do not infer parity or mark work complete without execution evidence.

## Deliverables

- Implementasi Rust, migrasi bila perlu, policy checks, redaction, audit events dan focused tests.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` where decisions affect scope; document UNKNOWN blockers.

- Relevant audit/baseline tasks and approved spec/contract; do not begin dependent implementation before these are satisfied.

## Acceptance criteria

- [ ] Denied command has zero execution side effects, including terminal backend and shortcut paths.
- [ ] Timeout/unanswerable contexts, session-vs-permanent approval, hardline-vs-user deny policies, and requester/configured approver/platform-admin authorization have separate positive and negative tests.
- [ ] Unauthorized responders cannot approve; approval decision and denial are auditable without exposing secret values.
- [ ] Evidence-derived candidate: Hermes docs `website/docs/user-guide/security.md:15-22,24-67,141-188,196-207,254-309` at pinned snapshot `40d1e95f37e409f9f618f32c170661c66b86a4f4`; inspected approval test ranges `tests/tools/test_approval.py:30-60,69-91,95-151` were not executed. These references are not parity evidence.

- [ ] Setiap aksi terlindungi melewati policy boundary; deny/revoke tidak menghasilkan side effect terlarang.

- [ ] Secret hanya direferensikan melalui mekanisme disetujui; redaction diuji pada log/export/error.
- [ ] Approval mencatat actor/scope/expiry/result; audit immutable sesuai kontrak dan aksesnya diuji.

- [ ] Toolchain/lockfile/fmt/clippy/focused tests/security checks sesuai R14 lulus; test memakai fixture sintetis.

## Parallel execution

- Lane: B (identity, policy and secrets)
- Owned paths (exclusive write): `src/policy/` (new: approvals, policy checks), `src/secrets/` (new: secret references, redaction)
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked approval_secret` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.

## Progress (2026-11-19)

Implemented: secret store with redaction, external-effect approval gate (VS-15), JSON-lines audit export with redaction. Remaining: persisted approval records and an owner-approved approval flow. Status stays backlog until every acceptance criterion passes.
