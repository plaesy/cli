# plaesy advisor

Read-only, model-backed advice: compare explicit options, or triage one existing task. It uses a configured delegate provider or chain and never edits tasks or configuration. The answer is generated advice, not verified research.

## Syntax

```sh
plaesy advisor compare --prompt "<question>" --option <A> --option <B> [--option ...] \
  (--provider <NAME> | --chain <NAME> | --laya-endpoint <URL>) [--confirm] [--timeout 120s] [--max-tokens 0] [--path .]
plaesy advisor triage <TASK> (--provider <NAME> | --chain <NAME>) [--confirm] [--timeout 120s] [--path .]
```

- `compare` needs at least two `--option` values.
- `--laya-endpoint` posts to a running Laya SystemOne HTTP server and returns a typed choice with probabilities, without a narrative. Use a trusted local endpoint or HTTPS.
- `triage` accepts a task folder name or a unique title, reads `task.md` and `issue.json`, and recommends type, priority, rationale, uncertainties, and a question. It applies nothing.
- `--confirm` is required for a billable provider.

## Examples

```sh
plaesy advisor compare --prompt "Which storage format fits this CLI?" \
  --option "SQLite" --option "JSON files" --provider claude-cli
plaesy advisor triage low_task-example --chain local
```

## Next step

Apply a triage recommendation yourself, for example with `plaesy tasks update-issue`, after checking it.

`plaesy advisor <sub> --help` is authoritative.
