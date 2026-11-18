# Plaesy replacement program — supporting backlog crosswalk

> **Authority:** Executable tasks and their statuses live in `.plaesy/tasks/`; the canonical requirements and program priorities are in `.plaesy/spec/plaesy-agent-platform/spec.md` and `.plaesy/plan/plaesy-agent-platform/plan.md`. This document preserves a supplementary 46-row RP taxonomy and provisional mapping; it is not the executable task list, and its RP-01 numbering is not interchangeable with canonical RP-001 IDs. Reconcile each row with the canonical index before using it for decisions.

**Status:** Historical supporting crosswalk; not an execution/status authority. All mappings are provisional. Read canonical task status and dependency metadata from `.plaesy/tasks/`; do not start conditional implementation tasks until owner decisions and dependencies are recorded there.

## Priority and mapping

The P0–P5 labels below are historical/supporting planning hints. They do not set canonical priority or override `.plaesy/plan/plaesy-agent-platform/plan.md`; RP-01–RP-16 numbering in the supporting spec is distinct from this RP-001–RP-046 taxonomy. Mappings remain provisional until reconciled against canonical task contracts and evidence. Task status is authoritative only in `.plaesy/tasks/`.

P0 = scope/evidence gates; P1 = contracts/architecture; P2 = shared foundations; P3 = selected workflow families; P4 = hardening; P5 = migration/cutover. IDs are stable, unique, and capability-centered. Each implementation task must be split further only when distinct ownership or independently verifiable outcomes warrant it; do not create duplicate tasks per product.

