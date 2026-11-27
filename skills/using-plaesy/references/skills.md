# plaesy skills

Lists and copies agent skills between provider directories. Copying never overwrites: a skill is copied only when the target lacks it.

| Provider | Directory |
|---|---|
| `claude` | `~/.claude/skills` |
| `codex` | `~/.codex/skills` |
| `agents` | `~/.agents/skills` |
| `hermes` | `~/.hermes/skills` |
| Plaesy store | `--dir`, else `PLAESY_SKILLS_DIR`, else `skills.dir` in `~/.plaesy/settings.json`, else `~/.plaesy/skills` |

## Syntax

```sh
plaesy skills list [--dir <DIR>]
plaesy skills sync --from <PROVIDER> --to <PROVIDER> [--apply] [--dir <DIR>]
```

`sync` prints the plan only; add `--apply` to copy.

## Example

```sh
plaesy skills sync --from claude --to codex           # review the plan
plaesy skills sync --from claude --to codex --apply
```

## Verify

`plaesy skills list` shows the skill under the target provider.

`plaesy skills <sub> --help` is authoritative.
