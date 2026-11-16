# Security and privacy

**Status:** normative target controls. This is not a security certification or evidence that current code satisfies these controls.

## Threat model

Protect user and organizational data, credentials, agent/integration permissions, instance integrity and availability against: unauthenticated Internet actors; compromised user/operator accounts; malicious or compromised dependencies/extensions; SSRF and malicious URLs; injection and deserialization flaws; cross-tenant access; stolen endpoint credentials; host compromise; supply-chain compromise; accidental operator error; backup loss; exposed logs/support bundles; resource exhaustion and data poisoning.

Document trust assumptions for local user, OS, VPS provider, reverse proxy, DNS/TLS authority, identity provider, artifact registry and extension author. A self-hosted instance does not defend against a fully compromised host administrator.

## Identity and authorization

- Local mode uses OS identity/filesystem permissions; loopback binding alone is not treated as authentication against other local processes.
- Remote Web/API requires authenticated users and server-side authorization on every protected operation. Default-deny; least privilege; role and resource scope documented.
- Isolate organization/workspace data at query, storage and cache layers; test for IDOR, confused deputy and cross-tenant leakage.
- Bootstrap credentials are one-time, short-lived, safely delivered, and invalidated after setup. No shipped shared default passwords.
- Support credential/session/token revocation, expiry, rotation and audit; protect recovery and owner-transfer paths.
- SSO/OIDC/SAML, MFA, SCIM and enterprise identity options require separate threat/design review; don't imply support until tested.

## Secrets and cryptography

Never put secrets in argv, source, logs, error messages, backups without encryption, generated docs or support bundles. Use OS secret storage locally and protected secret providers/server files with restrictive permissions remotely. Redact before output. Validate TLS certificates and remote identity; never disable verification automatically to “fix” connections.

Specify encryption in transit and at rest, algorithms, key creation/storage/rotation/backup/recovery, data-key boundaries, cryptographic erasure limits and dependency lifecycle. Avoid custom cryptography. Treat secret deletion from active state separately from copies in backups, snapshots, logs and version history.

## Web and API defenses

Use TLS for remote access; secure session cookies and CSRF strategy; validated origins/hosts; content security policy and secure headers; strict input/schema validation; output encoding; rate and body limits; pagination bounds; safe file handling; SSRF restrictions; timeouts; idempotency for mutations; request authorization and audit. Protect internal metadata endpoints and cloud instance metadata from SSRF. Avoid CORS wildcard with credentials. Do not expose admin/management endpoints publicly by default.

## CLI and local runtime

Verify signed/checksummed releases and provenance where available; permissions are least-privileged; quote shell inputs and avoid shell interpolation; restrict local IPC to authorized user; protect config/state permissions; defend against symlink/path traversal and unsafe archive extraction; avoid destructive cleanup outside Plaesy-owned directories. Diagnostic commands redact by default. Update checks disclose network access and have an opt-out policy.

## Extensions and integrations

Treat plugins/tools as untrusted unless explicitly approved. Define permissions/capabilities, sandboxing, filesystem/network/process access, resource limits, package identity, signature/provenance, review, update and revocation policy. Secrets are scoped to integration and not exposed to unrelated modules. Tool outputs are untrusted content; defend against prompt injection and data exfiltration. If a secure sandbox is unavailable, constrain extension execution to an explicit trusted-admin opt-in and clearly state the risk.

## Agent collaboration security

Project messages, tasks, session references, shared summaries, presence leases and change proposals are separate authorization resources. Apply project membership checks to every read/write and recheck authorization when queued messages are delivered or queued patches are integrated; revoking access must stop future synchronization and invalidate cached access. Never expose a private session transcript to another agent by default. Treat message/task text and patch contents as untrusted instructions/data, not as authorization or trusted policy. Scope an agent's tools, repository paths, branch and project credentials to its assigned task. Log assignment, access changes, lease overrides, change review/integration and deletion without recording unnecessary private content. Enforce size/rate limits and retention/deletion policy for durable messages and artifacts.

