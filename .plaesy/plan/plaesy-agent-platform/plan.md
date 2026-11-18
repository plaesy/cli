# Plaesy agent platform and incumbent replacement — implementation plan

Status: draft; canonical Plaesy program plan
Authority: `.plaesy/spec/plaesy-agent-platform/spec.md` is the canonical spec; executable tasks live under `.plaesy/tasks/`. Supporting product research and evidence remain in `docs/platform/`.
Checklist: ../../../docs/platform/feature-gap-register.md

This plan targets the 11 incumbents named in the spec plus the separately scoped design↔code requirement R15. The current 57 task folders (56 backlog, 1 done) include audit, foundations, and conditional workflow work; they are not a complete per-capability implementation plan. First complete the all-target evidence audit and capability crosswalk; then derive deduplicated leaf implementation tasks from verified requirements. The 2026-11-18 bounded source-audit delta adds 18 leads with pinned revisions, evidence labels and next checks; they do not satisfy the source-coverage gate. No incumbent cutover is authorized by this plan. Plaesy-owned replacement core/domain, persistence, service/backend, CLI, and native runtime code must be Rust; UI technology remains subject to an owner-approved ADR, with authoritative business logic/state in Rust.

## Goal

Produce an owner-reviewable, evidence-backed capability inventory and deduplicated task plan for all 11 incumbents, then deliver verified Plaesy-owned replacement workflows through explicit gates rather than a broad, unverified rewrite.

## Program completion boundary

The planning effort is not complete until every requested incumbent has an auditable source-coverage ledger, each in-scope leaf capability maps to a requirement and acceptance scenario (or explicit UNKNOWN/out-of-scope disposition), all 46 canonical RP-001–RP-046 rows have a task-contract-verified disposition, and unique implementation/verification follow-up tasks exist for every approved gap. The current 57 task folders are scaffolding; their count does not imply contract completeness and they do not satisfy this boundary. Product owner approval is required for scope, baseline versions and implementation priorities. Runtime parity and cutover remain separate later gates.

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

### Design↔code track (canonical spec R15)

1. Decide the canonical IR schema, node set, lowering contracts, edit policy and targets in an ADR (`high_chore-decide-design-code-ir-schema-and-target-adr`). Reuse the existing token model in `src/designlint/` where it fits.
2. After the ADR is accepted and the owner selects the design workflow, implement the Rust IR, importer/exporter, codegen and parser for one target with a fidelity corpus (`high_feature-implement-design-code-ir-round-trip`).

**Gate:** ADR accepted by the owner; the fidelity threshold is recorded before any parity or round-trip claim.

## Program source of truth and supporting evidence

The canonical requirement IDs are R1–R16 in `.plaesy/spec/plaesy-agent-platform/spec.md`; the canonical plan, phase gates, priority and RP-001–RP-046 mappings are in this file; executable tasks and lifecycle statuses are in `.plaesy/tasks/`. These are separate but linked roles, not parallel copies. `docs/platform/replacement-spec.md`, `replacement-plan.md`, and `replacement-backlog.md` are supporting product research and a provisional historical crosswalk; do not use their RP-01–RP-16 taxonomy or priority statements as an authority. `docs/platform/feature-gap-register.md`, `feature-measurement-framework.md`, `replacement-source-inventory.md`, `hermes-api-acceptance.md`, and `hermes-capability-audit.md` remain evidence/acceptance resources and are linked rather than duplicated. Design↔code is explicitly in canonical R15 and its two named ADR/implementation tasks, outside the claim that the 11 incumbent audits alone cover the full program.

### Visual flow track (spec R16)

1. Accept ADR 0002 (`docs/adr/0002-visual-flow-ir.md`): node catalogue, CEL expressions, retry formula, loop limits, approval channel, and Rust as the first codegen adapter. Done 2026-11-18.
2. Implement the flow IR core in Rust, with schema version, stable node IDs, and validation (`high_feature-implement-visual-flow-ir-core-in-rust`, lane H).
3. Generate Rust code through the first adapter and check it against the IR fixtures (`high_feature-implement-visual-flow-rust-codegen-adapter`, lane H).
4. Build the Dioxus web editor on the Rust core (`high_feature-implement-visual-flow-dioxus-web-editor`, lane H). Desktop and mobile reuse the same core.
5. Add Go and JavaScript adapters later, as separate tasks. The umbrella `high_feature-implement-visual-flow-editor-and-codegen` closes when 2–4 pass.

**Gate:** IR fixtures pass for every node kind in the catalogue, and unsupported kinds are reported as losses.

