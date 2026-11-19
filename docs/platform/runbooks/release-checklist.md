# Release checklist: first replacement slice

Use this before any deployment of the shared profile. Each gate names the command or evidence required. A gate that is not met blocks the release.

## Gates that run here and now

| Gate | Command or evidence | Status on 2026-11-19 |
| --- | --- | --- |
| Formatting | `cargo fmt --all -- --check` | Passes |
| Lints | `cargo clippy --all-targets --locked -- -D warnings` | Passes |
| Unit and integration tests | `cargo test --locked` | 680 passed, 0 failed |
| Slice prefixes | `cargo test --locked identity_policy`, `task_run`, `runstore`, `adapter_conformance`, `bounded_runtime`, `approval_secret`, `shared_messaging` | Each passes |
| Markdown | `plaesy validate markdown docs .plaesy --no-baseline` | Passes |
| Task references | `plaesy tasks check <task>` for every backlog task | Passes |
| Lockfile | `cargo build --locked` | Passes |
| Process-kill recovery | `cargo test --locked --test run_crash_recovery` (kills a real worker process, then recovers) | Passes |
| Restore drill, synthetic data | `cargo test --locked --test run_crash_recovery` (backup, delete primary, restore, verify) | Passes; the mechanics only, not an operational drill |

## Evidence on the target host (ubuntu@43.157.205.91, 2026-11-19)

Host: Ubuntu 24.04, 2 vCPU, 7.4 GB RAM. Domain: `plaesy.mahbubzulkarnain.com`, served by the existing Caddy. Service: `plaesy-messages` (systemd, `DynamicUser`, `LoadCredential` for the key, bound to 127.0.0.1:8787).

| Gate | Evidence | Status |
| --- | --- | --- |
| Process-kill recovery on target host | `cargo test --locked --test run_crash_recovery` on the host, commit `f5d3e7e` | Passes |
| Full test suite on Linux | `cargo test --locked --all` on the host: 827 passed, 0 failed | Passes |
| TLS termination on target host | Caddy site `plaesy.mahbubzulkarnain.com`, TLS 1.2 accepted with a valid certificate, TLS 1.1 refused | Passes |
| End-to-end through the proxy | Public HTTPS: valid envelope 202, replay 409, bad tag 401, expired 401, `GET` 404, body over 64 KB rejected with 400 | Passes |
| Container isolation on target host | `cargo test --locked bounded_runtime_container` on ubuntu@43.157.205.91 (docker available): 15 passed, including live isolation, timeout, and cancel tests, commit `0421445` | Passes |

## Gates that are not yet met

These still block a real deployment.

1. **Operational restore drill.** The restore mechanics pass with synthetic data on this host. The inbox is in memory and no production data exists, so there is nothing to restore yet. A monthly drill against real data, timed against the 4-hour RTO, has not been run. Owner: operations.

## Sign-off

A release is ready when gates 1 to 5 above are met, and every gate in the first table still passes on the release commit. Record the commit hash, the date, and the name of each signer in `docs/platform/decisions.md`.
