# Deployment and lifecycle

**Status:** proposed operational contract. Docker Compose is a candidate default for single-VPS self-hosting, not an approved/implemented method. Remote Web instance lifecycle is exposed conceptually as `plaesy web deploy plan|apply|status|update|rollback|remove`, keeping it under the Web product namespace. Validate current `plaesy` installation and package capabilities before implementation.

## Deployment profiles

| Profile | Purpose | Default exposure | Durability and ownership |
|---|---|---|---|
| Local foreground | Development, evaluation, one-off use | loopback; process lifetime | user controls data; no implied service manager |
| Local managed | Daily use, autostart if selected | loopback | persistent local state; CLI owns lifecycle |
| Single VPS self-hosted | Remote/team access on one server | HTTPS through configured edge; admin endpoints restricted | operator owns host, network, storage, secrets and backups |
| HA / multi-node | Higher availability/scale | defined ingress and internal network | future profile only after consistency/failover validation |

Each profile documents supported OS/architecture, CPU/RAM/disk envelope, dependencies, ports, data location, firewall policy, upgrades, backup, restore, recovery, limitations and support status. Do not imply HA based on container orchestration alone.

## Provisioning flow

1. Preflight: authenticate to target; verify OS/architecture and runtime; check disk/RAM/ports/DNS/TLS; detect existing state/version; validate permissions and connectivity.
2. Plan: show exact resources, versions/images, endpoints, data paths, firewall changes, identities, secrets handling, costs where known, and destructive effects. `--dry-run` shows this without mutation.
3. Bootstrap: create least-privileged service identity; establish trust/host identity and secure initial admin; create durable data/secret/config locations with restricted permissions.
4. Install: deploy pinned artifacts; apply schema migrations; configure TLS/reverse proxy policy; health/readiness probes; start services.
5. Verify: wait boundedly for health; verify API version, auth, storage and Web; report endpoint and recovery/backup state.
6. Record: save deployment ID, versions, endpoint, non-secret config references, timestamps, current operation and audit evidence locally; remote instance is the source for its live status.

Partial failures remain inspectable and safely retryable. Cleanup is explicit and distinguishes generated artifacts from data.

## Upgrade and rollback

- Support pinned CLI, desktop, server, module and schema versions; publish support window and compatibility matrix.
- Preflight disk capacity, target compatibility, migration path, backup freshness/verification, active jobs, and maintenance requirements.
- Upgrade artifacts immutably; verify checksums/signatures; capture backup/snapshot; stage new version; migrate; health-check; switch traffic only after readiness.
- Define old-client/new-server and new-client/old-server compatibility range. API/schema changes are additive where feasible; use expand-migrate-contract for data changes.
- Never assume binary rollback reverses a database migration. For irreversible migrations, block downgrade and provide tested forward recovery or restore procedure.
- On failure, preserve logs and operation state, stop traffic if integrity is at risk, and report safe next steps. Automatic rollback is allowed only if data compatibility is proven.
- Security updates have severity-based timelines and an emergency mitigation procedure.

## Backup and recovery

Back up all durable state: application databases, uploaded objects/files, encryption/key metadata needed to decrypt, instance configuration (with secrets handled separately), identity/audit data subject to policy, and module-owned state. Exclude rebuildable caches only when verified. Define consistent snapshot coordination for databases and active jobs.

- Backup is encrypted in transit and at rest, integrity checked, versioned, access controlled, and independent of the VPS failure domain.
- Secret/key recovery is explicit; a backup whose keys are lost is not a usable backup.
- Retention and deletion reflect operator policy and legal obligations; disclose Git/Syncthing/provider snapshots that may retain deleted copies.
- Restore into a quarantine/test target first; verify checksums, schema, identity, module health, and sample data before switching users.
- Document RPO/RTO per profile only after a tested exercise establishes them. Run scheduled restore drills and record evidence.
- `plaesy backup verify` checks structure/integrity, not full service recoverability; label the distinction.

## Scaling and availability

For each supported size tier specify tested concurrency, request latency, job throughput, storage growth, and module limits. Horizontal scaling requires shared durable state, distributed job coordination, session strategy, idempotency, readiness and failover tests. Document single points of failure and maintenance behavior. No SLA is promised for community self-hosting.

## Networking and TLS

Document inbound/outbound ports, DNS, certificate issuance/renewal, reverse proxy headers, allowed origins, trust proxy configuration, websocket needs, egress restrictions, SSH/control channel, firewall rules and renewal failure alerts. Bind management ports privately; do not advise unrestricted firewall opening. Local mode remains loopback unless explicitly changed.

## Uninstall and data lifecycle

Treat these as distinct operations with explicit target identity: remove the CLI binary; remove the desktop client; stop a local runtime; remove remote deployment resources; revoke deployment credentials/identity; delete durable instance data; and expire retained backups according to policy. `plaesy web deploy remove` is a proposed deployment-resource operation, not a synonym for data purge; exact command names and provider capabilities remain pending approval. Before confirmation, show the selected instance/provider, resources affected, retained data and backups, and unavailable/revoked management channels. Removing remote resources must not silently delete separately stored backups or imply that identity revocation erased data. Every operation states what persists, what is erased, what cannot be guaranteed erased, and the export/recovery path. Purge is explicit, scoped, confirmed, idempotent where possible, and auditable when the service remains available; if control access is lost, require provider-native recovery rather than claiming completion.

## Provider-neutral provisioning interface

Represent providers as adapters implementing preflight, provision, inspect, update, backup, restore, logs, stop, remove. Expose capability flags; never silently emulate unsupported behavior. Keep infrastructure credentials outside project files and redact them from logs. Provider-specific instructions live under a provider guide; core lifecycle semantics remain stable.
