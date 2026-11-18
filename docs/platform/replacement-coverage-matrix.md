# Replacement coverage matrix — current evidence

**Authority:** Supporting evidence snapshot. Canonical requirements and priorities are in `.plaesy/spec/plaesy-agent-platform/spec.md` and `.plaesy/plan/plaesy-agent-platform/plan.md`; lifecycle is authoritative in `.plaesy/tasks/`. This matrix does not assert parity or owner approval.

## Program-level coverage

The inventory records 11/11 pinned discovery snapshots, but 0/11 owner-approved release baselines, exhaustive leaf-feature inventories, or incumbent/Plaesy-verified acceptance as of its recorded audit. Candidate documentation/source leads are not an exhaustive product ledger. Source: [`replacement-source-inventory.md`](replacement-source-inventory.md#coverage-status). Current task tree count or contract checklist presence does not satisfy the plan’s source-coverage boundary.

| Incumbent | Discovery pin recorded | Owner-approved release baseline | Exhaustive leaf ledger | Verified incumbent/Plaesy acceptance | Next bounded action |
|---|---|---|---|---|---|
| Hermes Agent | Yes — `40d1e95f37e409f9f618f32c170661c66b86a4f4` | UNKNOWN / none recorded | No | No | Complete source/API/runtime leaf audit; map each candidate to pinned implementation and focused tests. |
| Paperclip | Yes — `7685b7a4fc00fb5469c4e1fb38b1d8ba534322af` | UNKNOWN / none recorded | No | No | Categorize pinned docs, separate current contracts from plans/history, then reconcile each candidate to source/tests. |
| DeepSeek Harness | Yes — `39163dd30f703a79079e2d2446545cff3c453738` | UNKNOWN / none recorded | No | No | Confirm exact product/repository/runtime identity and bounded capabilities; classify prior inconclusive native-binding test result. |
| Nimbalyst | Yes — `30482d5b719cd7609fb02645008c2f5088565ab9` | UNKNOWN / none recorded | No | No | Finish workspace/transcript/sync/recovery leaf audit and focused test evidence. |
| Orca | Yes — `65c84d41101e2e6255f64155985de873fc81c902` | UNKNOWN / none recorded | No | No | Confirm exact product/component identity; audit authenticated transport, remote execution, recovery and test coverage. |
| Craft Anything | Yes — `ba46f341abf0c31e83d050bbc0f0d7ebba295367` | UNKNOWN / none recorded | No | No | Reconcile passing engine tests to end-user workflows, tool/API boundaries, approvals and fault cases. |
| FigCraft | Yes — `e1883565479f3c763488fe0d7a149fafee33ebb9` | UNKNOWN / none recorded | No | No | Build bounded authoring/import/export capability ledger and identify a reproducible fidelity corpus. |
| DeckCraft | Yes — `5c698493bc1f7ec36f90deb29df247b93330e654` | UNKNOWN / none recorded | No | No | Audit presentation authoring/playback/export and format fidelity; prior test output is not accepted as verified. |
| WordCraft | Yes — `29d862fabca05ba1e273a60041905c5ee8a7fb52` | UNKNOWN / none recorded | No | No | Audit editing/review/interoperability and DOCX fidelity; historical test report not rerun. |
| Herald OS | Yes — `d8733e9393c7186e5e8c843ea4abc97d9173ee45` | UNKNOWN / none recorded | No | No | Verify permission/approval/audit boundaries, platform support and failure/recovery behavior. |
| OpenProject | Yes — `729734e478b19b009b856c96d53eaad89d436254` | UNKNOWN / none recorded | No | No | Continue endpoint-family audit; resolve edition/license boundary and obtain Ruby/Bundler before scoped tests. |

The exact product-level audit state above is a reconciliation of previously recorded bounded audits; it is not a fresh checkout/test pass. Each product ledger must preserve source SHA/path, evidence class, inspected/uninspected areas, scenario and requirement mapping, current Plaesy baseline, disposition, and next action. Uninspected behavior stays `UNKNOWN`.

## Separate R15 design↔code gate

R15 is not an incumbent row and is not satisfied by the 11-source audit. Candidate scenarios `AC-R15-IR-001`–`004` are in [`feature-measurement-framework.md`](feature-measurement-framework.md#8a-r15-design-code-traceability-candidate-not-yet-acceptance-evidence). They remain UNKNOWN/BLOCKED: no owner-approved workflow/source/target, accepted IR ADR/edit policy, fidelity metric/threshold, implemented importer/exporter/codegen/parser, versioned corpus, or executed criterion-specific tests are evidenced. The source token model in `src/designlint/` is only a possible reuse point, not proof of conversion behavior.

**Next action:** complete pinned product ledgers and evidence links independently of owner decisions; keep implementation and fidelity claims gated until owner choices and executable acceptance evidence exist. Do not equate ledger population, task count, docs review, or test assertions with verified behavior/parity.
