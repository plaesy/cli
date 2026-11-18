---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Audit creative authoring products
phase: research
status: backlog
---

# Audit creative authoring products

## Goal

Produce source-cited, bounded capability ledgers for Craft Anything, FigCraft, DeckCraft, and WordCraft, including reproducible artifact interoperability evidence where feasible.

## Scope

- Use approved release/commit baselines when provided; otherwise clearly mark pinned discovery snapshots as non-approved.

- Inspect user workflows and supporting docs/source/tests for investigation/build/check/evidence, design/prototyping and document models, slide and word authoring, rendering, import/export, collaboration, persistence, recovery, automation interfaces, platform/edition and license boundaries.
- For formats, distinguish format claims from demonstrated round trips. Define inspected corpus and fixture provenance; execute only bounded, safe tests and record command/version/result. If execution is blocked, preserve UNKNOWN/inconclusive status.

- Assign canonical capability IDs; link immutable source evidence, Plaesy requirement/baseline, candidate acceptance, migration impact, and a deduplicated task/disposition.
- Explicitly list uninspected capabilities and unresolved fidelity boundaries.

## Non-goals

- Do not implement editors or claim full DOCX/PPTX/Figma interoperability.

- Do not modify upstream source or infer absence from missing evidence.
- Do not turn README/roadmap assertions into verified behavior.

## Deliverables

- Four source-cited leaf-capability ledgers plus updated inventory/framework/gap register.

- A small, provenance-labelled representative-format corpus and bounded interoperability findings when runnable; otherwise a precise blocker list.
- Deduplicated task recommendations by shared capability and evidence-backed coverage report.

## Dependencies

- Research recovery and umbrella incumbent audit; owner-approved baseline required before parity claims.

## Acceptance criteria

- [ ] Each product has pinned revision, license/edition notes, inspected/uninspected boundary, and evidence states.

- [ ] Each format claim separates documented support, source-reviewed implementation, and executed round-trip/render results.
- [ ] Each accepted leaf maps to a stable capability, requirement, scenario, and deduplicated task/disposition; unknowns remain explicit.

- [ ] Corpus fixtures have source/license/provenance and no confidential user data.
- [ ] Changed Markdown and local links validate; task integrity check passes.
