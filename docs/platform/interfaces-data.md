# Interfaces and data contracts

**Status:** requirements; concrete endpoint paths, schemas and storage technologies are pending architecture decisions.

## API contract

The API is versioned and documented with machine-readable schemas. Every endpoint states authentication, authorization scope, request/response schema, validation, errors, limits, idempotency, audit effect, and compatibility policy. The API exposes server version and supported capabilities. Remote clients negotiate capabilities before mutation and fail safely on incompatibility.

Use TLS for remote APIs. Define request size, timeouts, rate limits, pagination, concurrency, cancellation and retry semantics. Mutations support idempotency or explicit conflict detection. Errors use stable machine-readable codes plus safe explanatory text and correlation IDs. No API leaks filesystem paths, internal hostnames, secret values or unbounded stack traces to ordinary users.

CLI output, Web/desktop client operations, and automation use supported interfaces. Direct SQL/shared-filesystem edits are not supported operational interfaces. Internal APIs may differ, but contracts crossing a component boundary are versioned and tested.

## Events and integrations

Document event name/schema/version, producer, consumer, sensitivity, delivery/order/replay guarantees, retention, idempotency and authorization. Webhooks are signed, replay-resistant, allowlisted where feasible, retried with bounded policy, and include delivery status. OAuth integrations request minimum scopes, store tokens protected, support revocation and expose data flows. Plugin protocol/version and permission manifest are explicit.

## Data classification and ownership

For each domain table/object specify owner module, classification, tenant scope, retention, export/delete behavior, backup inclusion, encryption, audit requirement, migration policy and whether content is user-provided, derived or cache. Avoid multiple authoritative stores. The durable source of truth and rebuildable indexes/caches are clearly distinguished.

## Persistence and migrations

Schema migrations are numbered, checksummed, ordered, observable and tested against real supported upgrade paths. Detect partial migration and concurrent migrators. Use advisory locks or an equivalent single-migrator guarantee. Migration backups and compatibility must be defined. Downgrade is supported only when verified safe; otherwise block and document forward-recovery/restore.

Backups capture consistent cross-module state. Import validates schema/version, paths, checksums, identity mapping and secret exclusion before applying. Export is portable and documents unsupported data. Deletion handles references, tombstones, retention and backup copies honestly.

## Current memory subsystem boundary

The existing CLI documents an offline-first memory engine with Markdown as source of truth, a local SQLite index, HNSW snapshot cache, citations, MCP/JSON service interfaces, and single-user/local design constraints. Its current API and storage docs remain the authority for that subsystem. Do not assume its local database, MCP server, or single-process locking model constitutes the target Platform API, multi-tenant persistence, or remote deployment control plane. Any integration must define data ownership, permission mapping, lifecycle, remote access, backup and concurrency compatibility.

## Project collaboration contracts

Treat project collaboration data as a distinct domain with project-scoped authorization and auditable ownership. Define versioned resources/contracts for agent identities, session references and explicitly shared summaries, tasks/dependencies, durable messages/threads, presence leases, file/resource declarations, change proposals and conflict records. Each resource documents visibility, owner, retention, export/delete behavior, idempotency, timestamps/order guarantees and sensitivity. Do not expose private session transcripts through project/task APIs by default.

Message APIs distinguish accepted/queued/delivered/read/failed states and use stable message IDs for retries and deduplication. For active agent sessions, delivery also distinguishes `injected` (runtime confirmed it entered the active context/interrupt channel) from `queued` (will be delivered before a later turn); a sent message is not proof of receipt. Offline clients synchronize from a cursor and reconcile tombstones/revoked access. Ordering is scoped (for example per thread), not globally promised unless deliberately implemented. Presence and file leases are ephemeral TTL records; they are not durable work ownership or correctness guarantees.

File activity events identify project and stable repository/worktree instance, agent/session, task, canonical authorized path, event kind (`intent`, `editing`, `saved`, `conflict`, `resolved`), project/worktree-scoped opaque revision, monotonic per-worktree sequence, event ID, timestamp and optional bounded/redacted content only under explicit policy. Metadata-only is the default. Events are hints, not the source of truth: after reconnect, sequence gaps, duplicates or watcher uncertainty, clients reconcile against current file revision and Git state. For shared worktrees, patch application carries expected revision, idempotency/operation ID, and current fencing generation. The coordinator validates path and revision, atomically applies under its enforced write boundary or returns a conflict, and reports resulting revision/event. Define durable intent/result recovery for crashes and uncertain outcomes; never blindly replay. If arbitrary external editors/processes can bypass the coordinator, explicitly limit guarantees and require conflict/review. Git index/checkout/rebase/commit/worktree operations are separate shared resources and serialized or assigned to one integrator. Specify cursor replay, event retention/deletion, auth revocation and error behavior in the contract.

Change proposal contracts include project/repository identity, source branch/worktree, base commit/hash, changed paths, patch/commit reference, validation evidence and review/integration status. Applying a stale proposal must return an explicit conflict response with changed-path context; never silently overwrite. Integrations into a protected branch require an authorized actor/policy, and the resulting commit/revision is recorded against task and proposal IDs. Back up durable messages/tasks/change records; presence leases may be reconstructed.

## Compatibility and deprecation

Define supported CLI/API/server/client version matrix. Additive compatible changes are preferred. Breaking changes require versioning, upgrade path, migration tooling, release notes and support window. Deprecated fields/endpoints emit actionable notices with a sunset date. Maintain schema contract tests and fixtures; do not rely on undocumented JSON incidental behavior.

## Portability and export

Provide documented formats for configuration, audit, domain data, backups and module data. Include format version, encoding, checksums and import validation. Keep machine-specific paths, host credentials, private keys, access tokens and local caches out of portable exports by default. Provide explicit opt-in only when a secret export is safely encrypted and justified.
