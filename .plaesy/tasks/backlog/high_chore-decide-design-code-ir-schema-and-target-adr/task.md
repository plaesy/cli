---
spec: plaesy-agent-platform
topic: chore-decide-design-code-ir-schema-and-target-adr
title: Decide design-code IR schema and target ADR
phase: decision
status: backlog
---

# Decide design-code IR schema and target ADR

## Goal

Record, as an owner-reviewed ADR, the canonical versioned intermediate representation (IR) used for design-to-code and code-to-design conversion (spec R15), before any importer, exporter, or codegen is implemented.

## Scope

- Survey the existing token model in `src/designlint/` (`tokens.rs`, `colour.rs`, `fonts.rs`, `images.rs`) and decide what the IR reuses, extends, or replaces.
- Define the IR node set: frames, auto-layout constraints, components and variants, styles and tokens, vector/image assets, text, and stable node IDs.
- Define the lowering contract for each direction: design source to IR, IR to design, IR to each code target, and code to IR, including source maps and explicit loss reporting.
- Decide the first code target(s) and the first design source(s) for the fidelity corpus. Record unknowns as UNKNOWN with an owner.
- Record the edit policy for manual changes on either side of a round trip.
- Write the ADR under `docs/platform/decisions.md` (or a new ADR file linked from it).

## Non-goals

- Do not implement the importer, exporter, or codegen in this task.
- Do not claim Figma or any other design-tool interoperability.
- Do not fix the fidelity threshold without owner approval.

## Deliverables

- ADR with schema version, node model, lowering contracts, loss-reporting rules, edit policy, and target list.
- List of owner decisions still open, each with an owner.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` (owner selection of design workflow and targets).
- `high_chore-define-rust-domain-architecture-and-approved-deployment-boundaries` (Rust crate boundary, R14).

## Acceptance criteria

- [ ] ADR is owner-approved with approver and approval date, or remains explicitly proposed and this task remains open.
- [ ] A versioned schema artifact or structured ADR section defines every node's fields and invariants, stable ID creation/preservation across edits/imports, and the source-map schema and coordinate/path semantics.
- [ ] Every lowering direction names input/output, supported constructs, and structured loss behavior; no construct may be silently dropped.
- [ ] Edit policy is owner-approved for each supported round-trip direction; unknown choices name an accountable owner and review trigger.
- [ ] Target source and code target are owner-approved, or explicitly `UNKNOWN` with an accountable owner and review trigger; no implementation is authorized by a proposal.
- [ ] The ADR defines the representative corpus version, measurable fidelity metric, aggregation rule, and owner-approved threshold, or records these as unresolved gates that block implementation.
- [ ] An executable structured validator rejects missing schema fields, directions, mappings or loss semantics; keyword presence alone cannot pass acceptance.
- [ ] No upstream source is modified; no build is required for this decision task.
