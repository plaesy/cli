# System architecture

**Status:** target architecture; implementation choices require validation against the existing Rust CLI and repository dependencies. Do not interpret this as a claim that the services exist.

## Logical components

1. **`plaesy` CLI** — command parsing, local/remote target selection, installation orchestration, configuration, diagnostics, output rendering. Privileged tasks go through explicit adapters; CLI is not a permanent server process.
2. **Local supervisor/runtime** — owns local service lifecycle, health checks, process isolation, ports, logs and local data directories. Must support foreground/development mode and managed background mode where platform permits.
3. **Plaesy API service** — authenticated application API, version/capability endpoint, authorization, validation, audit emission, rate limiting and module routing. It is the supported remote interface; direct database access is not an API.
4. **Web client** — static/application frontend served locally or by the instance; browser session uses protected same-origin APIs where feasible.
5. **Desktop client** — packaged application that connects to an explicitly selected local or remote API. Local privileged operations are mediated by the CLI/runtime, not by arbitrary browser content.
6. **Domain modules** — bounded capabilities with declared API/event/schema versions, permissions, data ownership, health checks, migrations and backup participation. Existing memory engine is a current codebase component; whether its embedded/library or service shape is retained is a design review item.
7. **Persistence adapters** — development/local and production deployment stores, migrations, backups, encryption and restore. Product interfaces must abstract storage without pretending all backends have equal guarantees.
8. **Identity and policy** — local single-user identity boundary and remote instance identity/authn/authz; tenant/project isolation where multi-user mode is enabled.
9. **Extension/integration runtime** — separately sandboxed, permission-scoped integrations/plugins; registry and signed/versioned packages if adopted.

## Deployment topology

### Local

`plaesy CLI → local supervisor → API + Web + enabled modules → local persistent store`

Default bind is loopback only. Web may auto-open a browser, print the URL, or suppress opening. No LAN exposure unless the user explicitly configures it and is shown the security implications. Local mode must remain usable offline after required assets are installed.

### VPS / self-hosted

`plaesy CLI → authenticated encrypted management channel → instance API / deployment agent → services + persistent store`

The public edge terminates TLS and routes only documented endpoints. Administrative control is not exposed as an unauthenticated public port. Deployment credentials are short-lived where possible, scoped to one instance, and revocable. The VPS stores application data and secrets; the operator owns patching, domain, DNS, firewall, backup destination and recovery testing.

For initial supported profile, evaluate a single-host container stack (e.g. Docker Compose) with persistent volumes, health checks, resource limits, pinned images and documented backup/restore. This is a proposed default, not yet an approved technology decision. Architect interfaces so future managed providers, Kubernetes or multi-node deployment do not leak provider-specific behavior into user-facing concepts.

## Trust boundaries

- CLI process ↔ local runtime/service.
- User/browser ↔ Web/API and reverse proxy.
- CLI/operator workstation ↔ remote instance/control agent.
- API ↔ each domain module.
- Application ↔ database, object store, event broker and secret store.
- Trusted platform code ↔ third-party extensions/integrations.
- Organization/tenant ↔ another organization/tenant.

Every boundary requires authentication (where applicable), authorization, input validation, bounded resource use, audit coverage, and explicit data-flow documentation. Security details are in [Security and privacy](security.md).

## Control plane vs data plane

Control operations provision/configure and manage the instance. Application requests operate domain data. A single-host deployment may colocate them, but use separate authorization scopes and routes. Management channel failure must not corrupt or partially apply data-plane operations. CLI commands report whether an action is control-plane or application-level where relevant.

## Service and module contracts

Each service/module declares: stable identifier; semantic version; API/schema compatibility; required/optional dependencies; startup/readiness/liveness checks; configuration schema; required permissions; owned tables/files; migrations and rollback compatibility; backup/restore hooks; limits; metrics/log fields; shutdown behavior; and failure mode. Startup fails clearly on incompatible schema/API rather than silently downgrading.

Inter-module calls use documented APIs or events; avoid direct writes into another module's tables. Event delivery guarantees (at-most/at-least-once, ordering, replay and idempotency) must be specified per event family. No global transaction is assumed across independent services.

## Data and scaling model

Begin with a clearly bounded single-instance deployment and documented resource envelopes. Define the concurrency and consistency model for each data type. Scale vertically before enabling horizontal replication. If HA is later supported, specify leader/fencing, shared state, job ownership, failover, backup consistency, and split-brain behavior before advertising it. Do not infer that the current SQLite-backed memory engine is suitable for shared multi-user writes or distributed failover; benchmark and redesign/guard it if required.

## Failure isolation and lifecycle

A failed optional module must not render core status, diagnostics, backup, or uninstall unusable. Apply migrations before switching traffic; maintain a tested rollback or forward-fix path. Shutdown drains requests and jobs within a deadline. A crashed supervisor must detect stale state without deleting durable data. Partial provisioning leaves an inspectable state and supports retry or cleanup.

## Project collaboration and cross-session coordination

The target platform includes a project-scoped collaboration capability for agent identities and sessions, durable messages, task/dependency state, presence/leases, and change proposals. It must let independent sessions coordinate work while preserving session privacy. Sessions in the same checkout need project-authorized discovery of one another's declared task, scope, worktree/base revision and heartbeat so concurrent edits are visible before conflict; this is presence metadata, not transcript sharing or a write lock. See [Agent collaboration](agent-collaboration.md) for behavioral requirements and the two-agents/one-file conflict scenario. Treat collaboration as an explicit domain capability with authorization, persistence, API/events, audit and backup requirements—not as implicit access to transcripts or an unstructured shared folder.

Concurrent source edits support isolated worktrees by default and a separate, opt-in shared-worktree mode. Shared writes are permitted only through an enforced coordinator/OS boundary that atomically checks expected revisions and serializes managed writes; coordinator outage or unknown outcome fails closed for protected writes. If arbitrary editors/processes cannot be fenced, the product must state the residual race and cannot claim atomic conflict protection; any advisory mode requires explicit owner approval and visible warning. File activity is injected only after adapter acknowledgement at a safe point, otherwise explicitly queued. Same-file intent is visible; overlapping edits require explicit consent/policy and stale/same-line/semantic conflicts preserve proposals and block automatic application until authorized resolution. Leases are not write fences.

## Decisions to validate

API framework/protocol; process model; local supervisor per OS; application database(s); supported container runtime; remote control transport; tenancy model; service boundaries; plugin sandbox; event bus need; desktop packaging technology; frontend delivery; availability/scaling profile; collaboration service storage/delivery model; agent runtime integration; Git integration and merge authority. Record approved selections in architecture decision records and update this target document as decisions change.
