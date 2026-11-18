# Plaesy replacement program — plan

**Status:** Draft, phased program. **Spec:** [replacement-spec.md](replacement-spec.md). **Backlog:** [replacement-backlog.md](replacement-backlog.md). This is not an authorization to implement all targets at once or retire incumbents. The 46 RP rows are a program-level task map; the current `.plaesy` topic has 18 Plaesy task folders as higher-level work items, not a one-to-one import. Task-directory count is not an acceptance or coverage measure; the RP-to-task crosswalk remains provisional until each row is reconciled to an executable task contract or an explicit status. The audit umbrella task coordinates three distinct evidence work packages (agent/runtime/workspace; creative authoring; work-management/OS automation), not 11 product clones. Task presence does not prove its acceptance criteria are complete.

## Measurement and definition of done

Apply `docs/platform/feature-measurement-framework.md` to every target feature. The feature inventory is the source of truth for completeness; use canonical capability mappings to prevent duplicate implementation tasks. Do not close a product-replacement milestone when only its task list is complete: all mandatory feature acceptance scenarios, migration, security, operational readiness, user acceptance and rollback gates must pass for the approved boundary.

## Principles and sequencing

1. Recover existing evidence and establish one canonical feature map before repeating comparisons. Pin exact upstream release/commit and distinguish documented from verified behavior.
2. Use the captured local host envelope as a development constraint, not as production sizing: 2 vCPU on a KVM guest (AMD EPYC 9754 host model exposed), 7.4 GiB RAM (5.0 GiB available at capture), 1.9 GiB swap (1.3 GiB used), 15 GiB free disk, x86_64, Rust/Cargo 1.99.1. Capture is dated 2026-11-18 in Asia/Jakarta. No `nvidia-smi` executable was found; accelerator status remains unknown. Benchmark and re-check this mutable VM before sizing build/test work.
3. Product owner selects initial workflow(s), replacement boundary, profile, UI and release criteria. Until then, implementation scope is blocked; no agent decides these choices by inference.
4. Build shared foundations once (identity, policy, task/run lifecycle, persistence, adapters, migration, operator controls) and map multiple products to the same capability IDs. Do not create a task per product when the work is the same.
5. Implement a thin vertical slice, prove it with fault/security tests, then expand by workflow families. Keep incumbents running until an explicit cutover gate.
6. Use Rust best-practice gates in the spec; all Rust code lives in Plaesy, never upstream repositories. For approved implementation slices, pin the Rust toolchain, commit the application lockfile, use locked builds, enforce formatting/Clippy/focused tests, and collect dependency/license/advisory/security evidence. Any UI/platform exception must be approved in an ADR; no exception moves core domain logic outside Rust.

**Rust policy:** Core, domain, persistence, service, CLI and native runtime replacement code is Rust. UI implementation technology remains open pending owner-approved platform/UI ADR, but its business rules and authoritative state remain in Rust services/core.

## Phases and exit gates

### Phase 0 — ownership, inventory, evidence (P0)

- Resolve owner decisions; inventory the 11 repositories and reusable prior research.
- Create stable capability/workflow IDs; deduplicate overlapping needs; map source version → workflow → Plaesy requirement/module → test/evidence → task.
- Inspect the existing feature register and current implementation; mark unknowns honestly. Audit exact DeepSeek Harness and Orca targets.
- Establish Rust workspace/module boundaries, pin the toolchain and lockfile policy, and make baseline format/Clippy/focused-test/locked-build checks executable before the first Rust implementation task. Do not select UI/runtime/deployment targets before owner approval.
- Assign RP-028's early foundation subset to P1; repeat profile-specific target, supply-chain, security, and artifact-provenance evidence in P4 before release.
- Record the current VM envelope as a development-only constraint; execute `high_chore-establish-local-hardware-capacity-baseline` for repeatable build/test measurements before claiming local workload capacity.

**Exit:** owner-approved target boundary and first workflow; source/version map for all 11; baseline with citations and uncertainty; duplicate review; unresolved decisions named and assigned. Rust foundation-gate task and its executable checks are ready before implementation begins.

### Phase 1 — architecture and executable contracts (P1)

- ADRs for deployment profile, tenancy/identity, UI boundary, persistence, runtime/harness strategy, messaging, plugin sandbox, data formats and service boundaries.
- Define domain model/state machines, authorization matrix, versioned API/event/data contracts, migration format and threat model for selected workflows.
- Build deterministic fake adapters and conformance/fault suites before live integrations.

