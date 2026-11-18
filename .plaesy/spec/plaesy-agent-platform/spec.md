# Spec: Plaesy agent platform and incumbent replacement

Status: draft
Topic: plaesy-agent-platform
Tasks: backlog task folders under `.plaesy/tasks/backlog/`; IDs listed in the Task index section below (the index is authoritative).

## Purpose

Plaesy's product requirements target a self-hosted platform for workflows, knowledge, agent execution, integrations and user-facing experiences, while the current implementation is fragmented across the Plaesy CLI and Studio. This spec turns the existing product direction and feature-gap register into testable, phased requirements for a staged Rust-based replacement program covering Hermes Agent, Paperclip, DeepSeek Harness, Nimbalyst, Orca, Craft Anything, FigCraft, DeckCraft, WordCraft, Herald-OS and OpenProject. It does not assert full parity or authorize replacing every workflow at once.

## Scope and evidence boundary

This is the canonical program-level specification, not approval to implement every target feature in one release. Requirements are staged by dependency; owners must select and validate one first replacement workflow before runtime or deployment assumptions are frozen. Implementations and statuses are to be assessed across CLI, Studio and deployed services, not inferred from one repository. Product evidence, feature registers, and design-code research remain supporting artifacts in `docs/platform/`; the supporting `replacement-spec.md`, `replacement-plan.md`, and `replacement-backlog.md` are not parallel authorities. Executable tasks and their lifecycle are under `.plaesy/tasks/`. The feature checklist at `docs/platform/feature-gap-register.md` is the current bounded baseline; status `unknown` remains unknown until inspected and tested.

## Requirements

| ID | Requirement | Acceptance criterion (Given / When / Then) | Status |
| --- | --- | --- | --- |
| R1 | Plaesy shall maintain a traceable feature inventory distinguishing implemented, partial, design-only, not implemented in inspected scope, and unknown capabilities. | Given a capability row in the inventory, when an agent updates its status, then the row links implementation and focused acceptance evidence or explicitly states why the status is unknown. | open |
| R2 | Plaesy shall establish the first replacement workflow, deployment profile, runtime strategy, exclusions, and ownership decisions before dependent interfaces are frozen. | Given no owner-approved profile or workflow, when implementation planning begins for the vertical slice, then the work records the owner decision and the selected workflow/profile/runtime or remains blocked on named unknowns. | open |
| R3 | Plaesy shall expose stable task and run identities, truthful lifecycle state, ownership, and evidence for an agent-executed workflow. | Given a task is submitted to the selected runtime, when execution succeeds, fails, times out, is cancelled, or the controller restarts, then task/run state and evidence reflect the observed outcome and can be reconciled after restart. | open |
| R4 | Plaesy shall support replaceable execution backends through explicit, permission-scoped adapter contracts and must distinguish Plaesy-owned capabilities from backend-provided behavior. | Given a backend and adapter conformance suite, when the adapter handles a run, then its supported lifecycle, permissions, tools, cancellation, timeout, usage, error, and recovery behavior is reported and tested without claiming unsupported backend features as native Plaesy features. | open |
| R5 | Plaesy shall authenticate agent principals and authorize each project/task/runtime action using least privilege, with revocation. | Given an agent principal with a defined project/task/tool grant, when it attempts allowed and denied operations or its grant is revoked, then only authorized operations succeed and subsequent revoked operations fail with auditable outcomes. | open |
| R6 | Plaesy shall provide durable, authenticated, project-scoped inter-agent messaging for the selected shared deployment profile. | Given authorized agents on separate processes or hosts, when they exchange messages during network loss, retry, restart, duplicate delivery, or lost acknowledgement, then the service preserves project isolation and exposes truthful queued, accepted, acknowledged, failed, or unknown delivery state. | open |
| R7 | Plaesy shall distinguish agent identity, runtime/session liveness, task state, and message delivery state; it shall not claim an idle session was woken or a message was read without receiver evidence. | Given a recipient is active, busy, offline, expired, or terminated, when a message is sent, then the adapter/service reports the observed recipient and delivery state and applies the documented retry/queue behavior. | open |
| R8 | Plaesy shall support safe concurrent agent work through isolated workspaces or an enforced write/conflict boundary. | Given two cooperating agents and a non-cooperating writer modify the same project, when changes are applied or integrated, then stale or conflicting writes are detected, original work is preserved, and the documented policy fails safely when coordination is unavailable. | open |
| R9 | Plaesy shall enforce tool/policy permissions, budgets, cancellation, audit and secret-redaction boundaries for agent runs. | Given synthetic secrets and allowed, denied, over-budget, cancelled and failing actions, when the run executes, then denied work has no prohibited side effect, stop/budget outcomes are honored, audit evidence is useful, and secrets are absent from unauthorized logs/exports. | open |
| R10 | Plaesy shall provide operator-visible task/run/agent/queue health and supported recovery actions for the selected workflow. | Given a run queued, active, retrying, failed, blocked or completed, when an authorized operator inspects the system, then the UI/API/CLI shows the persisted state, reason, correlation identifiers and only permitted recovery actions. | open |
| R11 | Plaesy shall provide documented install/configure/start/stop/status/upgrade/backup/restore/export/import/rollback journeys for the declared first deployment profile. | Given a supported clean installation or recoverable backup, when an operator performs the documented lifecycle journey or injects a defined failure, then the system reaches the expected state, preserves data or reports an actionable failure, and rollback is demonstrated. | open |
| R12 | Plaesy shall compare incumbent products against the same concrete user workflow using versioned, cited evidence and shall reuse prior research rather than repeat it. | Given an incumbent comparison row, when an agent adds or revises a claim, then it records exact product/version, primary source or reproducible observation, review date, Plaesy implementation/test mapping, uncertainty and next action; unsupported claims remain unknown. | open |
| R13 | Plaesy shall gate replacement/cutover on verified workflow parity, authorization, reliability, operations, migration and rollback evidence while keeping the incumbent available until the gate passes. | Given a candidate Plaesy slice and incumbent workflow, when the release gate is evaluated, then each mandatory criterion has linked test/operational evidence, failures block cutover, and rollback/parallel-run remain available. | open |
| R14 | Plaesy-owned replacement application, service, CLI, domain, persistence and native runtime logic shall be implemented in Rust. Any UI or platform-specific exception must be explicitly documented and approved in an architecture decision; it must not move core business logic outside Rust. | Given an approved implementation slice and its source/build pipeline, when the release evidence is reviewed, then all Plaesy-owned core/service/CLI/native code for the slice is Rust, the toolchain and dependencies are pinned/locked, and required Rust checks pass; any non-Rust UI/platform exception links to an approved ADR. | open |
| R15 | Plaesy shall convert between design documents and code through one canonical, versioned intermediate representation (IR), so each design source or code target needs one importer or exporter rather than a converter per pair. | Given a design file with frames, auto-layout, components/variants, tokens and assets, when it is imported to IR and exported to a code target, then every mapped node keeps a stable ID and a source map, unsupported constructs are reported as explicit losses (never silently dropped), and an ADR records the IR schema version and target list. Given code produced from the IR, when it is parsed back to IR under the documented edit policy, then the round trip preserves the mapped structure within the corpus-tested fidelity threshold. Status: the threshold is UNKNOWN until owner approval. | open |
| R16 | Plaesy shall provide a visual flow editor whose graph is stored in one language-neutral flow IR, so a flow can be authored once and code-generated later into any supported target language chosen per project. | Given a flow authored in the visual editor, when it is saved, then it is stored as a versioned flow IR with stable node IDs and no target-language fields; when a target language is selected, then the generated code passes the same flow test fixtures as the IR, and unsupported node types are reported as explicit losses rather than dropped. Decisions recorded 2026-11-18 by the product owner: generated target language is Rust; first editor surface is web; the flow IR and core must stay reusable so desktop and mobile surfaces can be added later without changing the IR. Still UNKNOWN until owner approval: the web UI technology (Rust-to-WASM, which keeps R14 without an exception, or a JavaScript UI, which needs an ADR exception). Status: open. | open |

