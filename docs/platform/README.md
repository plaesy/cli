# Plaesy Platform — documentation set

> **Status: proposed product design, not implemented.** This documentation outlines a target platform; it is neither a promise that every subsystem exists today nor evidence that every requirement and lifecycle is fully specified. Plaesy CLI currently contains existing functionality; see the repository README and [current CLI documentation index](../README.md). Any implementation plan must reconcile that code with these specifications before claiming feature parity.

Plaesy means **Platform Enterprise System**. It is one product with `plaesy` as its command-line control surface and Desktop and Web experiences as managed components. Plaesy Web must support both local operation and self-hosting on a VPS or equivalent infrastructure. This design is deliberately platform-wide; “complete” means the requirements and lifecycle are specified across the product, not that every capability ships in one release.

## Read in this order

1. [Product requirements](product.md) — audiences, principles, complete scope and non-goals.
2. [System architecture](architecture.md) — components, trust boundaries, data flows and tenancy.
3. [CLI and configuration](cli.md) — command grammar, profiles, resource model and behavior.
4. [Autonomous operation and memory](automation.md) — bounded automatic knowledge capture, task execution, policy and recovery requirements.
5. [Web and desktop](experiences.md) — UX requirements and local/remote connection models.
6. [Deployment](deployment.md) — local, VPS, lifecycle, upgrades and disaster recovery.
7. [Agent collaboration](agent-collaboration.md) — cross-session messaging, task coordination and safe concurrent edits.
8. [Security and privacy](security.md) — threat model, controls, secrets and incident response.
9. [Operations and reliability](operations.md) — SLOs, monitoring, backup, restore and support.
10. [Interfaces and data](interfaces-data.md) — API, events, persistence, compatibility and migrations.
11. [Quality and delivery](quality.md) — test strategy, release gates, acceptance criteria and rollout.
12. [Open decisions](decisions.md) — matters requiring explicit product/engineering decisions.
13. [Current-state assessment](current-state-assessment.md) — evidence snapshot, gaps, safe simulations, implementation sequence and release blockers.
14. [Replacement source inventory](replacement-source-inventory.md) — pinned discovery snapshots and partial source/documentation audits for the 11 requested incumbents; snapshots are not owner-approved baselines and the full leaf audit remains incomplete.
15. [Replacement measurement framework](feature-measurement-framework.md) — feature/scenario-level evidence and replacement gates.
16. [Hermes API server acceptance candidates](hermes-api-acceptance.md) — source-derived draft scenarios for the pinned API server docs; not rerun and not parity evidence.
17. [Replacement specification](replacement-spec.md), [plan](replacement-plan.md), and [backlog](replacement-backlog.md) — program-level artifacts, not proof of complete feature inventory.
18. [Replacement UX study](replacement-ux-study.md) — experience architecture, workflow coverage, accessibility floor, and design gaps; target design only.

## Authority and terminology

