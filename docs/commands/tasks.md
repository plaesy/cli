# `tasks`

## Initialize a project

Run `plaesy init [PATH]` to create the `.plaesy/` marker expected by task commands. The path defaults to the current directory; use `--dry-run` to preview. Init is safe to repeat, refuses to replace an existing non-directory `.plaesy` path, and refuses to create a nested Plaesy project. It does not edit Git ignore rules, Memory settings, or agent instruction files. Task data lives under `.plaesy/tasks/`; decide whether those files should be shared with your repository before committing them.

Task lifecycle commands require the Plaesy executable; init does not install it. Task files are stored as Markdown and JSON, but a complete manual workflow without the CLI has not been established.

```bash

plaesy tasks [subcommand]
```

Task lifecycle management for `.plaesy/tasks/{backlog,todo,doing,done,blocked}/`.
Every task lives in a folder with `task.md`; its frontmatter identifies the spec/topic. Tasks created by `tasks create` start with editable Goal, Scope, Non-goals, Deliverables, Dependencies, and Acceptance criteria sections. Fill them in for the task; the template does not invent task-specific requirements. The topic specification and plan live separately under `.plaesy/spec/<topic>/` and `.plaesy/plan/<topic>/`.
`--spec <name>` filters tasks by spec. `tasks check` resolves referenced paths and only considers existing targets inside the canonical project root; relative links within the project (including `../` paths that stay inside) remain valid. Paths outside the project or escaping through symlinks are reported as missing.

## Subcommands

| Subcommand | Args | Description |
|---|---|---|
| `create` | `<priority> <category> <title>` | Create a task in the backlog, for the current spec (created first if missing) |
| `list` | `[--all]`, `[--sync-context]` | List tasks across every status; `--all` shows every spec with integrity problems; `--sync-context` rewrites the In-Flight section of `.plaesy/context.md` |
| `board` | | Show every spec's tasks as text columns, one per status folder, with a count per column; titles longer than 60 characters are cut with `…` |
| `list-status` | `<status>` | List tasks in one status |
| `create-subtask` | `<parent> <priority> <category> <title>` | Create a backlog task of type `subtask` under an existing parent. A missing parent is refused and no task is created |
| `children` | `<parent>` | List the direct child issues of a parent with type, status, and title; grandchildren are not included. Refuses an unknown parent or an invalid issue collection |
| `issues` | `[--spec NAME]`, `[--type TYPE]`, repeatable `[--label LABEL]`, `[--assignee NAME]`, `[--milestone NAME]`, `[--status STATUS]`, `[--dependencies]` | Filter issue metadata and optionally display dependency readiness (derived; does not rewrite lifecycle status) |
| `update-issue` | `<task-id>`, metadata flags, explicit `--clear-*` options | Update issue metadata while preserving fields not supplied; refuses invalid types, missing references, and cycles |
| `label add` | `<task-id> <LABEL>...` | Add labels to one issue; labels it already has are skipped |
| `label remove` | `<task-id> <LABEL>...` | Remove labels from one issue; labels it lacks are skipped |
| `next` | — | Move the next backlog task into `todo` and report it |
| `start` | `<task>` | Move task to `doing` |
| `complete` | `<task>` | Move a task from `review` to `done`. A reviewer runs this after checking the evidence. Refuses any other status. |
| `block` | `<task>` | Move task to `blocked` |
| `unblock` | `<task>` | Unblock a task |
| `move` | `<task_file> <from> <to>` | Move between arbitrary statuses |
| `show` | `<task_file> [status]` | Display task content |
| `run` | `--chain <name>` or `--provider <name>`, `[--max N]`, `[--parallel N]`, `[--timeout D]`, `[--confirm]` | Claim todo tasks of the current spec (highest priority first) and send each to a delegate provider or chain, in the project root. `--parallel N` runs up to N tasks at once in the same project directory; it does not create an isolated branch/worktree per task. Avoid parallel tasks that edit overlapping files. Output goes to `<task>/runs/`; the task stays in `doing` until reviewed, and a failed run moves it to `blocked` |
| `validate` | `[task]`, `[--spec <folder>]` | Run the checklist of each unfinished task (`contracts.json`) and record each result. A task whose checks all pass and whose items invoke the `plaesy` binary moves to `review`. It never moves to `done`. |

