# Reference

Complete API reference for library, CLI, and protocols.

## Architecture Overview

```text
┌─────────────────────────────────────────┐
│  Entry Points                           │
│  • CLI (plaesy memory search/index)            │
│  • Library (plaesy::memory)             │
│  • JSON Service (Hermes-compatible)     │
│  • MCP Server (Claude Code/Cursor)      │
└────────────────┬────────────────────────┘
                 │
         ┌───────▼────────┐
         │ SearchEngine   │
         └───┬────┬───────┘
             │    │
    ┌────────▼─┐ ┌┴─────────┐
    │ SQLite   │ │ HNSW     │
    │ (FTS5)   │ │(usearch) │
    └──────────┘ └──────────┘
         │
    ┌────▼──────────┐
    │ ONNX Runtime  │
    │ (Embeddings)  │
    └───────────────┘
```

## Library (`plaesy::memory`)

### Module Declarations

| Module | Purpose | Location |
|--------|---------|----------|
| `error` | Error types and `Result<T>` alias | `src/memory/error/mod.rs` |
| `indexer` | Document extraction and chunking | `src/memory/indexer/mod.rs` |
| `embeddings` | ONNX embedding engine | `src/memory/embeddings/mod.rs` |
| `storage` | SQLite + HNSW persistence | `src/memory/storage/mod.rs` |
| `search` | Query engine and ranking | `src/memory/search/mod.rs` |
| `rerank` | Cross-encoder reranking (optional) | `src/memory/search/rerank.rs` |
| `hygiene` | Index health audit | `src/memory/hygiene/mod.rs` |
| `config` | Settings and validation | `src/memory/config/mod.rs` |
| `metrics` | Prometheus-compatible metrics | `src/memory/metrics/mod.rs` |
| `service` | Hermes-compatible JSON protocol | `src/memory/service/mod.rs` |
| `mcp` | MCP server tools | `src/memory/mcp/mod.rs` |

### Prelude

```rust
use plaesy::memory::prelude::*;
use plaesy::memory::config::MemoryConfig;
use plaesy::memory::search::SearchEngine;
```

Imports: `Result`, `MemoryError`, common types.

### Core Types

#### `SearchEngine`

**Location**: `src/memory/search/mod.rs:50` (struct definition)

**Constructor**:

```rust
impl SearchEngine {
    pub async fn new() -> Result<Self>
    pub async fn from_config(config: &MemoryConfig) -> Result<Self>
    pub fn set_watch_root(&mut self, root: PathBuf)
}
```

**Search methods**:

```rust
pub async fn search(
    &self,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>>

pub async fn search_with(
    &self,
    query: &str,
    limit: usize,
    mode: SearchMode,
    filter: &SearchFilter,
) -> Result<Vec<SearchResult>>

pub async fn search_expanded(
    &self,
    query: &str,
    limit: usize,
    mode: SearchMode,
    filter: &SearchFilter,
) -> Result<Vec<SearchResult>>
```

Default search mode is Hybrid (70% cosine + 30% BM25 keyword strength).

**Indexing**:

```rust
pub async fn index_directory(&self, path: &Path) -> Result<()>
pub async fn index_file(&self, path: &Path) -> Result<()>
pub async fn apply_events(&self, events: Vec<FileEvent>) -> Result<()>
```

Change detection via SHA-256 content hash. Unchanged files skipped. Deleted files removed from index.

**Metadata**:

```rust
pub async fn stats(&self) -> Result<IndexStats>
pub async fn metrics(&self) -> Metrics
pub async fn hygiene(&self, root: Option<&Path>) -> Result<HygieneReport>
pub async fn links(&self, path: &str, direction: LinkDirection) -> Result<Vec<Link>>
```

---

#### `SearchResult`

**Location**: `src/memory/storage/mod.rs:150`

```rust
pub struct SearchResult {
    pub chunk_id: u64,
    pub text: String,           // verbatim slice from source file
    pub score: f32,             // 0.0..=1.0
    pub kind: ChunkKind,        // Heading, Paragraph, Code, Quote
    pub line_range: (u32, u32), // 1-based, inclusive
    pub confidence: Confidence, // FACT or INFERRED
    pub source: SourceMetadata, // tier, trust, last_verified, status, stale
    pub revision: Option<String>, // git commit hash (if available)
    pub indexed_at: DateTime<Utc>,
}
```

---

#### `Citation`

**Location**: `src/memory/search/mod.rs:200`

