# ADR 0002: Language-neutral visual flow IR

- Status: proposed (not accepted). Approver: UNKNOWN (owner to name). Date accepted: UNKNOWN.
- Version: flow-ir/1 (proposed)
- Governs: spec R16; tasks `high_chore-decide-visual-flow-ir-and-codegen-adr` and `high_feature-implement-visual-flow-editor-and-codegen`
- Related: ADR 0001 (design-code IR). Flow IR is separate because flows are logic, not layout.

## Context

Users author flows in a visual editor. A flow is stored once as IR, then generated into a chosen language (Rust, Go, JavaScript, or others) through an adapter. The IR must not contain any target-language field. Plaesy itself is implemented in Rust (R14). The editor is Dioxus (owner choice).

## Decision drivers

- Language neutrality: the IR never names a language, runtime, or library.
- Explicit behaviour: every step has typed inputs and outputs and a defined failure outcome.
- Stable identity: node IDs survive edits, reordering, and re-import.
- Forward compatibility: unknown fields are kept, not dropped.
- Explicit loss: a node a target cannot express is reported, never silently skipped.

## Considered options

1. **Code-first flows (users write code, editor shows a graph).** Rejected for slice 1: ties the model to one language.
2. **Vendor flow format (copy another product's schema).** Rejected: copies a product, not a model we own.
3. **Plaesy-owned IR with a fixed node catalogue.** Chosen.

## Node catalogue (flow-ir/1)

Each node has `id`, `kind`, `inputs` (typed), `outputs` (typed), `on_error` (`fail`, `continue`, or a failure-handler node ID), and `source` (editor position and label, not semantics).

| Kind | Purpose | Notes |
| --- | --- | --- |
| `trigger` | Starts the flow | Manual, schedule, or webhook. Exactly one per flow. |
| `action` | Calls a named operation | Operation is a reference with a typed contract, never inline target code. |
| `input` / `pass` | Passes or maps values | Pure data mapping with an expression language defined in the ADR. |
| `branch_one` | If / else-if / else | First matching branch runs; a default branch is required. |
| `branch_all` | Runs all branches | Parallel or sequential, chosen per node. Each branch has its own failure policy. |
| `for_each` | Iterates a list | Has a bounded item limit and an explicit empty-list outcome. |
| `while` | Loops while a condition holds | Requires a maximum iteration count. |
| `subflow` | Calls another flow | Reference by ID and version; no inline copies. |
| `approval` | Waits for a human decision | Approve or reject, with an explicit timeout outcome. |
| `retry` | Wraps one node with retry policy | Count, backoff, and retryable error classes. |
| `timeout` | Bounds one node's runtime | Duration and timeout outcome. |
| `early_return` | Ends the flow with a value | Explicit output shape. |
| `failure_handler` | Runs on unrecoverable failure | One per flow; receives the failing node ID and error. |

Edges connect output ports to input ports. Every edge is typed. Cycles are allowed only through `while` and `for_each`, never by raw edges.

## Codegen contract

- Each target is an adapter implementing one trait: generate per node kind, or report a loss.
- The adapter must support every kind it claims. A kind it cannot express is listed in its loss report.
- Generated code passes the same flow fixtures as the IR for that adapter.

## Consequences

- Positive: one model for all target languages; loops, approvals, and error paths are explicit.
- Negative: the catalogue is larger, so the first editor build is larger.
- Neutral: no target language is chosen by this ADR.

## Evidence and gaps

- Windmill's OpenFlow model uses branch, loop, subflow, and failure-handler module types ([Windmill OpenFlow](https://windmill.dev/docs/openflow)). Its `branch_all` with per-branch failure policy informed `branch_all` here.
- Approval steps, early return, and per-step retry were not confirmed in the public Windmill page. They are included as Plaesy requirements, not as copies of Windmill behaviour. Verify against Windmill's OpenAPI definition before claiming parity.

## Open questions (owner to decide)

| # | Question | Owner |
| --- | --- | --- |
| 1 | Approver and acceptance date for this ADR | product owner |
| 2 | First codegen adapter (Rust, Go, JavaScript, or other) | product owner |
| 3 | Expression language for `input` / `pass` | product owner |
| 4 | Default limits for `for_each` and `while` | engineering owner |
| 5 | Approval channel for `approval` nodes | product owner |