## Data governance and lifecycle

Inventory collected, generated, indexed, cached, logged and transmitted data by component and purpose. The matrix below is a required design template; values marked “decision required” must be approved before the corresponding release. Define data classification, ownership, residency and telemetry controls. Data is not used for model training/analytics without an explicit policy and consent basis. Deletion completion must disclose asynchronous propagation, retained backups, legal holds and copies outside Plaesy’s control; never claim immediate global erasure without evidence.

| Data class | System of record / replicas | Retention and legal hold | Delete propagation and evidence | Export | Backup/snapshot expiry |
|---|---|---|---|---|---|
| Identity, sessions and authorization | Identity provider, instance DB, token/session caches, audit (decision required) | Policy and legal hold: decision required | Revoke credentials, invalidate sessions/caches, record completion and residual copies | Account/ownership export scope: decision required | Backup expiry bound: decision required |
| Project content, memory and attachments | Project storage, search index, object store, client caches (decision required) | Project/user policy: decision required | Delete source and derived indexes/replicas; report offline replicas and retry status | Data format and scope: decision required | Backup/version expiry bound: decision required |
| Messages, agent summaries, task artifacts and patches | Collaboration store, delivery queue, notification/cache copies (decision required) | Per-class TTL and legal hold: decision required | Cancel queued delivery; purge active copies and notify disconnected replicas on reconnect | Include message/artifact payloads by policy: decision required | Backup expiry bound: decision required |
| Security and administrative audit | Append-only audit store and exported SIEM (decision required) | Minimum/maximum retention and legal hold: decision required | Preserve where required; record deletion exception and scope | Redacted audit export policy: decision required | Backup expiry bound: decision required |
| Secrets, encryption keys and provider credentials | OS secret store/KMS, deployment manager, backup key envelope (decision required) | Rotation/revocation and recovery: decision required | Revoke/rotate active credentials; classify undecryptable or still-decryptable backups | Secret values excluded; metadata export policy: decision required | Key escrow and encrypted-backup expiry: decision required |
| Diagnostics, telemetry and support bundles | Local logs, optional telemetry destination, downloaded support archives (decision required) | Collection opt-in, TTL and support-copy policy: decision required | Delete/expire per destination; disclose copies already downloaded or externally retained | User/admin-accessible bundle scope: decision required | Snapshot/log rotation bounds: decision required |

Deletion workflows must expose durable operation status, scope, failures, retries and completion evidence; account for disconnected clients, replicas, indexes, exports, caches, notification providers, audit retention and backups. Define backup expiry limits, legal-hold authority, export contents and deletion status before production deployment.

## Audit and incident response

Audit security-sensitive events: authentication/revocation; permission and policy change; user/admin changes; integration/secret changes; export/restore; deployment/update; module enablement; destructive data actions. Include actor, action, resource, outcome, timestamp and correlation ID; exclude secret values and unnecessary content. Protect audit integrity and document retention/export.

Publish vulnerability reporting, severity classification, patch process, dependency scanning, SBOM/artifact provenance policy, incident containment, credential rotation, customer/operator notification criteria, and recovery steps. A security incident playbook must cover suspected compromise, token revocation, forensic preservation, backup integrity and safe restoration.

## Security verification gates

Threat-model each trust-boundary change. Run static analysis, dependency/license and secret scans, fuzz/property tests for parsers/protocols, authz matrix and cross-tenant tests, web security checks, installer/archive tests, TLS validation tests, backup-secret review, and manual security review before remote-access releases. Resolve or formally accept findings; no severity-critical/high unresolved issue without explicit owner, rationale and mitigation.

## Privacy and telemetry

Document exact telemetry fields, destination, retention, opt-out and offline behavior. Default to no content telemetry. On-prem installations should operate without contacting a Plaesy-hosted endpoint except clearly disclosed optional update/artifact checks. Record outbound network destinations and allow administrators to restrict egress.
