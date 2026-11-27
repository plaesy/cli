---
name: using-plaesy
description: Runs the Plaesy CLI (`plaesy`) correctly for Markdown memory, the `.plaesy/` task lifecycle, inter-agent relay, MCP servers, delegate providers, validation, and install health. Use when running any `plaesy` command, searching or writing Plaesy memory notes through the CLI or the memory MCP tools (memory_search, memory_remember, memory_update), creating, starting, validating, or completing `.plaesy/tasks`, messaging other agents through the relay, claiming files or reporting sessions, serving Plaesy over MCP, ACP, or HTTP, delegating a prompt to a model provider, linting Markdown with `plaesy validate`, or diagnosing an installed `plaesy.exe` that lags behind the source.
license: MIT
compatibility: Requires the plaesy executable on PATH or a build from this repository. Some commands need git, Bash, network access, or configured providers.
metadata:
  cli-version: "0.0.1"
  command-tree: "plaesy --help at 6560894"
---

# Using Plaesy

`plaesy` is one executable with about forty command groups. The executable is authoritative: run `plaesy <command> --help` (and `plaesy <command> <sub> --help`) before using a flag you have not seen in its help. The files in `references/` describe purpose, syntax, side effects, and checks for each command.

## Before any command

1. Pick the binary. `plaesy version` prints the running and installed paths. The installed copy (`%LOCALAPPDATA%/Plaesy/bin/plaesy.exe` on Windows, `~/.local/bin/plaesy` on Unix) can lag behind a source checkout. In the Plaesy repository, build with `cargo build -p plaesy --locked` and run `target/debug/plaesy.exe` when a recent change matters.
2. Put global options before the subcommand: `plaesy --plain tasks list`, `plaesy memory --vault <PATH> search "q"`.
3. Select the memory vault explicitly for every write: `plaesy memory --vault <PATH> ...` or `plaesy memory --project ...`. Run `plaesy memory config` to see which vault is configured.
4. Task commands act on one spec. `--spec` defaults to the current git branch name, so on an unexpected branch pass `--spec <name>`.

## Command map

Read the linked reference before the first use of a command in a session, and always before a command marked **write**, **network**, or **destructive**.

### Memory and knowledge

| Command | Use it to | Reference |
|---|---|---|
| `memory` | Search, list, and write Markdown notes; hygiene and curation; MCP and JSON service (**write**) | [references/memory.md](references/memory.md) |
| `onboard` | First-run vault, settings, model download, and first index (**write**, **network**) | [references/onboard.md](references/onboard.md) |
| `trim` | Compress command output and prose files to save context (**write** for `compress`) | [references/trim.md](references/trim.md) |
| `analyze` | Build and query a project dependency and symbol graph under `.plaesy/analysis/` | [references/analyze.md](references/analyze.md) |

### Tasks and project state

| Command | Use it to | Reference |
|---|---|---|
| `init` | Create the `.plaesy/` marker in a project (**write**) | [references/init.md](references/init.md) |
| `tasks` | Create, move, validate, and complete tasks under `.plaesy/tasks/` (**write**; `validate` runs Bash) | [references/tasks.md](references/tasks.md) |
| `export` | Write the task list or delegate run summary as Markdown or CSV | [references/export.md](references/export.md) |
| `dashboard` | Write a static HTML dashboard of tasks and delegate usage | [references/dashboard.md](references/dashboard.md) |
| `voice` | Parse a transcribed sentence into a task command (prints only) | [references/voice.md](references/voice.md) |
| `loop` | Run a bounded autonomous loop from `.plaesy/loop.json` (**write**, runs commands) | [references/loop.md](references/loop.md) |

### Agents and coordination

