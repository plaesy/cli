# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Toolchain: `rust-toolchain.toml` pins Rust 1.99.1 (rustfmt and clippy included). Always pass `--locked`; `Cargo.lock` is tracked.

- Build: `cargo build --workspace --locked` (debug) or `make release`. The root is a virtual manifest; `default-members` is the `plaesy` package, so bare `cargo test` covers only that package.
- Run from source: `cargo run --locked -- <args>`.
- All tests: `cargo test --workspace --locked` (or `make test`).
- One test or filter: `cargo test --workspace --locked <name-substring>`, or `make test-one TEST=<name>`. Use `-- --exact` with the full path for a single test. Example: `cargo test --locked identity_policy` runs every test whose path contains that string.
- Single integration test file: `cargo test --locked -p plaesy --test <file_stem>`, for example `--test run_crash_recovery`.
- Lint: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Format: `cargo fmt --all` to write, `cargo fmt --all -- --check` to verify.
- Full gate before a change is done: `make verify` (build, test, lint, fmt-check). Without make, run the Cargo commands above.
- Markdown lint: `plaesy validate markdown <paths> --no-baseline`. The lint must pass for any Markdown you change.
- Benchmarks: `cargo bench --locked`. Relevance evaluation: `make eval`.

Windows notes:

- Git Bash does not put `~/.cargo/bin` on PATH. Run `export PATH="$HOME/.cargo/bin:$PATH"` first.
- `plaesy mcp` keeps the installed `plaesy.exe` locked. Stop it before replacing the binary.
- The installed `plaesy.exe` in `%LOCALAPPDATA%\Plaesy\bin` can lag behind the source. Use `target/debug/plaesy.exe` or `target/release/plaesy.exe` when a fix in this checkout matters.

## Conventions the code enforces

- Library code returns typed errors. No `unwrap()` or `expect()` outside tests, except static regex patterns and provable invariants, each with a message that names the invariant (see CONTRIBUTING.md).
- The runtime may use the network when a feature needs it (mobile or remote access, chat integrations, collaboration, providers). Approved by the owner on 2026-11-23, replacing the earlier offline-only rule. Each networked path must be explicit, opt-in by configuration, and documented in `docs/platform/decisions.md`. Local-only features (memory, tasks, boards, exports) stay local by default.
- Stdout is a protocol in JSON and MCP modes. Send diagnostics to stderr.
- Schema changes migrate on open, with coverage for opening an older database.
- Tests live beside their module in `<name>_test.rs`, wired with `#[cfg(test)] #[path = "<name>_test.rs"] mod <name>_test;`. Integration tests live in `crates/plaesy/plaesy/tests/`, and architecture checks run from `architecture_test.rs` (ADR 0003). Tests that need the ONNX model must skip with a clear message when it is missing.
- Update `CHANGELOG.md` for user-visible changes.

## Architecture

The repository is a Cargo workspace (virtual root manifest). Every package lives under `crates/plaesy/` and is named `plaesy-*`, except the facade `plaesy` (`crates/plaesy/plaesy`), which holds the binary `src/main.rs`, the library re-exports in `src/lib.rs`, the integration tests, the benches, and the examples. Layout and dependency rules are in `docs/adr/0003-crate-structure.md` and `docs/adr/0011-client-structure.md`. The module-to-crate map is in ADR 0003 (section "Pemetaan modul"). `crates/plaesy/interface/client` (`plaesy-interface-client`) is the skeleton for the visual flow editor.

- **Domain (`crates/plaesy/domain/*`).** IR rules without I/O: `flow`, `data`, `api`, `app`, `design`, `identity` (identity, authz, secrets), `workflow` (runstore, task_run, policy). `plaesy-domain-*` must not reach tokio, axum, rusqlite, ureq, notify, ort, usearch, keyring, reqwest, or hyper; `tests/architecture_test.rs` checks this.
- **Adapters (`crates/plaesy/adapter/*`).** I/O: `memory`, `relay`, `runtime` (the execution adapter contract and the runtime), and `design-ir` (design-ir/1 validator and codegen; feature `designir-dioxus`).
- **Support (`crates/plaesy/support/*`).** Modules that ADR 0003 does not yet place in a context: `common`, `mdlint`, `tasks`, `analysis`, `designlint`, `loop`.
- **Interface (`crates/plaesy/interface/*`).** `cli` (the command tree) and `client` (the editor skeleton).
- The facade re-exports each module under its old `plaesy::<module>` path, so tests and the binary keep their paths.

- **Memory engine (`crates/plaesy/adapter/memory/src/memory/`, `docs/memory/architecture.md`).** Markdown notes go through an extractor and chunker, then into SQLite (FTS5 keyword leg) and an HNSW index built from ONNX embeddings. Results come from a 70/30 semantic and keyword fusion, each with a citation as path, line range, and revision. The index is rebuildable, and the notes are the source of truth.
- **Task system (`crates/plaesy/support/tasks/src/tasks/`, `crates/plaesy/interface/cli/src/cli/tasks*.rs`).** A task is a folder under `.plaesy/tasks/<status>/` with `task.md`, `issue.json` (dependencies, labels), and `contracts.json` (checklist). `tasks validate` runs each `contracts.json` command through Bash, so treat that file as code. A task moves to `done/` only after its checks pass. Claims use advisory locks under `.claims/`.
- **Relay (`crates/plaesy/adapter/relay/src/relay.rs`).** A local SQLite queue with polling and self-declared identity. This is the existing, local-only inter-agent channel.
- **Program in progress: replacement platform.** The spec and plan are in `.plaesy/spec/plaesy-agent-platform/` and `.plaesy/plan/plaesy-agent-platform/`. Its library modules are `identity`, `authz`, `adapter`, `runstore`, `task_run`, `policy`, `secrets`, `runtime`, `evidence`, and `relay::{shared, auth, keys, transport}`. They are not wired into the `plaesy` binary. Decisions are in `docs/platform/decisions.md`. Owner-approved items are recorded there; delegated items are marked revocable. Deployment gates are in `docs/platform/runbooks/release-checklist.md`.
- **Vendored code.** `vendor/laya-decision` is a path dependency. `vendor/esaxx-rs` is patched in through `[patch.crates-io]` to avoid an MSVC CRT conflict. Change vendored code only when you must, and say so in the commit.

## Repository state

- The default branch is `main`. Check the active branch with `git branch --show-current` before committing; integration work for agent features lives on `integrate/agent-features`.
- `.claude/worktrees/agent-*` holds worktrees from agent sessions. They are not source of truth; read `main` or the active branch instead.
- Disk: a full workspace build needs many GB under `target/`. `cargo clean` can fail while MCP servers hold `plaesy.exe`; stop those processes first.
- Commit policy: `CLAUDE.md` is tracked. After a change passes `make verify` (or the equivalent Cargo commands) and the Markdown lint, commit it without asking. Use the commit proposal flow and report the commit hash.
- CONTRIBUTING.md says the project runs no CI, but `.github/workflows/ci.yml` exists. It runs fmt, clippy, tests, and an installer end-to-end test on three OSes. Check the workflow before assuming it is inactive.
- `docs/platform/` describes target designs. Do not present a planned capability as implemented; check the code and tests first.