```rust
pub struct Citation {
    pub chunk_id: u64,
    pub file_path: PathBuf,
    pub line_range: (u32, u32),
    pub revision: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub confidence: Confidence, // FACT or INFERRED
    pub source: SourceMetadata,
}

pub struct CitationBuilder;

impl CitationBuilder {
    pub fn new() -> Self
    pub fn build(&self, result: &SearchResult) -> Result<String>
    // Output: "path:line-range [FACT|INFERRED] indexed <timestamp> @ <revision>"
}
```

---

#### `SearchFilter`

**Location**: `src/memory/search/mod.rs:280`

```rust
pub struct SearchFilter {
    pub tiers: Vec<String>,           // ["core", "procedural", …]
    pub min_trust: Option<Trust>,     // high, medium, low
    pub path_prefix: Option<String>,  // path must start with this
    pub include_inactive: bool,       // include superseded/deprecated
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub as_of: Option<NaiveDate>,    // keeps sources whose valid_from..valid_to contains this date
}
```

Tier is inferred from directory name if not declared in frontmatter:

- `10-episodic/`, `20-semantic/`, etc. → tier
- `00. Inbox/` → core

---

#### `MemoryConfig`

**Location**: `src/memory/config/mod.rs:30`

```rust
pub struct MemoryConfig {
    pub notes_dir: PathBuf,
    pub db_path: PathBuf,
    pub model_dir: PathBuf,
    pub chunk_size: usize,
    pub enable_vector_search: bool,
    pub stale_after_days: u32,
    pub search: SearchConfig,
    pub ranker: RankerConfig,
}

impl MemoryConfig {
    pub async fn load() -> Result<Self>
    pub async fn load_from_file(path: &Path) -> Result<Self>
    pub fn apply_env(&mut self)
    pub fn validate(&self) -> Result<()>
}
```

Defaults per OS (Windows: `%LOCALAPPDATA%/Plaesy`, Linux: `~/.local/share/plaesy`). Environment variables override:

- `PLAESY_ROOT` — Settings root
- `PLAESY_NOTES_DIR`, `PLAESY_DB_PATH`, `PLAESY_MODEL_DIR`
- `MEMORY_EXCLUDE_DIRS` — Comma-separated paths to skip

---

#### `EmbeddingEngine`

**Location**: `src/memory/embeddings/mod.rs:80`

```rust
pub struct EmbeddingEngine;

impl EmbeddingEngine {
    pub async fn new(model_dir: &Path) -> Result<Self>
    pub async fn with_config(config: &EmbeddingConfig) -> Result<Self>

    pub async fn embed(&self, text: &str) -> Result<EmbeddingVector>
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<EmbeddingVector>>

    pub fn cosine_similarity(v1: &EmbeddingVector, v2: &EmbeddingVector) -> f32
}
```

Default model: `multilingual-e5-small` (384-dim). For an English-focused vault, set `MEMORY_MODEL_NAME=all-MiniLM-L6-v2`.

Output: 384-dimensional f32 vector, L2-normalized.

---

#### `Database`

**Location**: `src/memory/storage/mod.rs:60`

```rust
pub struct Database;

impl Database {
    pub async fn new(path: &Path) -> Result<Self>
    pub async fn open(path: &Path) -> Result<Self>

    pub fn get_document(&self, id: u64) -> Result<Option<Document>>
    pub fn list_chunks(&self, doc_id: u64) -> Result<Vec<Chunk>>
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<Chunk>>
    pub async fn index_document(&self, doc: Document) -> Result<u64>
    pub async fn delete_document(&self, path: &Path) -> Result<()>
}
```

Schema version 2 (documents, chunks, chunks_fts, links, index_metadata).

---

### Error Handling

All public functions return `Result<T> = std::result::Result<T, MemoryError>`.

```rust
pub enum MemoryError {
    EmbeddingError(String),
    IndexError(String),
    StorageError(String),
    SearchError(String),
    NotFound(String),
    InvalidConfig(String),
    Io(std::io::Error),
    Timeout,
    Internal(String),
}

impl std::error::Error for MemoryError { … }
```

Failing embeddings are **non-fatal** for hybrid search: keyword results are returned; failing recorded in metrics.

---

## CLI Commands

**Base**: `plaesy memory [--vault PATH | --project] [--config PATH] [--db PATH] COMMAND`

For a shared Markdown vault, pass `--vault PATH` before the memory command. A positional `[DIR]`
on `index`, `hygiene`, or `watch` sets the scanned directory; it does not select the vault's local
database. Use the same `--vault` value for indexing, searching, hygiene, and stats so they address
the same per-vault index.

### index

```bash
plaesy memory index [DIR]
```

Syncs directory into index. New/changed files in, deleted files out. Unchanged files skipped (SHA-256 hash).

Output: `Indexed X files, removed Y, failed Z` + detailed list of changes.

