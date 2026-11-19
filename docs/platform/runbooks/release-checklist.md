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

## Gates that are not yet met

These block any real deployment, not the test run.

1. **Container or VM isolation for the native runtime.** The native adapter is a bounded host process. Real tools must not run until it is inside a container or VM. Owner: operations.
2. **Process-kill crash test.** Crash recovery is tested in-process. A test that kills a real process mid-run and restarts it is still required. Owner: engineering.
3. **TLS termination on the target host.** The runbook in `messaging-transport.md` defines it. The host has not been configured or verified. Owner: operations.
4. **End-to-end test over the transport.** The loopback round trip passes. A test through the proxy on the target host does not exist yet. Owner: engineering.
5. **Restore drill.** The RPO and RTO in `decisions.md` require a monthly restore drill. No drill has been run. Owner: operations.

## Sign-off

A release is ready when gates 1 to 5 above are met, and every gate in the first table still passes on the release commit. Record the commit hash, the date, and the name of each signer in `docs/platform/decisions.md`.
