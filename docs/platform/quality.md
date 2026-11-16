# Quality and delivery

**Status:** release criteria for the target platform. This file does not claim that these tests currently exist or pass.

## Verification layers

- **Unit:** CLI parsing, config precedence/validation/redaction, authz policy, schemas, migrations, retry/idempotency, state transitions, archive/path safety.
- **Component:** API and each module with real persistence adapters; local runtime/supervisor; Desktop IPC; Web security/session behavior; provider adapters.
- **Contract:** CLI JSON output, API schemas, events, plugin and integration protocols, supported version negotiation.
- **Integration:** CLI ↔ local instance; CLI ↔ remote deploy adapter; Web/Desktop ↔ local and remote API; backup/restore across modules.
- **End-to-end:** fresh install through use, update, backup, restore, diagnostics, export, uninstall; include failure injection and noninteractive CI.
- **Security:** threat-model validation, authz matrix, tenant isolation, SSRF/CSRF/CORS, injection, session/revocation, secret leakage, supply chain, sandbox and fuzz tests.
- **Compatibility:** supported Linux/macOS/Windows CLI and Desktop where offered; supported VPS/container profile; old/new client-server matrix; upgrade paths from documented supported versions.
- **Performance/reliability:** representative load, resource ceilings, long-running jobs, slow storage, network interruption, crash/restart, disk exhaustion and soak tests.
- **Accessibility/localization:** automated and manual Web/Desktop evaluation; keyboard/screen-reader tests; localization-ready content.

## Release gates

1. Reproducible build, signed/checksummed artifacts and provenance; SBOM and license review.
2. All relevant automated tests pass on the release commit and supported OS matrix.
3. No unresolved critical/high security findings without documented owner-approved exception and mitigation.
4. Upgrade, backup/restore, rollback or forward-recovery, and uninstall-retention journeys pass on each advertised deployment profile.
5. API/schema compatibility and CLI command docs match generated/help output; all examples are executed or clearly marked illustrative.
6. Telemetry, network access, secret handling, permissions and destructive effects reviewed.
7. Known limits, support profile, migration notes, recovery steps and release notes published.
8. Rollout/rollback owner and incident contact are defined; canary/gradual deployment where supported.

## Acceptance test scenarios

- Install pinned binary and verify signature/checksum; reinstall safely; verify PATH and state behavior.
- Run local foreground and managed modes; ensure loopback default, restart persistence and explicit remote binding.
- Deploy to a clean VPS; repeat deploy idempotently; simulate failures during each phase; verify inspect/retry/cleanup.
- Interrupt first-run bootstrap or lose the only owner; verify approved recovery authority, one-time credentials, audit, and no identity-provider outage bypass.
- Lose SSH/provider control access or change DNS/TLS; verify host identity before credentials, safe recovery/rotation, and truthful status when the channel is unavailable.
- Restore with missing/rotated keys or mismatched identity provider; verify non-destructive failure/recovery, integrity, and no unintended ownership reassignment.
- Delete while replicas are offline or legal holds/backups exist; verify pending/retained-copy status, deletion evidence, and no false global-erasure claim.
- Remove remote deployment resources while external backups remain; verify target/resource/retention summary, no implicit purge, and idempotent recovery after timeout.
- Check identity, project, collaboration, audit, keys, diagnostics and exports against the approved retention/deletion matrix, including replicas, caches, logs and backup expiry.
- Verify first-release capabilities and explicit exclusions match help/docs; no unsupported or full-completeness claim is implied.
- Interrupt provisioning or lose the management channel; verify inspectable partial state, host revalidation, safe retry, and no duplicate provider resources.

