---
spec: plaesy-agent-platform
topic: feature-implement-visual-flow-editor-and-codegen
title: Implement visual flow editor and codegen
phase: implementation
status: backlog
---

# Implement visual flow editor and codegen

## Goal

Implement the visual flow editor, the flow IR storage, and one codegen target, so a flow authored once can be generated into the owner-selected language (spec R16).

## Scope

- Implement the flow IR as a versioned Rust module with serialization and validation.
- Implement the visual editor for web first, in Dioxus (owner choice, 2026-11-18). Start the web editor directly; desktop and mobile follow on the same codebase. The editor must talk to the Rust core through a surface that desktop and mobile can reuse.
- Generated code uses pluggable adapters. The first adapter is Rust (owner decision, 2026-11-18). Go and JavaScript follow as later adapters. Plaesy's own code stays Rust.
- Implement one codegen target and the loss reporting for unsupported nodes.
- Add a fixture set of flows under `tests/fixtures/flows/`, one file per node kind in the ADR 0002 catalogue, plus malformed inputs.
- Run fixture tests with `cargo test --locked visual_flow`. Generated Rust code must compile and produce the same results as the IR fixture expects.

## Non-goals

- Do not add more than one target in this slice.
- Do not claim parity with any visual workflow product.

## Deliverables

- Flow IR module, editor, one codegen target, and fixture tests.

## Dependencies

- `high_chore-decide-visual-flow-ir-and-codegen-adr` must be accepted first.
- `high_chore-establish-rust-security-and-release-quality-gates`.

## Acceptance criteria

- [ ] Saved flows are versioned IR with stable node IDs and no target-language fields.
- [ ] Generated code passes the same fixtures as the IR for the selected target.
- [ ] Unsupported node types are reported as explicit losses, never dropped.
- [ ] Malformed flow input returns a typed error without panic.
- [ ] Every node kind in ADR 0002 has at least one passing fixture under `tests/fixtures/flows/`.
- [ ] `cargo test --locked visual_flow` passes, and generated Rust compiles.
- [ ] Retry, loop limits, and approval timeout follow the ADR 0002 values, checked by fixtures.
- [ ] Rust core is locked, formatted, and passes Clippy and tests (R14).
