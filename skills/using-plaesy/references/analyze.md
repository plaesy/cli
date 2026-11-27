# plaesy analyze

Builds project analysis (documentation reports, a dependency graph, and symbols) under `.plaesy/analysis/`, and queries that graph. Query flags read the existing graph; run a plain `plaesy analyze` first to create or refresh it.

## Syntax

```sh
plaesy analyze [PROJECT_PATH] [--force] [--no-graph] [--watch [--watch-interval 3]]
plaesy analyze [PROJECT_PATH] <query option>
```

## Query options

| Option | Result |
|---|---|
| `--query <TEXT>` | Keyword match on node ids |
| `--fuzzy-query <TEXT>` | Fuzzy or substring match (`--search-depth` for display) |
| `--path-query-from <A> --path-query-to <B>` | Shortest path between two nodes |
| `--explain <NODE>` | One node's edges and symbols |
| `--impact-check <NODE>` | Nodes affected by changing it (`--impact-depth`, default 2; `--impact-visualize` writes HTML) |
| `--semantic-queue`, `--business-logic` | Export inferred edges to JSON queues |
| `--apply-semantic <FILE>` | Merge `{source,target,rationale}[]` annotations into `reports.md` |
| `--generate-paths` | Write a `learning-paths.json` stub |

The fingerprint fast path skips work when nothing changed; `--force` regenerates anyway.

## Examples

```sh
plaesy analyze
plaesy analyze --impact-check crates/plaesy/adapter/relay/src/relay.rs --impact-depth 3
plaesy analyze --path-query-from cli --path-query-to relay
```

`plaesy analyze --help` is authoritative.
