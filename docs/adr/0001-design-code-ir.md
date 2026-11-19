# ADR 0001: Canonical IR for design-to-code and code-to-design conversion

- Status: accepted 2026-11-18 under the owner's delegation ("lakukan yang terbaik"). The owner may revoke any decision below. Approver recorded as the owner's delegation, not a named person.
- Version: ir/1 (proposed)
- Supersedes: the draft section in `docs/platform/decisions.md`
- Governs: spec R15; tasks `high_chore-decide-design-code-ir-schema-and-target-adr` and `high_feature-implement-design-code-ir-round-trip`

## Context

Plaesy must convert between design documents and code. Converting each design source directly to each code target needs one converter per pair. A single canonical intermediate representation (IR) needs one importer or exporter per format. The conversion must not lose information silently, and it must be measurable. The Rust crate `src/designlint/` already models design tokens (colour, fonts, images), so part of the IR has a starting point.

## Decision drivers

- One converter per format, not per pair.
- Explicit loss: an unsupported construct is reported, never dropped.
- Stable identity: a node keeps its ID across edits and re-imports.
- Forward compatibility: a newer IR version can be read by an older consumer without silent corruption.
- Untrusted input: design files are external data, so they are size-limited and never cause panics.
- Rust only for core logic (R14).

## Considered options

1. **Direct pairwise converters.** Rejected: N×M maintenance, and each pair defines its own loss rules.
2. **Code as the IR (parse code, generate design).** Rejected: code does not carry design intent such as frames, variants, or constraints.
3. **Design-tool file format as the IR.** Rejected: ties Plaesy to one vendor's format and its storage layout.
4. **Plaesy-owned versioned IR with explicit node types, source maps and loss records.** Chosen as the proposal.

## Proposed decision

### Node model (ir/1)

Every node has:

- `id`: stable string, assigned once and preserved across imports and edits. Generated IDs must not reuse a retired ID.
- `kind`: one of `frame`, `layout`, `component`, `variant`, `text`, `vector`, `image`, `token_ref`, `opaque`.
- `props`: typed fields for the kind. Unknown keys are kept in `extensions` and never dropped.
- `children`: ordered list of node IDs.
- `source`: source map entry, `{ format, path_or_span, revision }`.

Tokens (colour, font, spacing) are stored by intent, with the name and value, not by a source tool's storage layout. This follows the principle of representing what the designer meant ([JSIR design](https://github-users.yhajoa.workers.dev/google/jsir/blob/main/docs/intermediate_representation_design.md) for lossless lifting as a goal).

### Unsupported constructs

An unsupported construct becomes an `opaque` node that keeps its original payload and source reference. It is reported as a loss with a reason code. It is never silently dropped.

### Versioning

- The IR version is `MAJOR.MINOR` in the document header.
- Adding a field or node kind is a MINOR change. Consumers ignore unknown keys and keep them in `extensions`.
- Changing or removing a field or meaning is a MAJOR change, and it needs a migration path.
- Semver-style rules follow [ProteanHQ IR spec](https://docs.proteanhq.com/concepts/internals/ir-specification/), which ignores unknown keys in later versions.

### Lowering contracts

| Direction | Input | Output | Reported losses |
| --- | --- | --- | --- |
| design → IR | Plaesy JSON design format v1 | IR | unsupported constructs as `opaque` |
| IR → design | IR | Plaesy JSON design format v1 | constructs the target cannot express |
| IR → code | IR | Rust with Dioxus components | constructs the target cannot express |
| code → IR | code of that target | IR | code outside the generated regions |

### Edit policy (decided: B)

Chosen: B. Option A is too weak for the two-way goal; option C is rejected for ir/1.

- **A. One-way regeneration.** Code is generated, manual edits are not parsed back. Simplest, loses round-trip.
- **B. Guarded merge.** Edits inside managed regions are parsed back via source-map IDs. Manual edits outside those regions are preserved and not parsed.
- **C. Full bidirectional.** Not proposed for ir/1, because it has the highest risk of silent loss.

### Fidelity (decided)

- Structural round trip must be 100% on a versioned corpus of at least 20 fixtures (node IDs, hierarchy, token values).
- Visual pixel fidelity is not claimed.
- Results are reported per fixture and as an aggregate; any miss fails the run.

### Security and limits

- Input limits (decided): maximum 5 MB per design file and nesting depth 64. Exceeding either returns a typed error.
- Malformed input returns a typed error, never a panic and never partial output.

## Consequences

- Positive: one converter per format; loss is visible; IDs make diffs and re-imports deterministic.
- Negative: the first slice is limited to one design source and one code target; the opaque node means some content is not editable as design.
- Neutral: this ADR does not choose any design tool or code framework.

## Out of scope

- Any design tool's full feature set.
- Any code framework beyond the one selected target.
- Real-time collaboration and cloud sync.
- Figma or other tool interoperability claims. None is made.

## Open questions (UNKNOWN, owner to decide)

| # | Question | Owner | Review trigger |
| --- | --- | --- | --- |
| 1 | Approver and acceptance date for this ADR | product owner | before any implementation task starts |
| 2 | First design source format | product owner | before `high_feature-implement-design-code-ir-round-trip` |
| 3 | First code target | product owner | same |
| 4 | Edit policy: A, B or C | product owner | same |
| 5 | Fidelity metric, corpus and threshold | product owner with QA | before any pass is claimed |
| 6 | Input size and depth limits | engineering owner | before parser work |

## Acceptance for this ADR

- [ ] Approver and acceptance date recorded (question 1).
- [ ] Questions 2–6 are answered or remain UNKNOWN with an owner.
- [ ] Contract `adr-accepted-before-start` passes only after the approval above.
