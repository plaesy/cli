---
spec: plaesy-agent-platform
topic: chore-decide-visual-flow-ir-and-codegen-adr
title: Decide visual flow IR, editor technology and codegen targets ADR
phase: decision
status: backlog
---

# Decide visual flow IR, editor technology and codegen targets ADR

## Goal

Record, as an owner-reviewed ADR, the language-neutral flow IR, the editor technology (an R14 UI exception), and the first codegen targets (spec R16), before any editor or codegen is built.

## Scope

- Define the flow IR: nodes (trigger, action, branch, loop, subflow), typed ports and edges, stable node IDs, schema version, and no target-language fields.
- Define how node behaviour is referenced without hardcoding a language (for example a named operation with a typed contract).
- Choose editor technology and record it as an approved R14 exception, or reject the exception.
- Target language is Rust (owner decision, 2026-11-18). Record it, and define how later targets are added.
- First editor surface is web. UI technology is Dioxus (owner choice, 2026-11-18), which keeps the UI in Rust. No proof-of-concept gate is required (owner decision). Note that Dioxus desktop renders through a system webview, so the rendering layer is not pure Rust; record that as a stated limit.
- Define loss reporting for node types a target cannot express.

## Non-goals

- Do not implement the editor or any codegen in this task.
- Do not copy a competitor's flow format or claim parity with any product.

## Deliverables

- ADR with schema version, node and edge model, editor exception decision, target list, and loss rules.
- Owner-assigned list of open decisions.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` (owner decision on first workflow).
- `high_chore-define-rust-domain-architecture-and-approved-deployment-boundaries` (Rust core boundary, R14).

## Acceptance criteria

- [ ] ADR records approver and approval date, or remains proposed and this task stays open.
- [ ] Flow IR node and edge model is written with stable ID rules and a schema version.
- [ ] Editor technology decision is recorded as an approved R14 exception or rejected.
- [ ] Target list and loss rules are written, with unknowns named and owned.
- [ ] No implementation or parity claim is made.