- Confirm authentication, role boundaries, cross-tenant isolation, token expiration/revocation and audit events.
- Upgrade from every supported prior version with populated data; test migration interruption and recovery.
- Verify backups, restore to isolated instance, compare data integrity and measured RPO/RTO.
- Simulate network loss, DNS/TLS failure, database unavailable, disk full, process crash, expired credentials and incompatible client/server.
- Ensure diagnostics/logs/JSON output/exports never reveal synthetic test credentials or sensitive fixture content.
- Uninstall CLI, Desktop, local runtime, remote service and data as distinct cases; verify documented persistence/deletion behavior.
- Validate CLI commands are idempotent or clearly reject unsafe repeats; JSON error codes and exit codes remain stable.
- Start two agent sessions on distinct tasks in one project; verify durable addressed messages, active-context injection confirmation (or explicit queued delivery), offline recovery, permission isolation and no private transcript leakage.
- In a shared worktree, verify same-file intent is visible and overlapping edits require approved consent; stale/same-line/semantic conflicts preserve both proposals and block automatic apply. Verify coordinator outage fails closed for protected writes, and no atomicity claim is made for unfenced external writers.
- Crash an agent holding a file lease; verify expiry/recovery preserves its unintegrated change and an authorized override is audited.

## Pre-implementation decision and evidence gates

Before implementation freezes shared contracts, record owner-approved decisions for: product/tenant hierarchy and ownership transfer; minimum identity/authz model; local loopback/browser/IPC authentication boundary; first supported deployment/persistence/remote-management profile and its user/concurrency/data envelope; encryption key custody/rotation/recovery; extension/plugin first-release policy; and accessible language/baseline requirements. Open items must have an owner and explicit assumption or remain release-blocking; they must not silently become implementation defaults.

Define a common durable operation resource for deploy, upgrade, restore, migrations and destructive mutations: operation/idempotency ID, state transitions, commit point, retry/reconciliation behavior, cancellation boundary, progress, error schema and recovery. Contract tests must simulate a successful server-side commit followed by client timeout and safe retry.

Before claiming supported operation, name the operational owner and support boundary; measure and approve service objectives, supported scale, maintenance/security response expectations, and tested RPO/RTO for each supported profile. Define encryption/key loss and rotation outcomes across backups and replicas.

## Shared-worktree collaboration fault matrix

Before release, automate the failure simulations in `agent-collaboration.md`, including simultaneous same-base writes, stale retries/idempotency, safe rebase and overlapping edit conflicts, external writer race at apply boundary, coordinator crash before/after replacement and event persistence, lease expiry with stale fencing token, event loss/reorder/reconnect, active-turn queue/interrupt/idle/termination behavior, path/symlink/worktree replacement, shared Git index and branch mutation collisions, permission revocation with cached filesystem credentials, service outage, secret redaction and prompt injection. Use deterministic barriers and fault injection; assert resulting file hashes, preserved patch evidence, delivery states, audit events, staged paths, operation reconciliation and absence of unauthorized data. Do not treat a happy-path two-agent demo as concurrency proof.

For local security, test hostile local process/browser-origin access to loopback endpoints and IPC, session/CSRF/origin protections, and local privilege boundaries. For backup/security, test key loss/rotation, restore integrity and compromise response. For extensions, test denied capabilities and sandbox absence according to the chosen release policy. For accessibility, include manual keyboard/screen-reader checks on critical journeys and contractually defined language coverage.

Design simulations are not runtime evidence. Each release-blocking criterion needs an executable test or explicitly recorded manual evidence before the capability is declared supported.

## Evaluation and evidence

Track test result, code revision, environment, dependencies, command, dataset/fixture version, and known exclusions. Separate automated coverage from manual review. Performance/availability claims need reproducible methodology and resource profile. Do not publish benchmark figures from unlike workloads as direct comparisons. Document customer-impacting limitations and unresolved decisions.

## Rollout lifecycle

Development → internal integration → security review → release candidate → canary → stable release → supported maintenance → deprecation → end of support. Define rollback eligibility and data migration constraints before stable release. Emergency security releases may shorten normal stages but retain artifact integrity, regression checks and post-release review.
