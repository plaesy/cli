# Port status: Go `scripts/` → Rust `crates/plaesy`

The Go tree was removed after parity. Its last revision is in git history.
Status values: `todo`, `in progress`, `done`. `loop` is named `loop_run` in Rust
because `loop` is a Rust keyword.

| Go package | Rust module | Status |
|---|---|---|
| `cmd/plaesy` (30 files, removed) | `cli/` modules (clap), binary in `src/main.rs` | done |
| `internal/common` | `common` | done |
| `internal/config` | `config` | done |
| `internal/scaffold` | `scaffold` | done |
| `internal/installer` | `installer` | done |
| `internal/assets` | `assets` (embeds the repo tree directly) | done |
| `internal/validate` | `validate` | done |
| `internal/tasks` | `tasks` | done |
| `internal/checklist` | `checklist` | done |
| `internal/exclude` | `exclude` | done |
| `internal/expected` | `expected` | done |
| `internal/detectstack` | `detectstack` | done |
| `internal/mcpserver` | `mcpserver` | done |
| `internal/designlint` | `designlint` | done |
| `internal/evals` | `evals` | done |
| `internal/imagegen` | `imagegen` | done |
| `internal/cleaner` | `cleaner` | done |
| `internal/delegate` | `delegate` | done |
| `internal/loop` | `loop_run` | done |
| `internal/trimmer` | `trimmer` | done |
| `internal/mdlint` | `mdlint` (pulldown-cmark in place of goldmark) | done |
| `internal/quality` | none: test-only package | n/a |
| `internal/graph` (2552 lines) | `graph` | done |
| `internal/analyze` (2319 lines) | `analyze` | done |
| `internal/reach` (4261 lines) | `reach` (keyring crate for the OS keyring) | done |
| `internal/search/*` (5127 lines) | removed by the user: vector and document search move to the sibling `memory` repo | removed |

Removed in the Go tree before this port: `internal/semantic`, and the `search` and
`stats` commands.

## Known deviations from the Go behavior

- `trimmer::layer1::run`: stdout and stderr are shown one after the other. Go merged them as written.
- `trimmer::layer1`: a filter's panic is not caught. Go recovered and passed the raw output through.
- `trimmer::filters::git_log`: Go kept the `Author:` line as the subject. Rust skips `Author:`, `Date:`, and `Merge:`, as its doc comment says.
- `loop_run::state`: the Go `keyring` toggle is not ported. It was stored and never read.
- `delegate::providers`: CLI timeouts poll the child process. Go used a context.
- `mdlint`: `pulldown-cmark` replaces goldmark. Node positions can differ in corner cases.
- `graph::incremental`: the Go global `fileWork` counter is returned as `FileResults::recomputed`.
- `graph::render`: `--apply-semantic` returns an error for an annotation with no rationale. Go returned the count applied so far with the error.
- `analyze::output`: an empty frameworks list is written as `[]`. Go wrote `null`.
- `analyze`: the framework version is the embedded `VERSION` file. Go also read a `VERSION` file beside the executable.
- `reach::cache::MemoryCache`: Go ran a background sweep every five minutes. Rust removes expired entries when they are read and when an eviction is needed.
- `reach::cache`: cache files use the Rust field layout, so a Go-written cache is not read. Entries are rebuilt on the next miss.
- `reach::exec::run_backend`: Go's `CombinedOutput` interleaves stdout and stderr. Rust gives stdout, then stderr.
- `reach::rss`: summaries are cut at 200 characters. Go cut at 200 bytes, which could split a UTF-8 character.
- `reach::registry::Registry::load_config`: Go's `ReloadConfig` held a mutex. Rust takes `&mut self`.
- `reach::config::ConfigManager::save`: Go mutated the caller's config to store the keyring placeholder. Rust works on a copy.
- `reach::router`: the adapter table is locked for reading during a call, so a `configure` waits for in-flight calls.

## Decisions

- `search` is removed by the user. Vector and document search live in the sibling `memory` repo. `analyze` writes no `docs/` index and `eval` is not available.
- `cmd/plaesy` drops the deprecated `validate-*` aliases and `reach --index`.

## Testing

Each ported package has Rust unit tests. The Go tests are not ported one for one.
`cargo test` and `cargo clippy --all-targets -- -D warnings` must pass before a row moves to `done`.

## CLI deviations

- `cli::validate`: the deprecated `validate-docx`, `validate-pptx`, `validate-xlsx`, and `validate-markdown` aliases are not carried over. Use `plaesy <target> validate`.
- Help text follows clap's layout, so spacing and wording differ from cobra's.
- `cli::clean`: the backup flag is `--backup[=BOOL]` with `--no-backup`, which is the same behaviour spelled through clap.
