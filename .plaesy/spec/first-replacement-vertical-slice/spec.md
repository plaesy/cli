# Spec: first replacement vertical slice (agent execution)

Status: draft for owner review. Not approved. Implementation claims stay blocked until the owner approves this draft. The integrator then assigns owned paths (section 9).
Parent: `.plaesy/spec/plaesy-agent-platform/spec.md` (requirements R2–R7, R9).
Task: `critical_feature-specify-first-replacement-vertical-slice`.

## 1. Decisions applied

Recorded by the product owner on 2026-11-19 (`docs/platform/decisions.md`):

- Workflow: agent execution and learning.
- Deployment profile: shared self-hosted. Local-first and offline rules from `docs/platform/product.md` still apply. Remote trust boundaries are defined in this spec.
- Runtime: deterministic fake adapter first, then a bounded native runtime. External harnesses (Claude Code, Codex) are not enabled in this slice.
- Identity: single organization. Agent principals are scoped per project, with grants and revocation.

## 2. Actors

- **Operator:** a human who owns the project. Submits tasks and cancels runs. Read-only inspection of run state through the CLI.
- **Agent principal:** an authenticated identity bound to one project, holding explicit grants (project, task, tool).
- **Controller:** the Plaesy service that persists tasks and runs, enforces policy, and drives the adapter.
- **Adapter:** the replaceable execution backend. The first adapters are the fake adapter and the bounded native runtime.
- **Recipient agent:** another principal in the same project that receives messages in the shared profile.

## 3. Scope and non-goals

In scope: one agent-execution task submitted by a principal, executed through an adapter, with its state persisted and recovered. Messages between principals in the same project, with truthful delivery state. Policy checks, budgets, cancellation, audit, and secret redaction for the run.

Out of scope for this slice:

- External harness backends and any real external side effect. Tests use synthetic fixtures only.
- Multi-tenant hierarchy, cross-organization sharing, and cross-host delivery beyond the single shared service.
- Workspace isolation for concurrent writers (R8, lane G). This slice has a single writer per run. A later slice must add it before parallel writers exist.
- Operator UI and full R10 controls. Only read-only state inspection is included.
- Production data, migration, and cutover (R11, R13).

## 4. Task and run state machine

A task has one or more runs. Each run has a stable `run_id`, an attempt number, and an append-only event log. The state changes only when an event is persisted.

Run states: `queued` → `running` → `succeeded` | `failed` | `timed_out` | `cancelled`. On restart, a run found in `running` becomes `interrupted`. An interrupted run is re-queued as a new attempt only while attempts remain under the retry policy.

