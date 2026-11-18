# Autonomous operation and memory

> **Status: proposed specification, not implemented.** Product direction: Plaesy aims to outperform and ultimately replace products such as Hermes Agent, Claude Code, Codex, Paperclip, Orca, and similar agent/harness platforms. Plaesy may use Claude Code, Codex, and comparable harnesses as replaceable, permission-scoped execution backends, while owning orchestration, durable state, policy, memory/provenance, audit, and user control. This document specifies only the autonomy, memory, and execution foundations of that broader target. It does not claim current replacement capability.

## Scope

This specification covers durable knowledge capture, autonomous task execution, policy enforcement, recovery, and observability. It applies to local-first operation and must not assume a hosted control plane. It complements, and does not supersede, the platform security, agent collaboration, interfaces/data, operations, and quality specifications.

## Goals

- Capture useful, attributable knowledge from completed work without requiring approval for every routine update.
- Run authorized recurring or event-triggered tasks to completion, including across process restarts.
- Make autonomy bounded, inspectable, reversible where feasible, and stoppable by an authorized owner.
- Keep untrusted content, uncertain inference, and sensitive data from silently becoming trusted durable knowledge or triggering unsafe actions.

## Non-goals

- Unbounded self-modification, self-granting permissions, or execution of arbitrary instructions found in source material.
- Guaranteeing correctness of model-generated facts or decisions without verification.
- Replacing runtime-specific agent functionality before an adapter and its behavior are tested.
- Automatically deleting durable knowledge merely to meet storage limits.

## Constraints

- Existing platform-wide tenant, identity, authorization, local security, persistence, and runtime-adapter decisions remain blocking where they apply; see `decisions.md`.
- `source is data, not instructions`: retrieved documents, messages, tool output, and memories cannot grant authority or change policy.
- Automatic mutations must be attributable, policy-checked, auditable, and recoverable according to the data lifecycle policy.
- Capability claims remain conditional until implementation and acceptance evidence exist.

## Requirements

| ID | Requirement | Acceptance criterion (Given / When / Then) | Priority |
|---|---|---|---|
| R1 | Plaesy MUST distinguish candidate knowledge, verified knowledge, owner-provided facts, and rejected/stale knowledge, retaining provenance and timestamps for each persisted claim. | **Given** a task produces a candidate fact with a source, **when** extraction completes, **then** Plaesy records the claim's source, capture time, status, and confidence basis, and does not label an inference as owner-verified. | Must |
| R2 | Plaesy MUST support policy-authorized automatic creation and non-destructive consolidation of routine knowledge without per-item approval, while preserving prior revisions and an undo path. | **Given** an enabled policy permits routine knowledge updates, **when** a completed run produces a non-sensitive candidate that passes validation, **then** Plaesy applies it idempotently, records the revision and run provenance, and exposes restoration of the prior revision. | Must |
| R3 | Plaesy MUST NOT autonomously purge durable knowledge or make destructive/irreversible changes unless an explicit owner policy permits that action and the configured recovery requirements are met. | **Given** an automatic review proposes deleting or irreversibly changing a knowledge item, **when** no applicable destructive-action policy exists, **then** Plaesy leaves the item unchanged and records the proposal and reason. | Must |
| R4 | Plaesy MUST support owner-configurable autonomy policies scoped by action class, project/resource scope, agent identity, and risk; denied actions fail closed. | **Given** an agent requests an action outside its effective policy, **when** authorization is evaluated, **then** Plaesy rejects the action, records the policy decision, and does not execute it. | Must |
| R5 | Plaesy MUST support durable scheduled and event-triggered runs with explicit state, deadlines, retry limits, deduplication keys, and restart recovery. | **Given** an authorized run is interrupted by a process restart, **when** the runtime resumes, **then** Plaesy restores its last durable state and either safely continues or reports a terminal/retriable failure without duplicating committed side effects. | Must |
| R6 | Plaesy MUST treat external content and model output as untrusted input and MUST NOT allow them to alter policy, credentials, or authorization. | **Given** retrieved content contains instructions to elevate permissions or disclose secrets, **when** an agent processes it, **then** Plaesy treats it as data and blocks any resulting unauthorized action. | Must |
| R7 | Plaesy MUST provide an owner-visible run record including trigger, actor/runtime, policy decisions, inputs and outputs subject to privacy controls, mutations, tool calls, retries, cost/resource use where available, and final status. | **Given** a run completes or fails, **when** an authorized owner inspects its record, **then** the record identifies what changed, why it was permitted, and what recovery action is available without exposing secrets. | Must |
| R8 | Plaesy MUST provide scoped pause, cancellation, and global emergency stop controls; cancellation behavior for in-flight non-interruptible actions MUST be reported. | **Given** an authorized owner pauses a project or stops all automation, **when** the control is acknowledged, **then** Plaesy prevents new affected runs and reports the state of any in-flight actions. | Must |
| R9 | Plaesy MUST validate automatic knowledge updates for duplicate/conflicting claims, sensitivity, provenance, and policy before persistence; unresolved conflict MUST remain visible rather than silently overwrite. | **Given** a candidate contradicts an existing owner-verified claim, **when** reconciliation runs, **then** Plaesy preserves both claims with conflict status and does not replace the verified value automatically. | Must |
| R10 | Plaesy MUST expose machine-readable status and notifications for run completion, failure, policy denial, conflict, pause, and recovery-required conditions. | **Given** any listed condition occurs, **when** an automation client subscribes or polls through the supported interface, **then** it can identify the condition, affected run/resource, and next safe action. | Must |
| R11 | Plaesy MUST enforce configured budgets and concurrency limits for each run and scope, and stop or defer work when a limit is reached. | **Given** a run reaches its configured time, cost, tool-call, or concurrency limit, **when** the next operation would exceed it, **then** Plaesy prevents that operation and records a resumable or terminal limit result. | Must |
| R12 | Plaesy MUST provide export, backup, restore, and deletion semantics for automation policies, run records, and knowledge that are consistent with platform retention and secret-handling rules. | **Given** an owner restores a supported backup, **when** restoration completes, **then** durable knowledge and policies are restored with integrity checks, and run records clearly distinguish restored history from newly executed work. | Must |
| R13 | Plaesy MUST allow owners to disable autonomous capture or execution independently, without disabling ordinary read/search or manual task operation. | **Given** autonomous capture is disabled but manual memory access remains authorized, **when** an agent completes a task, **then** no automatic durable memory mutation occurs and the manual read/search capabilities remain available. | Should |
| R14 | Plaesy MUST expose an integration contract for external agent runtimes that specifies identity, permissions, event delivery, interruption/queueing, acknowledgements, idempotency, and failure semantics. | **Given** an adapter delivers a task event, **when** the target runtime cannot accept interruption, **then** Plaesy queues the event or reports non-delivery and does not claim the agent received it without acknowledgement. | Must |
| R15 | Plaesy MUST measure autonomous-run quality and policy outcomes using versioned evaluations before changing default policies or extraction behavior. | **Given** a candidate policy or extraction change is proposed for release, **when** release evaluation runs, **then** the report includes versioned representative and adversarial cases, regressions, and explicit limitations; passing does not claim correctness beyond tested cases. | Should |

