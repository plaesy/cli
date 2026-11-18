---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Establish Rust security and release quality gates
phase: verify
status: backlog
---

# Establish Rust security and release quality gates

## Goal

Define and enforce the Rust toolchain, CI, dependency, security, and release checks needed for the approved replacement slice.

## Scope

- Before the first Rust implementation, establish the baseline toolchain pin, lockfile/`--locked` policy, format, Clippy, and focused-test CI jobs; these foundation checks do not require selecting deployment targets.

- After the owner selects a first slice and supported targets, extend CI to its supported-target matrix, integration/doc tests, supply-chain controls, and release artifact provenance.
- Set a dependency policy and automate license/advisory review, secret scanning, and artifact provenance appropriate to the approved release profile.

- Define secure defaults: forbid `unsafe` unless a reviewed exception is justified; require typed/contextual errors, input validation, bounded async work, cancellation/timeouts, and graceful shutdown where applicable.
- Add focused fuzz/property/adversarial tests for selected trust boundaries and data formats; record exclusions and residual risks.

- Keep quality thresholds and required checks traceable to the approved workflow and NFRs. Do not introduce a product implementation or assume deployment targets.

## Non-goals

- Do not choose the first workflow, deployment profile, supported OS/architecture, or NFR on behalf of the owner.

- Do not claim release readiness from local checks alone.
- Do not modify incumbent upstream source code.

## Deliverables

- Versioned Rust quality-gate policy and CI configuration for the approved slice.

- Focused security/dependency test evidence and release checklist, with explicit unsupported targets and exceptions.

## Dependencies

- Owner decisions and approved first-slice spec.

- Rust workspace and CI environment selected by the project.

## Acceptance criteria

- [ ] `rust-toolchain.toml` (or an explicitly approved equivalent) pins toolchain and required components; the CI log proves the expected `rustc`/`cargo` versions ran.

- [ ] An application `Cargo.lock` is committed and CI resolves/builds/tests with `--locked`; no quality gate silently updates dependencies.
- [ ] CI executes formatting (`cargo fmt --all -- --check`), Clippy with warnings denied on the approved target/features, focused unit/integration/doc tests, and the owner-approved target matrix. Each has a machine-readable pass/fail result and actionable failure log.

- [ ] Test commands and feature/target combinations are explicitly listed; mutually exclusive or unsupported combinations are not hidden behind `--all-features`.
- [ ] Dependency purpose, maintenance, transitive footprint and license are reviewed; configured advisory/license checks run in CI and each finding has a linked disposition. Secret scanning runs on the proposed change set without storing credential values in evidence.

- [ ] Crates deny `unsafe` by default; each approved exception links to a review documenting its soundness invariant, supported platforms, and focused tests.
- [ ] Selected untrusted-input and persistence/protocol boundaries have focused fuzz/property/adversarial tests, or a named owner records a scoped reason and follow-up.

- [ ] Release artifact provenance, signing/attestation (if required), and reproducible build expectations match the approved release profile and have verifiable CI artifact evidence.
- [ ] Checks, target exclusions, MSRV/feature support, and accepted residual risks are linked to the approved slice and owner decisions; local-only output is not represented as release CI evidence.