- **MUST / MUST NOT**: normative requirement; **SHOULD / SHOULD NOT**: default requirement with documented exception; **MAY**: optional.
- **Target design** describes desired behavior. **Current implementation** describes only behavior verified in the code and current product documentation. The target design does not supersede existing CLI interfaces until migration is approved.
- **Product direction (owner confirmed 2026-11-16):** Plaesy aims to outperform and ultimately replace agent/coding/orchestration products such as Hermes Agent, Claude Code, Codex, Paperclip, and Orca. Plaesy may use external agent harnesses or model APIs as replaceable execution backends, while owning orchestration, durable state, policy, memory/provenance, audit, and user control. This is a product goal, not present capability or blanket approval to cut over. See [owner direction and remaining decisions](decisions.md#direction-recorded-from-product-owner-2026-11-16).
- **Instance**: one running Plaesy Platform installation, whether local or remote.
- **Control client**: the `plaesy` CLI; it issues commands to local components or an explicitly selected remote instance.
- **Deployment**: a managed instance plus its persistent data, configuration, identity, and lifecycle state.
- **Workspace / organization / project**: logical scopes; their exact hierarchy and policy are an open decision.

## Pre-implementation blocking gates

This document set is an evolving target-platform design, not an assertion that the complete enterprise platform is fully specified. The product owner must select the first deliverable and explicit exclusions before acceptance baselines are frozen. The platform is not implementation-ready for any affected shared/remote contracts until blocking decisions are approved. Keep affected schemas, APIs, security guarantees and release claims conditional. Each decision record must name accountable product/engineering/security owners, chosen option, rationale, affected components/documents, required evidence/tests, and approval date.

Implementation gates: first-release scope; tenant/ownership hierarchy together with identity and minimum authorization; shared-write enforcement boundary and fail-closed/degraded behavior; shared Git/lease protocol; conflict/overlap consent behavior; one stable CLI namespace; and selected deterministic pre-implementation concurrency/security tests. These block only dependent capabilities; isolated local-only work may proceed if it does not encode unresolved assumptions.

Release gates: adapter-specific interrupt/queue acknowledgement evidence; remaining failure-injection matrix; rehearsed owner/provider/key recovery and data-lifecycle evidence; critical accessibility checks; and evidence for every externally advertised deployment or security capability. Live injection or shared-write safety must not be advertised before relevant tests pass. Product-owner decisions are not inferred by implementers; open gates remain open until an authorized owner records an approval.

| Gate | Required decision/evidence | Main dependencies |
|---|---|---|
| Product and tenancy | First-release scope; single-user vs multi-user; organization/workspace/project hierarchy; data ownership and transfer/recovery; minimum identity and authorization model | `product.md`, `security.md`, `architecture.md`, `interfaces-data.md` |
| Local security boundary | Threat model and tested auth/session/origin/CSRF/IPC boundary for local browser, CLI, Desktop and other local processes; disclose same-OS-user limits | `security.md`, `experiences.md`, `architecture.md` |
| Deployment and persistence | Supported first profile, database/memory integration, remote-management mechanism, OS/provider matrix, and measured data/concurrency envelope | `architecture.md`, `deployment.md`, `interfaces-data.md`, `operations.md` |
| Shared-write guarantees | Which writes are mediated; atomic revision check/write boundary; external-editor limitation; durable recovery; Git index/branch serialization and authorized conflict integrator | `agent-collaboration.md`, `architecture.md`, `interfaces-data.md`, `quality.md` |
| Agent runtime | Initial supported runtime(s), adapter contract, interrupt-safe points, queued delivery, acknowledgment and actual adapter conformance evidence | `agent-collaboration.md`, `interfaces-data.md`, `quality.md` |
| Keys, plugins and operations | Key custody/rotation/loss recovery across backups; extension first-release allow/deny and sandbox policy; named operational owner, support boundary, approved SLO/RPO/RTO | `security.md`, `deployment.md`, `operations.md`, `quality.md` |
| Client/release baseline | Versioned API/error/operation/idempotency contract; supported OS/client matrix; accessibility baseline and language scope | `cli.md`, `interfaces-data.md`, `experiences.md`, `quality.md` |

If a gate cannot be closed before exploratory work, explicitly constrain that work to replaceable prototypes with no production data migration, public compatibility promise, security guarantee, or supported-release claim. Revisit and approve the gate before dependent implementation is merged or released.

## Design invariants

1. A local install remains useful without a Plaesy-hosted account or service.
2. Remote access is opt-in, authenticated, encrypted, and distinguishable from local access.
3. CLI actions show the selected target before consequential mutations; destructive actions require explicit confirmation and offer a recovery/export path where feasible.
4. Durable user data is separate from binaries, caches, and rebuildable indexes. Upgrade and uninstall do not silently erase user data.
5. Installations are versioned, observable, recoverable, and operable without undocumented manual database edits.
6. Components communicate through versioned, documented interfaces rather than hidden shared state.
7. Security boundaries, retention and telemetry behavior are visible to administrators and users.
8. The command surface and documentation must not imply functionality that is not actually implemented.