| Command | Use it to | Reference |
|---|---|---|
| `relay` | Send, wait for, claim, and acknowledge durable messages between agents (**write**) | [references/relay.md](references/relay.md) |
| `gateway` | Poll the relay for one agent name and print new messages | [references/gateway.md](references/gateway.md) |
| `agent` | Serve the agent-to-agent HTTP protocol on loopback | [references/agent.md](references/agent.md) |
| `files` | Declare advisory claims on project files | [references/files.md](references/files.md) |
| `sessions` | Publish and list self-declared session state | [references/sessions.md](references/sessions.md) |
| `projects` | Register and list local Plaesy projects | [references/projects.md](references/projects.md) |
| `workspace` | Group registered projects into named workspaces | [references/workspace.md](references/workspace.md) |
| `skills` | List and copy skills between Claude, Codex, agents, Hermes, and Plaesy stores | [references/skills.md](references/skills.md) |

### Models and providers

| Command | Use it to | Reference |
|---|---|---|
| `delegate` | Send a prompt to a configured provider or chain; usage report (**network**, may bill) | [references/delegate.md](references/delegate.md) |
| `advisor` | Compare options or triage one task with a provider (read-only on tasks) | [references/advisor.md](references/advisor.md) |
| `decisions` | Typed choices from the local Laya decision model (**network** for `model pull`) | [references/decisions.md](references/decisions.md) |
| `chat` | Interactive multi-turn chat with a provider, tools, and relay name | [references/chat.md](references/chat.md) |
| `chat-schedule` | Install OS timers that rerun saved chat goals (**write**) | [references/chat-schedule.md](references/chat-schedule.md) |
| `acp` | Serve an ACP v1 agent over stdio backed by a provider | [references/acp.md](references/acp.md) |
| `images` | Generate an image file through a provider API (**network**, may bill) | [references/images.md](references/images.md) |
| `reach` | Query external platforms such as web, GitHub, and YouTube (**network**) | [references/reach.md](references/reach.md) |
| `doctor` | Check install, vault, reach platforms, and provider status | [references/doctor.md](references/doctor.md) |

### Servers and integrations

| Command | Use it to | Reference |
|---|---|---|
| `mcp` | Serve a project's `.plaesy/` folder or the shared memory over MCP | [references/mcp.md](references/mcp.md) |
| `remote` | Serve a read-only HTTP status and task API | [references/remote.md](references/remote.md) |
| `approvals` | Approve, revoke, or check external action ids (**write**) | [references/approvals.md](references/approvals.md) |
| `audit` | Read the audit trail of authorization decisions | [references/audit.md](references/audit.md) |
| `telemetry` | Summarize local opt-in telemetry | [references/telemetry.md](references/telemetry.md) |

### IR and code generation

| Command | Use it to | Reference |
|---|---|---|
| `import` | Turn a SQLite schema or Rust/Python source into an IR document | [references/import.md](references/import.md) |
| `codegen` | Generate JS or Go from a `flow-ir/1` document | [references/codegen.md](references/codegen.md) |
| `openapi` | Generate OpenAPI 3.1 from an `api-ir/1` document | [references/openapi.md](references/openapi.md) |

### Validation

| Command | Use it to | Reference |
|---|---|---|
| `validate` | Lint Markdown, check links, contracts, constitution, OOXML files, design output | [references/validate.md](references/validate.md) |

### Install and health

| Command | Use it to | Reference |
|---|---|---|
| `status` | Show version, git, install path, and PATH health | [references/status.md](references/status.md) |
| `version` | Print version and running and installed paths | [references/version.md](references/version.md) |
| `install` | Copy the running binary to the per-user bin directory (**write**) | [references/install.md](references/install.md) |
| `upgrade` | Replace the installed binary with the running newer build (**write**) | [references/upgrade.md](references/upgrade.md) |
| `repair` | Restore a missing or different installed binary (**write**) | [references/repair.md](references/repair.md) |
| `update` | Compare the installed version with a local manifest | [references/update.md](references/update.md) |
| `uninstall` | Remove the installed binary and PATH entry (**destructive**) | [references/uninstall.md](references/uninstall.md) |

## Core workflows

### Recall before work

