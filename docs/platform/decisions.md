# Open decisions and decision log

No items below should be treated as approved solely because they are documented. The product owner and engineering owner should record consequential choices with rationale, alternatives, consequences, date, and supersession history before implementation.

## Product and governance

1. **Platform scope:** Which concrete business capabilities belong in Plaesy Platform beyond the current memory/knowledge engine? Define module boundaries and measurable outcomes.
2. **Tenant and ownership model (implementation gate):** choose and document one hierarchy (single-user instance or organization/workspace/project), resource ownership/transfer, principal identities, and minimum authorization rules together. This blocks shared/remote schema, API and collaboration implementation; local-only work may proceed only if it does not assume or expose the unresolved model.
3. **First-release scope:** product owner selects the first deliverable and explicit exclusions before acceptance baselines are frozen; target-platform breadth is not a release commitment.
4. **Licensing and edition:** open-source/community/enterprise split, contribution policy, support model and feature availability.
5. **Telemetry and hosted services:** default telemetry (recommended: none/content-free opt-in), update check behavior, hosted registry/identity services, offline guarantees.
6. **Supported profile:** exact OS, CPU architecture, VPS distributions and support window.

## Architecture

1. **API framework/protocol:** HTTP/JSON, gRPC or other; schema format and compatibility guarantees.
2. **Local runtime/supervisor:** foreground-only initially or managed background service; per-OS service manager approach and privilege boundaries.
3. **Web/Desktop stack:** packaging, frontend delivery, Desktop framework, signing/notarization and update channels.
4. **Persistence:** database choice per profile, object/blob storage, search/index backend, transaction boundaries and migration tooling.
5. **Current memory engine integration:** embedded module/process, API service, or separately managed local daemon. Validate single-user SQLite constraints against desired concurrency and remote tenancy.
6. **Remote control:** direct authenticated API, SSH-based provisioning adapter, remote agent, or provider API; define trust bootstrap and credential rotation.
7. **Deployment packaging:** approve or reject Docker Compose as the first-class single-VPS profile; supported alternatives and provider matrix.
8. **Extensions:** plugin registry, package signing, sandbox standard, permission manifest and review/governance model.
9. **Multi-tenancy and scaling:** resolve consistently with the tenant/ownership implementation gate above; do not freeze shared-instance schema or cross-project behavior until the hierarchy is approved.

## Security and operations

1. **Identity and access control (implementation gate):** choose identity authority and bootstrap/recovery model, principal types and minimum resource authorization together with the tenant/ownership hierarchy. Specify MFA/SSO/SCIM scope if applicable; do not infer it from account identifiers.
2. **Encryption and keys:** at-rest coverage, key storage/rotation/recovery, external KMS support and backup key custody.
3. **Retention/deletion:** audit retention, user data export/deletion, backups/versioned copies and erasure statements.
4. **SLOs and support:** supported scale, service objectives, severity response, security update cadence and maintenance policy.
5. **Backup service:** included tooling only vs supported external destinations; consistency hooks, retention and restore target.

## CLI contract

1. **Command grammar:** implementation gate for a stable public contract. Choose and document one collaboration namespace (`plaesy project ...` or `plaesy collab ...`) and align it with existing commands; keep remote Web provisioning under `plaesy web deploy ...` if approved. Confirm exact verbs, target selection, help and error/exit behavior before freezing CLI/API compatibility.
2. **Config precedence:** names and precedence for flags, environment, profiles, config files, secrets and project-level config.
3. **Compatibility:** API/CLI/server support window, JSON schema stability, deprecation period and migration strategy.

## Agent collaboration

1. **Scope and storage:** decide whether cross-session agent collaboration is local-only, self-hosted shared service, or both.
2. **Agent runtime integration:** clarify whether Plaesy runs agents or coordinates external agent runtimes.
3. **Shared state:** approve task, message, session-summary and presence/lease resources and their access rules.
4. **Concurrent writes (implementation gate):** name the engineering owner and define the enforced write boundary before implementing shared mode: which agent/editor/tool writers are mediated, how expected-revision/CAS is atomic, how out-of-band writers are fenced or detected, and what fail-closed behavior applies if the coordinator is unavailable. Shared mode must be disabled or explicitly degraded with a visible warning whenever its approved guarantee cannot be enforced; no silent last-writer-wins. Preserve external-writer races as an explicit residual risk if they cannot be fenced.
5. **Delivery/runtime (release gate):** select supported adapters and verify safe interrupt points, queued delivery, acknowledgement, idle/terminated session handling, deduplication, authorization recheck and revocation behavior. Do not promise live injection until conformance tests pass for each supported adapter/profile.
6. **Git and leases (implementation gate):** choose serialization/fencing for shared index, HEAD/branch/worktree operations, monotonic lease generations, expiry/recovery, offline behavior and authorized override. Do not discard unintegrated edits on expiry/crash.
7. **Conflict UX (implementation gate):** explicitly shared same-file work requires registered intent and visible notification; same-line/semantic conflicts preserve both proposals and block automatic application. State whether user confirmation is required before overlapping edits; default to confirmation unless a project owner authorizes the shared-file collaboration policy. Isolated worktrees remain the default otherwise.
8. **Deterministic test set (implementation gate):** engineering owner must select and automate at least same-base concurrent writes, overlapping patch, coordinator-outage/fail-closed, external-writer race at apply, crash between write and event, stale lease fencing, event loss/reconnect/idempotent retry, Git index collision, permission revocation with queued delivery, and safe-point vs queued/idle/terminated runtime behavior. Record hashes, delivery states, audit evidence and residual limitations. Convert remaining failure simulations to release tests before advertising support.
9. **Privacy and retention:** define what session summaries/artifacts are shared, message retention, project ACLs, notifications and deletion.
10. **Automation authority:** decide if Plaesy can create branches/commits/merge, and which roles must approve integration.

