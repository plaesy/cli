# Setup

## Requirements

- Rust 1.92+ (the minimum required by the locked ONNX Runtime Rust bindings); on Windows use the **MSVC** toolchain (`stable-x86_64-pc-windows-msvc`) with Visual Studio Build Tools
- ~3 GB disk for build artifacts; first build downloads ONNX Runtime binaries (`ort download-binaries`)

```bash
cargo build --release --locked
cargo test --locked           # real-ONNX tests run only if the model is installed
```

## Embedding model (one-time)

```bash
make model                     # scripts/install-model.sh
```

Downloads `model.onnx` (fp32, ~90 MB) and `vocab.txt` from `sentence-transformers/all-MiniLM-L6-v2` into
the models folder for your OS (table below). Lookup order at runtime: `embeddings.model_dir` (config),
`$PLAESY_MODEL_DIR`, the models folder, then the legacy `~/.plaesy-models/<model_name>`. After the download nothing touches the network.

## File locations

Defaults follow each OS's convention (implemented in `src/memory/paths/mod.rs`, using the `dirs` crate). Models are a
cache: they can be deleted and `make model` downloads them again. Data (database) is kept.

| OS      | Models (cache)                                      | Database (persistent local data)                    |
| ------- | --------------------------------------------------- | ---------------------------------------------------- |
| Linux   | `$XDG_CACHE_HOME/plaesy/models` (default `~/.cache/plaesy/models`) | `$XDG_DATA_HOME/plaesy/memory/storage.db` for direct engine use; `plaesy memory` uses `$XDG_DATA_HOME/plaesy/memory/<vault-id>.db` |
| macOS   | `~/Library/Caches/plaesy/models` (exclude from backup) | `~/Library/Application Support/plaesy/memory/storage.db` for direct engine use; `plaesy memory` uses a per-vault `<vault-id>.db` beside it |
| Windows | `%LOCALAPPDATA%\plaesy\models` | `%LOCALAPPDATA%\plaesy\memory\storage.db` for direct engine use; `plaesy memory` uses a per-vault `<vault-id>.db` beside it |

`<vault-id>` is derived from the selected vault path, so each vault has a separate index on each device. Override any of them with `MEMORY_DB_PATH`, `MEMORY_MODEL_DIR`, or the config keys. Datasets for evaluation
(for example LongMemEval) belong in `<data>/plaesy/datasets/`, not in the models folder.

No model ⇒ keyword-only mode (a warning is logged).

### Multilingual model (optional, for non-English notes)

The default `multilingual-e5-small` supports 100+ languages, including Indonesian. `all-MiniLM-L6-v2` remains available for English-only vaults. `multilingual-e5-small` (384 dimensions) is
optional and works on Indonesian queries better in the probe we ran (see README, "Quality evidence"). Install it by hand:

```bash
# Linux: MODELS=${XDG_CACHE_HOME:-$HOME/.cache}/plaesy/models
# macOS: MODELS=$HOME/Library/Caches/plaesy/models
# Git Bash on Windows: MODELS=$LOCALAPPDATA/plaesy/models
E5="$MODELS/multilingual-e5-small"
mkdir -p "$E5"
curl -L -o "$E5/model.onnx" \
  https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/onnx/model.onnx      # ~470 MB, fp32
curl -L -o "$E5/tokenizer.json" \
  https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json       # ~17 MB
```

Select it with `MEMORY_MODEL_NAME=multilingual-e5-small`. E5 queries and passages get the `query: ` and `passage: `
prefixes automatically. Switching models re-embeds every document on the next sync (the index records the model it was
built with, and a database built before recording is treated as `all-MiniLM-L6-v2`). A warning is logged when that happens.
Keep one database per model if you want to compare them.

## Configuration

Layering (later wins): defaults → `Plaesy/settings.json` from `memory onboard` →
`~/.hermes/memory.toml` → `.plaesy/memory.toml` → environment. Partial files are
fine; only the keys you set are overridden. The settings file is under the
platform's local application data folder, or under `PLAESY_ROOT` when set.

Passing `--config <PATH>` replaces the standard settings/TOML lookup for that invocation: the
explicit file layers over built-in defaults, then `MEMORY_*` environment values override it. The
outer `plaesy memory --vault` / `--project` selectors still choose the notes root. When using the
CLI wrapper, leave `database.db_path` unset to keep a separate local index for each vault; an
explicit path is honored as-is.

