# Operations and reliability

**Status:** target operator contract. SLO values and supported scale must be set from measurements before publication.

## Health and observability

Expose liveness, readiness, version/capabilities and dependency health separately. Health endpoints return minimal, non-sensitive data and are restricted as appropriate. Use structured logs with timestamps, severity, component, operation/correlation ID, target and safe error category. Never log tokens, passwords, private keys, full sensitive payloads or secret-bearing URLs.

Metrics cover request volume/latency/error, job queue and failures, resource use, storage growth, database health, backup age/verification, update/migration state and module-specific signals. Define bounded label cardinality. Traces are opt-in/controlled and redact content. Provide a support bundle with preview, redaction and inclusion summary; collect only with operator approval.

## Reliability objectives

For each supported profile define availability/latency/throughput targets and measurement windows after load tests. Separate platform guarantees from user VPS/network/identity dependencies. Define graceful degradation when an optional module, model, SMTP, object store, or external integration is down. Core status, logs, backup and recovery should remain usable where possible.

Long-running operations have durable IDs, phase/progress, timeout and retry policy, idempotency, cancellation safety, and an inspectable terminal state. Retries use bounded backoff and avoid duplicate external side effects.

## Routine operations

- Monitor disk, memory, service readiness, certificate expiry, backup completion and restore test age.
- Review security updates and supported-version notices.
- Verify backups on schedule and conduct restore drills to an isolated target.
- Review accounts, roles, API tokens, integrations and audit logs.
- Rotate/revoke compromised or unused credentials; test recovery access.
- Maintain host OS, container runtime, firewall, DNS and TLS in the self-hosted responsibility boundary.
- Keep an inventory of instance ID, endpoint, versions, data volume locations, owners and backup destination, excluding secrets.

## On-call / incident runbooks

For service outage, disk exhaustion, failed migration, compromised credential, suspected data corruption, lost owner access, TLS expiry, failed backup and upgrade rollback, document: detection, impact assessment, safe containment, evidence preservation, commands, escalation, recovery, validation, communications and post-incident action. Do not recommend deleting indexes/databases unless the data classification and recovery path are verified.

## Configuration and change management

Changes are reviewable, versioned, validated before apply, and auditable. Secret rotation avoids printing old/new values. Service config reload behavior is explicit; restart-required fields are identified. Feature flags have owner, default, expiry/removal plan and audit. Environment overrides are enumerated, not silently accepted for arbitrary settings.

## Support boundaries

Core team can support documented artifacts and profiles. Self-hosting operators own host security, network, DNS, reverse proxy, storage reliability and backups. Community support does not imply an SLA. Provide a diagnostics checklist and supported versions; reject unsupported configuration with useful guidance rather than guessing.

## Operational acceptance

Test clean install, repeated install, restart, reboot, disk-full behavior, process crash, service dependency outage, TLS renewal, backup verification, isolated restore, upgrade success/failure, rollback/forward recovery, credential revocation, uninstall retention choices and support-bundle redaction on every supported profile/OS.