---

### search

```bash
plaesy memory search QUERY \
  [-n LIMIT]                                          # default 10
  [--mode hybrid|semantic|keyword]                    # default hybrid
  [--format text|json|md]                             # default text
  [--tier TIER] [--tier TIER2]                        # filter by tier
  [--path PREFIX]                                      # path prefix match
  [--min-trust high|medium|low]                       # filter by trust
  [--include-inactive]                                # show superseded/deprecated
  [--as-of YYYY-MM-DD]                                # date validity window
  [--expand-links]                                    # add one-hop linked passages
  [--explain]                                          # show per-leg scores
```

Output (text):

```text
1. score 0.95  path/to/file.md:40-52 [FACT] indexed 2026-10-18T17:25:19Z @ 576b7b5e…
   - **Search & retrieval**: Web search for broad discovery; …
```

JSON output: `[{chunk_id, text, score, confidence, citation: {file_path, line_range, …}}]`

Markdown output: Bulleted list with score prefix.

---

### links

```bash
plaesy memory links NOTE [--outgoing]
```

Show wikilinks referencing NOTE (incoming, default) or links from NOTE (outgoing).

Output: `[[target]]` references with file path and line number.

---

### hygiene

```bash
plaesy memory hygiene [DIR] [--strict] [--json]
```

Audit notes for stale or incomplete metadata, graph consistency issues, broken wikilinks and
Markdown links/anchors, machine-specific paths, dead paths, and likely credentials. The credential
scan is heuristic and does not print matched values. `--strict` returns exit status 1 when any
finding is reported; without it, findings are still printed but do not fail the command.

---

### missing

```bash
plaesy memory missing [--all] [--resolve GAP_ID]
```

Show unanswered queries (recorded by agents), broken links, stale notes. Limit 5 per kind unless `--all`.

With `--resolve ID`: mark gap as resolved (writes to gap log).

---

### stats

```bash
plaesy memory stats
```

Index metadata: document count, chunk count, index size, last sync, coverage %, cold-start time.

---

### watch

```bash
plaesy memory watch [DIR]
```

Keep index current until Ctrl-C. Debounced file-system events (notify crate, 500 ms window).

---

### config

```bash
plaesy memory config                              # print effective configuration
plaesy memory config mcp [--app APP] [--list]      # claude-code, crewai, n8n, dify, kilocode, paperclip
```

---

### serve

```bash
plaesy memory serve [--mcp]                                   # JSON service on stdin/stdout
plaesy memory serve --mcp --transport http --port 8080       # HTTP Streamable transport for n8n/Dify/CrewAI
```

See [api.md](api.md) for protocol reference.

---

### schedule

```bash
plaesy memory schedule once   # run due jobs and exit
plaesy memory schedule run    # keep running until Ctrl-C
```

