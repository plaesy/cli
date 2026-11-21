# Contributing

## Requirements

- Rust 1.92 or newer.
- On Windows, use the MSVC Rust toolchain and Visual Studio Build Tools.
- GNU Make is optional; the equivalent Cargo commands work on every platform.

## Local development

```sh
cargo build --locked
cargo run --locked -- --help
```

Use `make reload` when you intentionally want to install the current debug build into the normal
user-level Plaesy location for local end-to-end use. It replaces the installed binary and updates
the user's PATH when needed.

## Before submitting a change

```sh
make verify
```

This runs the build, test suite, Clippy with warnings denied, and formatting check. Run the relevance
evaluation with `make eval` when changing retrieval, ranking, chunking, or tokenization. Tests that
depend on downloaded ONNX models must skip with a clear message when the model is unavailable.

## Manual release checks (no CI)

This project does not run CI. Run these by hand before tagging a release:

```sh
make verify     # build, test, clippy, fmt check
make audit      # needs: cargo install cargo-audit --locked
make package    # release binary + SHA256SUMS under dist/
```

`make` is optional on Windows. Without it, run the equivalent Cargo commands from `make verify`
directly. `make check-artifacts` lists machine-local `.plaesy` paths that are still tracked by git.

## Project conventions

- Return typed errors from library code; do not use `unwrap()` or `expect()` outside tests.
  Two narrow exceptions, each with an `expect` message that states the invariant: regexes compiled
  from literal patterns, and serialization of in-memory values that cannot fail.
- Keep the runtime offline. Network access belongs in explicit install or model-download commands.
- Treat stdout as a protocol for JSON and MCP service modes; send diagnostics to stderr.
- Preserve source citations and exact line ranges in retrieval results.
- Migrate persisted schema changes on open and add coverage for opening an older database.
- Update the relevant command/API documentation and `CHANGELOG.md` for user-visible changes.

## Quality gates

These gates are measured, not judged. Each has a command that produces the number. Values below were measured on 2026-11-18 on branch `chore/plaesy-program-refactor`.

| Gate | Command or check | Measured | Limit and source |
| --- | --- | --- | --- |
| Largest production file | `find src -name '*.rs' ! -name '*_test.rs' -exec wc -l {} + \| sort -rn \| head -2` | 902 lines | 1,000 lines. This is a review trigger, not a validated limit: studies find that size confounds most code metrics ([TOSEM/EMSE 2022, s10664-022-10193-8](https://research.tudelft.nl/files/130282524/s10664_022_10193_8.pdf)). |
| `unwrap()` in production code | `cargo clippy --workspace --lib --bins -- -W clippy::unwrap_used` | 0 | 0. No study found a density benchmark; the Clippy lint is the measurable check. |
| `expect()` in production code | `cargo clippy --workspace --lib --bins -- -W clippy::expect_used` | 57 | Only for static regex patterns and provable invariants, each with a message that names the invariant. |
| `pub(super)` visibility | Compile each item with `pub(super)` removed; keep it only if the build fails | 74 probed: 11 narrowed, 63 needed | Minimize visibility ([Effective Rust, Item 22](https://www.lurklurk.org/effective-rust/visibility.html)). No empirical study found. |
| Intermittent test failure | Record a root cause or quarantine the test before merging | 1 open: `concurrent_workers_claim_a_task_only_once`, 1 failure in about 270 runs | Concurrency flaky tests have no single fix, so each needs its own root cause (Luo et al., FSE 2014, cited from a search summary and not opened). Reproduction is hard ([FlakeRake, 2024](https://people.cs.gmu.edu/~winglam/publications/2024/RahmanETAL24FlakeRake.pdf)). |
| Build, lint and format | `cargo fmt --check`, `cargo clippy --workspace --locked --all-targets -- -D warnings`, `cargo test --workspace --locked` | Green; 613 tests pass | Zero warnings, zero failures. |

Clippy's usage in real projects is described in [Unleashing the Power of Clippy in Real-World Rust Projects](https://ar5iv.labs.arxiv.org/html/2310.11738).
