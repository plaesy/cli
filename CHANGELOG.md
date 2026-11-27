# Changelog

Format: [Keep a Changelog](https://keepachangelog.com). Versions follow SemVer once 0.1.0 is tagged; nothing is tagged yet.

## [Unreleased]

### Added

- The memory MCP HTTP server accepts one bearer token per client: `plaesy memory token add|rotate|revoke|list` manages named tokens stored as salted hashes in `~/.config/plaesy/mcp-tokens.toml`. Changes take effect without a restart, a rotated token keeps working for a grace period, and `PLAESY_MCP_TOKEN` still works as before.
- `plaesy agent serve` publishes an Agent Card at `GET /.well-known/agent.json`, with the name, description, version, capabilities, and skills of the agent protocol. It follows the same bearer-token rule as the other routes, and it carries no token or local path.
- `plaesy chat` `/goal <objective>` sets the goal and sends the objective as the next turn at once, so the provider starts work without another message. The judge after each turn decides whether to continue, as before. `/goal` alone still shows the goal, and `/goal clear` still removes it.
- The memory MCP server has `memory_fingerprint`, `memory_update`, and `memory_archive`. An agent reads a note's SHA-256 first; an update or archive is refused unless the hash still matches. Updates need a reason of correction, new-evidence, clarification, or other. Archives need completed, obsolete, superseded, consolidated, or other. The change is audited without note content and reindexed at once.
- Plan and threat model for the backend server (`.plaesy/plan/plaesy-backend-server/`), with nine slices and the network-path proposal in `docs/platform/decisions.md`. No server code changed yet.
- Project settings can live in `.plaesy/config.toml`, which allows comments. `plaesy chat`, the delegate providers, the memory engine, and local telemetry read it first and fall back to `.plaesy/settings.json` while it is absent. The first `/model` save copies `settings.json` into `config.toml` and leaves the JSON file unchanged. Home-level settings (`~/.plaesy/settings.json`) are still JSON.
- `plaesy chat` `/model` lists the configured providers with their models, and `/model <number>`, `/model <provider>`, or `/model <provider>@<model>` switches and saves the choice to `config.toml`.
- `plaesy chat` can trace each turn to a local file: set `PLAESY_TRACE=1` (writes `.plaesy/trace.jsonl`) or `PLAESY_TRACE=<path>`. Each line records the send, the reply's provider, size, and duration, or the error a provider returned. Message text is not recorded, and the chat screen is not affected.
- `plaesy chat --tui` status line shows how long the last reply took, once it is written.
- `plaesy chat --tui` shows a "waiting for the reply" line in the log, with the seconds so far, until the first text of the reply arrives.
- `plaesy chat` streams `claude-cli` replies as Claude writes them, and reports the input and output tokens of each reply. The default `claude-cli` arguments now ask for `--output-format stream-json`; a settings file that still lists only `-p` keeps the whole-reply behaviour.
- `plaesy chat` lets the model read a file or list a folder under the repository root, through a fenced `tool` block. Paths outside the root, `..`, and symlinks that leave it are refused; each reply runs at most 6 tool calls, and tool output is cut at 8 000 bytes. `write_file` and `run_command` (a program run directly, no shell, killed after 60 seconds) run only with `--allow-writes`; each call is shown in the chat, but there is no per-call approval yet. `/undo` restores the file before the last write of the session, or removes a file the write created. Use `--no-tools` to turn this off.
- `plaesy chat` estimates the context in use (about four characters per token, against a 32 000-token window) and `/context` prints it. Before a turn that would pass 80% of the window, the history is compacted into one summary first, the same as `/compact`. The status line shows the same percent. The window is a fixed estimate, not read from the provider.
- `plaesy chat` has `/skill <name>`, which loads a skill's `SKILL.md` into the session as a system message; `/skill` lists the loaded ones and `/skill clear` removes them.
- `plaesy chat` has a read-only `list_tasks` tool, and `/model` shows the provider and model in use. Changing the model in a session is not built yet.
- `plaesy chat --allow-writes` adds `create_task` and `schedule_once`. `web_fetch` is the only chat tool that uses the network; it is off unless `"chat": {"web": true}` is set in `settings.json`. `/subagent <task>` runs a task in its own git worktree under `.plaesy/subagents`; nothing is merged.
- `/resume` lists saved chats, continues one (`/resume <file>`), deletes one (`/resume delete <file>`), or deletes all (`/resume clear`). `/learn` proposes lessons and a skill from the session; `/learn save` records the lessons as decisions and writes the skill to the skills folder. Nothing is written until `save`.
- `plaesy chat --learn` asks the provider for a short summary of the session when it ends and saves it as a decision, through the same path as `/decision`.
- `plaesy chat` saves each session that has a turn to `.plaesy/chats/chat-<millis>.jsonl` when it ends (`--no-save` turns this off). `--resume <file>` starts from a saved session, and `/find <text>` searches all saved sessions. The files stay local.
- `plaesy chat` has `/todo`, a checklist for the session: add, mark done or undone, remove, or clear by number. It is kept in memory only.
- `plaesy chat` compaction keeps the last four turns verbatim and summarises the older ones; a short history is still summarised whole.
- `plaesy chat` says so when a provider answers with no text, instead of showing nothing.
- `plaesy chat --tui` shows markdown tables aligned in columns (a table too wide for the log stays as plain text), diff blocks in green, red, and cyan, fenced code with its language as a label and, for Rust, Python, JavaScript/TypeScript, and shell, keywords, strings, and comments coloured (other languages in green), rules, quotes, `[links](url)`, *italic*, ~~strikethrough~~, and numbered items with a bold number. Lines wrap to the log width, so the newest line stays visible, and bulleted items get a bullet. A `\|` or a pipe inside `code` stays in its table cell.
- `plaesy chat --tui` status line shows the provider and model in use, and follows `/provider` switches.
- `plaesy chat --tui` takes a multi-line message: Alt+Enter or Ctrl+J adds a line break, and Enter sends. The chat reads a block between `"""` lines as one message, in the line prompt too.
- `plaesy chat --tui` shows the session's input and output tokens in the status line, for providers that report them; replies are styled lightly (headings, `code`, **bold**).
- `plaesy chat` streams replies from HTTP providers (Anthropic, Ollama, OpenAI-compatible) as they are generated, so `--tui` and the line prompt show text while it arrives. CLI providers still return the whole reply at once.
- `plaesy chat --tui` shows the seconds since a line was sent while its reply is pending.
- `plaesy chat --tui` shows a busy indicator in the status line from when a line is sent until its reply is written.
- `plaesy chat --tui` recalls earlier lines with Up and Down when the command list is closed, and its status line shows the provider in use.
- Add a command list to `plaesy chat --tui`: typing `/` shows matching commands; Up and Down pick one, Enter or Tab completes it into the input without sending, and a second Enter sends. Esc closes the list.
- Add `plaesy chat --tui`, a full-screen view with the conversation, an input line, and a status line. The same chat loop runs underneath, so commands, hooks, and providers behave as in the line prompt. Enter sends, PageUp and PageDown scroll, and Esc ends the session after the current reply.
- Add `/lsp definition|references|hover <file>:<line>:<col>` to `plaesy chat`, read-only, answered by a language server from `lsp.servers` in settings.json. Repository servers need approval, as MCP servers do.
- Add `/providers` to list every registered provider with its status, and `/debug` to show what the session runs with.
- Add `/goal`, `/compact`, `/advisor <question>`, and `/council <question>` to `plaesy chat`. `/advisor` asks the provider in `chat.advisor` (settings.json) to review the conversation; `/council` asks every enabled provider the question alone and prints each answer. A task moved from `/tasks` fires the `task.changed` hook.
- Add `plaesy skills list` and `plaesy skills sync --from <p> --to <p> [--apply]` to inventory skills across Plaesy, Claude, Codex, agents, and Hermes, and copy missing skills without overwriting. Plaesy's store is `~/.plaesy/skills`; set it with `--dir`, `PLAESY_SKILLS_DIR`, or `skills.dir` in `~/.plaesy/settings.json`.
- Add `plaesy tasks board`, a text view of every spec's tasks as one column per status folder with a count per column and titles cut to 60 characters.
- Add optional global Codex and Claude lifecycle hooks and a shared `learning-loop` skill to retrieve Plaesy memory at session start and preserve durable project outcomes at closeout. MCP `memory_remember` now accepts typed graph metadata; read-only mode also rejects session-summary writes.
- Add `plaesy dashboard`: a static offline HTML page of task counts and delegate usage totals.
- Relay messages report a status (`menunggu`, `diambil`, `selesai`, `gagal`) and attempt count; a released or expired claim retries up to five attempts before it becomes `gagal`, and done messages can be purged after a retention period. The CLI does not yet expose release or purge.
- Add task comments: `plaesy tasks comment <id> <text>` appends a comment, `plaesy tasks comments <id>`
  lists them with their `@name` mentions, and `plaesy tasks comments <id> --delete <comment-id>` removes
  one. Comments are stored in `comments.jsonl` in the task folder.
- Add `plaesy tasks label add|remove` to change issue labels, and validate labels everywhere they are written or filtered: lowercase letters, digits, `-` or `_`, no spaces, at most 32 characters.
- Add a read-only `plaesy audit list` command and an audit-trail reader in the workflow run store, filtering entries by action and inclusive time range and printing the newest first as JSON lines.
- Offer the `.plaesy/` text files to MCP clients as resources (`resources/list`, `resources/read`, `resources/templates/list`), and answer malformed JSON-RPC messages with standard error codes.
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
- Add opt-in local telemetry: set `"telemetry": {"enabled": true}` in `.plaesy/config.json` to count command runs, errors,
  and durations in `.plaesy/telemetry.jsonl`; `plaesy telemetry summary` prints the totals. Nothing is sent over the network.
- Add `plaesy tasks board`, a text board with one section per status, a count per status, and task titles cut to 60 characters.
- Add `plaesy export tasks|runs --format md|csv [--out <file>]` to write the task list or the delegate run summary per provider as local Markdown or CSV. CSV fields that start with a formula character are prefixed with `'`.
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
- Agent protocol tasks move through `submitted`, `working`, `completed`, `failed`, and `canceled` as the recipient claims, acknowledges, or fails a message. `plaesy agent serve` reports the state over HTTP and SSE. Repeating a send with the same id returns the task's current state; a different body for the same id is refused with 409. An ack means the message was handled, not that the work succeeded. See the limits in `docs/platform/agent-protocol-design.md`.

### Fixed

- Memory settings files are read again in JSON as well as TOML. Commit `5d2986d` made the memory loader accept only `config.toml`, so an existing `settings.json` (for `--config` or in `.plaesy/`) failed with `invalid TOML`. The loader reads `config.toml` when it exists, falls back to `settings.json`, and still fails on a broken file. This restores the behavior from before `5d2986d`; it is a regression fix, not a new format.
- `plaesy chat --tui` no longer counts a line as still pending when a command such as `/compact` with an empty transcript skips the end of the turn loop.
- A model chain (`fallback_models`) passes streaming and message calls to each model. Before, only `complete` was forwarded, so chat fell back to one flattened, non-streamed call per model.
- `claude-cli` returns the answer text, not the raw `stream-json` lines, for `delegate`, the goal judge, tool turns, `/compact`, and `/advisor`. Before, those callers received the whole JSON stream as the reply.
- `plaesy chat --tui` counts one turn mark per user line. A `/goal` run that continues automatically no longer closes the line early, so the status stays busy until the goal ends.
- `plaesy memory serve` now reclaims a daemon lock left behind by a killed process. Before, it waited 30 seconds on that lock and then failed, so MCP clients timed out.
- `plaesy mcp memory start|stop|status|uninstall` now fail with a clear message instead of a build error; the background memory service is not available in this build, so use `plaesy mcp memory serve` in the foreground.

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
