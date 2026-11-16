# Agent collaboration and shared work

**Status: partial local prototype implemented; shared service and remote security remain unimplemented.** `plaesy agent-messages` provides a local SQLite-backed durable direct queue with idempotent sends, pending inbox reads, and explicit acknowledgments (`src/agent_messages.rs`, `src/cli/agent_messages.rs`). Focused unit tests passed for persistence across reopen, idempotency/conflict, ack routing, input limits, and two concurrent SQLite connections retrying the same message ID; a CLI smoke test passed. The concurrency test covers threads/connections in one process, not separate processes. On Unix, newly created database files use mode `0600`; an existing database with group/other permission bits is rejected rather than silently chmodded, while restrictive permissions are preserved. This does not secure parent directories, SQLite sidecars, backups, or non-Unix platforms. This is not yet an authenticated multi-agent service: CLI `from`/`agent` values are caller-supplied, there are no per-agent credentials, HTTP API/client, remote TLS listener, retention/dead-letter policy, or multi-process/multi-device deployment verification. Therefore it does not satisfy the approved local + self-hosted release goal.

## Goals

- Let an agent in one session send durable, addressed messages to another agent/session working in the same project.
- Coordinate task assignment, ownership, dependencies, handoffs, review and completion.
- Let agents in the same project/worktree send live coordination messages that are injected into the recipient agent's active session context, not just left in an inbox.
- Publish file activity (intent, editing, saved revision/diff, blocked/conflict) to other sessions working in that shared worktree, so they can adapt while work is in progress.
- Make concurrent edits visible and prevent silent overwrites when agents touch the same file.
- Preserve useful history and provenance across session restarts without exposing unrelated private conversations.
- Support both local collaboration and collaboration through an explicitly configured shared/remote Plaesy instance.

## Concepts

- **Project**: collaboration boundary and repository/workspace identity.
- **Agent identity**: stable project-level identity plus runtime/session instance; secrets and private chat are not shared implicitly.
- **Session**: one active or historical work context, with declared project and task scope.
- **Task**: owned work item with status, assignee, dependencies, file/resource scope, branch/worktree and acceptance criteria.
- **Message**: durable project-scoped communication, addressed to an agent/session/task or a channel, with author, timestamp, thread, references and visibility.
- **Presence/lease**: expiring indication that an agent is active or holds a resource; not proof that work is correct.
- **Change proposal**: a commit, patch, or workspace change submitted for integration; distinct from a message and from task completion.
- **Conflict**: concurrent incompatible modifications or resource ownership collision that requires reconciliation.

A session transcript is private to its owner unless that owner explicitly shares a bounded artifact or summary. Project messages and task updates are shared according to project ACLs. No agent receives all other sessions' transcripts by default.

## CLI interaction model (proposed)

Keep collaboration commands under a distinct namespace so `plaesy web` remains the interface launcher and deployment commands remain under Web or deployment design decisions. Candidate grammar:

```text
plaesy project status
plaesy project agents list
plaesy project tasks list|show|create|assign|claim|start|block|unblock|complete
plaesy project messages send|inbox|thread|subscribe
plaesy project sessions list|show|share|watch
plaesy project files who|watch|lock|unlock|conflicts
plaesy project changes propose|review|integrate
# Example options: --project <id> --worktree <path> --watch-files <path-glob> --inject|--queue
# Exact command names/options remain proposals, not implemented functionality.
```

Exact names are open for approval. Until the CLI owner approves one namespace, treat both forms as examples only and do not publish a stable command contract; choose either `plaesy project ...` or `plaesy collab ...`, not both. Commands must always identify the project and permission boundary. Support human and versioned JSON output; avoid leaking private session content in JSON, logs or notifications.

### Active-session discovery requirement

Before starting work in a shared checkout, each Plaesy CLI session should publish a minimal project-scoped activity record: opaque session ID, agent identity, task/card, declared paths or scope, branch/worktree identity, base revision, last heartbeat, and state (`active`, `idle`, `blocked`, `completed`, `disconnected`). The proposed `plaesy project sessions list` should show authorized peers' activity and recent file-intent/change events, with timestamps and stale-presence indication. Presence is a best-effort signal, not proof a session is running or a write lock; on startup a client reconciles with the coordinator, and stale/expired sessions must never authorize writes. Do not publish transcripts or arbitrary process command lines by default. The actual CLI has no verified cross-session discovery or presence implementation today; local task claims and advisory locks are narrower mechanisms and do not reveal what another session is doing.

