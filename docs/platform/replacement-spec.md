# Plaesy replacement program — specification

**Status:** Draft; target scope, not a parity claim. **Language:** Rust for Plaesy-owned application and service code. **Evidence reviewed:** pinned discovery snapshots and partial source/documentation reviews described in [replacement-source-inventory.md](replacement-source-inventory.md); the snapshots are not owner-approved release baselines and the leaf audits remain incomplete. Re-pin approved versions before acceptance testing.

## Purpose and boundary

Build Plaesy as a modular, self-hostable platform that can progressively replace the selected user workflows of the eleven named products. “Replace” means users can complete explicitly selected workflows in Plaesy and migrate the associated data with tested recovery; it does not mean reproducing every upstream feature or replacing all products in one release. Keep incumbent tools available until the relevant parity and rollback gate passes. Never modify Hermes or other upstream source.

Target sources: [Hermes Agent](https://github.com/NousResearch/hermes-agent), [Paperclip](https://github.com/paperclipai/paperclip), [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness), [Nimbalyst](https://github.com/nimbalyst/nimbalyst), [Orca](https://github.com/stablyai/orca), [Craft Anything](https://github.com/storytold/craft-anything), [FigCraft](https://github.com/storytold/figcraft), [DeckCraft](https://github.com/storytold/deckcraft), [WordCraft](https://github.com/storytold/wordcraft), [Herald OS](https://github.com/iamlukethedev/Herald-OS), and [OpenProject](https://github.com/opf/openproject).

## Product-level outcomes

1. One coherent, modular Rust platform provides agent execution/orchestration, project/work management, knowledge and creative artifact workflows behind explicit interfaces.
2. A user can select a supported workflow, connect compatible models/runtimes, grant scoped permissions, inspect progress/evidence, review outputs, and recover safely from interruption.
3. User data remains portable; migration, backup, restore, export, and rollback are tested for every declared workflow.
4. Every replacement claim is tied to a pinned upstream version, a reproducible scenario, observable acceptance criteria, and evidence. Unsupported, uninspected, or ambiguous capabilities stay **UNKNOWN**.

## Product boundaries and safeguards

- Rust is mandatory for all Plaesy-owned replacement core/domain, persistence, service/backend, CLI, and native runtime logic. UI/web technology requires an owner-approved ADR; keep authoritative business rules/state in Rust and document any narrowly scoped UI/platform exception rather than silently treating Rust as a frontend mandate. Each implementation slice must pin its toolchain, lock dependencies, use locked builds, and pass focused format, Clippy, test, security, and dependency/license gates.
- Treat models, agent output, imported documents, messages, filenames, plugins, and tool output as untrusted. They cannot grant authority or alter policy.
- Do not treat caller-supplied agent IDs as authentication. Never expose SQLite as a network protocol or add unauthenticated endpoints. Do not store/forward/expand Hermes peer-pass authority through Relay.
- Do not claim realtime delivery, cross-host delivery, session wake, parity, compliance, HA, or durability without implementation-specific evidence. At-least-once effects require idempotency or explicit reconciliation.
- Integrations with external harnesses are replaceable adapters; do not edit their source. Distinguish Plaesy-owned features from backend-provided capabilities.
- Existing product/platform docs remain authoritative constraints; this program adds a cross-product traceability and acceptance layer, not permission to weaken security or deployment requirements.

## Capability map (research baseline, not exhaustive audit)

| Product | Publicly described capability family to assess | Evidence state and required verification |
|---|---|---|
| Hermes Agent | Agent loop, provider/model selection, CLI, messaging gateway, tools/MCP, skills, memory, scheduling, security and profiles | README/docs describe these; inspect pinned release/docs and run workflow-level tests. No parity inferred. |
| Paperclip | Multi-agent org/goals, task threads, heartbeats, budgets, governance/approvals, integrations, multi-organization | README describes these; verify exact supported user journeys, RBAC, cost accounting, and import/export. |
| DeepSeek Harness | Harness/runtime capabilities and agent execution | Exact intended project/version and feature baseline unresolved; UNKNOWN until source and workflows inspected. |
| Nimbalyst | Visual workspace/editors, concurrent agent sessions/worktrees, tasks, Git/terminal, MCP/extensions, mobile companion | README describes these; verify platforms, collaboration service boundary, compatibility and data formats. |
| Orca | Coding-agent fleet across desktop/mobile/remote runtime; relay/connectivity and runtime orchestration | Repository is broad/ambiguous; pin intended product components and version, inspect primary sources; UNKNOWN pending. |
| Craft Anything | Inspect/rebuild workflow; coding-agent-assisted clean-room project generation and comparison | README describes workflow; inspect supported targets, tools, safety, repeatability and evidence requirements. |
| FigCraft | Interface design/prototyping, vector/layout/components, command/CLI/MCP automation, collaboration, import/export | README describes broad design workflows; measure actual interoperability, offline and collaboration limits. |
| DeckCraft | Slide authoring, presentation format, CLI/MCP automation, import/export and rendering | README/roadmap describe capabilities and gaps; pin version and test representative PPTX round-trips. |
| WordCraft | Word processing, styles/tables/review and DOCX, CLI/MCP, native/web targets | README describes workflows; test real DOCX corpus, round-trip fidelity and unsupported formats. |
| Herald OS | Agent-mediated operating-system workflows, machine/file/app control, routines and permission system, built on Hermes | README says it is independent and runs Hermes; inspect installation, threat boundaries, supported OS/actions and Hermes dependency. |
| OpenProject | Project/portfolio/work-package management, agile boards/roadmaps/Gantt, time/cost, collaboration and integrations | README describes broad project management; define target edition/version and verify migration, permissions, reporting, and enterprise-only boundaries. |

This table is a discovery index, not a complete feature inventory or feature-by-feature source audit. Maintain a separate row per concrete workflow in `feature-gap-register.md`; attach permalink + commit/release, observed/documented label, Plaesy mapping, acceptance evidence, owner, and review date. Never infer absence from missing evidence.

## Functional requirements

| ID | Requirement / acceptance |
|---|---|
| RP-01 | Capability traceability: each target workflow has stable ID, source/version, Plaesy module, status, owner, test/evidence link, migration impact and next action. Duplicate capabilities share one canonical requirement with multiple source mappings, not copied tasks. |
| RP-02 | Product scope: every release names included workflows and explicit exclusions; no global “replacement complete” claim until every owner-approved target workflow passes its gate. |
| RP-03 | Agent runtime: user configures providers/models and replaceable harness adapters; capability negotiation exposes support, limitations, identity, cancellation, timeout, usage and evidence. |
| RP-04 | Work orchestration: goals/projects/tasks/dependencies/assignment/approval/review and durable lifecycle are available for approved workflow; transitions are authorized and auditable. |
| RP-05 | Execution and recovery: stable run IDs, durable state, idempotency/reconciliation, cancellation/timeouts, artifact provenance, restart recovery and truthful ambiguous outcomes. |
| RP-06 | Collaboration: project-scoped authenticated messages/events, delivery acknowledgements, retry/duplicate handling, presence semantics and offline behavior are specified; no false wake/read/realtime assertions. |
| RP-07 | Concurrent changes: isolate agent writes or enforce revision/fencing/conflict checks; detect external edits and preserve original work. Advisory locks alone do not qualify. |
| RP-08 | Trust and policy: authenticated principals, least privilege, per-project/task/tool grants, human approvals, revocation, budget limits, audit, secret isolation/redaction and prompt-injection resistance. |
| RP-09 | Knowledge/memory: provenance, scope, privacy, user control, retention/deletion, export/import and review of model-derived durable facts. |
| RP-10 | Creative/document workflows: selected artifact editors and file formats have explicit schema, import/export, round-trip, rendering, accessibility and recovery acceptance; no blanket parity based on file extension alone. |
| RP-11 | Operator experience: supported CLI/API/UI provides configuration, health, task/run inspection, evidence, approvals, recovery and actionable errors. UI surfaces are owner-approved. |
| RP-12 | Operations: declared install profiles, updates, backup/restore, migration, rollback, observability, retention, resource limits and support boundaries are tested. |
| RP-13 | Integration and extensibility: versioned, permission-scoped contracts for providers, harnesses, MCP, plugins and external systems; extensions are isolated or explicitly trusted. |
| RP-14 | Migration: each selected incumbent workflow has documented source schema, mapping, dry run, validation, idempotent import, audit and rollback; preserve original source data. |
| RP-15 | Quality/security: Rust quality and release gates below are mandatory; unresolved critical/high security findings block release absent explicit owner-approved exception and mitigation. |
| RP-16 | Replacement gate: pinned-version side-by-side scenario, data reconciliation, permission checks, failure/recovery, user acceptance, rollback and named approver pass before incumbent retirement. |

## Rust engineering rules (testable quality bar)

- **Toolchain and reproducibility:** use stable Rust unless an approved ADR justifies another channel; pin the toolchain (`rust-toolchain.toml`) and commit `Cargo.lock` for applications. CI uses `--locked`; format with `cargo fmt --all -- --check`; run Clippy on supported targets with warnings denied for changed crates. Minimize dependencies, review license/advisory status, and record provenance for release artifacts.
- **Unsafe and API design:** forbid `unsafe` by default; any exception needs a documented safety invariant, minimal encapsulated surface, independent review, and focused safety tests. Keep public APIs deliberate, documented, versioned, and backward-compatible or explicitly migrated.
- **Architecture:** organize the workspace by domain and dependency direction; keep domain logic independent of transport, UI, persistence, and vendor SDKs. Avoid cyclic crate dependencies and generic `utils` dumping grounds. Enforce dependency rules in CI or architecture tests.
- **Errors and input:** use typed errors internally (`thiserror` or equivalent), add actionable context at boundaries, and return errors rather than panic on user/network/storage input. Validate input size, paths, formats, timeouts, and resource consumption; use checked arithmetic where overflow affects safety or correctness. Do not unwrap/expect on fallible production paths without a proved invariant.
- **Async and concurrency:** define cancellation, bounded concurrency, backpressure, timeouts, and shutdown. Do not block executor threads. Avoid detached tasks; supervise and observe spawned work. Document lock ordering and never hold locks across unbounded I/O.
- **Persistence and compatibility:** schema changes require versioned, transactional migrations, integrity checks, tested backup/restore and rollback/recovery. Define transaction/isolation behavior and compatibility policy for stored data and external contracts before release.
- **Security:** least privilege, secure secret handling, constant-time comparison where applicable, TLS verification, origin/CSRF protections for browser surfaces, safe path/archive handling, and no shell interpolation. Update threat models for new boundaries. Audit logs omit secrets and minimize sensitive payloads.
- **Testing:** unit/property tests cover domain invariants; integration tests use disposable real persistence/transport; contract tests cover adapters; fault injection covers restart/network ambiguity; end-to-end tests cover approved user journeys; regression fixtures cover imports/round trips. Mocks are not sole evidence for persistence, concurrency, or recovery guarantees.
- **CI and release gates:** locked builds/tests, format, Clippy, doc tests, dependency/license/advisory and secret scans, supported-target build matrix, and reproducible release artifact/provenance. Track coverage trends; set numerical coverage thresholds, SLOs, and benchmark budgets in an approved ADR before release rather than inventing them.
- **Observability and operations:** structured correlated traces/logs/metrics with bounded cardinality, privacy-safe redaction, retention policy, health/readiness semantics, and operator runbooks.
- **Architecture decisions:** avoid premature microservices; choose modular boundaries via ADRs and measured operational needs. Record alternatives, consequences, migration, and review trigger.

## Non-functional requirements

Security, reliability, performance, availability, platform matrix, accessibility, localization, retention, data residency and cost targets remain measurable open decisions until owners set thresholds. Before a release, each selected workflow must define workload envelope, SLO/error budget (if hosted), data-loss/recovery objectives, latency/resource budgets, supported OS/runtime matrix, accessibility criteria, retention/deletion and support window. No target means no claim of meeting it.

## Source inventory status

The partial discovery baseline is recorded in [replacement-source-inventory.md](replacement-source-inventory.md). Immutable upstream head snapshots are pinned for all 11 targets, but they are not owner-approved release baselines. Source reviews are partial and no exhaustive leaf-feature audit is complete; do not treat them as parity evidence. Approve target releases and complete pinned-source scenario mapping before implementation completeness or replacement claims.

## Feature completeness measurement

Use `docs/platform/feature-measurement-framework.md` as the mandatory measurement rubric for all 11 incumbent targets. It defines per-feature identifiers, status states, acceptance scenario requirements, inventory coverage, release gates, reporting, and duplicate-task prevention. A product replacement claim is valid only for an owner-approved boundary after every mandatory workflow passes that framework; a completed backlog task alone never proves product replacement.

## Evidence states

`VERIFIED` (reproduced with exact command/test/version), `DOCUMENTED` (primary-source statement, not independently tested), `INFERRED` (reasoned, not a fact), `UNKNOWN` (insufficient evidence), `NOT PRESENT IN INSPECTED SCOPE` (bounded source inspection), and `PARTIAL` (specific supported subset). A README is not implementation evidence. Every status change includes date and source.

## Open decisions / blockers

Product owner must decide: (a) initial workflow(s) and ordering across the 11 targets; (b) replacement vs integration boundary for each; (c) local/shared deployment profile and target platforms; (d) first user cohort, tenancy/identity model and permissions; (e) UI scope and Rust/UI boundary; (f) migration/cutover appetite; (g) measurable NFR/SLO and support model. Research owner must identify exact DeepSeek Harness and Orca product/version/components. These are explicit gates; this specification does not choose on the owner's behalf.

## References

- `docs/platform/product.md`, `architecture.md`, `security.md`, `quality.md`, `deployment.md`, `interfaces-data.md`, `agent-collaboration.md`, `automation.md`, `feature-gap-register.md`, `current-state-assessment.md`, `decisions.md`.
- Backlog and phase gates: `docs/platform/replacement-plan.md`, `docs/platform/replacement-backlog.md`.