```toml
watch_dir = ".plaesy"

[database]
  db_path = "<data folder>/plaesy/memory/storage.db"   # direct engine default; the CLI wrapper uses a per-vault file
                                                           # HNSW snapshot is stored beside it as storage.db.hnsw (+ .meta.json, .gen)

[embeddings]
model_dir = "C:/models/all-MiniLM-L6-v2"
max_batch_size = 32
timeout_secs = 30

[search]
default_limit = 10
max_limit = 100
timeout_secs = 30
confidence_threshold = 0.0   # drop results scoring below this (0-1)
stale_after_days = 180       # last_verified older than this => flagged STALE (0 disables)
stale_penalty = 0.85         # score multiplier for stale sources

[logging]
level = "info"               # a bare level also silences ONNX Runtime's INFO chatter
```

Environment overrides: `MEMORY_WATCH_DIR`, `MEMORY_DB_PATH`, `MEMORY_MODEL_DIR`, `MEMORY_MODEL_NAME`, `MEMORY_SEARCH_DEFAULT_LIMIT`,
`MEMORY_SEARCH_MAX_LIMIT`, `MEMORY_SEARCH_TIMEOUT_SECS`, `MEMORY_ENABLE_VECTOR_SEARCH`,
`MEMORY_ENABLE_KEYWORD_SEARCH`, `MEMORY_LOG_LEVEL`, `MEMORY_LOG_JSON`, `MEMORY_LOG_FILE`. `RUST_LOG` overrides the log filter.

`MEMORY_EXCLUDE_DIRS` (also `search.exclude_dirs` in TOML) keeps files out of the index. It is a comma-separated list
of folder runs or file names, matched anywhere in the tree: `.plaesy,.claude,CLAUDE.md` excludes every `.plaesy` and
`.claude` folder and every file named `CLAUDE.md`. Excluded documents are removed from the index on the next sync.

## MCP (any compatible client)

The CLI exposes memory as a subcommand; there is no separate `memory` executable. Start the MCP
server with `plaesy memory serve --mcp`; it has read tools plus limited create-only note and gap
resolution tools. Use `plaesy memory config mcp --list` to print client-specific examples. See
[mcp.md](mcp.md).

## Notes vault

The engine indexes any folder of Markdown. For a ready-made structure (core / episodic / semantic / procedural / archive,

## Benchmark

`make lme DATA=/path/longmemeval_s_cleaned.json STRIDE=5` runs the LongMemEval-S retrieval benchmark through the real
engine (see README, "Quality evidence").

## Upgrading an existing index

Schema v2 adds a `links` table and a `body_start` column. Opening an older database migrates it in place and marks every
document for re-indexing on the next sync (so links and line offsets are filled in). Embeddings are recomputed for re-indexed files.

A sync skips files whose content hash has not changed. A chunking change (for example the heading rule in ARCHITECTURE)
therefore does not reach existing documents on its own. To rebuild, stop every process that uses the database, back up
the `.db`, `.hnsw*` files, delete them, and run `index` again. The index is a cache; the notes are the source of truth.
There is no `--force` flag yet.

If the vault folder structure changes (for example a folder is renamed), run `index` once: documents at the old paths are
removed and the new ones added.

## Troubleshooting

| Symptom                                                                                              | Cause / fix                                                                                                                                                            |
| ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dlltool.exe: program not found`                                                                   | GNU Rust toolchain without MinGW — use the MSVC toolchain                                                                                                             |
| Link error `nk_*` unresolved                                                                       | `usearch` must be built with `default-features = false` (already set)                                                                                              |
| `vector search: off (keyword-only)` in `stats`                                                   | model not found — `make model` or set `PLAESY_MODEL_DIR`                                                                                                          |
| First search after install returns few hits                                                          | initial index still running in the background; check `stats`                                                                                                         |
| Hermes answers `provider not initialized` but `hermes memory status` says the provider is active | the binary was never started. Run `hermes chat -v -q "..." --oneshot` and look for `initialize failed` (for example `WinError 267`, a missing working directory) |
| Hermes search returns `No results.` for a known note                                               | Hermes may be using the default database, because it was started before `MEMORY_DB_PATH` was set. Restart it, or check `stats` for the expected document count     |
| Results show only a heading, or a document that no longer exists                                     | the index predates a chunker change or a folder rename. Rebuild as described under "Upgrading an existing index"                                                       |
