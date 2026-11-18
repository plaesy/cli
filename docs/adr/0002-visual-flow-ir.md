# ADR 0002: Language-neutral visual flow IR

- Status: accepted in full. Accepted by the owner in chat on 2026-11-18: node catalogue, CEL expressions, retry formula, loop limits, approval channel, and Rust as the first codegen adapter. Implementation may start.
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
- `scope`: per-step permission scope (for example `domain:action[:resource]`). A step may only narrow the flow's scope, never widen it. Maps to R5 (least privilege) and R9 (tool and policy boundaries).
- `early_return`: ends the flow with an explicit output.
- `failure_handler`: one per flow; runs after retries are exhausted.
- `preprocessor`: runs before the first step on external triggers; script or rawscript only.

### Excluded from slice one (with reason)

- `aiagent`, `aidecision`, memory settings, and AI transforms: AI nodes are a separate capability and need their own ADR.
- `identity` used as an AI step: not needed.
- `debounce`: marked EE in Windmill's schema; not in slice one.
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

- Per-step scope is in Windmill's schema, and Plaesy adopts it because it serves R5 and R9.
- Module kinds, step settings, retry, approval, timeout, early return and failure module are taken from Windmill's public OpenFlow schema, not from the marketing site. Verify again against the current version before any parity claim.
- A community note reports that Windmill's exponential retry description disagrees with its formula ([issue 11545](https://github.com/windmill-labs/windmill/issues/11545)). Plaesy's retry must define the formula explicitly.
- Windmill's approval docs disagree on the default timeout (30 minutes versus indefinite). Plaesy requires an explicit timeout for every `approval` node.

## Proposed resolutions (research and simulation; owner confirms)

### Expression language for pass and branch

- **Chosen proposal: CEL** (Common Expression Language, the `cel` crate, MIT, targets full conformance with cel-spec, runs the conformance suite in CI). CEL is non-Turing-complete, so expressions always terminate. Evaluation can be interrupted through `cel::Interrupt`, which gives a time bound ([cel-rust](https://github.com/cel-rust/cel-rust)).
- **Rejected for expressions: Rhai.** It is a full scripting language. Its safety depends on `set_max_operations` and related caps, and an external Rust function counts as one operation regardless of its real cost ([Rhai max operations](https://rhai.rs/book/safety/max-operations.html)). A flow expression should not be a general program.

### Retry formula (explicit, simulated)

- Delay before attempt k (k ≥ 2) = min(base × multiplier^(k−2), cap). Defaults: base 1 s, multiplier 2, cap 60 s, no jitter (deterministic for fixture tests).
- Simulation: 3 attempts → waits 1 s, 2 s (total 3 s). 5 attempts → 1, 2, 4, 8 s (total 15 s). 8 attempts → total 123 s, cap engaged at attempt 8.
- `retry_if` is a CEL expression over the error object.

### Loop limits (simulated)

- `for_each`: wall time = ceil(N / P) × d. Proposed defaults: at most 10,000 items and parallelism at most 50. Simulation at d = 0.2 s: 10,000 items with P = 10 takes 200 s; with P = 50 takes 40 s. 100,000 items with P = 50 takes 400 s, so the 10,000-item default stays inside typical interactive bounds.
- `while`: effective bound = min(max_iterations, ceil(max_duration / d)). Proposed defaults: `max_iterations` 1,000 and `max_duration_secs` 3,600. Simulation: with d = 1 s the bound is 1,000 iterations (1,000 s); with d = 10 s the duration cap binds at 360 iterations (3,600 s). Both fields are required, so neither bound can be omitted.

### Approval channel (proposal)

- Primary: HTTP API that returns a single-use resume URL with an expiry, plus a web page in the Dioxus editor. Windmill also uses generated secret URLs for approvers ([Windmill approval docs](https://www.windmill.dev/docs/flows/flow_approval)).
- Approvals require `required_events`, an explicit `timeout`, and an outcome for timeout and rejection.

## Still owner decisions

Accepted on 2026-11-18: the proposals above (CEL, the retry formula, loop limits, and the approval channel).

| # | Question | Owner | Why it stays open |
| --- | --- | --- | --- |
| 1 | Approver and acceptance date for this ADR | product owner | Accepted in chat on 2026-11-18; a named approver can be added if required. |
| 2 | First codegen adapter | product owner | Decided 2026-11-18: Rust, for testing with Plaesy's own application. Go and JavaScript follow later as separate adapters. |
| 3 | Confirm or change the proposed expression language, retry defaults, loop limits and approval channel above | product owner | Accepted 2026-11-18. |

