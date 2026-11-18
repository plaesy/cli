---
spec: plaesy-agent-platform
topic: feature-add-provenance-aware-knowledge-and-memory-controls
title: Add provenance-aware knowledge and memory controls
phase: implement
status: backlog
---

# Add provenance-aware knowledge and memory controls

## Goal

Implement provenance-aware knowledge/memory lifecycle for an owner-approved workflow.

## Scope

- Reconcile existing stored knowledge, source attribution, review, retention, deletion, export and audit behavior.

## Non-goals

- Do not copy source content or claim evidence provenance unless lineage is retained.

## Deliverables

- Rust domain/service/persistence, schema migration, operator/user controls and tests.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Owner-selected scope and source classes are recorded; excluded classes are explicit.

- [ ] Every knowledge item records source identity/location and revision where available, ingesting actor/adapter, timestamps, trust/evidence state, update/review lineage, and access scope; transformations retain links to their inputs.
- [ ] Imported skill/source trust cannot be elevated by publisher-controlled identifiers; adapter origin, trust tier, quarantine/scan verdict, and deny outcome are testable and audited. Rejection must prevent activation/use, not merely annotate the item.
- [ ] Attended/background writes, provider switching, concurrent lifecycle hooks, and failed/interrupted sync have explicit lineage and recovery outcomes; test assertions distinguish pending review from accepted trust.
- [ ] Provenance survives export, restore, migration, deduplication and redaction, or any lossy transformation is disclosed and requires the approved disposition.
- [ ] Profile-specific memory providers, stores, and secrets remain isolated during concurrent calls and background lifecycle hooks; cross-profile requests fail closed.
- [ ] Retention, deletion, export and restore behaviors are specified and tested without exposing secrets.

- [ ] Migration preserves links/IDs or reports loss/conflicts; tests use synthetic data.
- [ ] Rust toolchain, locked build, fmt, focused tests, Clippy and applicable security checks pass.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.

## Parallel execution

- Lane: J (knowledge and memory controls)
- Owned paths (exclusive write): `OWNERSHIP-UNASSIGNED`: the integrator assigns owned paths from the approved vertical-slice spec before any agent claims this task.
- Integrator-only shared files, never edited inside a task: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`
- Focused tests: `cargo test --locked provenance` passes with a non-zero test count.
- Claim rule: claim only when every dependency in `issue.json` is `done` and no unassigned-owner marker remains. Work in its own `git worktree` and branch.
