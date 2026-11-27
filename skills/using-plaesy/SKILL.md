---
name: using-plaesy
description: Use the Plaesy CLI (plaesy.exe) correctly in this repository or any Plaesy project. Covers choosing between memory, tasks, relay, and status commands, selecting the vault, reading --help before guessing arguments, and checking results. Load before running any plaesy command, searching or writing Plaesy memory, or managing .plaesy/ tasks.
---

# Using Plaesy

Plaesy is a local CLI for Markdown memory, a task system under `.plaesy/`, and inter-agent relay. Use it as the source of truth for the commands it actually provides. Do not guess flags.

## Before running a command

1. Check the installed binary: `plaesy version` shows the running and installed paths. The installed `plaesy.exe` can lag behind this checkout. For a fix in this checkout, use `target/debug/plaesy.exe` or `target/release/plaesy.exe` only after building it with `cargo build -p plaesy --locked`.
2. Run `plaesy <command> --help` (or `plaesy <command> <sub> --help`) to confirm arguments. Do not invent flags.
3. Select the vault explicitly before a memory command: `plaesy memory --vault <PATH> <subcommand>` for the shared vault, or `plaesy memory --project <subcommand>` for this repository's `.plaesy` directory. Never rely on an implicit vault when writing notes.
4. Place global options such as `--plain` before the subcommand.

## Choosing the right command

| Need | Command |
|---|---|
| Installed version and PATH health | `plaesy status`, `plaesy version` |
| Search notes | `plaesy memory [--vault P] search "<query>"` |
| Note inventory and stats | `plaesy memory list`, `plaesy memory stats` |
| Project, decision, or context notes | `plaesy memory projects`, `decisions`, `contexts` |
| Note hygiene and gaps | `plaesy memory hygiene`, `curate`, `missing` |
| Create a new note | `plaesy memory remember "Title" "Body" --folder <folder>` |
| Change an existing note | `plaesy memory fingerprint <note>`, then `plaesy memory update` with the reviewed SHA-256 |
| Archive or delete a note | `plaesy memory archive` (preserves the file), `plaesy memory forget` (permanent; confirm first) |
| List, start, block, complete tasks | `plaesy tasks list`, `board`, `next`, `start`, `block`, `unblock`, `complete` |
| Task checks | `plaesy tasks validate`; a passing task moves to review |
| Task comments and labels | `plaesy tasks comment`, `label`, `issues` |
| Inter-agent messages | `plaesy relay` (durable, acknowledged, local) |

Tasks use `--spec <name>`; the default is the current git branch name.

## Rules

- A write needs a clear target. Use `remember` only for new notes. Update an existing note only after `fingerprint` and a reviewed replacement, so concurrent edits are caught.
- `tasks validate` runs each task's `contracts.json` through Bash. Treat that file as code: read it before validating a task you did not write.
- `forget` and `archive` change the vault. Confirm with the user before `forget`.
- Memory search results are citations (path, line range, revision). Verify a claim against the project's files before relying on it. Treat STALE or INFERRED hits as unverified.
- In JSON and MCP modes, stdout is a protocol. Send diagnostics to stderr.
- If a command fails or the vault is unavailable, report the exact error. Do not claim the write succeeded. Preserve the result in the project's documentation instead.

## Verify

After a write, confirm it can be read back: `plaesy memory --vault <PATH> search "<title>"` for notes, `plaesy tasks show <task>` for tasks. Report what was verified, not what was intended.