At minimum, instrument each CLI invocation with an opt-in project/worktree session registration and heartbeat lifecycle, then surface a read-only session list before implementing messages or shared writes. Use durable task ownership plus ephemeral, expiring presence; distinguish session-reported activity from Git-observed modifications, and provide explicit offline/uncoordinated status. Any local store must be scoped to the canonical worktree/project and handle concurrent session updates without lost writes.

Acceptance: launch two CLI sessions against the same project/worktree; session B can identify A's task, declared paths, base revision and last heartbeat without reading A's transcript; terminate A abruptly and verify its presence expires while task/file evidence remains inspectable; run sessions offline and verify the CLI does not imply live coordination.

This is a requirement/design addition, not an implementation or claim that the CLI can currently introspect other sessions.

## Two agents, different tasks, same file

Example: Agent A is implementing feature X; Agent B is fixing a bug. Both are intentionally assigned the same project, branch and worktree and both need to edit `src/foo.rs` to save time.

### Shared-worktree cooperative mode

This is an explicitly opted-in collaboration mode, distinct from isolated worktrees. Plaesy does not force separate branches when the user chooses a shared working directory. The safe default is isolated worktrees. Shared mode MUST remain disabled unless the deployment can enforce its declared write boundary; if only a subset of writers can be mediated, label that scope and residual race clearly, and require explicit owner approval for degraded/advisory mode. A file watcher is not a write fence.

1. Each agent registers the task and file(s) it intends to edit. A per-worktree coordinator (or configured project service) tracks sessions and subscriptions. Only writes submitted through the coordinator are covered by its CAS/serialization guarantee; other processes are outside the guarantee unless OS-level enforcement fences them.
2. For a managed write, coordinator serializes per-file operations or atomically checks expected revision/hash at the final apply boundary, verifies stable repository/worktree identity and canonical path, and rejects stale writes. Persist intent/idempotency before apply and reconcile actual bytes after uncertain outcomes; never silently use last-writer-wins.
3. On file activity/save, publish authorized path, revision/hash, timestamp, task/agent and bounded diff/summary under project policy. Reauthorize at delivery and write time; do not expose private transcript or unrelated content.
4. Delivery is `injected` only after runtime acknowledgment at an adapter-declared safe point. Otherwise queue for the next safe model/tool turn; idle/terminated sessions stay queued/failed with explicit status. No claim of interruption without runtime confirmation.
5. Before editing after another agent's save, reread the latest file and apply a patch against its revision. Non-overlapping patch contexts may be attempted only after revalidation and tests/review appropriate to the change; textually separate hunks do not prove semantic independence.
6. Same-file edits require registered intent and visible notification. Overlapping declarations trigger warning; an agent may proceed only under explicit shared-file consent/policy. Same-line, stale-base, rename/delete or semantic conflicts preserve both proposals and block automatic application until the authorized integrator resolves them. Require user confirmation by default unless the project owner has approved a policy for shared-file collaboration; never auto-merge on agent confidence alone.
7. Coordinator outage, unknown outcome or unmediated writer race fails closed for protected shared writes. An explicitly approved degraded mode may permit edits only with visible warning, no atomicity/conflict-protection claim, and a mandatory reread/review before integration. Preserve edit history and task provenance.

Agents can send explicit direct messages like “I’m changing the parser signature; please avoid lines X/Y and re-read before your patch.” Such a message is a coordination signal, not a reliable lock or proof that the recipient has seen it. The UI/CLI must show sent, injected, queued, acknowledged and failed states separately. Do not claim an agent was interrupted or saw a message without runtime confirmation.

### Isolated-worktree mode

When users have not requested shared editing, isolated worktrees/branches remain the safer default. Each change is a patch/commit with base revision, changed paths, tests and task ID. Integration checks whether the base is current and whether overlapping hunks changed. On conflict Plaesy preserves both changes and asks a human or designated integration agent to resolve/rebase/merge and rerun tests. Integration into a protected branch requires policy authorization; completion follows verified integration, not merely local file modification.

A user who explicitly requests a shared worktree opts into the collaboration workflow, but does not bypass the enforced-write gate above. If the coordinator cannot mediate/fence all writer classes covered by the selected guarantee, protected shared writes fail closed; any explicitly approved advisory mode must warn that external writers can race. Path-level leases are not a write fence and cannot replace Git conflict checks or review. Do not promise automatic semantic merge for arbitrary code.

