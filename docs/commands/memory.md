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
retrieval (`search`, `links`), health (`hygiene`, `missing`, `stats`), configuration and service
setup (`config`, `onboard`, `serve`, `schedule`), source-code lookup (`code-find`, `code-audit`),
and project graph views (`projects`, `decisions`, `contexts`).

Use the installed version's help for command-specific arguments:

```sh
plaesy memory search --help
plaesy memory hygiene --help
plaesy memory remember --help
```

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

## Validate the index

```sh
plaesy memory --vault <vault-path> index
plaesy memory --vault <vault-path> hygiene <vault-path> --strict
plaesy memory --vault <vault-path> stats
```

`hygiene` reports stale or incomplete metadata, broken wikilinks and Markdown links, invalid link
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
