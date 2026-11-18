---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Owner decisions for first replacement slice
phase: 0
due: none
status: backlog
---

# Owner decisions for first replacement slice

## Goal

Record the product owner's first replacement workflow, target cohort, deployment profile, runtime and UI boundaries, exclusions, measurable NFRs, migration/cutover appetite, and identity/ownership direction so dependent designs do not guess.

## Scope

- Resolve or explicitly leave UNKNOWN with an accountable owner and review trigger: first workflow/order; target cohort; replacement-vs-integration boundary per product; local/shared deployment and supported platform; runtime strategy; UI/Rust boundary; identity/tenancy; exclusions; measurable performance/reliability/security/accessibility/retention NFRs; migration/cutover appetite and rollback authority; support model; DeepSeek Harness and Orca target identity/version.

- Capture authorized decisions in `docs/platform/decisions.md`; retain unresolved items as explicit UNKNOWNs with owner and review trigger. Recommendations are not approvals.
- Identify which existing incumbent workflow and pinned version will be used as the reproducible initial baseline, or leave that choice explicitly UNKNOWN.

## Non-goals

- Do not choose for the owner or silently treat a recommendation as approved.

- Do not implement runtime, shared Relay, tenant, or deployment architecture.

## Deliverables

- Decision records/updates in `docs/platform/decisions.md`.

- Spec open questions updated with decision evidence or named owner/blocker.

## Dependencies

- None; product owner input required.

## Acceptance criteria

- [ ] First workflow/order, target cohort, success outcome, and incumbent baseline/version are recorded or explicitly UNKNOWN with accountable owner and review trigger.

- [ ] Every listed product has an explicit replacement/integration/exclusion disposition, or remains UNKNOWN; no implied approval.
- [ ] Deployment/platform, runtime, UI/Rust boundary, identity/tenancy, and support profile are recorded or explicitly UNKNOWN with owner.

- [ ] Measurable NFRs, migration/cutover appetite, rollback authority, and exclusions are recorded or explicitly UNKNOWN with owner; do not invent thresholds.
- [ ] Exact DeepSeek Harness and Orca identity/version are resolved from evidence or retained as UNKNOWN.

- [ ] Every authorized decision links to applicable spec requirements and preserves prior decision history; recommendations are clearly distinguished from decisions.
