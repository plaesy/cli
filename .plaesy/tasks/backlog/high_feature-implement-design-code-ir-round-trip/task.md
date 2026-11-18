---
spec: plaesy-agent-platform
topic: feature-implement-design-code-ir-round-trip
title: Implement design-code IR round trip
phase: implementation
status: backlog
---

# Implement design-code IR round trip

## Goal

Implement, in Rust, the importers, exporters, and code lowering/parsing for the IR decided in the ADR, so that an owner-selected design source and code target can round-trip under a measured fidelity threshold (spec R15).

## Scope

- Implement the IR data model and its serialization with a schema version, in a Rust crate that holds no UI or platform logic.
- Implement one design importer and one exporter for the selected source format, with loss reporting for every unsupported construct.
- Implement IR to code generation for the first selected target, emitting a source map from code spans to node IDs.
- Implement code to IR parsing for the same target, following the edit policy from the ADR.
- Build a fidelity corpus: a fixed set of representative designs, with expected IR, expected code, and round-trip comparison.
- Add focused unit tests, corpus tests, and a fault test for unsupported constructs and malformed input.

## Non-goals

- Do not support every design-tool feature or every code framework in one slice.
- Do not claim parity with any design tool beyond the measured corpus.
- Do not add a UI for this conversion in this task.

## Deliverables

- Rust crate or module for the IR, importer/exporter, and codegen/parser for the selected target.
- Fidelity corpus with a measured round-trip result and the list of losses.

## Dependencies

- `high_chore-decide-design-code-ir-schema-and-target-adr` must be accepted first.
- `high_chore-establish-rust-security-and-release-quality-gates` (pinned toolchain, locked build, Clippy, fmt).
- `high_feature-deliver-selected-design-and-prototyping-workflow` (owner selection of the design workflow).

## Acceptance criteria

- [ ] Every mapped node keeps a stable ID and a source map entry; no mapped node is dropped silently.
- [ ] Each unsupported construct is reported as an explicit loss in the output.
- [ ] Round-trip result on the fidelity corpus meets the owner-approved threshold; if the threshold is not yet approved, the result is reported as measured, not as passing.
- [ ] Malformed or oversized input returns a typed error and does not panic.
- [ ] All Plaesy-owned code for this slice is Rust with a locked build, and format, Clippy, and tests pass (R14).
- [ ] No claim of Figma or other design-tool parity is made in docs or output.
