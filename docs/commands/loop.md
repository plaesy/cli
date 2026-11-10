# plaesy loop

Runs the autonomous loop driver for the current project.

The loop and `stop-hook` commands are available in the CLI. Configure a compatible hook manually
with `plaesy loop stop-hook`.

## Usage

```bash
plaesy loop
plaesy loop stop-hook
```

## What it does

- `plaesy loop` runs iterations until the run ends. Each iteration runs the step set by `"step"` in `.plaesy/loop.json`:
  - `{"kind":"command","command":"<cmd>","args":[...],"timeout_seconds":N}` runs the command with the step prompt on stdin. The step passes when the command exits 0, and its stdout is saved to `.plaesy/loop/jobs/step-<n>.out`. A command that is not on PATH is a failed step that names the command.
  - `{"kind":"openai","base_url":"...","model":"...","api_key_env":"ENV_NAME"}` sends the step prompt as one chat message to an OpenAI-compatible endpoint (`base_url` includes the version prefix, for example `.../v1`). `api_key_env` names the environment variable that holds the key; the key is never read from or written to the file. A missing variable or a non-2xx response is a failed step with the reason.
  - Both kinds use the provider types of `plaesy delegate` (`cli` and `openai`).
  - The `command` kind is an example shape only; no CLI flags are verified for any tool. Write the command your tool needs.
  - With no `step`, the older `"step_command"` shell command runs. With neither, every iteration is not clean, and the run stops after `max_consecutive_failures` failed cycles in a row (default 2).
- The run has hard caps: `max_run_iterations` (default 500), `max_wall_minutes` (240), and `max_tokens` (5,000,000). These settings are read from `.plaesy/loop.json`; set a cap to `0` to disable it. Iteration and token caps are checked between steps. The wall-clock cap also limits the time available to the active step; timed-out shell steps are terminated with their process tree.
- The token cap can stop a run only after a step reports token usage. If the wall-clock start time cannot be parsed, the configured wall-clock cap stops the run safely because elapsed time cannot be trusted.
- `step.timeout_seconds` bounds each configured `command` or `openai` call; `0` means no separate step timeout, though the run's remaining wall-clock cap still applies. Legacy `step_command` uses `legacy_step_timeout_seconds` from `.plaesy/loop.json` (default 900 seconds); `0` disables that timeout. On timeout, the shell command and its process tree are terminated.
- Completion and Stop reports list configured caps and known or unknown usage; Stop Reports also retain the last step failure reason.
- A run that stops writes a report to `.plaesy/loop/stop-report.md` and sets `autonomous_loop.status` in `.plaesy/state.json`.
- `plaesy loop stop-hook` reads the Stop hook input from stdin. While the state is `running` and `stop_hook_active` is false, it prints a message to stderr and exits 2, which keeps the turn going. When `stop_hook_active` is true, it exits 0, so the turn can end once.

## Hook setup

Configure the hook through the AI client's settings. It keeps a turn open only while a live Plaesy
loop driver is running and no Stop or Completion report, background task, scheduled wake-up, or
active continuation has released it.