`complete` is the only way to reach `done`, and it is reserved for a reviewer. It refuses a task that is not in `review`. `validate` moves a passing task to `review` and never to `done`.

`update-issue` supports `--type`, `--parent` / `--clear-parent`, repeatable `--dep` / `--clear-deps`, repeatable `--label` / `--clear-labels`, `--assignee` / `--clear-assignee`, and `--milestone` / `--clear-milestone`. Repeated `--dep` and `--label` values replace their respective lists; use `--clear-*` to remove them. Parent types are unrestricted: hierarchy is independent of issue type and dependency edges. Parent references must exist, cannot point to the issue itself, and parent/dependency graphs must remain acyclic.

### Concurrent claims

`tasks run` uses a persistent per-task OS advisory lock under `.claims/` while claiming tasks through `backlog → todo → doing`. The lock file is not removed or replaced. On Unix, symbolic links at either `.claims/` or an individual lock-file path are rejected. This coordinates only cooperating workers on a filesystem whose locking semantics are suitable; network/shared filesystem behavior is not verified. Old and new claim implementations must not run concurrently during an upgrade because their lock protocols do not interoperate. Keep `.claims/` writable only by trusted project users and do not remove, replace, or rename lock files: advisory locks attach to the opened file, so replacing its path can let another worker lock a different file for the same task. Process termination releases the OS lock, but interruption between lifecycle moves may leave a task in an intermediate state that needs inspection.

`tasks update-issue` takes a persistent collection-level advisory lock at `.claims/issue-metadata.lock` before loading metadata and holds it through validation and rollback. Concurrent updates using this CLI serialize, preventing lost updates between these cooperating invocations. This does not coordinate arbitrary tools, older binaries, or lifecycle operations that write the same `issue.json` without taking this lock. The lock file must not be removed or replaced; `.claims/` must be trusted. Network/shared filesystem semantics are not verified.

Validation prints the running Plaesy version and binary path. It warns when the `plaesy` executable found on `PATH` resolves to a different file; the warning does not change validation results.

Valid statuses: `backlog`, `todo`, `doing`, `review`, `done`, `blocked`. `review` means the checks passed and a reviewer decides. `blocked` is only for a task waiting on something outside the work.

## `contracts.json` expectations

Each checklist item has an `expected` value. Its command must exit with code `0`; except for
`pass`/`yes`/`true`/`present`, the CLI compares the last non-empty output line with `expected`.

**Trust boundary:** `tasks validate` executes each checklist item's `run` command through Bash in the project root, with the permissions of the Plaesy process. Treat `contracts.json` as executable code: validate only repositories and task metadata you trust. This is not a sandbox; a failed check may still have changed project files or caused other side effects.

The output comparison and behavior-evidence rule do not limit what a checklist command can do; use checks with effects you understand and avoid destructive commands.

**Simulation safety:** when testing validation behavior, use a disposable project directory and synthetic fixtures. Never point destructive checks at a real vault, credentials, production services, or a shared working tree.

| Expected value | Pass condition |
|---|---|
| `pass`, `yes`, `true`, `present` | Command exits with code `0` |
| `zero`, `none` | Output is `0` or `none` |
| `no`, `false`, `absent` | Output is `0`, `none`, `no`, `false`, or `absent` |
| `all`, `every`, or a qualifier such as `all routes` | Output is `pass` or a non-zero `N of N` count |
| A comparison such as `>= 9` or `<= 2 s` | Last output line is a matching number; time units convert between `ms`, `s`, `min`, and `h` |

Unsupported values do not pass and the validation history reports the accepted forms.

See [command index](README.md).
