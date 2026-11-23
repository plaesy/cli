# `delegate`

Send a bounded sub-task to a model provider configured under `delegate.providers`.
Only enabled providers run. A billable provider (for example the Anthropic API) also
needs `--confirm`, and it is refused without it.

```bash
plaesy delegate --list                          # providers, enabled and billable state
plaesy delegate --provider claude-cli --prompt "summarise docs/reference.md"
plaesy delegate --chain local --prompt "..."    # named fallback chain
plaesy delegate report                          # token usage and cost per provider, chain, and task
```

Usage is logged to `.plaesy/analysis/delegate-usage.jsonl`, one line per answered call. Each line
records the provider, chain, task (for `plaesy tasks run`), tokens, and wall time.

Cost is computed only from prices in the local config, and is never fetched from the network. Prices
are micro-USD per million tokens (USD 3 per million is `3000000`):

```json
{
  "name": "anthropic",
  "billable": true,
  "input_micro_usd_per_mtok": 3000000,
  "output_micro_usd_per_mtok": 15000000,
  "daily_token_budget": 2000000,
  "daily_cost_budget_micro_usd": 5000000
}
```

A billable provider is refused once the day's estimate would exceed `daily_token_budget` (tokens) or
`daily_cost_budget_micro_usd` (micro-USD). A `0` value means no cap. A cost budget needs a price, and
is refused without one. A call with unknown token counts, or with no price, has no cost and is counted
under `NO_COST` in the report.