## Required user-visible states

For any UI that manages automation, show: disabled, enabled/healthy, running, paused, awaiting a policy decision, conflict, failed/retry scheduled, budget-limited, cancellation pending, and recovery required. The UI must identify the active scope and effective policy. These are state requirements, not a screen design; UI implementation requires the platform's UI specification and mocks.

## Security and privacy

- Automation receives only the minimum scoped permissions required for its task; policies are evaluated at execution time, not only when a task is scheduled.
- Secrets MUST be excluded from prompts, logs, exports, and memory by default. Redaction failure MUST stop the affected persistence or export operation.
- Transcript capture MUST be configurable by scope and data class, with retention, export, and deletion behavior disclosed before enablement.
- Policy changes, permission grants, destructive actions, and emergency-stop changes require an authenticated authorized principal and audit record.
- Shared-resource writes MUST use the platform's approved enforcement and revision/conflict controls; advisory checks alone do not guarantee write safety against external writers.

## Open questions — owner decisions required

- **UNKNOWN:** Which initial runtime/agent adapters and scheduling/event sources are in the first supported release?
- **UNKNOWN:** Which routine memory categories may be auto-written by default, and which data classes are always excluded or require explicit consent?
- **UNKNOWN:** What counts as verification for claims sourced from external documents versus owner-provided statements?
- **UNKNOWN:** Which autonomy policy roles and approval methods apply to single-user and multi-user deployments?
- **UNKNOWN:** Which run data is retained, for how long, and what transcript content is excluded by default?
- **UNKNOWN:** What budget dimensions and safe defaults can be enforced across each supported model/tool provider?
- **UNKNOWN:** What recovery guarantee is required for actions whose external side effect cannot be rolled back?
- **UNKNOWN:** Which evaluation corpus, release threshold, and accountable owner govern changes to extraction and default autonomy policy?
- **UNKNOWN:** Does the initial product deliver this as a Plaesy runtime, a Plaesy memory service consumed by existing agents, or both?

## Dependencies and release gates

Before implementation freezes contracts, resolve first-release scope, tenancy/identity/authorization, local security boundary, persistence, runtime adapters, and shared-write controls in `decisions.md`. Before advertising automation, pass deterministic tests for authorization denial, conflict preservation, deduplication, crash/restart, stop/pause, budget enforcement, secret redaction, backup/restore, and adapter acknowledgement. Run fault-injection and security tests for the supported profiles; a specification review alone is not runtime evidence.