## Task ownership and coordination

Tasks have immutable ID, project ID, title/description, acceptance criteria, status, owner, dependencies, declared file/resource scope, branch/worktree, timestamps, and linked messages/change proposals. State transitions are validated and auditable. Only one agent is primary owner at a time; collaborators can be named separately. Reassignment records a handoff summary and remaining work.

Task assignment is not a guarantee of an agent's capability or availability. Dependencies prevent start until prerequisites are complete unless explicitly waived with recorded rationale. Completion requires artifacts/evidence and required review status according to policy.

## Messaging and sessions

Messages support direct agent/session delivery, task threads, and project channels. Delivery semantics distinguish queued, delivered, read, and failed. Offline recipients receive inbox messages when next connected. Support reply threads, mentions, attachments/patch references, and task/change links. Define retention, deletion, export, moderation, notification controls, rate limits and anti-spam policy.

Agents send explicit work summaries across sessions: goal, decisions, files/paths, branch/base revision, commands/tests actually run, unresolved blockers and next action. Plaesy may provide bounded project memory, but must cite provenance and distinguish facts, proposals and stale information. Cross-session context retrieval is ACL-filtered and scoped to the selected project/task.

## Collaboration service architecture

- **Local single-user:** local coordination store and filesystem/Git state; no assumption of multi-host shared state.
- **Shared project:** authenticated collaboration API/message store, task/state store, presence/lease service, and change integration boundary. These may be colocated initially but have explicit contracts.
- Durable messages/tasks require persistence; ephemeral presence/leases are TTL-bound. Synchronization must tolerate reconnects and duplicate delivery using message IDs/idempotency.
- Authorize each project, task, message, session artifact, file lease, branch and integration action. Cross-project data is private by default.
- Audit assignment, permission change, message visibility changes, lease override, change integration and deletion without logging private content unnecessarily.
- Avoid making a hosted Plaesy control plane mandatory. A shared on-prem/VPS instance can own collaboration state; offline local work can be queued and reconciled.

## Conflict handling and shared write boundary

- Never silently overwrite another agent's edits or treat a stale patch as current.
- Bind each write to stable repository and worktree instance identities plus base commit and per-file revision. Validate canonical paths beneath an approved repository root; reject traversal/symlink escapes and revalidate repository identity on each operation. Define behavior for linked worktrees, submodules, case/Unicode normalization and replaced worktree paths.
- Define the revision token and atomicity boundary. Apply against the exact observed revision. If changed, reject or rebase/recompute against current bytes with validated patch context; do not infer semantic safety from non-overlapping hunks alone. Atomic replace must be protected by a per-file coordinator lock/CAS and detect out-of-band writes immediately before commit. If arbitrary filesystem writers cannot be fenced, clearly limit guarantees and require conflict/review.
- Route cooperating writes through one per-worktree coordinator. Give each operation an idempotency ID; persist intent and expected revision, atomically apply, then record resulting revision/event in a recoverable protocol. On restart or uncertain outcome, reconcile actual content before retry/reporting; never blindly replay. Preserve original content/patch/conflict evidence through disk-full, permission loss and process/service failure.
- Distinguish registered Plaesy agent writes from arbitrary editor, formatter, codegen or external process writes. A file watcher is not a write fence; detect out-of-band changes immediately before apply. Never imply atomic protection against writers outside the enforced boundary.
- Treat Git index and branch/HEAD as separate shared mutable resources. Serialize staging/index rewrites, checkout/reset/clean, rebase, commit and worktree operations; otherwise designate one integrator or use an isolated index. Verify exact staged paths/tree/commit and authorship before committing. Never stash/reset/discard another agent's uncommitted changes automatically.
- Provide safe options: reread and reapply against latest content, rebase/regenerate patch, manual merge, split tasks, or designate an integrator. Non-overlapping text hunks can still have semantic dependencies; require relevant tests/review.
- Preserve original patches and conflict evidence until explicit resolution. Do not auto-merge on agent confidence alone.
- Leases require monotonically increasing fencing generations at every coordinated write; reject stale holders after expiry/override. Define TTL, renewal/heartbeat, clock assumptions, disconnect grace, override authority and audit. Expiry never means uncommitted work is safe to discard.
- Filesystem watchers may coalesce, duplicate, reorder or miss events. Treat events as hints with monotonic per-worktree sequence/cursor; consumers reconcile against current file hash and Git state. Resync after reconnect/gap; deduplicate by event ID.
- A shared working directory requires explicit user consent and warning about non-cooperating writers. State supported guarantees separately for managed agent edits, other Plaesy agents, and arbitrary external processes.

