# plaesy chat

Interactive multi-turn chat with a configured delegate provider. It is meant for a person at a terminal; an agent should use `plaesy delegate` for one-shot prompts instead.

## Syntax

```sh
plaesy chat [--provider <NAME>] [--confirm] [--tui] [--role <ROLE>] [--agent <NAME>] [--lang en|id] \
  [--path .] [--vault <DIR>] [--project <ID>] [--resume <FILE>] [--no-save] [--no-tools] [--learn] \
  [--allow-writes [--supervised]] [--timeout 120s] [--max-tokens 0]
```

| Option | Meaning |
|---|---|
| `--provider` | Default: the first enabled, non-billable provider; `--confirm` allows a billable one |
| `--tui` | Full-screen terminal view instead of the line prompt |
| `--role` | A role from `roles` in settings.json (default `default`) |
| `--agent` | This session's relay name; overrides `agent.name` in settings.json |
| `--no-tools` | Turn off read tools (`read_file`, `list_dir`, `list_tasks`) |
| `--allow-writes` | Let tools write files and run commands (`write_file`, `run_command`, `create_task`, `schedule_once`) |
| `--supervised` | With `--allow-writes`, require a persisted approval (see `approvals.md`) for each mutating tool call |
| `--learn` | At exit, summarise the session and save it as a decision note |
| `--no-save` / `--resume` | Skip saving to `.plaesy/chats`, or resume a saved session by file name |

## In-chat commands

`/provider <name>`, `/model`, `/role`, `/clear`, `/compact`, `/context`, `/undo`, `/resume`, `/exit`; memory and decisions with `/decision <title> | <choice> | <reason>`, `/decisions`, `/ask`, `/pending`, `/learn`, `/find`; work with `/tasks`, `/todo`, `/goal`, `/subagent`, `/agent`, `/advisor`, `/council`, `/skill`, `/mcp`, `/plugin(s)`, `/lsp`, `/debug`.

## Safety

`--allow-writes` lets the model change the repository and run commands. Prefer `--supervised` with it. A saved goal can be scheduled with `chat-schedule`.

`plaesy chat --help` is authoritative.
