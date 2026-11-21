# Plaesy replacement program — deduplicated backlog

**Status:** Proposed backlog; tasks are not executed. Spec: [replacement-spec.md](replacement-spec.md). Plan: [replacement-plan.md](replacement-plan.md). Dependencies are task IDs below. All tasks carry product mappings through capability IDs rather than cloning work per incumbent. Do not start implementation tasks until owner scope decisions pass.

## Priority and mapping

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
| RP-012 | P2 | Implement durable project/task/run lifecycle, idempotency/reconciliation, cancellation, restart recovery and artifacts | As RP-008; RP-04/05 |
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
| RP-028 | P4 | Establish Rust CI/release quality gates: pinned toolchain and locked builds, format, Clippy, tests/doc tests, dependency/license/advisory and secret scans, supported-target matrix, artifact provenance, and enforceable crate dependency rules | All; RP-15 |
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