## Failure simulations (design review; not executed runtime tests)

| Scenario | Required behavior | Pass condition |
|---|---|---|
| A and B read the same file hash; A saves before B applies | B's expected-hash write is rejected as stale; B receives A's new revision/diff event and rereads | A's content remains intact; B can rebase/reapply; no lost update |
| A and B submit non-overlapping patches concurrently | Coordinator serializes or compare-and-swaps; second patch is validated against the resulting current revision | Both edits exist and each event reports its actual resulting revision |
| A and B patch the same lines | Both patch proposals and base revisions are preserved; mark conflict; notify/inject or queue both agents | No implicit winner; authorized resolution yields a new revision and tests run |
| Message arrives while recipient is in an uninterruptible model turn | Mark queued; deliver before next tool/model turn; require acknowledgement if supported | UI never reports injected/read until runtime confirms it |
| Watcher drops, reorders, or duplicates events during reconnect | Detect cursor gap; dedupe event IDs; resync against current file hash and Git state | Agents converge to authoritative file state before further patching |
| Agent crashes while editing or holding a lease | Lease expires; partial/untracked changes remain inspectable; another agent is warned and rereads | Recovery does not erase content; override is authorized/audited |
| Runtime accepts an interrupt request while a destructive tool call is in flight | Inject only at adapter-declared safe points; otherwise queue, expose state and require freshness check before the next tool/write | No silent mutation of in-flight context or assumption that an interrupt stopped the tool |
| Message is delivered to a session that remains idle or terminates | Keep delivery queued/failed with reason, expiry and retry/escalation semantics; never label read/injected without acknowledgment | Sender can inspect, cancel or reassign; no false receipt claim |
| Two agents concurrently stage/commit in shared Git index | Serialize repository operations or use explicitly isolated index/integrator; verify exact paths and tree before commit | Neither agent's staged content is lost or accidentally included |
| External writer modifies target in the gap between revision check and atomic replace | Use an enforced OS-level write boundary or reject guarantee as advisory; re-read and compare immediately at commit boundary | No atomicity claim unless every writer in scope is fenced |
| Revoked agent retains same-OS filesystem credentials | Revoke scoped coordinator credentials/mounts where available; disclose residual same-principal access | Spec does not represent API revocation as filesystem isolation |
| Write completes but coordinator crashes before recording the result event | Reconcile persisted intent/idempotency ID against actual file revision on restart | Retry does not duplicate edit; status/event reflects actual outcome |
| Path alias, symlink, Unicode/case variant or worktree replacement changes target | Canonicalize and revalidate path under approved root plus repository/worktree identity per operation | No write escapes approved repository or silently targets a replacement checkout |
| User revokes an agent while events/messages are queued | Recheck ACL at delivery and write time; cancel/discard unauthorized queued operations and invalidate access | Revoked identity receives no future data and cannot apply edits |
| Diff/message contains secret or prompt injection | Apply redaction and size policy; mark content untrusted; do not execute embedded directions as authority | Other agents receive only authorized, bounded content; audit avoids secret payloads |
| Shared coordinator/service is unavailable | Stop coordinated writes or fall back only to an explicit user-approved uncoordinated mode with warning | Never silently claim conflict protection while coordination is down |
| Client times out after deploy/migration/delete may have committed | Query durable operation ID/status before retry; apply defined idempotency and cancellation boundary | No duplicate side effects and no false report of completion |
| Encrypted backup is restored after key loss, rotation or compromise | Follow defined recovery and affected-backup policy; verify integrity before restoring into service | Restore succeeds with verified data or fails non-destructively with actionable recovery guidance |
| Project ownership/permissions change while a session has cached authorization | Reauthorize at fetch/delivery/write/integration boundaries and invalidate stale credentials/cache | Revoked identity cannot access future protected state |
| Extension requests unsupported capabilities or runs without an approved sandbox | Deny by default unless approved policy explicitly permits and constrains it | No ambient secrets/filesystem/process access; policy is release-tested |
| Incident or restore exceeds advertised recovery/support targets | Follow named runbook/owner; preserve evidence and report actual recovery state | Supported SLO/RPO/RTO and notification commitments are measured and tested |
| Critical flow fails keyboard/screen-reader evaluation | Execute selected manual accessibility and language-scope checks | Release blocked on agreed critical barriers; no unsupported conformance claim |
| Agent/tool output supplies a malicious filename or attachment payload | Keep payload bounded/untrusted and structurally separate from privileged runtime instructions | No path escape, tool permission increase, or secret disclosure |

