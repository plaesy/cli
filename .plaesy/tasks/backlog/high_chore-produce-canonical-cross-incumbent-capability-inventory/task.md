---
spec: plaesy-agent-platform
topic: chore-produce-canonical-cross-incumbent-capability-inventory
title: Produce canonical cross-incumbent capability inventory
phase: discovery
status: backlog
---

# Produce canonical cross-incumbent capability inventory

## Goal

Deliver the canonical, deduplicated inventory of observable user and operator workflows and leaf capabilities across the 11 incumbents, with evidence provenance, current Plaesy baseline, ownership/disposition, and next action. This is the RP-004 inventory outcome; it is distinct from inspecting incumbents for evidence (RP-003/RP-035–046), auditing current Plaesy implementation status (RP-005), and reconciling the overall crosswalk in the umbrella audit.

## Scope

- Reuse pinned source snapshots and evidence already recorded in `docs/platform/replacement-source-inventory.md`; do not repeat research when an existing artifact is adequate, and flag unverified or stale inputs.
- Define stable, unique capability IDs. Deduplicate shared behavior across products while preserving product-specific variants, preconditions, modes, roles, platform/edition limits, and failure or recovery semantics where evidence supports them.
- For each row record: capability ID; incumbent/product and source revision/location; observable workflow and leaf behavior; conditions/limits; evidence state (`documented`, `source-reviewed`, `test-read`, `test-executed`, `runtime-verified`, `inferred`, or `unknown`); Plaesy requirement and current baseline evidence or explicit `UNKNOWN`; owner/scope disposition (`in-scope`, `deferred`, `excluded`, or `unresolved`) with decision provenance; deduplicated task/dependency or explicit next action; open questions.
- Maintain links to incumbent ledgers and acceptance candidates. A capability without adequate evidence remains `UNKNOWN`; do not infer absence from missing docs/tests or a bounded search.
- Record a reproducible coverage summary: targets and source revisions represented, inspected and uninspected areas, counts by evidence/disposition, duplicate/shared mappings, and unresolved owner decisions. Counts are diagnostics, not proof of exhaustiveness.
- Update `docs/platform/replacement-source-inventory.md`, `feature-measurement-framework.md`, `feature-gap-register.md`, and relevant crosswalk references consistently; preserve existing evidence and unrelated edits.

## Non-goals

- Do not decide product replacement boundaries, owner priorities, first workflows, cohort, or exclusions.
- Do not claim comprehensive incumbent coverage, runtime parity, or implementation completion from inventory rows or page counts.
- Do not implement capabilities, execute incumbent tests/builds, migrate data, or change upstream checkouts as part of this task.
- Do not create product-specific duplicate tasks for shared outcomes.

## Dependencies

- Reuse the 11 product capability-ledger tasks and three family audit packages; source audits may remain partial while the inventory explicitly records unknowns.
- Reuse `high_chore-verify-feature-baseline-across-repositories` for bounded Plaesy source/test baseline evidence; do not substitute inventory membership for that verification.
- Owner-decision task is a dependency only for disposition choices. Record unresolved dispositions explicitly rather than blocking evidence capture.

## Deliverables

- Canonical inventory rows with stable IDs and fields defined above, linked to immutable source evidence and relevant Plaesy artifacts.
- Crosswalk updates identifying where every inventory capability is traced, deferred, excluded, unknown, or awaiting evidence/owner decision.
- A coverage and unresolved-decisions report with reproducible counts and links to all 11 product ledgers.

## Acceptance criteria

- [ ] The deliverable is a canonical, checked-in capability inventory with stable unique IDs, one row per auditable leaf capability (not product/page headings), and fields for source repo, pinned SHA, repository-relative source location, behavior/conditions/failure semantics, evidence class, requirement/scenario/task links, owner disposition, and next action.
- [ ] All 11 incumbent products have a coverage ledger. Each product is either audited to the stated bounded scope or explicitly lists uninspected capability families as `UNKNOWN`; no product is represented as complete from a task count or source-page count.
- [ ] Evidence classes distinguish documented, source-reviewed, test assertion reviewed, test executed, behavior reproduced, and unknown; each test-executed/reproduced claim links to exact command/result or reproduction evidence.
- [ ] Every canonical R1–R15 requirement maps to capability IDs and an acceptance scenario/task, or has an explicit disposition and named owner decision still open.
- [ ] Audit of ID uniqueness, source revision pinning, and referential integrity between capability, requirement, scenario, and task IDs is executable and its output is attached. The current Plaesy `tasks check` skips `spec.md` and `plan.md` with `[!] incomplete`; it cannot satisfy this acceptance criterion until that validator gap is fixed or a separate verified audit command is supplied.
- [ ] RP-01–RP-16 supporting taxonomy and RP-001–RP-046 canonical IDs are not conflated; every provisional mapping is explicitly labeled and linked to a canonical task or `UNKNOWN`.
- [ ] Implementation parity, product completeness, or owner approval is not claimed merely because the inventory is populated.

### Per-product evidence criteria

- [ ] All 11 product ledgers are linked, and each inventory row identifies its source revision/location or a specific evidence blocker; no unsupported source claim is promoted to fact.
- [ ] Every inventory row has a unique stable ID, explicit evidence state, Plaesy baseline status/evidence or `UNKNOWN`, disposition status/provenance or `unresolved`, and a deduplicated task/dependency or concrete next action.
- [ ] Shared capabilities are represented once with explicit incumbent mappings; materially different modes/roles/platform conditions remain separately traceable rather than falsely merged.
- [ ] Source areas reviewed and unreviewed are both reported per product; no claim of exhaustive coverage is based only on counts, headings, or ledger presence.
- [ ] Existing canonical evidence is preserved and stale/conflicting claims are identified with provenance; unknown or conflicting results remain `UNKNOWN`/inconclusive pending resolution.
- [ ] Inventory, gap register, measurement framework, and backlog crosswalk references are mutually consistent; every new/changed ID resolves and every unexplained orphan is listed.
- [ ] A reproducible validation reports row/ID uniqueness, target coverage, evidence-state/disposition counts, and unresolved rows; validation output is recorded without presenting structural checks as source or parity evidence.
- [ ] Focused Markdown and local-link validation and `git diff --check` pass for changed artifacts; no tests/builds are required or claimed for this documentation task.