## Parallel work boundaries

Lanes. Each implementation task carries a `lane-<X>` label and a `## Parallel execution` section. That section is the authority for its owned paths.

- A: adapter and runtime contract
- B: identity, policy and secrets
- C: shared messaging and delivery (only if the chosen workflow crosses agents or hosts)
- D: run and task persistence and recovery
- E: test harness, evidence and release gates
- F: operator surface
- G: workspace isolation (R8)
- H: visual flow (R16)
- I: design-code (R15)
- J: knowledge and memory controls

Rules:

1. Each agent works in its own `git worktree` and branch, named after the task, created from one pinned base commit. Remove the worktree with `git worktree remove` after merge. Do not delete worktree folders by hand.
2. An agent claims a task only when every dependency in `issue.json` is `done` and its `ownership-assigned` check passes.
3. Owned paths are exclusive. The integrator-only files (`Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`) change only in integrator commits. Agents request registration there.
4. One integrator merges branches in dependency order, then reruns `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, and the focused tests of each touched task.
5. `tasks run --parallel` runs all claimed tasks in one shared directory. Use it only for tasks with disjoint paths. Otherwise use separate worktrees.
6. Focused checks use `cargo test --locked <prefix>` and sum the `N passed` lines. Cargo exits 0 when a filter matches nothing (checked locally on 18 Nov 2026), so an exit-code-only check would pass vacuously.
7. Contract `run` commands use `.plaesy/tasks/*/<id>/`, not the backlog folder, so checks still resolve after a task moves to `doing` or `done`.

Practice references (community sources, not official Git guidance): one worktree and one branch per agent, with ownership boundaries written down, is described at [openreplay](https://blog.openreplay.com/git-worktrees-ai-agents/), [davidloor](https://davidloor.com/en/blog/how-to-keep-multiple-coding-agents-from-overwriting-each-other), and [tonyreviewsthings](https://www.tonyreviewsthings.com/git-worktrees-for-agents/). Splitting work by domain and avoiding shared files is described at [MindStudio](https://www.mindstudio.ai/blog/git-worktrees-parallel-ai-coding-agents). Cargo filter behavior is discussed in [cargo issue #6151](https://github.com/rust-lang/cargo/issues/6151).

## Execution readiness (2026-11-18)

- Closed: status-agnostic contract paths (163 `run` commands in 54 tasks). Visual-flow ADR task is `done`. Visual flow is split into three child tasks with disjoint paths. The `issue.json` dependencies match `task.md`. The two R9 implementation tasks depend on the owner decisions. The Rust toolchain is pinned to 1.99.1 (`rust-toolchain.toml`). `cargo fmt --check` and `cargo clippy -D warnings` pass on the current tree. Focused-test checks fail on a zero match.
- Open, owner decision (not decided by the planner): first replacement workflow, deployment profile, runtime strategy, identity hierarchy (spec open questions). Until recorded, the blocked tasks stay blocked and their owned paths stay `OWNERSHIP-UNASSIGNED`.
- Open, integrator: assign owned paths for the unassigned tasks after the vertical-slice spec. Add the planned flow-editor crate to the Cargo workspace before the Dioxus task is claimed.

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
- The `plaesy.exe` on PATH (installed 17 Nov 2026) fails `tasks update-issue` on Windows with "outside the locked collection". A binary built from this repository works. Reinstall the CLI or run `target/debug/plaesy.exe` before task commands.

## Links

These paths are relative to the repository root, matching the spec references above:

- Spec: `.plaesy/spec/plaesy-agent-platform/spec.md`
- Feature register: `docs/platform/feature-gap-register.md`
- Product: `docs/platform/product.md`
- Decisions: `docs/platform/decisions.md`
- Current-state assessment: `docs/platform/current-state-assessment.md`
- Task command safety/semantics: `docs/commands/tasks.md`
- Backlog index: `.plaesy/tasks/backlog/` (57 active task folders: 56 backlog, 1 done; see spec task index; these are scaffolding, not a complete per-capability backlog)
- **Authority and supporting artifacts:** `.plaesy/spec/plaesy-agent-platform/spec.md` and this plan govern program requirements and sequencing; `.plaesy/tasks/` contains executable work. `docs/platform/replacement-spec.md`, `replacement-plan.md`, and `replacement-backlog.md` are explicitly supporting research/crosswalk documents, not parallel authorities. Product evidence, feature registers, and design-code research may remain in `docs/platform/` and must be linked from the canonical spec/plan.
