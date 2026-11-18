---
spec: plaesy-agent-platform
topic: feature-deliver-selected-project-portfolio-management-workflow
title: Deliver selected project portfolio management workflow
phase: implement
status: backlog
---

# Deliver selected project portfolio management workflow

## Goal

Deliver selected portfolio/project-management workflows from the audited OpenProject capability set.

## Scope

- Owner-selected subset of project/task lifecycle, agile, scheduling, time, reporting and integrations.

## Non-goals

- No assumption that every module/edition/license is selected or supported.

## Deliverables

- Rust domain/service, source-specific import/export, workflows and acceptance fixtures.

## Dependencies

- `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents` and `critical_feature-specify-first-replacement-vertical-slice`; owner approval is required before committing scope.

- Relevant owner decision, Rust gates, security/contracts and migration dependencies must be satisfied before implementation.

## Acceptance criteria

- [ ] Exact workflow, edition/release, modules and license boundaries are recorded.

- [ ] Lifecycle, permissions, scheduling/time/reporting calculations have independent test cases.
- [ ] Import/export and reconciliation preserve approved fields and report conflicts/loss.

- [ ] Scale and performance criteria come from approved workload/NFR task.
- [ ] Rust and focused user-visible acceptance checks pass.

- [ ] Update measured capability and acceptance crosswalk; do not mark complete based only on design/task existence.
