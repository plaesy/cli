---
spec: plaesy-agent-platform
topic: feature-deliver-selected-work-management-and-governance-workflow
title: Deliver selected work management and governance workflow
phase: implement
status: backlog
---

# Deliver selected work management and governance workflow

## Goal

Deliver owner-selected planning/governance work management capabilities.

## Scope

- Only audited, selected hierarchy, assignment, approval, budgets, work tracking and reporting capabilities.

## Non-goals

- No assumption that Paperclip and OpenProject are identical or that every edition feature is in scope.

## Deliverables

- Rust domain/service, persistence, migration mapping and acceptance tests for selected scenarios.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Unknown assignee produces an explicit skipped/blocked outcome with audit record; no silent fallback worker.
- [ ] Claim/run lifecycle has exactly-one terminal outcome across success, failure, cancellation and interruption; retries cannot duplicate admission or handoff.
- [ ] Worker descendant mutation limits and their non-confinement boundary are documented and tested; do not claim OS isolation.
- [ ] Both same-card review and downstream review-card graph shapes have explicit acceptance; structured handoff and per-task logs are retained and attributable.
- [ ] Platform-specific spawn behavior and OS confinement remain UNKNOWN unless separately approved and evidenced.

- [ ] Selected user roles, objects and workflows are owner-approved and source-evidenced.

- [ ] Permissions, hierarchy, lifecycle transitions and audit/reporting outcomes have acceptance tests.
- [ ] Edition/license limits, import/export and failure behavior are documented.

- [ ] Data migrations use versioned mappings, idempotency and rollback fixtures.
- [ ] Rust gates and user-observable acceptance pass.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.
