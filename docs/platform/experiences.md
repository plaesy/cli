# Web and desktop experiences

**Status:** target UX and lifecycle requirements; not a statement that either application exists.

## Shared interaction model

Web and Desktop are clients of an identified Plaesy instance. At launch they show the instance name, local/remote location, connection/security state, user identity and product version. Remote sessions never silently fall back to a different target. Users can inspect effective permissions, enabled capabilities, recent activity and help/diagnostics. Accessibility, keyboard navigation, localization, responsive layouts, error recovery and safe confirmation are release criteria.

## Plaesy Web

### Local mode

`plaesy web` opens the local instance in the default browser, starting its managed services if needed. It supports `--no-open`, explicit bind settings, foreground development mode, status and logs. Default networking is loopback; LAN/public exposure requires explicit opt-in and documented authentication/TLS consequences. If local Web is not installed, the CLI explains the supported install path.

### Self-hosted mode

An operator deploys the instance through CLI or the documented administration flow, then users access the configured HTTPS URL. Initial setup must establish owner/admin identity, secure initial credentials, recovery method, domain/origin policy, SMTP or other optional services, and backup destination as applicable. No default password or permanently reusable bootstrap token. The Web app must identify supported server/client compatibility and handle expired/revoked sessions.

### Web requirements

- Separate public application endpoints from operator-only management endpoints.
- Validate host/origin and CSRF protections appropriate to the chosen session/auth architecture.
- Support secure session lifecycle, logout/revocation, password/identity recovery, and audit where appropriate.
- Clearly communicate offline, degraded, reconnecting, unauthorized, incompatible-version and maintenance states.
- Do not persist credentials or sensitive domain data in browser storage without a documented threat analysis.
- Offer export/backup/admin capabilities only to authorized roles.

## Plaesy Desktop

CLI manages Desktop install/update/uninstall; `plaesy desktop` opens an installed app or explains its installation. Desktop can connect to local or remote instances, may optionally bundle a local runtime only if the packaging and lifecycle design supports that safely, and should not duplicate server state. Desktop update and server upgrade are separate operations. Local privileges are mediated by an authenticated local IPC boundary; renderer/web content must not receive arbitrary shell or filesystem access.

The native installer must verify signatures/provenance, handle OS signing/notarization requirements, preserve user settings/data on app uninstall unless explicitly purged, support repair/reinstall, and disclose background services. Define auto-update opt-in/policy, staged rollout, rollback and offline update behavior before offering them.

## Cross-client requirements

- Feature availability is capability-driven and shown consistently.
- Server is authoritative for remote data; clients reconcile offline edits only under defined conflict rules.
- Notifications expose no sensitive content on lock screen by default.
- Accessibility target: document WCAG 2.2 AA evaluation for Web, and equivalent platform accessibility review for Desktop; do not claim compliance before audit.
- Accessibility names, focus order, contrast, screen-reader labels, reduced motion, and localization-ready strings are tested.
- Telemetry is disclosed and configurable; diagnostic exports redact secrets and user content by default.

## UX states and recovery

For each major workflow document loading, empty, success, validation failure, permission denial, connectivity loss, server error, conflict, partial completion and recovery. Long-running actions expose progress, safe cancellation rules, and durable operation status. Deleting a project/account/instance shows scope and retention/backups; “remove client” is not presented as “delete all data.”

## Acceptance journeys

1. Fresh local install → initialize → `plaesy web` → stop/restart → confirm data persists.
2. Desktop install → open local → switch to remote → verify target and identity.
3. Remote setup → secure first administrator → invite member → verify role boundaries.
4. Revoke session/token → confirm access denied promptly.
5. Upgrade server and clients across supported version boundaries; show incompatible version safely.
6. Connectivity outage → recover without duplicate writes or accidental target switching.
7. Uninstall client/runtime while keeping data; separately test explicit instance/data removal flow.
