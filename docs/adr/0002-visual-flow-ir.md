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

## Node catalogue (flow-ir/1, slice one)

Verified against Windmill's public OpenFlow schema ([openflow.openapi.yaml](https://github.com/windmill-labs/windmill/blob/main/openflow.openapi.yaml), schema version 1.835.0). Each module has `id`, `kind`, `inputs` and `outputs` (typed), and an error policy. Plaesy renames kinds and does not copy Windmill's JSON.

### Module kinds included in slice one

| Plaesy kind | Windmill equivalent | Purpose |
| --- | --- | --- |
| `action` | `script` (PathScript), `rawscript` | Calls a named operation with a typed contract. Inline code is not stored in the IR. |
| `subflow` | `flow` (PathFlow) | Calls another flow by ID and version. |
| `pass` | `identity` | Passes values through, for mapping and debugging. |
| `branch_one` | `branchone` | First matching branch runs; a default branch is required. |
| `branch_all` | `branchall` | Runs all branches, sequential or parallel, each with its own failure policy. |
| `for_each` | `forloopflow` | Iterates a list with `iterator`, `parallel`, `parallelism`, and `skip_failures`. |
| `while` | `whileloopflow` | Loops with `max_iterations` and `max_duration_secs`, both required. |
| `approval` | a script with `suspend` | Waits for `required_events` approvals, with `timeout` and an outcome on timeout. |

### Step-level settings included in slice one

- `retry`: `constant` or `exponential`, with a `retry_if` expression.
- `timeout`: maximum runtime in seconds.
- `stop_after_if`: early termination after a step.
- `skip_if`: conditional skip.
- `continue_on_error`: flow continues when the step fails.
- `early_return`: ends the flow with an explicit output.
- `failure_handler`: one per flow; runs after retries are exhausted.
- `preprocessor`: runs before the first step on external triggers; script or rawscript only.

### Excluded from slice one (with reason)

- `aiagent`, `aidecision`, memory settings, and AI transforms: AI nodes are a separate capability and need their own ADR.
- `identity` used as an AI step: not needed.
- `debounce` and per-step scopes: enterprise features in Windmill; not in slice one.
- `mock`: kept as a test fixture concept, not a runtime node.
- `sleep` and `cache_ttl`: deferred until the runtime exists.

## Codegen contract

- Each target is an adapter implementing one trait: generate per node kind, or report a loss.
- The adapter must support every kind it claims. A kind it cannot express is listed in its loss report.
- Generated code passes the same flow fixtures as the IR for that adapter.

## Consequences

- Positive: one model for all target languages; loops, approvals, and error paths are explicit.
- Negative: the catalogue is larger, so the first editor build is larger.
- Neutral: no target language is chosen by this ADR.

## Evidence

- Module kinds, step settings, retry, approval, timeout, early return and failure module are taken from Windmill's public OpenFlow schema, not from the marketing site. Verify again against the current version before any parity claim.
- A community note reports that Windmill's exponential retry description disagrees with its formula ([issue 11545](https://github.com/windmill-labs/windmill/issues/11545)). Plaesy's retry must define the formula explicitly.
- Windmill's approval docs disagree on the default timeout (30 minutes versus indefinite). Plaesy requires an explicit timeout for every `approval` node.

## Open questions (owner to decide)

| # | Question | Owner |
| --- | --- | --- |
| 1 | Approver and acceptance date for this ADR | product owner |
| 2 | First codegen adapter (Rust, Go, JavaScript, or other) | product owner |
| 3 | Expression language for `input` / `pass` | product owner |
| 4 | Default limits for `for_each` and `while` | engineering owner |
| 5 | Approval channel for `approval` nodes | product owner |
