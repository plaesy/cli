# plaesy loop

Runs an autonomous loop for the current project until a Stop or Completion condition holds. Configuration lives in `.plaesy/loop.json`.

## Syntax

```sh
plaesy loop               # run the loop
plaesy loop stop-hook     # hidden: Stop hook for an AI client, reads hook input on stdin
```

## Configuration

Each iteration runs the `step` from `.plaesy/loop.json`:

```json
{"step": {"kind": "command", "command": "claude", "args": ["-p"], "timeout_seconds": 900}}
```

```json
{"step": {"kind": "openai", "base_url": "http://127.0.0.1:11434/v1", "model": "qwen3", "api_key_env": "OPENAI_API_KEY"}}
```

- `command` passes the step prompt on stdin and passes when the command exits 0; stdout goes to `.plaesy/loop/jobs/step-<n>.out`.
- `openai` sends one chat message to an OpenAI-compatible endpoint; `api_key_env` names the variable holding the key.
- Caps: `max_run_iterations` (default 500), `max_wall_minutes` (240), `max_tokens` (5,000,000), `max_consecutive_failures` (2). `0` disables a cap.

## Output

A stopped run writes `.plaesy/loop/stop-report.md` and sets `autonomous_loop.status` in `.plaesy/state.json`.

`stop-hook` exits 2 (keep the turn going) while the loop is `running` and `stop_hook_active` is false, and exits 0 otherwise.

## Safety

The loop runs commands and model calls repeatedly without a person in between. Set caps before starting, and ask the user before running it.

`plaesy loop --help` is authoritative.