- Retry policy: maximum 3 attempts per task. **Proposed default, owner to confirm.** It is a design parameter, not a product threshold.
- Each side-effecting step carries an idempotency key and is recorded in a side-effect ledger before and after the call. A replacement attempt reads the ledger first and does not repeat a completed step.
- Practice basis: queues deliver at least once, so duplicates must be expected. Effective exactly-once comes from at-least-once delivery plus idempotent effects, not from the transport. Code after the last durable boundary is treated as re-runnable. Sources: [hackernoon: idempotency before checkpoints](https://hackernoon.com/durable-agent-workflows-need-idempotency-before-they-need-checkpoints), [gravitee: workflow engine patterns for AI agents](https://gravitee.io/corpus/gen-1286/workflow-engine/workflow-engine-patterns-for-reliable-long-running-ai-agent-processes-retries-compensation-and-timeouts.html), [Conductor: durable execution](https://docs.conductor-oss.org/architecture/durable-execution). These are practitioner sources, not standards.

## 5. Delivery semantics (shared profile)

Message states: `queued`, `accepted`, `acknowledged`, `failed`, `unknown`. Recipient states: `active`, `busy`, `offline`, `expired`, `terminated`.

- The service never reports a message as read, and never reports a session as woken, without receiver evidence.
- A message carries an idempotency key. Duplicate delivery produces one effect; the second delivery returns the stored result.
- A lost acknowledgement leaves the state `unknown` until the sender gets evidence. Retries reuse the same key.

## 6. Permissions, budgets, and audit

- Every action (submit, start, tool request, message send, cancel) checks the principal's grant at the time of the action. A revoked grant takes effect on the next action.
- A denied action has no side effect. The denial is an audit event with the correlation IDs.
- Budgets: a step budget and a time budget per run. Exceeding either ends the run as `failed` with reason `budget_exceeded`. **The numeric values are owner-defined and UNKNOWN until set.**
- The external-effect flag is off in this slice. An adapter attempt at an external call is blocked and recorded.
- Secrets are referenced by name. Raw values are never written to events, logs, or exports.

## 7. Requirements and acceptance scenarios

Each scenario is Given / When / Then. The test prefix is the focused test filter (`cargo test --locked <prefix>`).

| ID | Req | Scenario | Test prefix |
| --- | --- | --- | --- |
| VS-01 | R3 | Given principal P with grant for project X and tool T, when P submits a task, then a task and run `attempt 1` are persisted in `queued`, with an event and stable IDs. | `task_run` |
| VS-02 | R3, R4 | Given a queued run, when the fake adapter completes, then the run is `succeeded` only after an evidence artifact is stored and hash-linked. | `task_run`, `adapter_conformance` |
| VS-03 | R4 | Given the fake adapter fails, times out, or is cancelled, then the run ends in `failed`, `timed_out`, or `cancelled` with a reason, and no state is written without its event. | `adapter_conformance` |
| VS-04 | R5 | Given P lacks a grant for T, when the adapter requests T, then the request is denied before any side effect and the denial is audited. | `identity_policy` |
| VS-05 | R5 | Given a grant is revoked during a run, when the next action is attempted, then it is denied and the run ends `failed` with reason `grant_revoked`. | `identity_policy` |
| VS-06 | R9 | Given a step budget of N, when the run needs step N+1, then the run ends `failed` with reason `budget_exceeded` and no further adapter step runs. | `bounded_runtime` |
| VS-07 | R9 | Given an operator cancels a running run, then the adapter cancel is invoked and the run ends `cancelled` within the time budget. | `bounded_runtime` |
| VS-08 | R9 | Given the external-effect flag is off, when the adapter attempts an external call, then the call is blocked and recorded. | `bounded_runtime` |
| VS-09 | R3 | Given the controller is killed while a run is `running`, when it restarts, then the run becomes `interrupted`, is re-queued as a new attempt within the retry policy, and a completed step is not repeated. | `task_run`, `runstore` |
| VS-10 | R3 | Given the run log ends in a partial record, when recovery runs, then valid records are kept, the corruption is reported, and no success is reported for the torn record. | `runstore` |
| VS-11 | R6 | Given a message with idempotency key K is delivered twice, then one effect occurs and the second delivery returns the stored result. | `shared_messaging` |
| VS-12 | R6 | Given the acknowledgement of a message is lost, when the sender retries with the same key, then the state is `unknown` until evidence arrives, and no second effect occurs. | `shared_messaging` |
| VS-13 | R7 | Given a recipient is offline, expired, or terminated, when a message is sent, then the delivery state is reported truthfully and no wake or read is claimed. | `shared_messaging` |
| VS-14 | R5, R6 | Given a principal of project B, when it polls for messages, then no message from project A is visible. | `identity_policy`, `shared_messaging` |
| VS-15 | R9 | Given a synthetic secret in task input, when logs, audit events, and exports are produced, then the secret value is absent from all of them. | `approval_secret` |
| VS-16 | R3 | Given any transition or denial, then an audit event with `task_id`, `run_id`, and principal is persisted. | `task_run`, `approval_secret` |

Coverage: R2 is covered by section 1. R3 by VS-01, 02, 09, 10, 16. R4 by VS-02, 03. R5 by VS-04, 05, 14. R6 by VS-11, 12, 14. R7 by VS-13. R9 by VS-06, 07, 08, 15.

## 8. Failure injection

The fake adapter and fixtures must inject: success, denial, timeout, cancellation, crash mid-run, duplicate delivery, lost acknowledgement, restart recovery, torn log tail, and grant revocation mid-run. Each injection maps to a scenario above. Fixtures use synthetic data only.

## 9. Ownership proposal (applied only after approval)

The paths marked (new) do not exist yet. The integrator assigns them when this draft is approved. The matching tasks then drop their `OWNERSHIP-UNASSIGNED` marker.

| Task | Lane | Proposed owned paths |
| --- | --- | --- |
| `critical_feature-implement-durable-task-run-lifecycle-and-recovery` | D | src/task_run/ (new) |
| `high_feature-implement-durable-run-append-retry-torn-tail-recovery-and-shutdown-drain` | D | src/runstore/ (new) |
| `high_feature-define-runtime-adapter-and-fake-conformance-suite` | A | src/adapter/ (new), tests/adapter_conformance.rs (new) |
| `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting` | A | src/runtime/ (new) |
| `critical_feature-implement-authenticated-agent-identity-and-policy-boundary` | B | src/identity/ (new), src/authz/ (new) |
| `high_feature-implement-approval-secret-reference-and-audit-controls` | B | src/policy/ (new), src/secrets/ (new) |
| `high_feature-add-authenticated-shared-messaging-and-session-delivery` | C | `src/relay.rs`, `src/relay_test.rs`, src/relay/ (new), `tests/relay_cli.rs` |

Integrator-only files: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `.github/workflows/ci.yml`, `rust-toolchain.toml`.

## 10. Open questions (UNKNOWN, owner and review trigger)

- Target cohort and success outcome. Owner: product owner. Review before the first implementation claim.
- Incumbent baseline for agent execution: which product and pinned version. Owner: product owner, with the research owner. Review after `high_chore-recover-and-compare-incumbent-research`.
- NFR thresholds: latency, throughput, recovery time, retention. Owner: product/operations owner. Not set here; no thresholds are invented.
- Budget values in section 6 and the retry default in section 4. Owner: product owner. Proposed defaults only.
- Shared profile hosting topology and backup/restore support model. Owner: product/operations owner.
- Cutover appetite and rollback authority. Owner: product owner. Relevant to R13, not to this slice.

## 11. Exit criteria for this draft

- Owner approves this draft, or requests changes.
- Every VS scenario has a focused test name prefix that exists in code before the scenario is marked passed.
- No implementation starts until section 9 is applied.
