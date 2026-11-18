# Plaesy agent platform and incumbent replacement — implementation plan

Status: draft; replacement inventory and implementation backlog remain incomplete
Spec: ../../spec/plaesy-agent-platform/spec.md
Checklist: ../../../docs/platform/feature-gap-register.md

This plan targets the 11 incumbents named in the spec. The 18 backlog task folders are program-level foundations, not a complete per-capability implementation plan. First complete the all-target evidence audit and capability crosswalk; then derive deduplicated implementation tasks from verified requirements. The 2026-11-18 bounded source-audit delta adds 18 leads with pinned revisions, evidence labels and next checks; they do not satisfy the source-coverage gate. No incumbent cutover is authorized by this plan. Plaesy-owned replacement core/domain, persistence, service/backend, CLI, and native runtime code must be Rust; UI technology remains subject to an owner-approved ADR, with authoritative business logic/state in Rust.

## Goal

Produce an owner-reviewable, evidence-backed capability inventory and deduplicated task plan for all 11 incumbents, then deliver verified Plaesy-owned replacement workflows through explicit gates rather than a broad, unverified rewrite.

## Program completion boundary

The planning effort is not complete until every requested incumbent has an auditable source-coverage ledger, each in-scope leaf capability maps to a requirement and acceptance scenario (or explicit UNKNOWN/out-of-scope disposition), all 46 RP rows have a contract-verified disposition, and unique implementation/verification follow-up tasks exist for every approved gap. The current 18 task folders are scaffolding; their count does not imply contract completeness and they do not satisfy this boundary. Product owner approval is required for scope, baseline versions and implementation priorities. Runtime parity and cutover remain separate later gates.

## Planning assumptions

- This is a staged program; it does not commit to implementing all target requirements in one release.
- Owner decisions for the first workflow, deployment profile, runtime strategy and identity hierarchy are unresolved. Tasks that depend on them must remain blocked until recorded.
- Do not edit Hermes or other upstream source. Use supported adapter interfaces only.
- Preserve current worktrees and unrelated local changes. Use focused tests and evidence from the integrated checkout.
- The current feature register is a bounded baseline, not an exhaustive audit. Agent owners must update status with source/test evidence.

## Work plan

### Phase 0 — establish scope and reusable evidence

1. Resolve owner questions in the program spec and capture durable choices in `docs/platform/decisions.md`.
2. Recover/import prior comparisons and inventory documented/source-backed leaf capabilities for all 11 requested incumbents into one reusable, pinned evidence index. Research only uncovered inventory areas; distinguish source snapshots from owner-approved baselines. After broad inventory, run deep comparative runtime tests only for the owner-selected workflow.
3. Reinspect claimed implementations across CLI, Studio, adapters and deployed profile. Correct checklist status only with code paths and focused test results.
4. Before any Rust feature implementation, establish the foundation quality gates: pinned toolchain, locked dependency build, format, Clippy, and focused tests. Keep selected-target, supply-chain, and artifact-provenance checks profile-specific until owner decisions exist.

**Gate:** all 11 source-coverage ledgers and the evidence/capability index are reviewed with unknowns explicit; baseline Rust gates are executable before implementation. No dependent architecture contract is frozen while first workflow, deployment profile or runtime strategy remains unknown; broad documentary/source inventory may continue before those decisions.

### Phase 1 — specify a runnable vertical slice

1. Create a child spec for the chosen workflow with concrete actors, state transitions, data, threat model, adapter behavior, failure recovery and exact acceptance tests.
2. Define a minimal run/task/adapter contract and explicit supported capability matrix; separate backend-provided behavior from Plaesy-owned state/policy.
3. Build a fake adapter and deterministic fixtures for success, denial, timeout, cancellation, crash, duplicate delivery, lost acknowledgement and restart recovery.

**Gate:** contract tests demonstrate truthful state and no false success under injected failures.