**Exit:** reviewed ADRs and threat model; contracts map to requirements; fake tests cover denial, timeout, cancellation, crash/restart, duplicates, lost acknowledgements and ambiguous external effects.

### Phase 2 — shared platform foundations (P2)

- Implement approved identity/policy boundary; durable project/task/run lifecycle; audit and safe secret references.
- Add replaceable runtime/provider adapters and bounded execution supervision; no false success on unknown outcomes.
- Add isolated worktrees or verified write/conflict controls; authenticated collaboration only if scope requires it.
- Add migration/export/import primitives and operator-visible state.

**Exit:** focused integration, security, persistence/recovery and concurrency tests pass in disposable environments; user-visible journey succeeds end-to-end with evidence.

### Phase 3 — workflow family pilots (P3)

Use backlog capability families, not product-by-product clones. Candidate families (ordering owner-approved): agent execution/learning; organization/governance/work planning; visual workspace/parallel coding; clean-room investigation/build; creative design/slides/documents; OS-level automation; enterprise project/portfolio management. For each selected family, implement only its prioritized workflows and map shared components once.

**Exit per workflow:** pinned-incumbent side-by-side acceptance, import/export validation, operator recovery, security authorization matrix, limitations documented and user approval.

### Phase 4 — hardening and operational readiness (P4)

- Run supported-platform matrix; performance/resource benchmarks against owner-defined workload; accessibility/localization checks as required.
- Complete dependency/license/security scans, fuzz/property tests for parsers/protocols, backup/restore and upgrade/rollback drills, observability and runbooks.
- Validate data retention, deletion, privacy and support posture; close or formally accept findings.

**Exit:** release candidate meets numeric NFRs and all mandatory security/operations gates; no unresolved critical/high issue without approved mitigation.

### Phase 5 — migration, parallel run, cutover (P5)

- Provide source-specific dry-run migration and reconciliations; retain originals; rehearse rollback.
- Run approved scenarios in parallel against pinned incumbent and Plaesy versions; record outcomes, data diffs, permission decisions, failures and operator actions.
- Cut over per workflow only with named approver, observation period, rollback trigger/owner, support plan and tested recovery.

**Exit:** RP-16 gate passes. Retirement is per workflow, not global; keep unselected incumbent workflows explicitly out of scope.

## Dependencies and parallelism

P0 blocks P1 decisions; P1 blocks production P2 interfaces; P2 foundations block family pilots; P3 blocks cutover. Independent research work can proceed in P0. After contract freeze, parallelize by disjoint module/file ownership: identity/security review; adapter conformance; persistence/lifecycle; migration; operator surface; workflow-specific editors. Use isolated branches/worktrees and an integrator; never parallel-write a shared directory without enforced isolation. Preserve unrelated worktree changes.

## Verification and reporting

- Each task names focused tests and exact evidence; test changed behavior, not the whole suite by default. Add broader checks only when justified by shared risk.
- Capture commands, pinned versions, outputs/artifact identifiers, environment and untested areas. No synthesis of test results.
- Run Markdown/link validation on changed docs, `git diff --check`, relevant Rust format/lint/tests; run security/release gates before release.
- At each phase report: verified, documented-only, inferred, unknown, blocked owner decisions, residual risk, and next gate.

## Risks / controls

- Scope spans unrelated large products: control through owner-selected workflows, capability deduplication, staged replacement and per-workflow gates.
- Upstreams change quickly: pin commits/releases and review evidence at each comparison.
- “Rust throughout” conflicts with platform UI constraints: require explicit UI-boundary ADR; avoid hidden non-Rust core logic.
- Native creative/document formats are deep compatibility work: define representative corpora and round-trip fidelity tiers, never claim full format compatibility from smoke tests.
- Local-first and shared deployment have distinct trust boundaries: test separately; local Relay/polling is not evidence of remote real-time semantics.
- `.plaesy/` is Git-ignored; these canonical deliverables are in tracked `docs/platform/` instead. Do not change ignore rules without approval.

## Decisions still required

Product owner: initial workflow/ordering, replacement-vs-integration per product, deployment/platform, user cohort/tenancy, UI boundary, release NFRs and migration/cutover appetite. Research owner: exact DeepSeek Harness and Orca project/version. These are blockers, not defaults to guess.