## Security and privacy

- Enforce per-agent/session principals and least-privilege capabilities at collaboration API, filesystem/IPC, and Git-credential boundaries. Reauthorize on every fetch, event delivery, patch application and integration; revoke cached credentials, mounts and local artifacts where enforceable. If agents share an OS principal and isolation cannot be enforced, explicitly label shared-worktree mode as trusted-agent operation; do not claim task path scope is a security boundary.
- Keep credentials out of events, diffs and shared summaries. Use project/task-scoped short-lived credentials where possible; explain residual access after revocation.
- Default file activity to metadata only (authorized path, opaque project/worktree-scoped revision token, event kind, task/session IDs). Share snippets only under explicit policy and recipient authorization. Apply classification/redaction before persistence, fan-out, logs and notifications; fail closed if required classification fails. Define retention/deletion for payloads, retries, caches and notification copies.
- Validate canonical paths beneath an approved root; reject traversal/symlink escapes and revalidate repository/worktree instance identity at each operation. Invalidate subscriptions and leases if a path is replaced or repository identity changes.
- Treat messages, filenames, diffs, summaries and attachments as untrusted data. Runtime adapters must preserve author/source/scope boundaries and structurally encode them as untrusted context, never concatenate them as privileged system/developer instructions. Collaboration payloads cannot grant permissions, bypass approvals or authorize secret access.
- Private chat, credentials, environment values and unrelated user data are never injected into project memory. Users can inspect which summaries/artifacts were shared; audit metadata without logging private payloads unnecessarily.

## Acceptance criteria

1. Two agents in distinct sessions can discover one shared project and communicate through durable addressed messages; offline delivery recovers without duplicates.
2. Agents can create/assign/depend on tasks, claim ownership, hand off with summary, and record verifiable completion evidence.
3. Isolated worktrees are the default. In opted-in shared mode, each same-file intent is visible; overlapping declarations require explicit shared-file consent/policy, and same-line/semantic conflicts block automatic application. A visible warning alone does not authorize writes.
4. Two branches editing the same file cannot silently overwrite one another; integration detects stale base/conflict and preserves both changes. Shared-worktree guarantees apply only to the enforced writer boundary; coordinator outage fails closed unless an owner-approved, clearly labeled advisory mode is selected.
5. Explicit merge/rebase resolution is auditable, and required tests run before integration is marked complete.
6. Access checks prevent an agent in another project or unauthorized session from reading tasks/messages/artifacts.
7. Presence leases expire after disconnect and can be recovered/overridden without deleting work.
8. Local-only mode works without hosted services; shared mode documents connectivity requirements and data residency.
9. Message/task retention, export, deletion and notifications are configurable and documented.
10. Tests cover reconnect, duplicate delivery, agent crash, lease expiry, permission revocation, stale patch, same-file overlap, rename/delete conflict and rollback.

## Decisions required

- Shared project coordination: local-first single-machine only, shared self-hosted server, or both.
- Whether task board, messaging, sessions, and code-change review are one module or separate bounded capabilities.
- Whether Plaesy runs agents itself or coordinates externally hosted agent runtimes only.
- Session artifact sharing defaults and retention.
- Git worktree/branch strategy, explicit shared-worktree mode, and whether Plaesy is allowed to create branches, commits or merge changes automatically.
- Runtime support for true mid-turn interrupt/context injection versus queue-before-next-turn; delivery acknowledgement and fallback behavior.
- Lease granularity, per-file write serialization/compare-and-swap mechanism, timeout, conflict policy and authorized override roles.
- Message delivery, notification, encryption, search and retention requirements.
- Whether remote collaboration uses WebSocket/SSE/polling or another transport; transport is an implementation choice after service contract is defined.

These reasoned cases are design review, not runtime evidence. Convert selected release-blocking rows into deterministic fault-injection and contract tests before implementation/release; keep targets that require product-owner choices explicitly open.