### Phase 2 — implement the minimum secure workflow

1. Implement persisted task/run lifecycle, stable correlation IDs, idempotent operation boundaries and recovery for the selected workflow.
2. Implement or adapt authentication/authorization at the execution boundary; add task/project/tool least-privilege and revocation tests before enabling real tools.
3. Add authenticated messaging/runtime handoff only if the chosen workflow crosses agents/hosts; keep current local Relay semantics explicitly local until replacement is proven.
4. Add workspace isolation or an enforced write/conflict boundary before parallel agents can modify shared files.

**Gate:** security, reliability and concurrency tests pass on a disposable profile; real external effects are disabled in test fixtures.

### Phase 3 — operator experience and operations

1. Expose the persisted state and supported controls through the selected operator surface (CLI/API/Studio); show queue/run health, failure reason and correlation IDs.
2. Add structured audit/diagnostics, secret redaction, backup/export and recovery procedures for the chosen profile.
3. Exercise installation, restart, upgrade, backup/restore and rollback; record supported OS/profile and limits.

**Gate:** an operator can diagnose a failed run and recover it without inferring state from agent prose.

### Phase 4 — compare, parallel-run and cut over

1. Run the same representative workflow through incumbent and Plaesy; compare observable outcomes, permissions, recovery, operations and data portability with cited evidence.
2. Reconcile imported task/artifact/state data; run Plaesy and incumbent in parallel with a rollback trigger and owner.
3. Cut over only when all R13 mandatory gates pass; retain incumbent rollback path until the agreed observation period ends.

**Gate:** evidence is attached for every release checklist item; unresolved safety or parity gaps block replacement claims.

## Parallel work boundaries

After Phase 0 decisions, assign one owner each to: (A) adapter/runtime contract, (B) identity/policy threat review, (C) Relay/delivery only if required by workflow, (D) run/task persistence and recovery, (E) test harness/evidence, and (F) operator surface. Define interfaces first; prohibit simultaneous ownership of the same source/test files. One integrator reviews the combined diff and reruns focused tests. Do not run `tasks run --parallel` for tasks that edit overlapping files because the current task runner shares one project directory.

## Verification strategy

- For every implementation task: test-first where applicable, focused unit tests, process/integration fault test, formatter/linter, `git diff --check`, and review actual output/artifacts.
- Run CLI and Studio tests in their respective repositories/environments; report which tests were not rerun.
- Never use production credentials/data in fault fixtures. Treat `contracts.json` commands as executable code, inspect them before validation, and run only non-destructive checks in disposable contexts.
- Keep comparison claims sourced and versioned. A design, README, or subagent report alone does not satisfy implementation evidence.
- Before cutover: migration integrity, backup/restore, upgrade/rollback, security, operator recovery, and incumbent rollback must each pass on the declared profile.

## Risks and blockers

- The user has not yet chosen the first replacement workflow/profile/runtime strategy; downstream designs remain conditional.
- The feature register is bounded; broader repositories/deployments need targeted verification before parity claims.
- Existing CLI Relay is local SQLite + polling and caller-supplied labels; converting it to remote delivery requires authenticated service and authorization design.
- Shared directory parallel task execution can create file conflicts; use disjoint files or isolated workspaces.
- This checkout has many unrelated user changes; task execution must preserve them and avoid broad reset/cleanup.

## Links

These paths are relative to the repository root, matching the spec references above:

- Spec: `.plaesy/spec/plaesy-agent-platform/spec.md`
- Feature register: `docs/platform/feature-gap-register.md`
- Product: `docs/platform/product.md`
- Decisions: `docs/platform/decisions.md`
- Current-state assessment: `docs/platform/current-state-assessment.md`
- Task command safety/semantics: `docs/commands/tasks.md`
- Backlog index: `.plaesy/tasks/backlog/` (18 task folders; see spec task index; these are scaffolding, not a complete per-capability backlog)
