# plaesy memory

Local Markdown memory: an index (SQLite FTS5 plus HNSW embeddings) over a vault of notes. The notes are the source of truth; the index is rebuildable and lives in local application data, outside the vault.

## Contents

- [Vault selection](#vault-selection)
- [Subcommands](#subcommands)
- [Search and browse](#search-and-browse)
- [Create a note](#create-a-note)
- [Update, archive, forget](#update-archive-forget)
- [Note rules](#note-rules)
- [Maintenance](#maintenance)
- [Services and MCP](#services-and-mcp)
- [Verify](#verify)

## Vault selection

Vault options go **before** the subcommand:

```sh
plaesy memory --vault <PATH> search "query"   # a specific shared vault
plaesy memory --project search "query"        # this repository's .plaesy directory
```

Precedence: `--vault` / `--project`, then `MEMORY_WATCH_DIR`, `PLAESY_MEMORY_VAULT`, the configured vault, then fallbacks. `--config <FILE>` replaces the standard settings lookup. `--db <FILE>` (or `MEMORY_DB_PATH`) picks the index database. `plaesy memory config` prints the effective configuration. Always pass `--vault` or `--project` for writes.

## Subcommands

| Subcommand | Purpose | Writes |
|---|---|---|
| `search <QUERY>` | Hybrid (semantic + keyword) search with citations | index only |
| `list` | Paginated inventory without bodies | no |
| `links <NOTE> [--outgoing]` | Wikilink backlinks or outgoing links | no |
| `projects [ID] [decisions\|contexts]` | `type: project` notes and their decisions or contexts | no |
| `decisions`, `contexts` | `type: decision` / `type: context` notes across projects | no |
| `remember <TITLE> [BODY]` | Create a new note; never overwrites | vault |
| `fingerprint <PATH>` | SHA-256 for a conflict-checked change | no |
| `update <PATH>` | Replace a note from stdin | vault |
| `archive <PATH>` | Set status `archived`, keep the file | vault |
| `forget <PATH>` | Permanently remove a note, leave a tombstone | vault (**destructive**) |
| `history` | Hash-only lifecycle events | no |
| `index [DIR]`, `watch [DIR]` | Sync a directory into the index, once or continuously | index |
| `hygiene [DIR]` | Stale metadata, broken links, dead paths, possible secrets | no |
| `curate [DIR]` | Read-only maintenance report | index refresh |
| `missing [DIR]` | Unanswered queries and structural gaps; `--resolve <id>` closes a gap | gap state |
| `stats` | Index statistics | no |
| `code-find --symbol <S> <DIR>`, `code-audit <DIR>` | Find a reusable API; audit duplicate APIs | no |
| `serve` | Long-running JSON or MCP service | index |
| `schedule <once\|run\|list\|install\|uninstall>` | Scheduled jobs from config; OS timer every 30 minutes | OS scheduler |
| `config [mcp]` | Effective config; MCP client snippets | no |
| `token add\|rotate\|revoke\|list` | Per-client bearer tokens for the MCP HTTP server | token file |

`plaesy memory onboard` moved to `plaesy onboard` (see `onboard.md`).

## Search and browse

```sh
plaesy memory --vault <V> search "release checklist" -n 5
plaesy memory --vault <V> search "auth decision" --format json --path projects/plaesy-cli/
plaesy memory --vault <V> search "token budget" --mode keyword --include-inactive
plaesy memory --vault <V> list --type decision --status active --format json
plaesy memory --vault <V> projects plaesy-cli decisions
```

Search options: `--mode hybrid|semantic|keyword`, `--format text|json|md`, `--tier <T>` (repeatable: core, episodic, semantic, procedural, archive), `--path <PREFIX>`, `--min-trust high|medium|low`, `--include-inactive`, `--as-of YYYY-MM-DD`, `--expand-links`, `--explain`.

Each hit carries a citation (file, line range, revision) and an evidence label. `FACT` means a lexical match; `INFERRED` means a match by meaning only. Treat `INFERRED`, `STALE`, or low-trust hits as leads and verify them against the source.

## Create a note

```sh
# Plain capture into the default inbox
plaesy memory --vault <V> remember "Title" "Markdown body"

# Body from stdin
printf '%s\n' "Body" | plaesy memory --vault <V> remember "Title" --folder knowledges/rust

# Managed note from <vault>/templates/<name>.md
plaesy memory --vault <V> remember "Knowledge title" \
  --template knowledge --folder knowledges/system-design \
  --set ONE_SENTENCE_SUMMARY="A concise statement of the idea."

# Project note: creates projects/<slug>/README.md
plaesy memory --vault <V> remember "Project name" --template project
```

Graph options: `--type project|decision|context|knowledge|capture|preference`, `--id <ID>` (required with `--type`; must be a UUID with `--template`), `--status`, `--project <ID>`, repeatable `--scope <ID>`, repeatable `--supersedes <ID>`. Also `--tier` (default `semantic`), `--confidence` (default `medium`; an agent's `high` is stored as `medium`), and `--json` to print the created path as JSON.

## Update, archive, forget

All three change the vault. Fingerprint first so a concurrent edit is caught.

```sh
plaesy memory --vault <V> fingerprint knowledges/topic/note.md
plaesy memory --vault <V> update knowledges/topic/note.md \
  --if-match <sha256> --reason new-evidence --yes < revised-note.md
plaesy memory --vault <V> archive knowledges/topic/old.md \
  --if-match <sha256> --reason superseded --yes
plaesy memory --vault <V> forget knowledges/topic/dup.md --reason duplicate --yes
```

| Command | `--reason` values |
|---|---|
| `update` | `correction`, `new-evidence`, `clarification`, `other` |
| `archive` | `completed`, `obsolete`, `superseded`, `consolidated`, `other` |
| `forget` | `owner-request`, `duplicate`, `obsolete`, `privacy` |

- `update` reads the complete replacement from stdin, keeps front matter, refuses a stale fingerprint or a changed `id`, and writes atomically.
- `archive` sets `status: archived`; the file and its links stay. Restore with `update` and `status: active`.
- `forget` removes the source file and index entry and writes a tombstone. It does not rewrite git history or sync-tool version folders. Ask the user before running it.

## Note rules

- Search before creating; update a matching note instead of adding a duplicate.
- Use the vault's template for a managed note and keep its front matter: schema, stable UUID `id`, type, status, created and updated dates, scope, source, tags, trust.
- Use lowercase kebab-case names, `[[wikilinks]]` or relative links, and forward slashes.
- Decisions are durable. Record a replacement as a new decision note with `--supersedes <old-id>`, then set the old note's `status: superseded` and `superseded_by: <new-id>` through `update`. Never silently rewrite or delete a prior decision.
- A project note's `status` is its lifecycle state (active, paused, completed), not a progress message.
- Reference source code by repository plus a stable URL or commit, not by a machine-specific absolute path.
- Keep secrets, tokens, and private transcript text out of notes. Store verified results and mark inferences as such.

## Maintenance

```sh
plaesy memory --vault <V> hygiene --json --strict-facts
plaesy memory --vault <V> curate --limit 50 --project-root plaesy-cli=<repo-path>
plaesy memory --vault <V> missing --all
plaesy memory --vault <V> missing --resolve <gap-id>
plaesy memory --vault <V> index
```

`hygiene --strict` exits 1 on any finding; `--strict-facts` only on confirmed findings. `curate` never moves, edits, or deletes notes. A `dead_path_reference` finding is heuristic: check the note's context before changing a path.

## Services and MCP

```sh
plaesy memory --vault <V> serve --mcp                       # MCP over stdio
plaesy memory --vault <V> serve --mcp --transport http --port 9123 --auth-token <T>
plaesy memory --vault <V> serve --mcp --read-only           # read a vault another daemon owns
plaesy memory config mcp --app claude-code                  # print a client snippet
```

Give each HTTP client its own token instead of one shared value:

```sh
plaesy memory token add laptop                    # printed once; store it in the client's secret config
plaesy memory token rotate laptop --grace-hours 24
plaesy memory token revoke laptop
plaesy memory token list                          # labels and state only, never tokens
```

The server keeps only salted hashes in `~/.config/plaesy/mcp-tokens.toml` (or `PLAESY_MCP_TOKENS_FILE`), which must stay outside the synced vault, and re-reads it without a restart. `PLAESY_MCP_TOKEN` still works as the single token labelled `default`. Never paste a token into a note, log, or commit.

For a background shared server use `plaesy mcp memory start|status|stop` (see `mcp.md`). MCP tools include `memory_search`, `memory_remember`, `memory_fingerprint`, `memory_update`, `memory_archive`, `memory_curate`, `memory_hygiene`, `memory_missing`, `memory_resolve_gap`, `memory_stats`, and `memory_backlinks`. Only one writer daemon owns a vault; use `--read-only` for a second reader.

## Verify

- After `remember` or `update`: `plaesy memory --vault <V> search "<title>"` or `list --path <prefix>` shows the note.
- After `archive`: the note appears only with `--include-inactive`.
- After any lifecycle change: `plaesy memory --vault <V> history -n 5`.

`plaesy memory <sub> --help` is authoritative for flags.
