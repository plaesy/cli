---
spec: plaesy-agent-platform
topic: chore-audit-deepseek-harness-capability-ledger
title: Audit DeepSeek Harness capability ledger
phase: research
status: done
---

# Audit DeepSeek Harness capability ledger

## Goal

Produce a cited, bounded, leaf-capability ledger for DeepSeek Harness that can be reconciled into the program backlog without implying parity or owner-approved scope.

## Scope

- Pin source revision and establish product identity, license/edition boundaries, documentation scope, and inspected/uninspected areas.
- Decompose documented workflows into observable leaf capabilities; inspect corresponding source modules and test assertions in bounded slices.
- Distinguish documented, source-reviewed, test-read, test-executed, runtime-verified, inferred, and UNKNOWN evidence. No compile/test execution is required for this discovery task unless separately approved.
- For each leaf, record stable capability ID, immutable source path/line or permalink, preconditions, success/error/failure/recovery semantics, Plaesy requirement/scenario mapping, current baseline evidence, and disposition.
- Recommend a deduplicated follow-up task only when source evidence establishes a unique capability gap; otherwise record shared task mapping, owner decision dependency, explicit out-of-scope, or UNKNOWN.
- Preserve existing incumbent checkout and avoid builds; do not delete or rewrite canonical inventory wholesale.

## Non-goals

- Do not modify upstream source or run broad/expensive builds.
- Do not claim exhaustive product parity, absence, implementation success, or cutover approval.
- Do not select owner scope, deployment, UI, NFR thresholds, or replacement workflow.

## Deliverables

- Per-product evidence ledger with exact pinned revision and coverage boundaries.
- Source inventory/framework/gap-register updates using stable IDs and explicit evidence states.
- RP crosswalk disposition for relevant rows and deduplicated, acceptance-ready task recommendations.
- Short coverage report listing inspected and uninspected areas, unknowns, and decisions required.

## Dependencies

- Parent family task: `high_chore-audit-agent-runtime-and-workspace-products`.
- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `high_chore-verify-feature-baseline-across-repositories`.
- Use pinned discovery snapshots unless owner-approved baselines are supplied; label the distinction.

## Acceptance criteria

- [ ] Revision, identity, license/edition boundaries, and exact inspected/uninspected source scope are evidenced or explicitly UNKNOWN.
- [ ] Every accepted leaf has stable ID, source citation, behavior/evidence state, Plaesy mapping, and next disposition; unsupported behavior stays UNKNOWN.
- [ ] Candidate scenarios include relevant denied/error/failure/recovery behavior where evidenced; read assertions are not described as executed.
- [ ] Relevant RP rows have task/status/dependency/rationale, with no unexplained orphan and no duplicate product task.
- [ ] Any new implementation follow-up has a bounded deliverable, dependencies, observable acceptance, and owner-decision gate.
- [ ] Inventory updates preserve prior content; Markdown/link/task references validate; no upstream modifications or unapproved builds occur.
