# `tasks`

```bash

plaesy tasks [subcommand]
```

Task lifecycle management for `.plaesy/tasks/{backlog,todo,doing,done,blocked}/`.
Every task of the project lives here, in folders. Each task records the spec it belongs to in its `spec.md`.
The spec is the current git branch, which is also its name. `--spec <name>` selects another.

## Subcommands

| Subcommand | Args | Description |
|---|---|---|
| `create` | `<priority> <category> <title>` | Create a task in the backlog, for the current spec (created first if missing) |
| `list` | `[--all]`, `[--sync-context]` | List tasks across every status; `--all` shows every spec with integrity problems; `--sync-context` rewrites the In-Flight section of `.plaesy/context.md` |
| `list-status` | `<status>` | List tasks in one status |
| `next` | — | Move the next backlog task into `todo` and report it |
| `start` | `<task>` | Move task to `doing` |
| `complete` | `<task>` | Move task to `done` |
| `block` | `<task>` | Move task to `blocked` |
| `unblock` | `<task>` | Unblock a task |
| `move` | `<task_file> <from> <to>` | Move between arbitrary statuses |
| `show` | `<task_file> [status]` | Display task content |
| `run` | `--chain <name>` or `--provider <name>`, `[--max N]`, `[--timeout D]`, `[--confirm]` | Claim todo tasks of the current spec (highest priority first) and send each to a delegate provider or chain, in the project root. Output goes to `<spec>/runs/`; the task stays in `doing` until reviewed, and a failed run moves it to `blocked` |
| `validate` | `[task]`, `[--spec <folder>]` | Run the checklist of each unfinished task (`contracts.json`), record each result, and move a finished task to `done`; tasks already in `done` are skipped |

Validation prints the running Plaesy version and binary path. It warns when the `plaesy` executable found on `PATH` resolves to a different file; the warning does not change validation results.

Valid statuses: `backlog`, `todo`, `doing`, `done`, `blocked`. `blocked` is only for a task waiting on something outside the work.

## `contracts.json` expectations

Each checklist item has an `expected` value. Its command must exit with code `0`; except for
`pass`/`yes`/`true`/`present`, the CLI compares the last non-empty output line with `expected`.

| Expected value | Pass condition |
|---|---|
| `pass`, `yes`, `true`, `present` | Command exits with code `0` |
| `zero`, `none` | Output is `0` or `none` |
| `no`, `false`, `absent` | Output is `0`, `none`, `no`, `false`, or `absent` |
| `all`, `every`, or a qualifier such as `all routes` | Output is `pass` or a non-zero `N of N` count |
| A comparison such as `>= 9` or `<= 2 s` | Last output line is a matching number; time units convert between `ms`, `s`, `min`, and `h` |

Unsupported values do not pass and the validation history reports the accepted forms.

See [CLI Reference](../reference.md).
