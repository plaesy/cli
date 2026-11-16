# Current-state assessment and validation log

> **Assessment snapshot:** 2026-11-16. This is a bounded, evidence-backed assessment of the checked-out repository and built CLI, not a security audit, complete workflow inventory, or proof of production readiness. Repository status contains extensive pre-existing modifications and untracked files; no unrelated source changes were reverted or edited for this assessment.

## Executive result

**Disposition: not ready to replace the named agent/harness platforms.** The checkout contains useful building blocks—memory/knowledge, task lifecycle, model delegation, an autonomous iteration loop, local decision-model support, and project-scoped tools—but its CLI help describes these as developer/repository and agent workflows. It does not expose a cohesive instance-management/runtime product with the full control, identity, deployment, collaboration, lifecycle, and migration surfaces specified in `README.md` and the other platform documents. Passing unit and integration tests verifies implemented code paths only; it does not establish parity with Hermes, Claude CLI, Codex, DeepSeek harness, or Paperclip.

## Evidence collected

| Check | Result | What it establishes | What it does not establish |
|---|---|---|---|
| `git status --short --branch` in `/home/ubuntu/cli` | `main...origin/main`; extensive modified/deleted/untracked paths, including `docs/platform/` | Workspace is not clean; ownership of prior edits cannot be inferred. | Which changes are safe to discard or whether they belong to this effort. |
| `cargo test --locked --no-run` | Passed; built library, binary and integration-test executables. | Current checkout compiles test targets. | Release packaging, supported OS builds, deployment or runtime behavior. |
| `cargo test --locked loop_run` | 30 passed, 0 failed. | Existing autonomous-loop unit tests cover selected completion, pause, measurement, receipt, state and stop-hook cases. | Durable scheduler, policy enforcement, service supervision, multi-agent delivery, or external side-effect recovery. |
| `cargo test --locked` | 603 unit tests passed; integration suites passed (crash recovery 1, explain 4, format 3, integration 3, metadata 9, relevance 3); 1 installer E2E ignored because it writes the current user's persistent PATH; 11 doctests ignored. | The current Rust test suites pass in this environment. | Untested capabilities, OS matrix, adversarial security, live service/deployment, or replacement acceptance. |
| `cargo run --locked -- --help` | Help says “Plaesy CLI for memory and agent workflows”; commands include `advisor`, `analyze`, `decisions`, `delegate`, `init`, `loop`, `memory`, `reach`, `tasks`, and related utilities. | The exposed command surface in this build; `advisor`, `decisions`, and `init` are present. | Whether an undocumented, unexercised product surface exists elsewhere. |
| `target/debug/plaesy loop --help` | Description: “Run iterations until a Stop or Completion condition holds”; bare `plaesy loop`, no documented flags in help. | Loop is exposed as an iteration command. | That it is a general unattended agent runtime or durable event scheduler. |
| `target/debug/plaesy memory --help` | Includes local index/search/serve/schedule/remember/update/archive/forget and related operations. | Existing memory CLI surface. | Platform-wide identity, tenant policy, transcript lifecycle, or autonomous extraction behavior. |
| `plaesy files` implementation, tests and disposable CLI simulation | Local per-path claims support claim/list/who/release, TTL expiry, ownership checks, concurrency serialization, and symlink escape rejection; five focused tests pass. Scratch-project runs verified claim visibility, conflicting claim rejection, release, TTL expiry, and persistence across separate CLI invocations. An out-of-band shell write changed a claimed file while the claim remained visible. A small CLI fix ensures `files list` also displays the optional task label. | Cooperating CLI users can inspect claims and detect conflicts; expiration/release/persisted reads worked synthetically, and task context is now shown in list output. | Full session presence, declared scope/worktree/base revision, event streaming, delivery acknowledgements, fencing, or protection from writers that ignore claims. Claim visibility is not session discovery. |
| Current quality rerun | `cargo test --locked`: 603 unit tests pass; listed integration suites pass. `cargo clippy --locked --all-targets -- -D warnings` and `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` pass after two lint fixes. `cargo fmt --all -- --check` still fails with repository-wide formatting differences and was not auto-applied. Markdown validation (61 files), repo link validation and `git diff --check` pass. | These exact checks completed in this checkout after the latest code edits. | Formatter cleanliness, cross-platform behavior, and the safe simulations below are not established. |

