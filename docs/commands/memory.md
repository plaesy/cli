# `memory`

`plaesy memory` runs the memory engine inside the Plaesy executable. Select a vault before the
memory subcommand:

```sh
plaesy memory --vault <vault-path> search "query"
plaesy memory --project search "query"
```

Vault selection precedence is `--vault` / `--project`, `MEMORY_WATCH_DIR`,
`PLAESY_MEMORY_VAULT`, configured vault, then the shared `~/_` vault and repository `.plaesy/`
fallbacks. Standard configuration combines onboarding settings, `~/.hermes/memory.toml`, and
`.plaesy/memory.toml` in that order. When `--config <PATH>` is supplied, its `watch_dir` controls
the vault unless a CLI or environment selector overrides it; it replaces the standard TOML lookup.

An explicit `--db`, `MEMORY_DB_PATH`, or configured `database.db_path` is honored. Otherwise the
CLI stores the index under local application data using a vault-specific filename, outside the
vault, so each device builds its own index from synchronized Markdown files.

## Commands

The command groups are shown by `plaesy memory --help`. They cover indexing (`index`, `watch`),
retrieval (`search`, `links`), lifecycle (`remember`, `fingerprint`, `update`, `archive`, `forget`), maintenance (`hygiene`, `missing`, `curate`, `stats`), configuration and service
setup (`config`, `onboard`, `serve`, `schedule`), source-code lookup (`code-find`, `code-audit`),
and project graph views (`projects`, `decisions`, `contexts`).

Use the installed version's help for command-specific arguments:

```sh
plaesy memory search --help
plaesy memory hygiene --help
plaesy memory curate --help
plaesy memory remember --help
plaesy memory update --help
plaesy memory archive --help
```

## Curate memory

Prepare a bounded maintenance report without changing vault notes:

```sh
plaesy memory --vault <vault-path> curate
plaesy memory --vault <vault-path> curate --limit 50 --json
```

The report combines bounded hygiene findings, unanswered searches, exact duplicate files, notes with no
incoming or outgoing Wikilinks, and knowledge folders with at least five direct Markdown notes but
no `README.md`, `index.md`, or `MOC.md`. Unlinked notes and missing topic indexes are review
candidates, not errors. Each category and each duplicate group is limited to `--limit` examples
(1–100); hygiene totals remain complete, while `memory hygiene` provides the full finding list. The
command refreshes the local index first, unless a live memory daemon owns the vault; then it reads
the daemon-maintained index instead of competing for the write lock. It never moves, edits, archives,
or deletes Markdown files. Use `--project-root SLUG=PATH` to check code paths in local repositories.

The same report is available to MCP clients through the read-only `memory_curate` tool and can be
scheduled with `action = "curate"`.

## Schedule maintenance

Register or remove a per-user OS schedule for the selected vault:

```sh
plaesy memory --vault <vault-path> schedule install
plaesy memory schedule uninstall
```

