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
