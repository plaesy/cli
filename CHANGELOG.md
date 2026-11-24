# Changelog

Format: [Keep a Changelog](https://keepachangelog.com). Versions follow SemVer once 0.1.0 is tagged; nothing is tagged yet.

## [Unreleased]

### Added

- Offer the `.plaesy/` text files to MCP clients as resources (`resources/list`, `resources/read`, `resources/templates/list`), and answer malformed JSON-RPC messages with standard error codes.
- Add optional global Codex and Claude lifecycle hooks and a shared `learning-loop` skill to retrieve Plaesy memory at session start and preserve durable project outcomes at closeout. MCP `memory_remember` now accepts typed graph metadata; read-only mode also rejects session-summary writes.
- Add `plaesy tasks board`, a text view of every spec's tasks as one column per status folder with a count per column and titles cut to 60 characters.
- Add `plaesy dashboard`: a static offline HTML page of task counts and delegate usage totals.
- Relay messages report a status (`menunggu`, `diambil`, `selesai`, `gagal`) and attempt count; a released or expired claim retries up to five attempts before it becomes `gagal`, and done messages can be purged after a retention period. The CLI does not yet expose release or purge.
- Add task comments: `plaesy tasks comment <id> <text>` appends a comment, `plaesy tasks comments <id>`
  lists them with their `@name` mentions, and `plaesy tasks comments <id> --delete <comment-id>` removes
  one. Comments are stored in `comments.jsonl` in the task folder.
- Add `plaesy tasks label add|remove` to change issue labels, and validate labels everywhere they are written or filtered: lowercase letters, digits, `-` or `_`, no spaces, at most 32 characters.
- Add a read-only `plaesy audit list` command and an audit-trail reader in the workflow run store, filtering entries by action and inclusive time range and printing the newest first as JSON lines.
- `plaesy doctor` now checks the binary version, the memory vault, an orphaned daemon lock (not refreshed for over 15 seconds), and free disk space before the reach checks. The checks run offline.
- Record delegate cost per call from local token prices, report totals per task, and refuse a billable
  call that would exceed a daily cost budget (`daily_cost_budget_micro_usd`) as well as the token budget.
- Add `plaesy tasks create-subtask` and `plaesy tasks children` for parent-child task hierarchies. A
  missing parent is refused under the issue graph lock, so no orphaned subtask is created.
- Add `plaesy voice parse "<text>"`, which turns a transcribed Indonesian or English sentence into a create,
  complete, or list task command and prints it as JSON without running it. Audio capture and speech
  recognition are not included.
- Add a read-only `plaesy memory curate` review across hygiene, unanswered searches, exact duplicates,
  unlinked notes, and knowledge folders that may need an index; expose it through MCP and scheduled jobs.
- Consolidate the Plaesy CLI and memory engine into one `plaesy` binary, with memory, validation,
  task, install, and development reload commands.
- Add cross-platform install lifecycle coverage and CI/release workflows.
- Add `plaesy workspace` to group registered projects into named workspaces (create, delete, add, remove, list). Missing project roots are reported as `missing` and kept in their group.
- Add a synthetic secret-scanner regression corpus that reports precision and recall without using real credentials.
- Add `plaesy tasks board`, a text board with one section per status, a count per status, and task titles cut to 60 characters.
- Add `plaesy export tasks|runs --format md|csv [--out <file>]` to write the task list or the delegate run summary per provider as local Markdown or CSV. CSV fields that start with a formula character are prefixed with `'`.
- Add opt-in local telemetry: set `"telemetry": {"enabled": true}` in `.plaesy/config.json` to count command runs, errors,
  and durations in `.plaesy/telemetry.jsonl`; `plaesy telemetry summary` prints the totals. Nothing is sent over the network.
- Save a local version of a task's `task.md` and `issue.json` under `.plaesy/history/<task>/` before `plaesy tasks`
  start, complete, block, unblock, move, next, and update-issue change it; list them with `plaesy tasks history <task>`
  and restore one with `plaesy tasks restore <task> <version>`, which saves the current state first.
- Let a delegate provider list `fallback_models`, tried in order on the same endpoint after a transient or rate-limit error; a terminal error stops at once.
- Add `plaesy version`, which prints the version and the running and installed binary paths.
- Add `plaesy update --check --manifest <PATH>`, which compares the installed version with a local
  JSON manifest without network access. Downloading releases is not implemented.
- Add `memory_remember_session` to the memory MCP server: saves a session summary (title, date, summary, http(s) links) as an indexed note in the `sessions` folder, found by the existing search.
- Add `plaesy remote serve --port <n>`, a read-only HTTP API for the project's task board: `GET /health`, `GET /tasks` (JSON summary), and `GET /events` (Server-Sent Events). It binds to loopback by default; a non-loopback `--bind` requires a bearer token (`--auth-token` or `PLAESY_REMOTE_TOKEN`, at least 32 bytes).
- Add a global `--plain` flag (place it before the subcommand) and honor `NO_COLOR`: log lines then
  carry no ANSI color. `--plain` also turns off decorative glyphs for the call sites that use them.

### Fixed

- `plaesy tasks complete` now prints that a manual completion has no checklist verification when the task has no checklist items.

- Allow `memory curate` and scheduled curation to read the daemon-maintained index without attempting
  a conflicting vault sync.
- Detect additional provider-formatted credentials during memory hygiene without exposing matched values.

### Changed

- Move first-run setup to `plaesy onboard`; `plaesy memory onboard` now points to the new command.
- Replace the memory TOML config with `settings.json`: user settings at `~/.plaesy/settings.json` (the same on every OS)
  (written by `plaesy onboard`) and project settings at `.plaesy/settings.json`. Both use the same
  nested schema. `memory.toml` and `~/.hermes/memory.toml` are no longer read.
- The schedule snapshot is now `<config dir>/plaesy/schedule/settings.json`.
- Read `PLAESY_MCP_TOKEN` from `<root>/.plaesy/.env` (same `KEY=VALUE` format as Hermes) when the
  environment variable is not set. The environment variable still wins.

### Removed

- Remove the unused Spec-Kit commands and standalone Hermes Python plugin; Hermes can use the
  built-in JSON service or MCP integration.
