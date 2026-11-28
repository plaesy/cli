# plaesy delegate

Sends one bounded prompt to a model provider or an ordered fallback chain configured under `delegate.providers` and `delegate.chains` in the project config: `.plaesy/config.toml`, or `.plaesy/settings.json` while `config.toml` is absent. A missing file or section gives the built-in defaults. Also reports token usage and cost.

## Syntax

```sh
plaesy delegate --list [--path <DIR>]
plaesy delegate (--provider <NAME> | --chain <NAME>) --prompt "<text>" [--timeout 120s] [--max-tokens 0] [--confirm] [--out <FILE>] [--path <DIR>]
plaesy delegate report [--path <DIR>]
```

| Option | Meaning |
|---|---|
| `--list` | Show providers, whether each is enabled, and whether it is billable |
| `--provider` / `--chain` | Choose one provider, or a chain that moves on only after a retryable failure |
| `--confirm` | Required for a billable provider; refused without it |
| `--max-tokens` | `0` uses the provider default; CLI providers ignore it |
| `--out` | Write the JSON result to a file instead of stdout |

## Provider configuration

The same structure in `settings.json` form:

```json
{
  "delegate": {
    "providers": [
      {"name": "claude-cli", "type": "cli", "enabled": true, "command": "claude", "args": ["-p"]},
      {"name": "codex-cli", "type": "cli", "enabled": true, "command": "codex", "args": ["exec", "--ephemeral", "-"]}
    ],
    "chains": {"local": ["claude-cli", "codex-cli"]}
  }
}
```

Provider types include `cli`, `anthropic`, `openai`, and `ollama`. `claude-cli` is enabled by default; `codex-cli` is built in but disabled. A billable provider can set prices (`input_micro_usd_per_mtok`, `output_micro_usd_per_mtok`) and daily caps (`daily_token_budget`, `daily_cost_budget_micro_usd`; `0` means no cap). `fallback_models` are tried on the same endpoint after a transient or rate-limit error.

## Examples

```sh
plaesy delegate --list
plaesy delegate --provider claude-cli --prompt "Summarise docs/commands/relay.md in five bullets"
plaesy delegate --chain local --prompt "..." --out .plaesy/analysis/answer.json
plaesy delegate report
```

## Side effects

- Sends the prompt to the provider: do not include secrets or private notes without the user's approval.
- Appends one line per answered call to `.plaesy/analysis/delegate-usage.jsonl`.
- Cost is computed from local prices only; nothing is fetched.

`plaesy delegate --help` is authoritative.