## Task index

All 52 active task folders under `.plaesy/tasks/` are indexed below; this count includes work in backlog and done, not just `.plaesy/tasks/backlog/`. This index is a directory inventory, not a mapping of task acceptance to each requirement; traceability remains incomplete until every requirement has an acceptance scenario and task or explicit disposition. Conditional implementation work is gated on audited evidence and owner selection; task existence does not authorize implementation or assert parity.

- `critical_chore-run-incumbent-parity-parallel-run-and-cutover-gate`
- `critical_feature-implement-authenticated-agent-identity-and-policy-boundary`
- `critical_feature-implement-durable-task-run-lifecycle-and-recovery`
- `critical_feature-specify-first-replacement-vertical-slice`
- `high_chore-audit-agent-runtime-and-workspace-products`
- `high_chore-audit-craft-anything-capability-ledger`
- `high_chore-audit-creative-authoring-products`
- `high_chore-audit-deckcraft-capability-ledger`
- `high_chore-audit-deepseek-harness-capability-ledger`
- `high_chore-audit-figcraft-capability-ledger`
- `high_chore-audit-herald-os-capability-ledger`
- `high_chore-audit-hermes-agent-capability-ledger`
- `high_chore-audit-nimbalyst-capability-ledger`
- `high_chore-audit-openproject-capability-ledger`
- `high_chore-audit-orca-capability-ledger`
- `high_chore-audit-paperclip-capability-ledger`
- `high_chore-audit-wordcraft-capability-ledger`
- `high_chore-audit-work-management-and-os-products`
- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents`
- `high_chore-decide-design-code-ir-schema-and-target-adr`
- `high_chore-decide-visual-flow-ir-and-codegen-adr`
- `high_chore-define-measurable-workflow-nfrs-and-benchmark-thresholds`
- `high_chore-define-rust-domain-architecture-and-approved-deployment-boundaries`
- `high_chore-establish-local-hardware-capacity-baseline`
- `high_chore-establish-rust-security-and-release-quality-gates`
- `high_chore-implement-source-specific-migration-and-reconciliation-for-selected-workflow`
- `high_chore-owner-decisions-for-first-replacement-slice`
- `high_chore-produce-canonical-cross-incumbent-capability-inventory`
- `high_chore-recover-and-compare-incumbent-research`
- `high_chore-specify-data-portability-and-source-migration-contracts`
- `high_chore-specify-identity-tenancy-threat-model-and-authorization-contracts`
- `high_chore-specify-versioned-project-task-run-artifact-and-event-contracts`
- `high_chore-verify-feature-baseline-across-repositories`
- `high_chore-verify-install-backup-restore-and-rollback-lifecycle`
- `high_feature-add-authenticated-shared-messaging-and-session-delivery`
- `high_feature-add-operator-observability-and-recovery-controls`
- `high_feature-add-provenance-aware-knowledge-and-memory-controls`
- `high_feature-define-runtime-adapter-and-fake-conformance-suite`
- `high_feature-deliver-selected-agent-execution-and-learning-workflow`
- `high_feature-deliver-selected-approved-os-automation-workflow`
- `high_feature-deliver-selected-design-and-prototyping-workflow`
- `high_feature-deliver-selected-document-authoring-workflow`
- `high_feature-deliver-selected-investigate-build-compare-workflow`
- `high_feature-deliver-selected-presentation-authoring-workflow`
- `high_feature-deliver-selected-project-portfolio-management-workflow`
- `high_feature-deliver-selected-remote-runtime-or-relay-workflow`
- `high_feature-deliver-selected-visual-workspace-and-parallel-coding-workflow`
- `high_feature-deliver-selected-work-management-and-governance-workflow`
- `high_feature-enforce-isolated-concurrent-agent-changes`
- `high_feature-implement-approval-secret-reference-and-audit-controls`
- `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting`
- `high_feature-implement-design-code-ir-round-trip`
- `high_feature-implement-durable-run-append-retry-torn-tail-recovery-and-shutdown-drain`
- `high_feature-implement-visual-flow-editor-and-codegen`

The evidence audit must refine capability families into deduplicated leaf tasks for approved scope, and retain UNKNOWN/out-of-scope dispositions for everything else.

## Non-goals

- Replacing every incumbent or implementing the full product vision in one release.
- Editing any of the 11 incumbent upstream source repositories.
- Declaring real-time push, cross-host delivery, session wake, identity, safety, or parity without implemented and tested evidence.
- Selecting a runtime backend, public API, database, deployment topology, tenant hierarchy, or first workflow on behalf of the product owner.
- Treating an agent's prose or a task status transition alone as proof of successful work.

## Constraints

- Preserve existing user work and local repository changes; no cleanup/revert of unrelated files.
- Keep credentials and secrets out of specs, task descriptions, logs, comparisons, and fixtures.
- Keep local-first/offline and self-hosted requirements from `docs/platform/product.md`; define remote trust boundaries explicitly.
- Existing Relay's local SQLite queue/polling, self-declared identity and advisory file claims are not equivalent to authenticated shared realtime service or enforced file locks.
- Every implementation task must identify focused tests, observable acceptance evidence, dependencies and file ownership. Parallel work must avoid overlapping write ownership.
- Plaesy-owned replacement core/service/CLI/domain/persistence/native runtime logic is Rust (R14). Any UI/platform exception requires an owner-approved ADR; implementation evidence must show pinned toolchain, locked dependencies/build, format, focused tests, Clippy, and applicable security/license checks. Existing non-Rust code in the current repository does not waive this requirement.
- Do not classify a capability as implemented solely because upstream runtime supports it or a design document describes it.

## Open questions

- UNKNOWN: Which single incumbent workflow is the first replacement target? Owner: product owner.
- UNKNOWN: Which local/shared deployment profile must the first slice support? Owner: product owner.
- UNKNOWN: Should Plaesy initially orchestrate external harnesses, or provide a native runtime first? Owner: product owner.
- UNKNOWN: What organization/project/agent identity and ownership hierarchy should be canonical? Owner: product/architecture owner.
- UNKNOWN: Which operational SLO, retention, backup, restore and support profile is required for first release? Owner: product/operations owner.
- UNKNOWN: Exact DeepSeek harness and Orca product/version intended for comparison. Owner: research owner.
- UNKNOWN: Whether earlier comparative research exists outside the inspected repository paths. Owner: research owner; link it before repeating research.

## References

- `docs/platform/feature-gap-register.md` — bounded feature inventory and gaps.
- `docs/platform/product.md` — product vision, target scope, principles and lifecycle acceptance.
- `docs/platform/current-state-assessment.md` — current implementation boundaries and evidence limits.
- `docs/platform/decisions.md` — open owner decisions and external runtime direction.
- `docs/platform/agent-collaboration.md` — local Relay, collaboration, presence and file-claim boundaries.
- `docs/platform/automation.md` — autonomous execution, memory and policy requirements.
- `docs/commands/tasks.md` — Plaesy task lifecycle, dependencies, validation and local storage behavior.
- `spec/files-realtime-collaboration/spec.md` — existing narrower file collaboration spec; remains separate and is not superseded by this program spec.
