# Product requirements

## Vision

Plaesy is a self-hostable, CLI-managed Platform Enterprise System: a coherent environment for organizing and operating enterprise workflows, knowledge, agents, integrations, and user-facing experiences. The platform is accessible through one `plaesy` executable and, when installed, Desktop and Web interfaces. This vision is broader than the repository's currently documented memory-engine scope; capabilities not present today remain proposals.

## Users and outcomes

- **Individual developer/operator:** install locally, use core functions without a hosted dependency, inspect data and logs, and upgrade safely.
- **Team member:** access authorized organization workspaces from Web/Desktop, collaborate without sharing personal credentials, and understand ownership and provenance.
- **Administrator/operator:** provision and govern instances, identities, integrations, policies, audit, backups, upgrades, and recovery.
- **Developer/integrator:** use documented APIs/SDKs, plugins, CLI automation, and stable compatibility contracts.

Success means users can discover, install, configure, connect, administer, update, back up, restore, and remove Plaesy through documented interfaces, while preserving data and security boundaries.

## Product principles

- CLI-first control plane; GUI complements rather than replaces automation.
- Local-first and self-hostable; no mandatory Plaesy cloud control plane.
- Secure defaults; least privilege; explicit trust and data boundaries.
- Modular capabilities with stable contracts and independently testable services.
- Human-readable state/configuration where safe; secrets remain in an OS secret store or protected server-side secret source.
- Interoperability and exportability; no avoidable lock-in.
- Enterprise operation includes audit, identity, policy, retention, restore, upgrades, and supportability—not only UI and feature breadth.

## Full target scope

### Platform foundation

Installer/update manager; CLI command framework; configuration and profile management; local runtime/supervision; remote instance registry and secure transport; health/status/diagnostics; structured logs and audit; version/capability negotiation; extension lifecycle; import/export; backup and restore; policy and permission model.

### Experiences

CLI for all administrative and automation workflows; desktop app for interactive local and remote access, notifications, and secure local integration; responsive Web app for browser access and administration. Web UI must work on local deployments and self-hosted remote deployments. Desktop install/open/upgrade/uninstall flows are separate from Web deployment lifecycle.

### Platform capabilities

The product architecture must permit domain modules (including memory/knowledge, autonomous agent execution, and workflow orchestration) to be enabled, configured, permissioned, monitored, upgraded, and backed up without coupling unrelated modules. Plaesy is intended to own its user-facing control plane, orchestration, durable task/workflow state, policy, project context, memory/provenance, audit, and backend selection rather than depend on a single agent/harness product. The product owner permits integrations with external harnesses such as Claude Code and Codex, and with model APIs, as replaceable execution backends behind explicit, permission-scoped adapters. Plaesy must distinguish its own capabilities from backend-provided behavior and avoid binding user data or core state to one backend. These are target requirements, not evidence of current implementation.

### Enterprise readiness

Organizations and projects; role-based access control; user lifecycle; SSO/OIDC/SAML options; audit export; policy enforcement; configurable retention; encryption/key management; compliance-oriented controls; multi-instance administration; deployment automation; documented scaling/HA profiles; support bundle with secret redaction.

## Non-goals / explicit boundaries

- The CLI alone is not the product experience; it is the control interface.
- Local and hosted operation are not assumed to share identical trust boundaries.
- “Self-hosted” does not automatically mean highly available, compliant, or backed up.
- No hosted vendor service, SaaS billing, proprietary telemetry, or mandatory account is implied by these specifications.
- Do not promise certifications, compliance status, unlimited scale, or zero-downtime upgrades without evidence and a defined validation program.
- “Complete documentation” does not require implementing every capability in one monolithic release; it requires full lifecycle and cross-cutting requirements before implementation begins.

## Personas, permissions, and user journeys

Document end-to-end journeys for: fresh local install; first-run configuration; local Web launch; Desktop install/open; VPS provisioning; adding and selecting a remote; inviting/removing a member; configuring an integration; rotating credentials; backup/restore; upgrade/rollback; migration; incident diagnostics; full export; uninstall with data-retention choice.

Each journey must state actor, preconditions, confirmation points, state transitions, success evidence, failure recovery, permissions, and data changed. Permission roles and organization hierarchy remain explicit decisions in [Open decisions](decisions.md).

## Functional requirements

- Installation detects supported OS/architecture, verifies artifact integrity/provenance, supports pinned versions, and reports PATH/config changes.
- `plaesy status` distinguishes CLI, local services, selected target, target health, version and connection/auth status.
- `plaesy web` launches/opens Web for the selected local instance by default; `--target` or equivalent explicit selection is required to operate remotely.
- The CLI can create, inspect, configure, start, stop, upgrade, back up, restore, diagnose, and remove managed installations, subject to OS and provider capabilities.
- Every persistent resource has stable identity, ownership, lifecycle state, timestamps, version and origin; list/get output supports human and machine formats.
- All mutating commands have deterministic exit codes, actionable errors, idempotency expectations, and noninteractive mode suitable for CI.
- Export/import formats are versioned and documented; exports include schema/version, checksums, and clear exclusion of secrets by default.
- Admin and user experiences explain effective permissions, selected instance, connectivity, enabled modules, data retention and telemetry settings.

## Acceptance at product level

A release is not complete merely because the app opens. Required lifecycle journeys must be executable end-to-end on supported platforms; security and recovery gates must pass; all advertised commands and deployment profiles must be tested; documentation must match the released behavior. See [Quality and delivery](quality.md).