| ID | Phase | Unique outcome | Maps to products / spec |
|---|---|---|---|
| RP-001 | P0 | Owner decisions: first workflows/order, replacement boundary, user cohort, profile/platform, UI boundary, NFR and cutover appetite; log decisions/owners | All; RP-02 |
| RP-002 | P0 | Recover and index prior comparison research with source/version/date; avoid reruns | All; RP-01 |
| RP-003 | P0 | Pin all 11 upstream repos/releases and verify product identity, license, components, documented workflows; resolve DeepSeek Harness and Orca ambiguity | All; RP-01 |
| RP-004 | P0 | Build canonical workflow/capability inventory, deduplicate overlap, map status/evidence/owner/test/next action | All; RP-01/02 |
| RP-005 | P0 | Bounded Plaesy baseline audit for mapped capabilities across CLI/Studio/services; update statuses only with code/test evidence | All; RP-01 |
| RP-006 | P1 | Define domain architecture, Rust workspace/module boundaries, dependency rules and ADR record for approved deployment/UI/service strategy | All; RP-15 |
| RP-007 | P1 | Define threat model, principal/tenant/project model, authorization matrix, secrets, approval and audit policies | Hermes, Paperclip, Nimbalyst, Orca, Herald OS, OpenProject; RP-08 |
| RP-008 | P1 | Define versioned domain/API/event/data contracts for project/task/run/artifact and lifecycle state machines | Hermes, Paperclip, DeepSeek, Nimbalyst, Orca, OpenProject; RP-03/04/05 |
| RP-009 | P1 | Define runtime/provider/harness adapter contract, capability negotiation and deterministic fake conformance/fault suite | Hermes, DeepSeek, Paperclip, Nimbalyst, Orca, Craft Anything, Herald OS; RP-03/05 |
| RP-010 | P1 | Define data portability/import/export and per-source migration mappings, checksums, dry-run and rollback contract | All; RP-10/14/16 |
| RP-011 | P2 | Implement authenticated principal, scoped authorization, approval, revocation, secret references and audit | As RP-007; RP-08 |
| RP-012 | P2 | Implement durable project/task/run lifecycle, idempotency/reconciliation, cancellation, restart recovery and artifacts; for session persistence include explicit append-vs-flush durability, ordered retained-batch retry, torn-tail recovery, and shutdown drain guarantees | As RP-008; RP-04/05/038 |
| RP-013 | P2 | Implement supervised runtime adapter execution with bounds, cancellation, truthful status/usage/errors and fake-tested failure handling | As RP-009; RP-03/05 |
| RP-014 | P2 | Implement safe concurrent workspace/worktree isolation and conflict/stale-write detection | Nimbalyst, Craft Anything, FigCraft, DeckCraft, WordCraft, Paperclip; RP-07 |
| RP-015 | P2 | Implement authenticated project-scoped collaboration/message delivery only where selected workflows require it; distinct queued/accepted/acknowledged states | Paperclip, Nimbalyst, Orca, FigCraft; RP-06 |
| RP-016 | P2 | Implement operator CLI/API/UI for task/run/agent health, approvals, evidence and authorized recovery | All selected workflows; RP-11 |
| RP-017 | P2 | Implement provenance-aware knowledge/memory controls, retention/deletion, export and review | Hermes, Paperclip, Herald OS; RP-09 |
| RP-018 | P3 | Deliver owner-selected agent execution/learning workflow family; provider/harness integration and memory/skill behavior where selected | Hermes, DeepSeek Harness; RP-03/05/09 |
| RP-019 | P3 | Deliver selected organization/goals/governance/work-planning workflows: hierarchy, assignment, approvals, budgets, work tracking/reporting | Paperclip, OpenProject; RP-04/08 |
| RP-020 | P3 | Deliver selected visual workspace/parallel coding workflow: sessions, worktrees, task links, editors, Git and extension surface | Nimbalyst; RP-07/10/11/13 |
| RP-021 | P3 | Deliver selected investigate/build/compare clean-room workflow with reproducible evidence and safe coding-agent integration | Craft Anything; RP-03/05/13 |
| RP-022 | P3 | Deliver selected interface design/prototyping workflow: document model, command API, rendering, collaboration/import-export as scoped | FigCraft; RP-10/13 |
| RP-023 | P3 | Deliver selected slide authoring/presentation workflow with PPTX round-trip and render corpus | DeckCraft; RP-10 |
| RP-024 | P3 | Deliver selected word-processing/document workflow with DOCX round-trip and corpus-based fidelity | WordCraft; RP-10 |
| RP-025 | P3 | Deliver selected permission-gated OS automation workflow on explicitly supported OS; isolation, approval and recovery tested | Herald OS; RP-08/11 |
| RP-026 | P3 | Deliver selected portfolio/project-management workflow, including selected agile/roadmap/Gantt/time/reporting scope | OpenProject; RP-04/10 |
| RP-027 | P3 | Deliver selected remote-runtime/mobile/relay workflow only if owner-approved; authenticated transport, explicit liveness and delivery semantics | Orca, Nimbalyst; RP-06/11 |
| RP-028 | P1 (foundation before Rust implementation; release-specific evidence rechecked in P4) | Establish Rust CI/release quality gates: pinned toolchain and locked builds, format, Clippy, focused tests/doc tests, dependency/license/advisory and secret scans, supported-target matrix, artifact provenance, and enforceable crate dependency rules | All; RP-15 |
| RP-029 | P4 | Establish measurable per-workflow NFRs, benchmarks, workload and supported-platform matrix; test against approved thresholds | All selected; RP-15 |
| RP-030 | P4 | Complete security/fuzz/property/adversarial tests, privacy/redaction review, accessibility/localization checks as required | All selected; RP-08/10/15 |
| RP-031 | P4 | Exercise install/upgrade/backup/restore/rollback, observability, retention and operator runbooks on declared profiles | All selected; RP-12 |
| RP-032 | P5 | Build per-source migration tooling and execute dry-run/reconciliation for owner-selected workflows; preserve source data | Each selected incumbent; RP-14 |
| RP-033 | P5 | Run pinned-version side-by-side parity and user acceptance; publish evidence, differences, unknowns and residual risks | Each selected workflow; RP-16 |
| RP-034 | P5 | Conduct parallel run, recovery/rollback rehearsal and obtain explicit per-workflow cutover approval; no incumbent retirement before gate | Each selected workflow; RP-16 |
| RP-035 | P0 | Complete Hermes feature-page leaf audit from pinned source: manually decompose documented behavior, limits, platform/edition conditions, security semantics, failure cases and code/test evidence; map every accepted capability to scenarios, Plaesy baseline and follow-on tasks | Hermes; RP-003/004/005 |
| RP-036 | P0 | Audit Hermes Runs SSE transport details against pinned route/store/tests: cursor/Last-Event-ID support and replay gaps, subscriber overflow/detach, 300-second buffer expiry, post-expiry reconnect/pollability, and separation from live run control; add only evidence-backed scenarios and record undocumented implementation details separately | Hermes; RP-035 |
| RP-037 | P0 | Complete Paperclip leaf-capability audit across API, runtime, UI, adapters/plugins, migrations, and operator docs; reconcile normative/target specs against pinned implementation and tests, and map each accepted capability to scenario + Plaesy baseline + deduplicated task | Paperclip; RP-003/004/005 |
| RP-038 | P0 | Audit DeepSeek Harness from pinned source by executable workflow and contract (CLI, orchestration, tools, state, failure/recovery, security); resolve product identity/scope and map documented behavior to code/tests without assuming docs imply implementation | DeepSeek Harness; RP-003/004/005 |
| RP-039 | P0 | Audit Nimbalyst by user-observable workflow and source module (workspace/session, parallel agents, Git/worktree, collaboration, integrations, persistence, recovery); record platform/edition constraints and evidence per leaf capability | Nimbalyst; RP-003/004/005 |
| RP-040 | P0 | Audit Orca from pinned source and resolve repository/product identity; map supported runtime, remote/mobile/relay, transport/auth, delivery/liveness, and recovery behavior to exact code/tests or keep unknown | Orca; RP-003/004/005 |
| RP-041 | P0 | Audit Craft Anything using pinned source and reproducible user workflows; identify product boundaries, agent/build orchestration, evidence artifacts, integrations, safety controls, and failure semantics, distinguishing observed implementation from marketing/docs | Craft Anything; RP-003/004/005 |
| RP-042 | P0 | Audit FigCraft design/prototyping behavior from pinned source: document model, editing commands, rendering, import/export, collaboration, persistence, and fidelity limits; derive acceptance candidates only from inspected evidence | FigCraft; RP-003/004/005 |
| RP-043 | P0 | Audit DeckCraft presentation workflows from pinned source: authoring model, slide/layout/theme operations, export formats, rendering/fidelity, interoperability and recovery; map leaf capabilities to reproducible scenarios | DeckCraft; RP-003/004/005 |
| RP-044 | P0 | Audit WordCraft document workflows from pinned source: authoring/editing model, formatting, import/export, DOCX fidelity, collaboration, persistence and recovery; map leaf capabilities to reproducible scenarios | WordCraft; RP-003/004/005 |
| RP-045 | P0 | Audit Herald OS from pinned source: supported OS/actions, agent/tool execution, permissions/approval, isolation, secrets, auditability, failure recovery and user-visible status; keep platform-specific or unverified behavior explicit | Herald OS; RP-003/004/005 |
| RP-046 | P0 | Audit OpenProject by module and user workflow from pinned source/docs: portfolio/project/task lifecycle, agile boards, scheduling/Gantt, time/cost, reporting, permissions, integrations, migration and operations; identify edition/license boundaries and map inspected leaves to acceptance scenarios | OpenProject; RP-003/004/005 |