Jobs are configured in `[[schedule.jobs]]`; see [operations](operations.md#scheduled-jobs).

---

### onboard

```bash
plaesy memory onboard [--vault PATH] [--root PATH] [--yes] [--no-model] [--rerank] [--no-index]
```

Options: `--vault PATH`, `--root PATH`, `--yes`, `--no-model`, `--rerank`, and `--no-index`.
`--yes` needs an existing `--vault` unless settings already contain one. The setup command chooses
its vault itself and does not require the ordinary memory vault selector.

First-run setup: choose or confirm a vault, download the embedding model, optionally index notes,
and write settings to `<local app data>/Plaesy/settings.json` (Windows:
`%LOCALAPPDATA%\Plaesy\settings.json`; Linux: `~/.local/share/Plaesy/settings.json`; macOS:
`~/Library/Application Support/Plaesy/settings.json`).

---

## MCP Server

**Tools** (six read-only; two bounded write tools):

### memory_search

```json
{"method": "memory_search", "params": {"query": "str", "limit": 5, "mode": "hybrid|semantic|keyword", "tier": "str|[str]", "path_prefix": "str", "min_trust": "high|medium|low", "include_inactive": false, "as_of": "YYYY-MM-DD", "expand_links": false, "explain": false}}
```

Returns array of hits with citations.

---

### memory_hygiene

```json
{"method": "memory_hygiene", "params": {}}
```

Audit report: stale, missing-metadata, broken-links arrays.

---

### memory_backlinks

```json
{"method": "memory_backlinks", "params": {"path": "str", "direction": "outgoing|incoming"}}
```

Wikilink graph: resolved paths and line numbers.

---

### memory_stats

```json
{"method": "memory_stats"}
```

Index size, coverage %, cold-start time, last sync.

---

### memory_missing

```json
{"method": "memory_missing", "params": {"limit": 20}}
```

List unanswered search queries and structural gaps. It does not record queries.

---

### memory_curate

```json
{"method": "memory_curate", "params": {"limit": 20}}
```

Read-only curation report: hygiene findings, unanswered queries, exact duplicate files, unlinked
note candidates, and knowledge folders that may need an index. No vault notes are changed.

---

### memory_remember

```json
{"method": "memory_remember", "params": {"title": "str", "body": "str", "folder": "relative/path", "tier": "semantic", "confidence": "medium"}}
```

Creates a new note inside the vault; it never overwrites an existing note.

---

### memory_resolve_gap

```json
{"method": "memory_resolve_gap", "params": {"id": 1}}
```

Closes a query gap previously listed by `memory_missing`; a later failed search may reopen it.

---

### Graph commands and remember

The CLI also provides `projects`, `decisions`, `contexts`, and `remember`. See the
[graph guide](graph.md) and [memory command reference](commands/memory.md) for their options and
note-writing workflow.

---

## JSON Service Protocol (Hermes)

**Format**: Line-delimited JSON. Request and response identified by `id`.

```json
→ {"id":1,"method":"ping"}
← {"id":1,"ok":true,"result":{"pong":true,"vector_search":true}}
```

**Methods**: `ping`, `search`, `index`, `backlinks`, `links`, `hygiene`, `stats`, `metrics`.

See [api.md](api.md) for full protocol specification.

---

## Type Aliases and Enums

### SearchMode

```rust
pub enum SearchMode {
    Hybrid,       // 70% cosine + 30% BM25 (default)
    Semantic,     // cosine only
    Keyword,      // BM25 only
}
```

### Confidence

```rust
pub enum Confidence {
    Fact,        // lexical match in source
    Inferred,    // embedding similarity only
}
```

### Trust

```rust
pub enum Trust {
    High,
    Medium,
    Low,
}
```

### ChunkKind

```rust
pub enum ChunkKind {
    Heading,
    Paragraph,
    Code,
    Quote,
}
```

### LinkDirection

```rust
pub enum LinkDirection {
    Outgoing,  // links from this note
    Incoming,  // links to this note
}
```

---

## Configuration Keys (TOML)

```toml
[memory]
notes_dir = "C:/_"
db_path = "%LOCALAPPDATA%/Plaesy/memory/storage.db"
model_dir = "%LOCALAPPDATA%/plaesy/models"
chunk_size = 512
enable_vector_search = true
stale_after_days = 180

[search]
exclude_dirs = ["target", ".git", "node_modules"]

[ranker]
ranker_type = "weighted"  # or "rrf"
semantic_weight = 0.7
keyword_weight = 0.3
stale_penalty = 0.85
```

Environment variable overrides: `PLAESY_*`, `MEMORY_*`.

---

## Performance Baselines

**Machine**: AMD Ryzen 7 5700U, CPU only, 20-document vault, 4.6K chunks.

| Operation | p50 | p95 | Measurement |
|-----------|-----|-----|-------------|
| Hybrid search | 2.7 ms | 4.2 ms | sequential, 200 requests |
| Keyword search | 1.1 ms | 1.8 ms | FTS5/BM25 only |
| Embedding (cold) | 11 ms | — | single query, no cache |
| Embedding (batch) | 5.7 ms/text | — | batch of 32 |
| Cold start (4.6K chunks) | 0.37 s | — | with HNSW snapshot cache |
| Index rebuild (4.6K chunks) | ~4 s | — | without HNSW snapshot |
| Index (145 files, 4.6K chunks) | ~2 min 10 s | — | embedding-bound |

**LongMemEval-S** (94 questions, every 5th of 470):

| Mode | R@1 | R@3 | R@5 | MRR |
|------|-----|-----|-----|-----|
| Keyword | 0.798 | 0.904 | 0.936 | 0.861 |
| Semantic | 0.840 | 0.936 | 0.957 | 0.889 |
| Hybrid | 0.840 | 0.926 | 0.968 | 0.891 |

---

## Limits and Trade-offs

- **One SQLite connection per daemon** behind an async mutex: concurrent SQL readers within one daemon currently serialize. Use one daemon per vault; do not open SQLite/HNSW directly from multiple agents.
- **Single writer boundary**: SQLite WAL permits concurrent reads but still has one writer. Multi-host or sustained high-write workloads should migrate the metadata store to a client/server database.
- **Multilingual default model**: multilingual-e5-small. all-MiniLM-L6-v2 remains available for English-focused vaults.
- **No network at runtime**: model download once by `make model` or `plaesy memory onboard`.
- **ONNX RC version**: 2.0.0-rc.14 (stable release pending).
