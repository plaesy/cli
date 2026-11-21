# Orca capability ledger — bounded discovery

**State:** Partial, provisional hypotheses only; accepted leaf ledger is empty because archive-to-source provenance could not be reconciled. This is not a parity assessment, approved target baseline, or owner-approved scope. No upstream source code was modified; no build, test, or runtime verification was performed.

## Identity, revision, and boundaries

- The pinned discovery revision recorded in the Plaesy source inventory is `65c84d41101e2e6255f64155985de873fc81c902` (non-owner-approved). It is not present in this checkout's Git object database (`git rev-parse <sha>^{commit}` failed), and the Orca archive could not be located inside this task worktree. The neighboring `/home/ubuntu/cli` checkout contains an untracked archive, but its own HEAD is `68a3bd51c941b223e04aa09a52c592cf41539a3e`; this does not establish archive-to-Orca-SHA provenance. Product identity, selected product/components, and commit-specific behavior therefore remain **UNKNOWN** for accepted evidence.
- License/edition boundaries: **UNKNOWN**. Existing inventory refers to a README license badge, but no verified README at the pinned revision or license text was inspected in this task worktree.
- Exact inspected upstream source scope: none; no Orca repository, source module, or test file was verified. Plaesy context inspected: source inventory Orca row, this task's existing ledger, and `replacement-backlog.md` RP rows/table. Build, tests, and runtime were not run.
- Uninspected: all pinned Orca docs/source/tests, release/edition terms and runtime; all Plaesy baseline code/test mappings for purported Orca leaves. No claim of exhaustive coverage.

## Accepted leaf capabilities

None. Every prior README-derived candidate below is retained solely as an **unverified hypothesis**, not an accepted/documented leaf, because the cited archive content cannot be bound to the pinned Orca commit. Do not use those lines or linked web docs as evidence of behavior at the pinned SHA. For each hypothesis, preconditions, success semantics, errors, failure and recovery remain **UNKNOWN**; no Plaesy baseline claim or implementation gap is established.

| Hypothesis ID (not accepted leaf) | Unverified README claim (not evidence) | Disposition |
|---|---|---|
| `F-ORCA-CHAT-001` | Chat UI/raw CLI mode switching with conversation context retention. | UNKNOWN; verify pinned docs/source and map only after provenance and baseline review. |
| `F-ORCA-ORCH-001` | Coordinator splits work among workers in separate workspaces and returns results. | UNKNOWN; no unique gap/task. |
| `F-ORCA-REMOTE-001` | Remote server, SSH host, or cloud VM execution; agents continue while laptop sleeps. | UNKNOWN; RP-027 conditional only, owner scope unresolved. |
| `F-ORCA-MOBILE-001` | Mobile notifications, chat reply, and away-from-desk continuation. | UNKNOWN; target/platform and owner scope unresolved. |
| `F-ORCA-REVIEW-001` | Diff review, line annotations, feedback to agent. | UNKNOWN; no baseline or gap established. |
| `F-ORCA-GIT-001` | Draft commits/PRs and route failing checks/review comments to agent. | UNKNOWN; no baseline or gap established. |
| `F-ORCA-WORKTREE-001` | Parallel isolated worktree creation. | UNKNOWN; do not infer parity with existing workspace task. |
| `F-ORCA-DESIGN-001` | Browser element inspection and copying HTML/styles/selector to chat. | UNKNOWN; owner workflow scope unresolved. |
| `F-ORCA-ISSUES-001` | View GitHub/Linear tasks and start a workspace from an issue. | UNKNOWN; integration boundary unresolved. |
| `F-ORCA-CLI-001` | CLI worktree, worker, browser/desktop automation and scheduling. | UNKNOWN; no API/behavior evidence. |
| `F-ORCA-PLUGIN-001` | Install themes, editors, languages and panels. | UNKNOWN; security/extension and edition boundaries unresolved. |

These IDs are retained for trace continuity only; they must not be treated as accepted capabilities or as deduplicated task recommendations.

## RP crosswalk disposition

The canonical supporting crosswalk is `replacement-backlog.md` lines 15–60 (RP-001–046) and 68–102 (task/status mapping). RP-001 numbering is the replacement-backlog taxonomy, not the distinct RP-01–16 numbering mentioned in that file. “Status” below means task mapping/dependency and relevance disposition from that crosswalk, not completion status; live task state remains authoritative in `.plaesy/tasks/`.

