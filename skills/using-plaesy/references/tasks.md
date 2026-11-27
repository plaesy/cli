# plaesy tasks

Task lifecycle under `.plaesy/tasks/<status>/<task>/`. A task is a folder with `task.md` (front matter names the spec and topic), `issue.json` (type, parent, dependencies, labels, assignee, milestone), and `contracts.json` (the checklist). The spec and plan for a topic live in `.plaesy/spec/<topic>/` and `.plaesy/plan/<topic>/`.

## Contents

- [Spec selection](#spec-selection)
- [Lifecycle](#lifecycle)
- [Subcommands](#subcommands)
- [Examples](#examples)
- [contracts.json](#contractsjson)
- [Delegated runs and claims](#delegated-runs-and-claims)
- [Verify](#verify)

## Spec selection

Every subcommand takes `--spec <NAME>`. The default is the current git branch name. On `main` or a feature branch whose name is not the spec, pass `--spec` explicitly or the command reports no tasks. Run `plaesy init` first if the project has no `.plaesy/`.

## Lifecycle

Statuses: `backlog`, `todo`, `doing`, `review`, `done`, `blocked`.

```text
create -> backlog --next--> todo --start--> doing --validate--> review --complete--> done
                                              |  ^
                                         block|  |unblock (back to todo)
                                              v  |
                                             blocked
```

- `validate` moves a task to `review` only when every check passes. It never moves a task to `done`.
- `complete` is the reviewer's step: it refuses any task that is not in `review`.
- `blocked` is for a task waiting on something outside the work; give a reason.
- `move <TASK> <FROM> <TO>` moves between arbitrary statuses. Prefer the named transitions.

## Subcommands

| Subcommand | Arguments and key options |
|---|---|
| `create` | `<PRIORITY> <CATEGORY> <TITLE>`; priority is critical, high, medium, or low; `--type issue\|story\|bug\|epic\|task\|subtask`, `--parent`, repeatable `--depends-on`, repeatable `--label`, `--assignee`, `--milestone` |
| `create-subtask` | `<PARENT> <PRIORITY> <CATEGORY> <TITLE>`; refuses a missing parent |
| `children` | `<PARENT>`: direct children only |
| `issues` | Filter by `--type`, repeatable `--label`, `--assignee`, `--milestone`, `--status`; `--dependencies` shows derived ready or blocked state |
| `update-issue` | `<ISSUE>` with `--type`, `--parent`/`--clear-parent`, repeatable `--dep`/`--clear-deps`, repeatable `--label`/`--clear-labels`, `--assignee`/`--clear-assignee`, `--milestone`/`--clear-milestone`; repeated values replace the list; refuses cycles and missing references |
| `label add\|remove` | `<ISSUE> <LABEL>...`; lowercase, no spaces, at most 32 characters |
| `comment` | `<TASK> <TEXT>`; `@name` mentions are stored |
| `comments` | `<TASK>`; `--delete <ID>` removes one comment (**destructive**) |
| `list` | `--all` shows every spec with integrity problems; `--sync-context` rewrites the In-Flight section of `.plaesy/context.md` |
| `board` | Text columns per status with counts |
| `list-status` | `<STATUS>` |
| `next`, `start`, `block`, `unblock`, `complete`, `move` | Lifecycle moves; `block <TASK> [REASON]` |
| `show` | `<TASK> [STATUS]`: raw content |
| `check` | `<TASK>`: missing paths and links in the topic's spec.md and plan.md |
| `validate` | `[TASK]`: run checklists of unfinished tasks |
| `run` | Hand todo tasks to a delegate provider or chain (see below) |
| `history`, `restore` | `<TASK>` / `<TASK> <VERSION>`: saved versions of task.md and issue.json; restore saves the current state first |

## Examples

```sh
plaesy tasks --spec plaesy-agent-platform board
plaesy tasks create high feature "Add relay lease renewal" --label relay --depends-on medium_chore-relay-schema
plaesy tasks next
plaesy tasks start high_feature-add-relay-lease-renewal
plaesy tasks validate high_feature-add-relay-lease-renewal
plaesy tasks complete high_feature-add-relay-lease-renewal    # reviewer only
plaesy tasks issues --label relay --dependencies
```

## contracts.json

This example was run through `validate` and `complete` in a disposable project:

```json
{
  "id": "high_feature-relay-ack-works",
  "checklist": [
    {
      "id": "task-file-present",
      "check": "Task file exists",
      "run": "test -s .plaesy/tasks/*/high_feature-relay-ack-works/task.md",
      "expected": "pass",
      "status": "pending",
      "evidence": "doc"
    },
    {
      "id": "inbox-empty",
      "check": "The test agent has no pending messages",
      "run": "plaesy relay inbox --agent test | tr -d '[]' | wc -w",
      "expected": "zero",
      "status": "pending"
    }
  ]
}
```

`tasks create` writes an empty checklist; fill it in before validating. Avoid backslash escapes in `run` strings where a simpler pipeline works.

- **Trust boundary.** `validate` runs every `run` string through Bash in the project root with your permissions. It is not a sandbox. Read the file before validating a task you did not write, and use disposable fixtures for destructive checks.
- `run` executes with the `plaesy` found on PATH; `validate` warns when that differs from the binary running the validation.
- A command must exit 0. For `pass`, `yes`, `true`, or `present` that is enough. Otherwise the last non-empty output line is compared with `expected`, and only these forms are accepted (a literal such as `"[]"` never passes): `zero`/`none` need `0` or `none`; `no`/`false`/`absent` accept `0`, `none`, `no`, `false`, or `absent`; `all`/`every` need `pass` or a non-zero `N of N`; comparisons such as `>= 9` or `<= 2 s` need a matching number (ms, s, min, h convert).
- **Behaviour evidence.** An item without `"evidence": "build"` or `"doc"` must run `plaesy`, `plaesy.exe`, or `cargo test` in one of its `&&`, `;`, or `|` segments. Otherwise validation fails with "lacks behaviour-level evidence".
- Each passing item keeps a receipt; a receipt that does not replay counts as not passed. Done tasks are skipped.
- `plaesy validate contracts` checks that every `contracts.json` loads, without running anything.

## Delegated runs and claims

```sh
plaesy tasks run --chain local --max 3 --parallel 1 --timeout 10m
plaesy tasks run --provider anthropic --confirm      # billable provider
```

`run` claims todo tasks of the current spec, highest priority first, through a per-task OS advisory lock under `.claims/`, and sends each to the provider. Output goes to `<task>/runs/`. The task stays in `doing` until reviewed; a failed run moves it to `blocked`. `--parallel` shares one working tree, so avoid tasks that edit the same files. Never remove or rename files under `.claims/`.

## Verify

- `plaesy tasks show <task>` or `plaesy tasks board` shows the expected status.
- `validate` prints `passed/total` per task and, on success, "moved to review".
- `plaesy tasks list --all` reports integrity problems across specs.

`plaesy tasks <sub> --help` is authoritative for flags.
