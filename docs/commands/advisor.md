# `advisor`

Read-only, model-backed advice for general option comparisons and triage of one existing task. Advisor uses the project's configured delegate provider or chain; it does not install models, edit configuration, or mutate tasks. Provider costs and confirmation follow `plaesy delegate` rules.

## Compare general options

```bash
plaesy advisor compare --prompt "Which storage format fits this CLI?" \
  --option "SQLite" --option "JSON files" \
  --provider claude-cli
```

Provide at least two `--option` values. The response is generated advice, not verified research; validate factual claims independently.

## Compare with Laya

Plaesy can call a running Laya SystemOne HTTP server directly and display its typed choice and probabilities, without routing through a text-generation delegate:

```bash
plaesy advisor compare --prompt "Which storage fits an offline-first notes app?" \
  --option "SQLite" --option "JSON files" \
  --laya-endpoint http://127.0.0.1:8000/v1/systemone
```

The Laya server and model must be installed and running separately. The request uses Laya's `state` plus `questions` `choice` contract; this mode does not produce a natural-language rationale. Plaesy rejects malformed responses and choices that were not among the supplied options. The endpoint is sent an HTTP POST; use a trusted local endpoint or HTTPS. This feature has not yet been runtime-tested against an installed Laya server.

## Triage an existing task

```bash
plaesy advisor triage low_task-example --provider claude-cli
```

Triage accepts an existing task folder name or a unique task title. It reads the task Markdown and optional `issue.json`, then recommends a type, priority, rationale, uncertainties, and a question. It does not apply those recommendations. The task can be in any lifecycle status. Supply `--path` when invoking outside the project, and choose exactly one of `--provider` or `--chain`. Use `--confirm` when the selected delegate is billable.

Advisor does not yet implement multi-model Council deliberation, a model catalog, model evaluation, automatic task updates, or tool installation.