| RP row(s) | Disposition for Orca | Existing task/dependency mapping and rationale |
|---|---|---|
| RP-001 | Relevant as owner decision prerequisite; unresolved | `high_chore-owner-decisions-for-first-replacement-slice`; product boundary/workflow scope not approved. |
| RP-002 | Relevant shared research recovery; Orca evidence not verified | `high_chore-recover-and-compare-incumbent-research`; must recover versioned provenance rather than assume archive identity. |
| RP-003 | Directly relevant, open | `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents`, `high_chore-audit-agent-runtime-and-workspace-products`, plus Orca subtask; identity/license/SHA remain unknown here. |
| RP-004 | Shared inventory outcome, open | `high_chore-produce-canonical-cross-incumbent-capability-inventory` plus the audit tasks; no accepted leaves from this pass. |
| RP-005 | Shared baseline audit, open | `high_chore-verify-feature-baseline-across-repositories`; no Orca-specific baseline result established. |
| RP-006 | Shared architecture/owner gate; relevance conditional on selection | `high_chore-define-rust-domain-architecture-and-approved-deployment-boundaries`; no design/deployment choice implied. |
| RP-007 | Relevant security prerequisite if selected; specifics UNKNOWN | `high_chore-specify-identity-tenancy-threat-model-and-authorization-contracts`; no Orca auth behavior asserted. |
| RP-008 | Shared contract prerequisite if selected | `high_chore-specify-versioned-project-task-run-artifact-and-event-contracts`; no workflow contract inferred. |
| RP-009 | Relevant runtime adapter contract if selected | `high_feature-define-runtime-adapter-and-fake-conformance-suite` and `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting`; no runtime gap established. |
| RP-010 | Generic portability/migration row; no Orca-specific mapping justified | `high_chore-specify-data-portability-and-source-migration-contracts`; no data portability claim/evidence. |
| RP-011 | Security implementation conditional on selected workflow | `critical_feature-implement-authenticated-agent-identity-and-policy-boundary` and `high_feature-implement-approval-secret-reference-and-audit-controls`; no product-specific requirement established. |
| RP-012 | Lifecycle/recovery shared row; relevance conditional | `critical_feature-implement-durable-task-run-lifecycle-and-recovery` and `high_feature-implement-durable-run-append-retry-torn-tail-recovery-and-shutdown-drain`; Orca persistence semantics unknown. |
| RP-013 | Runtime execution shared row; conditional | Adapter/fake and supervised runtime tasks above; no Orca failure evidence. |
| RP-014 | Worktree isolation row is not mapped to Orca by the RP-014 source row (it lists other products); no Orca-specific disposition | Existing task `high_feature-enforce-isolated-concurrent-agent-changes` is for listed products; Orca hypothesis alone does not justify extending scope. |
| RP-015 | Relevant shared messaging/collaboration only if selected | `high_feature-add-authenticated-shared-messaging-and-session-delivery`; remote/mobile/relay hypotheses unverified. |
| RP-016 | Generic operator controls; relevance conditional | `high_feature-add-operator-observability-and-recovery-controls`; no Orca-specific operator requirement established. |
| RP-017 | Not mapped to Orca in source crosswalk (Hermes, Paperclip, Herald OS); no Orca disposition beyond out-of-scope for this evidence pass | `high_feature-add-provenance-aware-knowledge-and-memory-controls`; no memory evidence. |
| RP-018 | Not mapped to Orca in source crosswalk; no Orca-specific disposition | `high_feature-deliver-selected-agent-execution-and-learning-workflow` is scoped to Hermes/DeepSeek; agent-orchestration hypothesis is insufficient to extend. |
| RP-019 | Not mapped to Orca in source crosswalk; no Orca-specific disposition | `high_feature-deliver-selected-work-management-and-governance-workflow` scoped to Paperclip/OpenProject. |
| RP-020 | Not mapped to Orca in source crosswalk; no Orca-specific disposition | `high_feature-deliver-selected-visual-workspace-and-parallel-coding-workflow` scoped to Nimbalyst. |
| RP-021–026 | Not mapped to Orca in source crosswalk; unrelated named product families | Existing selected workflow tasks are for Craft Anything, design/presentation/document authoring, Herald OS and OpenProject respectively; no Orca evidence maps here. |
| RP-027 | Directly relevant but conditional; open | `high_feature-deliver-selected-remote-runtime-or-relay-workflow`; crosswalk says conditional Orca/Nimbalyst workflow, authenticated transport/liveness proof required, refine after audit and owner decision. This task's evidence does not establish those behaviors or a new gap. |
| RP-028–034 | Generic quality, NFR, security/release, migration and parity/cutover gates; only applicable if a workflow is selected | Existing mappings in `replacement-backlog.md` rows 97–102; no Orca-specific baseline or approval, and no parity/cutover claim. |
| RP-035–039, RP-041–046 | Other incumbent audit rows; not Orca-specific | Existing umbrella/product-family audit mappings; no Orca leaf or task duplication. |
| RP-040 | Direct Orca audit row; this subtask contributes only the provenance/crosswalk result and remains incomplete against the broader row objective | `high_chore-audit-orca-capability-ledger` plus umbrella/family audit mapped under RP-003/004/005; pinned source identity, supported runtime, remote/mobile/relay/auth/delivery/liveness/recovery remain to verify. |

Crosswalk row grouping follows the exact per-row mappings in `replacement-backlog.md`; rows marked shared/conditional are not status-complete or evidence of selected scope. No orphan is inferred from this audit: rows without an Orca mapping are explicitly marked not mapped, and generic/shared rows carry their existing task and gate rationale. No unique implementation follow-up is recommended because no provenance-verified capability gap was established. No duplicate product task is proposed.

## Remaining evidence and decisions

1. Recover an Orca checkout/archive whose contents can be verified against pinned SHA `65c84d41101e2e6255f64155985de873fc81c902`; then cite immutable paths/lines or permalinks.
2. Verify product identity, exact selected component, license text and edition/distribution boundaries from that snapshot.
3. Inspect bounded docs, modules and tests for the owner-selected workflow; record assertion-read separately from executed/runtime evidence, including denied/error/failure/recovery cases.
4. Reconcile accepted leaves against Plaesy code/tests and owner decisions before recommending any implementation tasks.
5. No builds, tests or runtime checks were run. The externally located untracked archive was not copied or modified; it is not accepted as source evidence.