## Direction recorded from product owner (2026-11-16)

The owner wants Plaesy to be better than, and ultimately able to replace, Hermes Agent, Claude Code, Codex, Paperclip, Orca, and similar agent/coding/orchestration products. The owner explicitly permits Plaesy to use Claude Code, Codex, and comparable agent harnesses as execution backends. This establishes product direction and a provider/harness-integration permission, not an implementation claim, approved release scope, or permission to terminate existing services before replacement capability and migration are verified.

1. **Product ownership boundary:** Plaesy should own the user experience/control plane, orchestration and durable workflow state, policy and approvals, project/task context, memory/provenance, audit, and backend selection. It may delegate execution to external harnesses (including Claude Code and Codex) or model APIs through replaceable adapters. Backend use must be explicit, observable, permission-scoped, and not silently treated as Plaesy-native capability. Plaesy should remain useful with supported alternatives; avoid hardwiring core state or user data to one backend.
2. **Backend support strategy:** decide which backends and versions are supported first, their authentication/credential boundaries, tool/filesystem permissions, cancellation and event semantics, cost/usage reporting, and degraded/offline behavior. Verify each adapter against its real contract; do not claim equivalent behavior across backends without conformance evidence.
3. **Replacement and migration acceptance:** inventory the user's real workflows for each named tool, define observable success/quality/reliability/cost criteria, data/config import needs, cutover order, parallel-run period, rollback, and a no-loss migration policy. Do not recommend disabling an incumbent before its critical workflows pass in Plaesy.
4. **First usable slice:** select one end-to-end workflow and target profile for an initial non-production vertical slice; name a primary user, permitted action boundary, and success evidence. Use synthetic data and keep production access/cutover gated.

## Recommended decision sequence

Resolve product/module scope and tenant/ownership hierarchy with identity/authorization together first. These are implementation gates for shared/remote APIs and data contracts. Next approve enforced shared-write boundary and fail-closed/degraded mode, overlap/confirmation semantics, Git/lease protocol, one CLI namespace and the deterministic pre-implementation test set. Those gates block implementation of the affected shared collaboration, schema/API and stable CLI contracts; isolated local-only work may proceed only if it does not bake in unresolved assumptions. Runtime adapter conformance, full fault-injection coverage, recovery drills, accessibility and data lifecycle evidence are release gates for the capabilities they cover; live injection/shared collaboration must not be advertised before required conformance evidence exists. Every gate needs named accountable product/engineering/security owner, chosen option, rationale, affected components, test/evidence, and approval date. Record decisions in dated ADRs and link them here. Until authorized owners approve, unresolved decisions remain open; recommendations here do not silently select policy on their behalf.

## Design-code IR (ADR, proposed)

Status: IR ADR (0001) accepted 2026-11-18 under the owner's delegation. Implementation of `high_feature-implement-design-code-ir-round-trip` may start. Spec R15 and task `high_chore-decide-design-code-ir-schema-and-target-adr` govern this record.

- **IR schema version:** `ir/1` as a proposal. The version is bumped on any breaking change to the node set or lowering contract.
- **Node set:** frame, auto-layout constraints, component and variant, style and token, vector and image asset, text, and stable node ID. Every node carries a source map entry to its code span or design path.
- **Reuse:** the token model in `src/designlint/` (`tokens.rs`, `colour.rs`, `fonts.rs`, `images.rs`) is the starting point. Decide per type whether it is reused, extended, or replaced.
- **Lowerings:** design → IR, IR → design, IR → code per target, and code → IR. Each direction lists what is reported as lost. No construct is dropped silently.
- **Edit policy:** owner decision open. Options are one-way regeneration, guarded merge with source-map preservation, or manual edits that are never parsed back.
- **Targets:** owner decision open. No design tool or code framework is approved yet.
- **Fidelity threshold:** not set. It is UNKNOWN until owner approval, and no round-trip result may be reported as passing before then.
- **Rust:** the IR and converters are core logic and must be Rust, per R14.

## Owner decisions for the first replacement slice (2026-11-19)

Source: product owner, answered in the Nimbalyst decision prompt on 2026-11-19. These are authorized decisions, not recommendations. Recommendations offered in the prompt were not all selected.

