---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Verify feature baseline across repositories
phase: implement
status: backlog
---

# Verify feature baseline across repositories

## Goal

Verify current implementation status for capabilities needed by the selected workflow across Plaesy CLI, Studio, adapters and relevant deployment artifacts.

## Scope

- Use `docs/platform/feature-gap-register.md` as inventory. For in-scope rows inspect implementation and focused tests; record exact paths and commands/results. Keep status unknown where source/test evidence is insufficient. Do not run the full suite absent a justified need.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Updated status/evidence rows in the feature register and bounded audit note if necessary.

## Dependencies

- None; update is bounded by existing inventory.

## Acceptance criteria

[ ] Every changed status includes an implementation path and exact focused test command/result or explicit scope-limited source finding.

- [ ] No design-only row is marked implemented.

- [ ] In-scope CLI and Studio evidence limitations are explicit.
- [ ] Markdown/link checks pass for changed docs.
