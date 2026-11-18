---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Recover and compare incumbent research
phase: implement
status: backlog
---

# Recover and compare incumbent research

## Goal

Recover prior evidence and create a sourced comparison index for the selected workflow without rerunning completed research.

## Scope

- Search project docs, bounded session history and identified repositories for existing evidence about exactly the 11 requested incumbents: Hermes Agent, Paperclip, DeepSeek Harness, Nimbalyst, Orca, Craft Anything, FigCraft, DeckCraft, WordCraft, Herald-OS, and OpenProject. Identify exact product/version/commit. Add sources, reviewed date, observed-vs-documented distinction, Plaesy mapping and unknowns to the existing comparison index. Do not make unsourced capability claims or substitute unrelated products.

- Before owner workflow selection, inventory source/documentation coverage for all 11 as required by the evidence-backed audit task. After that broad inventory, constrain deep comparative runtime tests to the owner-selected workflow; retain all other features as documented/inferred/unknown and do not claim runtime parity.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- A comparison evidence index linked from the register; source records for the first workflow; explicit unknowns for unavailable prior research.

## Dependencies

- Owner decision task identifies chosen workflow (parallel research recovery may proceed before workflow selection).

## Acceptance criteria

[ ] Existing evidence is linked with source/version/date; no repeated research is presented as new.

- [ ] Each selected comparison claim cites primary evidence or reproducible observation.

- [ ] Product/version ambiguity is recorded as UNKNOWN rather than guessed.
- [ ] Register links to the index and remaining research is bounded to selected workflow.