1. **First workflow (spec open question 1):** agent execution and learning workflow. Links: R3, R4, R9. Task `high_feature-deliver-selected-agent-execution-and-learning-workflow` is the workflow task. Its child spec is still required (see the vertical-slice task).
2. **Deployment profile (spec open question 2):** shared self-hosted. Links: R6, R7 (shared messaging is in scope). Local-first and offline requirements from `docs/platform/product.md` still apply. Remote trust boundaries must be defined explicitly. Lane C (shared messaging) is therefore in scope for the first slice.
3. **Runtime strategy (spec open question 3):** deterministic fake adapter first, then a bounded native runtime. External harness orchestration (Claude Code, Codex) is not enabled in the first slice. Links: R4, R9. Tasks: `high_feature-define-runtime-adapter-and-fake-conformance-suite`, `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting`.
4. **Identity hierarchy (spec open question 4):** single organization. Agent principals are scoped per project, with grants and revocation. Links: R5. Multi-tenant hierarchy is out of scope for the first slice.

**Still UNKNOWN (owner and review trigger to be named by the owner):** target cohort and success outcome; incumbent baseline workflow and pinned version; NFR thresholds (performance, reliability, security, retention); migration and cutover appetite, rollback authority; support model; exact DeepSeek Harness and Orca identity and version. Do not infer thresholds for these.

## Owner decision status: remaining items recorded as UNKNOWN (2026-11-19)

These items are not decided. Each has an accountable owner and a review trigger. No threshold or disposition is inferred.

| Item | Status | Owner | Review trigger |
| --- | --- | --- | --- |
| Target cohort and success outcome | UNKNOWN | product owner | before the first implementation claim |
| Incumbent baseline product and pinned version for agent execution | UNKNOWN | product owner with research owner | after `high_chore-recover-and-compare-incumbent-research` |
| Replacement, integration, or exclusion for each listed product: Hermes Agent, Paperclip, DeepSeek Harness, Nimbalyst, Orca, Craft Anything, FigCraft, DeckCraft, WordCraft, Herald-OS, OpenProject | UNKNOWN until each audit task closes | product owner | when each `high_chore-audit-*` task closes |
| Measurable NFRs: performance, reliability, security, accessibility, retention | UNKNOWN | product and operations owner | before the NFR task closes |
| Migration and cutover appetite, rollback authority | UNKNOWN | product owner | before R13 gate work |
| Support model | UNKNOWN | product and operations owner | before the first release |
| Exact DeepSeek Harness and Orca identity and version | UNKNOWN | research owner | when the audit tasks for those products start |
| UI and Rust boundary beyond the decided Dioxus web surface | UNKNOWN | product owner with architecture owner | before the desktop or mobile surface is planned |

Decided on 2026-11-19: first workflow, deployment profile, runtime strategy, identity direction (section above).

## Comparison identity evidence (2026-11-19, evidence only, not a decision)

Gathered for spec open question "exact DeepSeek Harness and Orca product/version". The owner still picks the comparison target.

- **Orca:** the GitHub Releases page for `stablyai/orca` (fetched 2026-11-19) lists desktop tags up to `v1.4.263` (18 Nov 2026). An earlier secondary source cited `v1.4.180` (Aug 2026). That is superseded. Name clash: `araa47/orca` is a different CLI project. Source: https://github.com/stablyai/orca/releases
- **DeepSeek Harness:** a news article (RuntimeWire, 14 Aug 2026) calls it "Harness v0.1". A separate article describes it as an open-source, plugin-based coding agent. The MIT license and the `dsh` / `@deepseek-ai/dsh` package name appear only in a search summary and are not confirmed by the articles fetched. Sources: https://runtimewire.com/article/deepseek-open-sources-agent-harness-v4-pro-api-price-hike and https://www.mindstudio.ai/blog/deepseek-harness-coding-agent/
- **Next step (owner):** confirm the exact repository and version to compare. Until then the item stays UNKNOWN.

## Default technical decisions applied (best practice, 2026-11-19, changeable by owner)

The owner asked for best-practice defaults for the open technical choices. These defaults are engineering choices, not owner approvals. Business items (cohort, success outcome, baseline, NFR thresholds, cutover, support, DeepSeek/Orca) are not covered here and stay UNKNOWN.

- **Native runtime:** bounded child-process adapter. Only programs on an explicit allowlist run. The environment is cleared except PATH, SYSTEMROOT, TEMP, TMP. stdin is closed. stdout and stderr are capped at 64 KiB each. Timeout and cancellation kill the process. External effects are off by default. This is a host process, not a sandbox. Container or VM isolation is required before real tools run.
- **Key distribution:** one HMAC key per deployment and key id, stored in the OS credential store under one service name (`KeyringKeySource`). Keys never appear in envelopes, logs, or the repository. Rotation adds a new key id and retires the old one.
- **Envelope freshness:** every envelope carries a send time and a random nonce. The receiver refuses nonces seen in the window and envelopes older or newer than 300 seconds. 300 seconds is a proposed default.
- **Transport:** HTTPS only, TLS 1.2 or later through the existing rustls stack, with the HMAC envelope on top as defense in depth. Not yet implemented. This is the next engineering step.