## Gap register

Severity is relative to the stated replacement direction, not a claim of defect in the existing CLI's narrower purpose.

| Priority | Gap | Evidence / impact | Closure evidence required |
|---|---|---|---|
| P0 | Replacement scope and acceptance are not enumerated per incumbent workflow. | `decisions.md` leaves first slice and migration acceptance open; no user workflow inventory was supplied in this run. | Owner-approved workflow inventory, observable parity criteria, import/export needs, cutover and rollback plan for each named incumbent. |
| P0 | Product direction is not an implemented integrated product surface. | Built help describes memory and agent workflows, while target docs specify managed local/remote instances, Desktop/Web, identity, deployment, recovery and administration. | Working end-to-end slice on a declared profile; generated help/API matches implementation; acceptance evidence against target requirements. |
| P0 | No verified native autonomous runtime / adapter conformance for replacement use. | Loop tests exist, but no runtime service, supported adapter matrix, queue/ack integration or live conformance simulation was exercised. | Durable run lifecycle, restart tests, adapter contracts and conformance per supported runtime/provider; honestly disclose unsupported ones. |
| P0 | Independence boundary remains ambiguous. | Owner wants no dependency on incumbent harness products; external model API exclusion and local-model quality envelope remain open in `decisions.md`. | Owner decision on provider/model boundary plus tested supported model/hardware/language/cost envelope, or explicit external-provider dependency disclosure. |
| P0 | Identity, tenant hierarchy, authorization and local security model unresolved. | Explicit implementation gates in `decisions.md` and `README.md`; no platform threat-model tests run here. | Approved principal/resource model; fail-closed authorization matrix; loopback/browser/IPC boundary and tenant isolation tests. |
| P1 | Autonomous memory and execution policy are not demonstrated end-to-end. | `automation.md` is normative proposed design; existing memory tools and loop do not alone prove policy-scoped unattended capture, conflict resolution, retention, emergency stop, or auditable recovery. | End-to-end policy tests for allowed/denied/sensitive/conflicting updates, provenance, undo, pause/stop, budget and crash recovery. |
| P1 | Deployment, backup, restore, upgrade and remote lifecycle lack exercised product evidence. | Target specs require these journeys; no deployment or recovery operation was run. | Disposable-instance E2E/fault tests for install, upgrade, backup/restore, rollback, remote access, partial failure and data-retention semantics. |
| P1 | Cross-session visibility exists only as advisory file claims, not as session discovery or safe shared writes. | `plaesy files claim/list/who/release` and focused tests are implemented. Claims identify a path, agent label, optional task, lease ID and expiry; they do not publish session/worktree/base revision/heartbeat or change events. Synthetic CLI runs verified claim visibility, conflicting claim rejection, release and TTL expiry. In an out-of-band write simulation, a plain shell write changed a claimed file while `plaesy files who` still reported the claim; the claim is not a filesystem lock or write fence. | Two CLI sessions in one project/worktree discover authorized task/scope/base revision/heartbeat without transcript access; adapter delivery acknowledgements; deterministic concurrency/fencing/conflict/out-of-band writer tests and documented residual risks. |
| P1 | Cross-platform packaging, security supply chain and usability evidence absent from this run. | Only Linux local tests were run; installer E2E was explicitly ignored. | Supported OS matrix, artifact signing/provenance, SBOM, security review, accessibility and tested install/uninstall flows. |
| P2 | Public compatibility/support and operations envelope remains undecided. | SLO/RPO/RTO, retention, release policy, API stability and ownership remain open in platform docs. | Named operational owner, supported versions, compatibility policy, measured limits, restore drills and maintenance lifecycle. |

