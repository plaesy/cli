---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Audit work management and OS automation products
phase: research
status: backlog
---

# Audit work management and OS automation products

## Goal

Produce bounded, source-cited capability ledgers for Paperclip, Herald OS, and OpenProject, separating observed implementation from public descriptions and edition-specific claims.

## Scope

- Use owner-approved release/commit baselines where available; otherwise label pinned snapshots as discovery evidence only.

- Inspect user/operator workflows, docs, API/source/test boundaries for organization/goals/tasks/approvals/budgets/integrations, OS automation and permissions, and project/portfolio/work-package/agile/scheduling/time/reporting/permissions/migration/operations.
- Map license and edition boundaries; flag material reuse obligations for legal review without copying upstream code.

- For each leaf record stable capability ID, immutable citation, evidence state, inspected boundary, Plaesy requirement/baseline, candidate observable acceptance, migration impact, and deduplicated task/disposition.
- Execute only focused tests in isolated disposable environments when prerequisites exist; report exact command/result and distinguish blocked/inconclusive from failed product behavior.

## Non-goals

- Do not decide owner scope, legal conclusions, or replacement boundaries.

- Do not implement features or edit upstream code.
- Do not generalize one endpoint's authorization semantics or one test's behavior across a product.

## Deliverables

- Three capability ledgers and updated source inventory/framework/gap register.

- Coverage/unknowns report, including license/edition issues and evidence limitations.
- Task recommendations deduplicated across common workflows.

## Dependencies

- Research recovery and umbrella incumbent audit; approved baseline needed for parity assertions.

## Acceptance criteria

- [ ] Each product has pinned revision, license/edition findings, inspected/uninspected boundary, and identity/scope uncertainty recorded; unsupported claims remain UNKNOWN.

- [ ] Every capability assertion is tied to immutable evidence and an explicit evidence state; tests read are not reported as run.
- [ ] Permissions and editions are described only at the scope directly evidenced.

- [ ] Every RP row relevant to this audit receives a task/disposition or explicit dependency; shared work is not cloned by product.
- [ ] Changed Markdown/local links validate and task integrity check passes.