```sh
plaesy memory config                                   # which vault is configured
plaesy memory --vault <VAULT> search "<topic>" -n 5    # cite path and line range from hits
plaesy memory --vault <VAULT> projects <project-id> decisions
```

When a Plaesy memory MCP server is connected, prefer its tools (`memory_search` with `query`, `limit`, `path_prefix`) over the CLI; the server already knows its vault. Treat hits marked `INFERRED` or `STALE` as leads, and confirm consequential claims in the project's own files.

### Write or change a memory note

1. Search first, so an existing note is updated instead of duplicated.
2. New note: `plaesy memory --vault <VAULT> remember "Title" "Body" --folder <folder>`, or `--template <name>` with `--set KEY=VALUE`. `remember` never overwrites.
3. Existing note: `plaesy memory --vault <VAULT> fingerprint <path>`, edit the full text, then pipe it to `plaesy memory --vault <VAULT> update <path> --if-match <sha256> --reason <reason> --yes`.
4. Read it back with `search` or `list --path <prefix>`.

Through MCP the same steps are `memory_search`, then `memory_remember` (`title`, `body`, optional `folder`, `note_type` with `id`), or `memory_fingerprint` followed by `memory_update` (`path`, `expected_sha256`, full `content`, `reason`). `memory_remember_session` saves a session checkpoint. `forget` and templates exist only in the CLI.

Details, the full MCP tool table, templates, and decision supersession rules: [references/memory.md](references/memory.md).

### Move a task through its lifecycle

```text
backlog --next--> todo --start--> doing --validate (all checks pass)--> review --complete--> done
                                    \--block--> blocked --unblock--> todo
```

- `tasks validate` runs every `contracts.json` item through Bash in the project root. Read that file before validating a task you did not write.
- A passing task moves to `review`, never to `done`. Only `tasks complete` moves `review` to `done`, and it is a reviewer's decision.
- Each checklist item without `"evidence": "build"` or `"doc"` must run `plaesy` (or `cargo test`), or validation reports missing behaviour-level evidence and the task stays put.

Details: [references/tasks.md](references/tasks.md).

### Coordinate with another agent

Use `relay send` with a stable `--id` (retries are idempotent), `relay claim` plus `relay ack --lease-token` on the receiving side, `files claim` before editing a shared file, and `sessions status` / `heartbeat` to publish what you are doing. All agent names are self-declared: these tools coordinate cooperating processes and do not authenticate anyone.

## Rules

- Do not guess flags. Check `--help`; the reference files can trail the binary.
- In JSON and MCP modes stdout is a protocol. Diagnostics go to stderr; do not mix log text into piped stdout.
- Billable providers (`delegate`, `advisor`, `chat`, `acp`, `tasks run`) refuse to run without `--confirm`. Pass it only when the user accepted the cost.
- Confirm with the user before **destructive** actions: `memory forget`, `uninstall`, `decisions model remove`, `workspace delete`, `tasks comments --delete`, and `trim compress` without `--dry-run`.
- Network use is explicit: `reach`, `delegate`, `images`, `decisions model pull`, `advisor --laya-endpoint`, and servers bound beyond loopback. Never send private notes or credentials to a provider without the user's approval.
- On Windows a running `plaesy mcp` or `plaesy memory serve` keeps `plaesy.exe` locked. Stop it before `install`, `upgrade`, or `repair`.
- Report failures with the exact error. Do not claim a write, validation, or install succeeded until a read-back or status check shows it.

## Verify

| After | Check |
|---|---|
| Memory write | `plaesy memory --vault <VAULT> search "<title>"` returns the note |
| Task move | `plaesy tasks show <task>` or `plaesy tasks board` shows the new status |
| Relay send | `plaesy relay inbox --agent <recipient>` lists the message |
| Install, upgrade, repair | `plaesy version` shows the expected installed path and version |
| Markdown edit | `plaesy validate markdown <paths> --no-baseline` reports zero violations |