## Plaesy executable task crosswalk (deduplicated)

The RP rows above are historical program-level planning, not an authority for task status or dependencies. This crosswalk predates the currently indexed task set. Folder count is not an acceptance-completeness measure; the crosswalk below groups related RP rows and is a planning aid only: a mapped task is not proof that its contract meets every RP acceptance condition. Row-by-row disposition remains open until every canonical RP row is traced to an inspected contract with acceptance evidence, an explicit status/dependency with rationale, or a justified unique follow-up. The audit task must resolve unverified or orphaned rows and recommend distinct tasks only for independently verifiable outcomes; no RP is closed by appearing in the crosswalk.

For any future Rust implementation task, acceptance must require the repository-pinned toolchain, committed lockfile and locked build (`cargo ... --locked` where applicable), `cargo fmt --all -- --check`, focused `cargo test`, Clippy on the changed crate/target with warnings denied, and relevant dependency/license/advisory/security checks. CI/release-gate tasks must make these checks executable and report evidence; a prose rule alone is not completion. UI technology remains subject to owner-approved ADR.

| RP rows | Plaesy task | Coverage / note |
|---|---|---|
| RP-001 | `high_chore-owner-decisions-for-first-replacement-slice` | First workflow, boundary/profile/runtime and exclusions; broader ordering/NFR decisions remain to be reflected in task/spec evidence. |
| RP-002 | `high_chore-recover-and-compare-incumbent-research` | Recover prior evidence; scope must include all 11 requested targets and source/version provenance. |
| RP-003, RP-035–RP-046 | Umbrella `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents`; `high_chore-audit-agent-runtime-and-workspace-products`; `high_chore-audit-creative-authoring-products`; `high_chore-audit-work-management-and-os-products` | Umbrella coordinates completion; agent/runtime/workspace, creative authoring, and work-management/OS evidence ledgers are distinct outcomes. Not exhaustive until each acceptance set is evidenced. |
| RP-004 | `high_chore-produce-canonical-cross-incumbent-capability-inventory` and audit task above | Distinct inventory outcome owns canonical deduplicated workflow/leaf capability rows and per-row evidence, baseline, disposition, and next action; umbrella audit supplies source evidence and reconciliation. |
| RP-005 | `high_chore-verify-feature-baseline-across-repositories` and audit task above | Baseline task covers bounded implementation-status verification only; do not infer implementation status from inventory membership. |
| RP-006 | `high_chore-define-rust-domain-architecture-and-approved-deployment-boundaries` | Architecture task records owner decisions, Rust boundaries and ADR; UI/runtime/deployment unknowns remain conditional. |
| RP-007 | `high_chore-specify-identity-tenancy-threat-model-and-authorization-contracts` | Threat model and identity/authorization contracts before implementation; unresolved owner choices remain UNKNOWN. |
| RP-008 | `high_chore-specify-versioned-project-task-run-artifact-and-event-contracts` | Versioned domain/API/event contracts; selected workflow scope remains a prerequisite. |
| RP-009 | `high_feature-define-runtime-adapter-and-fake-conformance-suite` and `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting` | Adapter specification/fake tests separate from production supervised execution. |
| RP-010 | `high_chore-specify-data-portability-and-source-migration-contracts` | Portability contract precedes any source-specific migrator. |
| RP-011 | `critical_feature-implement-authenticated-agent-identity-and-policy-boundary` and `high_feature-implement-approval-secret-reference-and-audit-controls` | Existing boundary task plus distinct approval/secrets/audit implementation. |
| RP-012 | `critical_feature-implement-durable-task-run-lifecycle-and-recovery` and `high_feature-implement-durable-run-append-retry-torn-tail-recovery-and-shutdown-drain` | Lifecycle task plus explicit storage append/flush, retry, torn-tail and shutdown guarantees; AC-DSH-SESSION-001–008 remain incumbent audit candidates, not replacement-test evidence. |
| RP-013 | `high_feature-define-runtime-adapter-and-fake-conformance-suite` and `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting` | Separate contract/fake conformance from supervised production execution and truthful usage/error reporting. |
| RP-014 | `high_feature-enforce-isolated-concurrent-agent-changes` | Workspace isolation/conflict safety where selected. |
| RP-015 | `high_feature-add-authenticated-shared-messaging-and-session-delivery` | Conditional shared collaboration/delivery. |
| RP-016 | `high_feature-add-operator-observability-and-recovery-controls` | Operator state and authorized recovery. |
| RP-017 | `high_feature-add-provenance-aware-knowledge-and-memory-controls` | Reusable knowledge/memory lifecycle outcome; refine scope from the 11-product audit. |
| RP-018 | `high_feature-deliver-selected-agent-execution-and-learning-workflow` | Conditional Hermes/DeepSeek workflow; backend and memory behavior must be evidenced and selected. |
| RP-019 | `high_feature-deliver-selected-work-management-and-governance-workflow` | Conditional Paperclip/OpenProject features; exact hierarchy/planning scope remains owner-selected. |
| RP-020 | `high_feature-deliver-selected-visual-workspace-and-parallel-coding-workflow` | Conditional Nimbalyst-style workspace; no broad parity implied. |
| RP-021 | `high_feature-deliver-selected-investigate-build-compare-workflow` | Conditional Craft Anything workflows, with reproducible evidence and safety controls. |
| RP-022 | `high_feature-deliver-selected-design-and-prototyping-workflow` | Conditional FigCraft workflow and fidelity corpus. |
| RP-023 | `high_feature-deliver-selected-presentation-authoring-workflow` | Conditional DeckCraft workflow and PPTX round-trip evidence. |
| RP-024 | `high_feature-deliver-selected-document-authoring-workflow` | Conditional WordCraft workflow and DOCX round-trip evidence. |
| RP-025 | `high_feature-deliver-selected-approved-os-automation-workflow` | Conditional Herald OS scope; platform and approval decisions required. |
| RP-026 | `high_feature-deliver-selected-project-portfolio-management-workflow` | Conditional OpenProject workflow; edition/license boundaries audited first. |
| RP-027 | `high_feature-deliver-selected-remote-runtime-or-relay-workflow` | Conditional Orca/Nimbalyst workflow; authenticated transport/liveness proof required. | Leverage the existing umbrella audit and owner decision task; refine scope and acceptance only after the relevant incumbent is audited. Create issue/task entries as distinct conditional capability-family deliverables, not as claims of inclusion or authorization.
| RP-028 | `high_chore-establish-rust-security-and-release-quality-gates` | Rust quality/security/release gates; RP-028 is P1 and profile-specific evidence is rechecked at release. |
| RP-029 | `high_chore-define-measurable-workflow-nfrs-and-benchmark-thresholds` | Owner-approved per-workflow NFRs and benchmark methodology; do not infer production sizing from development VM. |
| RP-030 | `high_chore-establish-rust-security-and-release-quality-gates` and selected workflow verification tasks | Security/adversarial/privacy/accessibility/localization tests must be scoped per approved workflow from audit evidence. |
| RP-031 | `high_chore-verify-install-backup-restore-and-rollback-lifecycle` | Profile-specific install/backup/restore/rollback. |
| RP-032 | `high_chore-specify-data-portability-and-source-migration-contracts` and `high_chore-implement-source-specific-migration-and-reconciliation-for-selected-workflow` | Source-specific migration/reconciliation is conditional on approved source/workflow and retains source immutability/rollback gates. |
| RP-033–RP-034 | `critical_chore-run-incumbent-parity-parallel-run-and-cutover-gate` | Side-by-side, parallel run and cutover only after evidence and owner approval. |

Execution and dependency status are authoritative only in `.plaesy/tasks/`; this crosswalk is a planning aid, not proof that existing task contracts include every mapped detail. The audit task must resolve orphaned rows and recommend distinct tasks only for independently verifiable outcomes; update both this table and the Plaesy task index when such tasks are created. Some capability families are represented by conditional tasks, not leaf-complete contracts; all product-derived leaf capabilities still require audit and owner disposition before task scope is final. Tasks do not establish implementation evidence or authorize a cutover.

- hermes-agent: `high_chore-audit-hermes-agent-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- paperclip: `high_chore-audit-paperclip-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- deepseek-harness: `high_chore-audit-deepseek-harness-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- nimbalyst: `high_chore-audit-nimbalyst-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- orca: `high_chore-audit-orca-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- craft-anything: `high_chore-audit-craft-anything-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- figcraft: `high_chore-audit-figcraft-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- deckcraft: `high_chore-audit-deckcraft-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- wordcraft: `high_chore-audit-wordcraft-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- herald-os: `high_chore-audit-herald-os-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.

- openproject: `high_chore-audit-openproject-capability-ledger` — bounded evidence-ledger subtask; no parity conclusion or upstream edits.