## Safe simulation plan

Run simulations only in disposable directories, fake adapters, and synthetic data. Do not point destructive, concurrency, recovery, or migration tests at the user's real vault, credentials, production services, or shared working tree.

| Scenario | Required injected condition | Pass assertion |
|---|---|---|
| Policy denial | Agent requests a scope/action that is not granted. | No side effect; explicit denial and audit evidence. |
| Untrusted prompt/content | Synthetic input requests secret disclosure or permission elevation. | Content cannot change policy; no secret in output/log/memory. |
| Memory conflict | Candidate contradicts owner-verified synthetic fact. | Existing fact preserved; conflict surfaced, no silent overwrite. |
| Duplicate delivery | Same event delivered repeatedly, including after restart. | At most one committed effect; durable dedupe/audit state. |
| Crash boundaries | Terminate before and after durable state/effect/event commits. | Recoverable state is explicit; no false success or duplicate non-idempotent effect. |
| Stop and budgets | Pause/stop during work; exceed each configured limit. | New work blocked; in-flight state truthful; next over-budget action prevented. |
| Secret redaction | Synthetic canary secret in prompts, tool results, errors and exports. | No canary in memory, audit/log, support bundle or export. |
| Concurrent writers | Two writes from same base; coordinator outage; out-of-band write at commit. | CAS/fencing rejects stale write or preserves both proposals; no unsupported atomicity claim. |
| Backup/restore | Corrupt/incomplete backup, rotated/missing key, interrupted restore. | Non-destructive failure; integrity/ownership verified; recovery status and limitations shown. |
| Cross-session discovery | Two sessions share one synthetic project/worktree; one is terminated abruptly; one is offline. | Authorized peer task/scope/base revision/heartbeat is visible without transcripts; stale presence expires; offline state is explicit and never treated as a write lock. |
| Adapter delivery | Safe point, busy session, idle, terminated, timeout, lost acknowledgement, revoked permission. | Queue/acknowledgement accurately reported; authorization rechecked; retry deduplicated. |
| Migration/cutover | Import synthetic incumbent data; interrupt; roll back. | No source deletion before verification; counts/integrity reconcile; documented rollback works. |

These are acceptance scenarios, not completed simulations. The current test run above did not perform them.

## Recommended implementation sequence

1. **Close owner decisions before dependent design is frozen:** first supported user workflow/profile; model/API independence boundary; single-user versus multi-user identity/ownership; migration workflow and cutover constraints. Keep affected shared/remote designs conditional until approved.
2. **Establish a verified baseline:** preserve this checkout; classify existing changes with the owner; generate a requirement-to-code/test/evidence traceability matrix. Do not use a clean-tree assumption or overwrite prior work.
3. **Build one vertical slice:** select an owner-approved workflow; implement local supervised execution with durable run state, explicit policy, bounded budgets, cancellation, structured audit, restart recovery and synthetic fault tests. Keep external side effects behind a fake adapter until contracts and authorization are tested.
4. **Add real adapter and data lifecycle only after gates:** implement one selected model/runtime integration, user-authorized tools, adapter conformance, memory provenance/conflict/undo, export/backup/restore and a documented provider boundary.
5. **Expand replacement coverage incrementally:** repeat workflow parity tests per incumbent capability; require data import checks, parallel-run period and rehearsed rollback before any cutover. Do not disable incumbent tooling based on the present evidence.
6. **Release only against verified gates:** implement identity/tenant and remote security boundaries before team/remote exposure; run deployment, supply-chain, cross-platform, accessibility and failure-injection checks for each advertised profile.

## Decision required to proceed beyond assessment

Select the **first user workflow and deployment profile** to replace (for example: local unattended project task execution, or memory/knowledge workflow). Also clarify whether “no dependency” excludes hosted model APIs or specifically agent/harness products. Without these choices, implementing one end-to-end product path would require inventing product scope. Safe repository test execution and this evidence-backed gap analysis can continue without them; production cutover and completion claims cannot.