Installation supports Windows Task Scheduler, Linux systemd user timers, and macOS LaunchAgents.
The command saves a private configuration snapshot and runs due jobs every 30 minutes. On a
headless Linux server, enable systemd user lingering for jobs to run after logout. See
[`operations.md`](../operations.md#scheduled-jobs) for behavior and manual setup details.

## Remember a note

Create a regular capture in the default inbox:

```sh
plaesy memory --vault <vault-path> remember "Title" "Markdown body"
```

Use a vault template for a managed note. Template paths are resolved under `<vault>/templates/`;
repeat `--set KEY=VALUE` to fill placeholders. Add `--folder` when the template has no default
destination:

```sh
plaesy memory --vault <vault-path> remember "Knowledge title" \
  --template knowledge \
  --folder knowledges/system-design \
  --set ONE_SENTENCE_SUMMARY="A concise statement of the idea."
```

For a regular note, omit the body to read Markdown from stdin. `remember` creates a new file and
does not overwrite an existing note. The `--type`, `--id`, `--status`, `--project`, `--scope`, and
`--supersedes` options attach graph metadata when appropriate; check `plaesy memory remember --help`
for the required combinations.

### Update a note

Read the note's current fingerprint, review and prepare the complete replacement Markdown, then
write it only if the source still matches that fingerprint:

```sh
plaesy memory --vault <vault-path> fingerprint knowledges/system-design.md
plaesy memory --vault <vault-path> update knowledges/system-design.md \
  --if-match <sha256-from-fingerprint> --reason new-evidence --yes < revised-note.md
```

`update` reads the replacement from stdin. Supported reason categories are `correction`,
`new-evidence`, `clarification`, and `other`; free-form reason text is not stored. It refuses stale
fingerprints and changes to a note's stable `id`, writes the replacement atomically, records an
intent/completion event with old/new hashes, and refreshes the local index. The durable index
outbox lets startup reconciliation finish if the process stops after the file replacement. The
audit contains no note body. Keep the note's front matter in the replacement; invalid YAML is
rejected before publication.

### Archive a note

Archive means changing the note's front-matter status to `archived`. The source file stays in place,
so links remain valid; default retrieval hides it, and `--include-inactive` can still retrieve it.
Review the fingerprint first because archiving edits the note and reindexes it:

```sh
plaesy memory --vault <vault-path> fingerprint knowledges/old-system-design.md
plaesy memory --vault <vault-path> archive knowledges/old-system-design.md \
  --if-match <sha256-from-fingerprint> --reason superseded --yes
```

Supported reasons are `completed`, `obsolete`, `superseded`, `consolidated`, and `other`. The
archive event stores the note id, scope, state transition, and old/new hashes, never its body. To
restore an archived note, use `update` with a reviewed replacement that sets `status: active`.

### Forget a note

Permanently remove one source file from the vault and its current search index:

```sh
plaesy memory --vault <vault-path> forget knowledges/obsolete-note.md \
  --reason owner-request --yes
```

`--yes` is required because this removes the source file. Plaesy first writes an intent event and
tombstones under the vault's hidden `.plaesy-memory/` directory, then removes the file and its
indexed chunks. Tombstones carry the note id when available, a hash of its relative path, scope,
timestamp, a content hash, and a structured reason category; they do not copy note content or
persist free-form reason text. Indexing and `remember` both
honour these markers, so a Syncthing conflict copy with the same stable note id or content is not
reintroduced into retrieval. `--reason` accepts `owner-request`, `duplicate`, `obsolete`, or
`privacy`.

This removes the active source and Plaesy index entry. It does not rewrite Git history or erase
Syncthing's retained file versions: when Syncthing versioning is enabled, remote deletions can be
stored in `.stversions`. See [Syncthing file versioning](https://docs.syncthing.net/users/versioning.html).

## Validate the index

```sh
plaesy memory --vault <vault-path> index
plaesy memory --vault <vault-path> hygiene <vault-path> --strict
plaesy memory --vault <vault-path> stats
```

`hygiene` reports stale or incomplete provenance (`source`, `last_verified`, and `confidence` for
claim-bearing tiers), broken wikilinks and Markdown links, invalid link
anchors, likely credential assignments and private-key headers without printing matched values, and
backticked path references that may be missing. `possible_secret` is inferred: the keyword and
assignment heuristic can still flag examples or redacted values, and it is not a secret scanner.
`dead_path_reference` is inferred: Plaesy checks the note's directory and parent folders up to the
project root, plus the vault root. It can still point to another repository or a path meaningful
only in its original context, so review it separately from broken Markdown links. The command does
not modify source notes; it may update the local search
index. `--strict` returns a failing exit status when any finding exists, including inferred findings.
Use `--strict-facts` when a gate should fail only on confirmed findings while still printing or
emitting inferred findings for review. The two modes are mutually exclusive.

When one or more project repositories are checked out separately, pass their local roots for that
project so repository-relative paths can be checked too. Repeat the option with the same project
slug to map multiple repositories. Plaesy also accepts one leading directory when it matches a
mapped checkout's directory name (for example, `cli/src/main.rs` when the checkout is named
`cli`). These mappings are supplied per invocation and are not written into the vault:

```powershell
plaesy memory --vault "<vault>" hygiene "<vault>" --project-root "plaesy-cli=<checkout-one>" --project-root "plaesy-cli=<checkout-two>"
```
