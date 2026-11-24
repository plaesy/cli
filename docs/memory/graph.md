# Projects, Decisions, Contexts — the cross-agent memory graph

Agents that use the same vault can retrieve the same project, decision, context,
and knowledge notes. They must still look up the relevant notes and record new
information explicitly; connecting an agent does not capture its conversations
or restore the last working state automatically.

No new database, no migration: a note becomes part of the graph purely by
declaring `type` in its frontmatter. Notes without `type` are unaffected and
still searchable as before — this is a strict opt-in addition on top of the
existing storage.

## Frontmatter contract

```yaml
---
type: project            # project | decision | context | knowledge | capture | preference
id: studio-a              # stable id, referenced by `project` / `scope` / `supersedes`
status: active            # see "Status values" below — per type
project: studio-a         # decision/context: the project it mainly belongs to
scope: [studio-a, studio-b]   # optional: also relevant to these projects, without being "owned" by them
supersedes: [old-decision-id] # decision only: ids of decisions this one replaces
title: Human readable title
---
```

- `id` is required for graph types. A graph note with no `id` is
  **skipped from every listing** (never crashes a command) and is reported by
  `plaesy memory hygiene` as `malformed_graph_note`.
- `project` is singular ownership. `scope` is a list for "also relevant to,
  without being owned by" — a decision reached via `scope` only (not
  `project`) is shown tagged `(shared)` when listed under that project.
- `supersedes` only applies to decisions and enables a reverse
  `superseded by: <title>` lookup on the decision it replaces.

Only the six graph types shown in the example participate in graph listings and relationship
checks. Vault metadata types such as `idea`, `company`, `task`, `notes`, `memory`, `proposal`, and
`validation-report` remain searchable but are not graph records. Files under a `templates/` folder
and files named `*.template.md` are excluded from the graph because their IDs and statuses are
placeholders.

### Status values

| type     | values                                   | hidden by default            |
|----------|-------------------------------------------|-------------------------------|
| project  | active, paused, completed, archived       | completed, archived          |
| decision | proposed, active, superseded, revoked     | superseded, revoked          |
| context  | active, archived                          | archived                     |

Missing `status` defaults to `active`. Pass `--include-inactive` or an
explicit `--status` to see hidden ones; `--status` always wins and shows
exactly what you asked for.

## Commands

```text
memory projects [<id>] [--include-inactive] [--format text|json]
memory projects <id|*> decisions [--include-inactive] [--status ...] [-n N]
memory projects <id|*> contexts  [--include-inactive] [--status ...] [-n N]
memory decisions [--project <id>]... [--status ...] [--include-inactive] [-n N]
memory contexts  [--project <id>]... [--status ...] [--include-inactive] [-n N]
plaesy memory hygiene <vault>   # also reports broken_project_reference, broken_scope_reference,
                          # conflicting_supersession, and malformed_graph_note
plaesy memory remember <title> [<body>] [--folder F] [--tier T] [--confidence C]
                [--type project|decision|context|knowledge|capture|preference]
                [--id ID] [--status S] [--project P] [--scope S]... [--supersedes ID]...
                [--json]
```

`plaesy memory remember` writes one new Markdown file (never overwrites) and
indexes it immediately. With `--type`, it adds the graph frontmatter and
validates the type, required id, and status before writing. Body is read from
stdin when omitted. The MCP `memory_remember` tool also creates an indexed note,
and accepts graph fields as `note_type`, `id`, `project`, `scope`, `status`,
and `supersedes`.
The MCP `memory_remember_session` tool saves a searchable session summary, not
a `type: context` graph note. Neither tool updates an existing note.

## Recording work today

This workflow can run through the optional global agent hooks described in
[operations](operations.md#6-automatic-agent-closeout). A project's
repository documents, task tracker, and code remain the source of truth for its
current state; a memory note is a pointer and summary to check against them.

1. Before work, inspect the relevant project with `memory projects <id>`, its
   decisions and contexts, and search for the task. Check linked project sources
   before treating a remembered status as current.
2. After substantive work, update the project's own source of truth first. Save
   a session summary with MCP `memory_remember_session` when the next session
   needs the outcome, open work, and source links. This is an episodic record,
   not the canonical current project state.
3. Preserve a durable decision as a `type: decision` note with a stable `id` and
   `project`; use `supersedes` when it replaces an earlier decision. Preserve a
   reusable fact or procedure as `type: knowledge`. Use `type: context` for
   project context that should outlive one session. Search first so a new note
   does not duplicate an existing one.
4. To change an existing note, get its hash with `memory fingerprint`, then use
   `memory update --if-match`; `remember` only creates notes.
   Change a project's `status` only for its lifecycle (`active`, `paused`,
   `completed`, `archived`). Put progress such as the current task, blockers,
   and next step in the note body or the project source, not in `status`.

See [memory commands](../commands/memory.md) for the create and conflict-checked
update syntax, and [session summaries](reference.md#memory_remember_session)
for the MCP input. Routine work with no durable outcome needs no new note.

### Agent automation

The optional Codex and Claude hooks prompt the active agent to retrieve relevant
memory at session start and perform the learning-loop closeout before its first
stop. The agent decides which verified outcomes are durable and writes them
without a user-run command. This depends on the agent following the hook and
having access to Plaesy; it is not background transcript extraction. The
scheduler still only indexes and audits notes. MCP supports typed graph create;
updates to existing notes use the CLI's conflict-checked update command.

### Id resolution

`<id>` accepts an exact id, a unique id prefix (git-style — e.g. `8f3a` for
`8f3a9c21-...`), or a case-insensitive title match.

- Ambiguous prefix → error listing every candidate, exit 1.
- No match → error with Levenshtein-based fuzzy title suggestions, exit 1.
- `*` means "every project", but **only directly before a subcommand**
  (`projects * decisions`). Used alone (`projects *`) it is an explicit
  error — never silently treated as a literal id, so scripts fail loudly
  instead of silently returning nothing.

### Filters vs. direct targets (important distinction)

- `projects <id>` (direct target): an unknown id is an **error**, exit 1.
- `--project <id>` (filter, on `decisions`/`contexts`): an unknown id
  **filters to an empty result**, exit 0, with a `note:` on stderr. This
  matches the existing `--tier` filter behavior and keeps scripts from
  breaking when a project is renamed or removed.
- Multiple `--project` flags are a union, not an intersection — consistent
  with `--tier`.

### Data integrity is availability-first

Two decisions both claiming to supersede the same decision, or a `supersedes`
id pointing nowhere, do **not** block the listing — both are shown with a
`⚠ conflicting supersession — run 'plaesy memory hygiene --strict'` warning instead.
Corruption is surfaced, never hidden, but a dirty vault still answers queries.
Run `plaesy memory hygiene` for the full detail and exact file paths.

## How agents share memory

Agents pointing at the same vault can retrieve notes written by another agent
after the vault is indexed. The `graph` module
(`crates/plaesy/adapter/memory/src/memory/graph/mod.rs`) reads the existing
`documents.metadata` JSON column, so graph listings require no schema
migration. Agents must still query the vault and verify current project sources
at the start of a task; the graph does not inject notes into their context.
