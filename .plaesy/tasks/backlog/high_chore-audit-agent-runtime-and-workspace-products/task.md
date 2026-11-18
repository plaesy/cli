---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Audit agent runtime and workspace products
phase: research
status: backlog
---

# Audit agent runtime and workspace products

## Goal

Produce a source-cited, bounded capability ledger for Hermes Agent, DeepSeek Harness, Nimbalyst, and Orca, resolving repository identity and version questions where evidence permits.

## Scope

- Use approved release/commit baselines when provided; otherwise label current pinned discovery snapshots as non-approved evidence.

- Inspect documentation, executable entry points, relevant source modules and focused test assertions by user-visible workflow: agent execution/providers/tools, state and recovery, workspaces/sessions/worktrees, collaboration, remote/mobile/relay behavior, security/permissions, supported platforms and import/export.
- Separate documented, source-reviewed, test-read, test-executed, runtime-verified, inferred, and unknown. Record source permalink/revision and inspected boundary for every claim.

- Map each leaf to canonical capability ID, Plaesy requirement, evidence/status, candidate acceptance scenario, current Plaesy baseline evidence, migration impact, and one deduplicated follow-up task or explicit disposition.
- Keep uninspected areas and broad claims UNKNOWN; do not infer that missing source evidence means absence.

## Non-goals

- Do not implement replacement features or modify upstream checkouts.

- Do not claim exhaustive product parity or select owner scope.
- Do not assert realtime, cross-host delivery, idle-session wake, or remote capability without direct executable evidence.

## Deliverables

- Audit ledger and updated source inventory/framework/gap register rows for these four products.

- Coverage summary by inspected source area, remaining unknowns, and owner decisions.
- Per-capability task recommendations deduplicated by deliverable, not product name.

## Dependencies

- `high_chore-recover-and-compare-incumbent-research` and `high_chore-complete-evidence-backed-feature-audit-for-11-incumbents`; research may proceed against pinned discovery snapshots while approved baselines remain unresolved.

## Acceptance criteria

- [ ] Each of the four products has a revision, license, inspected/uninspected boundary, and identity/version status supported by source evidence or explicitly UNKNOWN.

- [ ] Each inventoried capability has a stable ID, immutable citation, evidence state, Plaesy mapping, and a clear next action/disposition.
- [ ] Every candidate acceptance scenario is observable and traceable; test assertions read are not presented as executed tests.

- [ ] No claim exceeds the inspected boundary; no duplicate product-specific task clones are created.
- [ ] Changed Markdown and local links validate; task integrity check passes.
