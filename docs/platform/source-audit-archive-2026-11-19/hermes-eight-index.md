
## api-server.md
# API Server
## Quick Start
### 1. Enable the API server
### 2. Start the gateway
### 3. Connect a frontend
## Conversation and terminal-result semantics
- `/v1/runs` and `/v1/responses` retain the conversation declared by
- A nonempty `/v1/runs` `conversation_history` is authoritative, even with an explicit
- Runs polling and SSE agree on `cancelled` or `failed`. Responses uses the same
- A turn that runs out of its iteration budget (`agent.max_turns`) still answers with the
- Session-chat replies include `status`; their streams report matching terminal run
## Idempotency keys {#idempotency-keys}
- **Canonical mode** (the gateway runs a session authority): a key is a durable admission
- **Legacy mode** (no session authority): non-streaming requests are deduplicated by a
- **Responses** (JSON or streaming) and **non-streaming Chat Completions:** HTTP 409
- **Streaming Chat Completions:** the stream has already opened with HTTP 200, so the
- **Responses:** `input`, `instructions`, `previous_response_id`, `conversation`,
- **Chat Completions:** the last user message, the earlier messages (unless
- **Chat Completions:** an exact retry replays the original answer without recreating
- **Responses:** replay also needs the response store to keep the key's identity, so
## Endpoints
### POST /v1/chat/completions
- **Chat Completions**: Hermes emits `event: hermes.tool.progress` for tool-start visibility without polluting persisted assistant text. Strict OpenAI clients that choke on named SSE events can turn these frames off with `gateway.platforms.api_server.tool_progress_events: false` (default `true`); content chunks are unaffected. The opt-out applies o
- **Responses**: Hermes emits spec-native `function_call` and `function_call_output` output items during the SSE stream, so clients can render structured tool UI in real time.
- **Chat Completions**: reasoning deltas arrive as `choices[0].delta.reasoning_content` chunks (the DeepSeek-style field Open WebUI, opencode and the Vercel AI SDK render as a thinking block); answer text stays in `delta.content`.
- **Responses**: each thinking burst is a spec-native `reasoning` output item — `response.output_item.added` (`item.type: "reasoning"`), `response.reasoning_summary_part.added`, `response.reasoning_summary_text.delta` … `response.reasoning_summary_text.done`, `response.reasoning_summary_part.done`, `response.output_item.done` — closed before the ne
- **Non-streaming**: `/v1/chat/completions` returns the turn's reasoning on `choices[0].message.reasoning_content`; `/v1/responses` returns the same `reasoning` output item(s) ahead of the message (and of that step's `function_call` items), also on `GET /v1/responses/{id}` replay.
- Echoing a prior response's `output` list back as the next `input` (what Responses SDK clients do) is fine: `reasoning` items are ignored on input rather than parsed as empty user turns.
- Support is advertised as `features.reasoning_streaming: true` on `GET /v1/capabilities`.
### POST /v1/responses
#### Multi-turn with previous_response_id
#### Named conversations
### GET /v1/responses/\{id\}
### DELETE /v1/responses/\{id\}
### GET /v1/models
### GET /api/model/options
### GET /v1/capabilities
## Browser-extension control (removed)
## Per-request model selection
- `model` — the target model id for this turn
- `provider` — the Hermes provider slug to resolve credentials/runtime for this turn
- `model_options` — request-scoped reasoning / service-tier controls
- `POST /v1/chat/completions`
- `POST /v1/responses`
- `POST /v1/runs`
- `POST /api/sessions/{session_id}/chat`
- `POST /api/sessions/{session_id}/chat/stream`
### GET /health
### GET /health/detailed
## Runs API (streaming-friendly alternative)
### POST /v1/runs
### GET /v1/runs/\{run_id\}
### GET /v1/runs/\{run_id\}/events
#### Detached results and session history
### POST /v1/runs/\{run_id\}/resolve-unknown
### POST /v1/runs/\{run_id\}/stop
### POST /v1/runs/\{run_id\}/approval
### POST /v1/runs/\{run_id\}/clarify
## Jobs API (background scheduled work)
### GET /api/jobs
### POST /api/jobs
### GET /api/jobs/\{job_id\}
### PATCH /api/jobs/\{job_id\}
### DELETE /api/jobs/\{job_id\}
### POST /api/jobs/\{job_id\}/pause
### POST /api/jobs/\{job_id\}/resume
### POST /api/jobs/\{job_id\}/run
## Sessions API (session control over REST)
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/sessions` | List sessions (paginated — `limit`, `offset`, `source`, `include_children`) |
| `POST` | `/api/sessions` | Create an empty session |
| `GET` | `/api/sessions/{id}` | Read session metadata |
| `PATCH` | `/api/sessions/{id}` | Update title or `end_reason` |
| `DELETE` | `/api/sessions/{id}` | Delete a session |
| `GET` | `/api/sessions/{id}/messages` | Message history for a session. `include_compacted=true` also returns the turns a context compaction archived (default: live transcript only). `inline_images=false` renders image attachments as `[image]` placeholders instead of inline data URIs — the transcript travels in kilobytes, for clients reading over 
| `POST` | `/api/sessions/{id}/fork` | Branch the session via `SessionDB` lineage (matches CLI `/branch` semantics) |
| `POST` | `/api/sessions/{id}/chat` | Run one synchronous agent turn |
| `POST` | `/api/sessions/{id}/chat/stream` | SSE wrapper over a single turn — emits `assistant.delta`, `assistant.commentary` (mid-turn commentary: `message_id`, `text`, `already_streamed`; never folded into `assistant.completed`), `tool.started`, `tool.completed`, `tool.failed` (a tool that finished with an error), then a terminal `run.completed`
## Skills and toolsets discovery
## Long-term memory scoping (`X-Hermes-Session-Key`)
## System Prompt Handling
- Open WebUI system prompt: "You are a Python expert. Always include type hints."
- The agent still has terminal, file tools, web search, memory, etc.
## Authentication
### Multi-profile routing (`/p/<profile>/…`)
- Requests to `/p/<profile>/v1/...` must present that profile's own
- Unprefixed routes and `/p/default/...` keep using the default profile's key.
- A named profile with no `API_SERVER_KEY` of its own fails closed — its
- Runs are per-profile scoped: `/v1/runs/{run_id}` and its `events`, `stop`,
## Configuration
### Environment Variables
| Variable | Default | Description |
|----------|---------|-------------|
| `API_SERVER_ENABLED` | `false` | Enable the API server |
| `API_SERVER_PORT` | `8642` | HTTP server port |
| `API_SERVER_HOST` | `127.0.0.1` | Bind address (localhost only by default) |
| `API_SERVER_KEY` | _(required)_ | Bearer token for auth |
| `API_SERVER_CORS_ORIGINS` | _(none)_ | Comma-separated allowed browser origins |
| `API_SERVER_MODEL_NAME` | _(profile name)_ | Model name on `/v1/models`. Defaults to profile name, or `hermes-agent` for default profile. |
### config.yaml
### Concurrent-run cap
### Stored history size for `previous_response_id` chaining
## Security Headers
- `X-Content-Type-Options: nosniff` — prevents MIME type sniffing
- `Referrer-Policy: no-referrer` — prevents referrer leakage
## CORS
- **Preflight responses** include `Access-Control-Max-Age: 600` (10 minute cache)
- **SSE streaming responses** include CORS headers so browser EventSource clients work correctly
- **`X-Hermes-Session-Id`** is an allowed request header, so browsers on an allowlisted origin can request session continuation.
- **`Idempotency-Key`** is an allowed request header — clients can send it for deduplication (see [Idempotency keys](#idempotency-keys) for what a key covers and how long it is kept)
## Compatible Frontends
| Frontend | Stars | Connection |
|----------|-------|------------|
| [Open WebUI](../messaging/open-webui.md) | 126k | Full guide available |
| LobeChat | 73k | Custom provider endpoint |
| LibreChat | 34k | Custom endpoint in librechat.yaml |
| AnythingLLM | 56k | Generic OpenAI provider |
| NextChat | 87k | BASE_URL env var |
| ChatBox | 39k | API Host setting |
| Jan | 26k | Remote model config |
| HF Chat-UI | 8k | OPENAI_BASE_URL |
| big-AGI | 7k | Custom endpoint |
| OpenAI Python SDK | — | `OpenAI(base_url="http://localhost:8642/v1")` |
| curl | — | Direct HTTP requests |
## Multi-User Setup with Profiles
- `http://localhost:8643/v1/models` → model `alice`
- `http://localhost:8644/v1/models` → model `bob`
## Limitations
- **Response storage** — stored responses (for `previous_response_id`) are persisted in SQLite and survive gateway restarts. Max 100 stored responses (LRU eviction).
- **No file upload** — inline images are supported on both `/v1/chat/completions` and `/v1/responses`, but uploaded files (`file`, `input_file`, `file_id`) and non-image document inputs are not supported through the API.
- **Simple OpenAI clients still see an alias** — `/v1/models` advertises the
## Proxy Mode

## browser.md
# Browser Automation
| Tool | Purpose |
|------|---------|
| `browser_exec` | Navigate, read, click, type, scrape and screenshot by running Python with pre-imported browser helpers. |
| `browser_cdp` | Send one raw CDP command (cookies, network control, iframe-scoped evaluation, anything the helpers don't cover). |
| `browser_dialog` | Accept or dismiss a native JavaScript dialog (`alert` / `confirm` / `prompt` / `beforeunload`) blocking the page. |
| `browser_vault` | Fill saved logins, cards, addresses and 2FA codes into the current page without the secrets entering the conversation. |
- **Local Chromium (default)** — Hermes launches its own packaged Chromium, headless, one browser per session. No API key, no setup.
- **Cloud browsers** — the [Nous subscription](tool-gateway.md), [Browserbase](https://browserbase.com), [Browser Use](https://browser-use.com) cloud, [Firecrawl](https://firecrawl.dev), or a browser-provider plugin.
- **[Lightpanda](https://lightpanda.io)** — a lightweight headless engine for text-first browsing on small machines.
- **Your own browser** — attach to a running Chrome, Brave, Chromium or Edge with `/browser connect`.
- **Your real profile** — a managed copy of your everyday browser profile, logins included (opt-in).
## How `browser_exec` works
| Helper | Does |
|--------|------|
| `new_tab(url)` | Open a URL (use for the first navigation). |
| `goto_url(url)` | Navigate the current tab (later navigations). |
| `wait_for_load()` | Wait for the page to finish loading. |
| `page_info()` | Summarize the current page state. |
| `js(expr)` | Evaluate JavaScript and return the value (wrap function bodies as `js('(() => {...})()')`). |
| `fill_input(selector, text)` | Type into an input. |
| `click_at_xy(x, y)` | Click viewport coordinates. |
| `capture_screenshot()` | Save a screenshot and print its path. |
| `cdp('Domain.method', **kwargs)` | Raw CDP — e.g. `cdp('Accessibility.getFullAXTree')` for the accessibility tree, `cdp('DOM.getBoxModel', backendNodeId=n)` for click coordinates. |
| `ensure_real_tab()` | Recover from a stale or internal tab. |
## Local Chromium (default)
- **One browser per session**, with a fresh, throwaway profile by default: the
- **Headless by default.** Set `browser.headed: true` for a visible window (see
- **Automatic cleanup.** The browser is closed after `browser.inactivity_timeout`
- **Private hosts are reachable.** With a local terminal backend, the local
### Headed mode (visible browser window)
## Cloud browsers
### Browser Use cloud
### Browserbase
| Feature | Default | Notes |
|---------|---------|-------|
| Basic Stealth | Always on | Random fingerprints, viewport randomization, CAPTCHA solving |
| Residential Proxies | On | Routes through residential IPs for better access |
| Advanced Stealth | Off | Custom Chromium build, requires Scale Plan |
| Keep Alive | On | Session reconnection after network hiccups |
### Firecrawl
### Hybrid routing: cloud for public URLs, local for LAN/localhost
## Real profile browsing (use your own logins)
- **Windows, first use only:** a running Chrome/Edge/Brave holds its cookie
- **Your default browser isn't a supported Chromium** (Firefox, a Beta/Dev/Canary
- **The cookie database is unreadable or corrupt**: the message names it and the
- **Supported browsers:** Chrome, Edge, Brave, Brave Origin, Chromium (whichever is your OS
- **Works with any browser source.** Locally it's automatic once the toggle
- **Security framing:** this is a consent-gated convenience, not an isolation
- **Desktop:** toggle it in **Capabilities → Tools → Browser → Use My Real
### Scheduled and unattended runs
- **Turn the toggle on.** `browser.use_real_profile` defaults to `false`, and
- **Give the job the browser toolset:**
- **Assume nothing can be prompted for.** A cron, webhook, API or
## Lightpanda local engine
## Your own browser via CDP (`/browser connect`)
### WSL2 + Windows Chrome: prefer MCP over `/browser connect`
- `/browser connect` expects Hermes itself to reach a usable CDP endpoint
- modern Chrome live-debugging sessions often expose a host-local endpoint that is not directly reachable from WSL the same way a classic `9222` port is
- even when Windows Chrome is debuggable, the cleanest integration is often to let a Windows-side browser MCP server attach to Chrome and let Hermes talk to that MCP server
- [Use MCP with Hermes](../../guides/use-mcp-with-hermes.md#wsl2-bridge-hermes-in-wsl-to-windows-chrome)
## Bot Desktop screen
## Companion tools
### `browser_cdp`
### `browser_dialog`
| Policy | Behavior |
|--------|----------|
| `must_respond` (default) | Capture the dialog and wait for an explicit `browser_dialog()` call. Safety auto-dismiss after `browser.dialog_timeout_s` (default 300s) so a buggy agent can't stall forever. |
| `auto_dismiss` | Capture, dismiss immediately. The agent doesn't have to act. |
| `auto_accept` | Capture, accept immediately. Useful when navigating pages with aggressive `beforeunload` prompts. |
### `browser_vault`
## Turning browsing off
## Camofox
## Practical examples
### Filling out a web form
### Researching dynamic content
## Limitations
- **Session timeout** — cloud sessions expire based on your provider's plan settings.
- **Cost** — cloud sessions consume provider credits; sessions are cleaned up when the conversation ends or after inactivity. Local Chromium and `/browser connect` are free.
- **Lightpanda** — no screenshots and one page per session.
- **No terminal, no browser** — `browser_exec` runs Python, so sessions without the `terminal` toolset don't get it.

## cron.md
# Scheduled Tasks (Cron)
## What cron can do now
- schedule one-shot or recurring tasks
- pause, resume, edit, trigger, and remove jobs
- stop a run that is already in flight, without touching the job's schedule
- attach zero, one, or multiple skills to a job
- deliver results back to the origin chat, local files, or configured platform targets
- run in fresh agent sessions with the normal static tool list
- run in **no-agent mode** — a script on a schedule, its stdout delivered verbatim, zero LLM involvement (see the [no-agent mode](#no-agent-mode-script-only-jobs) section below)
- fire on **external events** — a webhook route with `cron_job` set fires the job the moment something happens (a PR gets feedback, a service posts an alert) instead of waiting for the next scheduled tick. See [Event-Triggered Cron Jobs](../messaging/webhooks.md#event-triggered-cron-jobs).
- **Per-job pin** — a job that carries its own model. Set it to a specific model via the dashboard, `hermes cron create/edit --model … --provider …`, or by editing `~/.hermes/cron/jobs.json`; or **lock in the current main model** with `hermes cron create/edit --pin` (the agent's `cronjob_manage` tool can do this too with `pinned=true`, but only whe
- **`cron.model` / `cron.model_provider`** — a cron-fleet default: every unpinned job runs on this model, independent of your chat model. Set it once (`hermes config set cron.model <name>`) and switching your chat model with `hermes model` or `/model` never touches your cron fleet.
- **Main agent model** — when neither of the above is set, a job runs on whatever `hermes model` / `/model` is set to **at the moment it fires**. Change your main model and every unpinned job follows on its next run.
## Admission recovery
## Creating scheduled tasks
### In chat with `/cron`
### From the standalone CLI
### Through natural conversation
## Pre-dispatch configuration validation
- the provider API key resolves (skipped for an unpinned job when a
- attached skills are ready (no missing required environment variables,
- delivery platform targets are known and have gateway credentials configured
- every MCP server the job names in its own `enabled_toolsets` resolved to at
## Moving unpinned jobs to a new global default
## Skill-backed cron jobs
### Single skill
### Multiple skills
## Running a job inside a project directory
- `AGENTS.md`, `CLAUDE.md`, and `.cursorrules` from that directory are injected into the system prompt (same discovery order as the interactive CLI)
- `terminal`, `read_file`, `write_file`, `patch`, `search_files`, and `execute_code` all use that directory as their working directory
- The path must be an absolute directory that exists — relative paths and missing directories are rejected at create / update time
- Pass `--workdir ""` (or `workdir=""` via the tool) on edit to clear it and restore the old behaviour
## Per-job timezone
- The expression is the pinned zone's **wall clock**: `0 9 * * *` fires at 09:00 in that zone, and stays at 09:00 across that zone's DST changes.
- Omit it (or pass `--timezone ""` on edit) to inherit the profile timezone. A job without the field behaves exactly as before.
- An offset-less one-shot timestamp (`2026-06-01T09:00:00`) is also read in the job's zone; a timestamp that carries its own offset is already an instant.
- Changing the zone recomputes `next_run_at` immediately, so the next fire happens at the new zone's wall clock.
- An `interval` job has a fixed duration and no wall clock, so a pin has no effect on it.
- The run-time line the job's prompt carries (`[CURRENT RUN TIME: ...]`) is stated in the pinned zone, so a 09:00 New York job is told 09:00 EST, not the profile's local time. `hermes cron list`, `/cron list` and `cronjob(action="list")` show the pinned zone next to the schedule.
### Pinned and local-time jobs in a shipped profile
## Catch-up window
## Editing jobs
### Chat
### Standalone CLI
- repeated `--skill` replaces the job's attached skill list
- `--add-skill` appends to the existing list without replacing it
- `--remove-skill` removes specific attached skills
- `--clear-skills` removes all attached skills
## Lifecycle actions
### Chat
### Standalone CLI
- `pause` — keep the job but stop scheduling it
- `resume` — re-enable the job. A recurring job whose slot came due while it was paused keeps that slot due, so the next tick fires one catch-up run (or logs the skip when `cron.catch_up_missed: false`) instead of silently jumping to the next occurrence; otherwise the next future run is computed
- `run` — trigger the job on the next scheduler tick. A manual run is an extra fire: the scheduled run still happens and the `repeat` count is not spent, so test-firing an `in 30m` reminder keeps the real one (a run once the slot is already due is that scheduled run)
- `cancel` — stop the job's **current** run (see [Stopping a run in flight](#stopping-a-run-in-flight))
- `remove` — delete it entirely
- `edit` — modify schedule, prompt, delivery, etc.
- `tick` — run one scheduler pass and exit (for a system crontab or other external
### Pausing everything: `hermes pause`
### Stopping a run in flight: `hermes cron cancel`
- **Script jobs** (`--script`, including [no-agent mode](#no-agent-mode-script-only-jobs)) get the
- **Agent jobs** have the in-flight turn interrupted at its next checkpoint.
### Creating a job paused (safe canary)
## Agent-managed scheduling (cron jobs that manage cron jobs)
- **One flat, user-owned table.** Jobs created from a cron run land in the
- **No dangling delivery.** A cron run is ephemeral, so `deliver: origin`
- **A job may remove itself and still report.** The "watch for X, tell me
## How it works
#### Cron store not writable (full disk, read-only mount, permissions)
- `hermes cron status` probes the store itself and leads with `⚠ Cron store is NOT writable — scheduled jobs are being skipped`. Below that it shows the store path, the OS error, the last successful write and how many due runs have not fired. `hermes cron list` prints a one-line banner.
- `hermes doctor` warns when the store is not writable, or when its filesystem has less than 100 MB free.
- The gateway posts one notice to the profile's home channels when the store becomes unwritable and one when it has stayed writable for an hour (a store that fails again within that hour posts nothing more, so the last notice always matches its state). Both respect `display.suppress_warning_notifications` and use the profile's `display.language`.
- Monitoring exports `hermes.cron.store.writable` (0 while any profile's store served by this gateway is unwritable, else 1) and `hermes.cron.store.skipped_runs` (summed over those stores).
### Gateway scheduler behavior
### Restart-safe workers under systemd
### Execution history
### Repeated-failure review nudge
### Automatic re-runs when the model was unreachable
### Holding a job through a closed provider usage window
### Failure incidents: alert once, remind on a cooldown, acknowledge
### Fleet health check: `hermes cron doctor`
- last run failed (`last_status` not ok, with the recorded error),
- last delivery failed (the output was produced but never reached you),
- last dispatch was late or caught up after a missed schedule (`last_dispatch`); this warning clears at the next on-time fire,
- a scheduled fire could not reach the runner (`last_fire_error`), with the recorded timestamp and a shortened reason; this warning clears after a successful run,
- `next_run_at` missing, or parked in the past beyond a 15-minute ticker
- script missing, not a file, or resolving outside `HERMES_HOME/scripts`,
- `no_agent` job with no script,
- configured `workdir` that no longer exists.
## Delivery options
| Option | Description | Example |
|--------|-------------|---------|
| `"origin"` | Back to where the job was created | Default on messaging platforms |
| `"local"` | Save to local files only (`~/.hermes/cron/output/`) | Default on CLI |
| `"telegram"` | Telegram home channel | Uses `TELEGRAM_HOME_CHANNEL` |
| `"telegram:123456"` | Specific Telegram chat by ID | Direct delivery; on the job's own origin chat it keeps the topic the job was created in (Telegram and Slack only) |
| `"telegram:-100123:17585"` | Specific Telegram topic | `chat_id:thread_id` format |
| `"telegram:-100123:1"` | The forum's General topic | Posts top-level even when the job was created in a topic |
| `"discord"` | Discord home channel | Uses `DISCORD_HOME_CHANNEL` |
| `"discord:#engineering"` | Specific Discord channel | By channel name |
| `"slack"` | Slack home channel | |
| `"whatsapp"` | WhatsApp home | |
| `"signal"` | Signal | |
| `"matrix"` | Matrix home room | |
| `"mattermost"` | Mattermost home channel | |
| `"email"` | Email | |
| `"sms"` | SMS via Twilio | |
| `"homeassistant"` | Home Assistant (plugin) | Uses `HASS_HOME_CHANNEL`; requires the [`homeassistant` plugin](../messaging/homeassistant.md) |
| `"dingtalk"` | DingTalk | |
| `"feishu"` | Feishu/Lark | |
| `"wecom"` | WeCom | |
| `"weixin"` | Weixin (WeChat) | |
| `"bluebubbles"` | BlueBubbles (iMessage) | |
| `"qqbot"` | QQ Bot (Tencent QQ) | |
| `"bot-chat"` | This profile's canonical Bot Chat — the bot reads the output and responds | Machine-local |
| `"bot-chat:research"` | Another local profile's Bot Chat | Validated at create time |
| `"all"` | Fan out to every connected home channel | Resolved at fire time |
| `"telegram,discord"` | Fan out to a specific set of channels | Comma-separated list |
| `"origin,all"` | Deliver to the origin **plus** every other connected channel | Combine any tokens |
### Delivery failures are a distinct status
### Bot Chat delivery (`bot-chat`)
- `bot-chat` (bare) targets the job's own profile.
- `bot-chat:<profile>` targets another profile **on the same machine**. Names are validated against `hermes profile list` when the job is created; profiles on other gateways or machines can never be targeted, so same-named profiles across machines are unambiguous.
- Each delivery costs the target bot one full agent turn — mind the schedule frequency.
- Composes with other targets (`bot-chat,telegram`) but is never included in `all`.
- If the canonical chat is open in a mailbox-capable Desktop/TUI backend, delivery is **durably queued immediately**, whether the bot is idle or busy. Only that live owner runs the incoming turn; cron does not start a competing CLI writer. A profile that has no Bot Chat yet gets one created on first delivery. If the target profile's gateway is not 
- **Queued is not completed.** Cron records receipt IDs and `queued`/`claimed` statuses in `last_delivery_queued`, with delivery outcome `queued` (neither delivered nor failed). A successful job shows `delivery_queued`; genuine errors on other targets still take precedence as delivery failures. The bot may complete later. The durable receipt in the
- Rechecking the same execution inspects its existing receipt, even if the owner has disappeared. It never falls back to another writer after acceptance. `failed`, `cancelled`, or `ambiguous` receipts are not automatically replayed; inspect the chat and receipt before intentionally starting new work. Each new cron execution has a distinct delivery 
### Routing intent (`all`)
### Telegram cron topic (`TELEGRAM_CRON_THREAD_ID`)
### Response wrapping
#### Per-job override
### Push notifications (`cron.delivery.notify`)
### Delivery confirmation and the `UNVERIFIED` state
### Continuable jobs (reply to a cron delivery)
- **Thread-capable platforms** (Telegram topics, Discord/Slack/Matrix threads): each
- **DM-only platforms** (WhatsApp, Signal, SMS): no threads exist, so the brief
- the **origin chat** the job was created in;
- the **home-channel fallback** when `deliver: origin` captured no origin (jobs
- a job's **single explicit `platform:chat` target**, but only when the job
#### Flat, in-channel continuation (Slack)
- **`cron_continuable_surface: in_channel`** — skips thread creation on delivery.
- **`reply_in_thread: false`** (required) — makes the bot answer your reply
- **`require_mention: false`** (or add the channel to `free_response_channels`)
- **`false`** — all top-level DMs share one rolling DM session, so a continuable
- **`true`** (default) — each top-level DM message is its own session, so a reply
### Silent suppression
### Declaring a failed run
## Script timeout
## Media send timeout
## Bot Chat delivery completion
## Standalone send timeout
## No-agent mode (script-only jobs)
- Script stdout (trimmed) → delivered verbatim as the message.
- **Empty stdout → silent tick**, no delivery. This is the watchdog pattern: "only say something when something is wrong".
- Non-zero exit or timeout → an error alert is delivered, so a broken watchdog can't fail silently.
- `{"wakeAgent": false}` on the last line → silent tick (same gate LLM jobs use).
- No tokens, no model, no provider fallback — the job never touches the inference layer.
#### Giving a script a credential
### The agent sets these up for you
## Chaining jobs with `context_from`
- When Job 2 fires, Hermes reads Job 1's most recent output from `~/.hermes/cron/output/{job1_id}/*.md`
- That output is prepended to Job 2's prompt automatically
- Job 2 doesn't need to hardcode "read this file" — it receives the content as context
- The chain can be any length: Job 1 → Job 2 → Job 3 → ...
| Format | Example |
|--------|---------|
| Single job ID (string) | `context_from="a1b2c3d4"` |
| Multiple job IDs (list) | `context_from=["job_a", "job_b"]` |
- Multi-stage pipelines (collect → filter → format → deliver)
- Dependent tasks where step N's work depends on step N−1's output
- Fan-out/fan-in patterns where one job aggregates results from several others
- Recurring scouts/monitors that should dedupe against their own previous report (`continuity=true`)
## Provider recovery
- **Rotate to the next credential** in your [credential pool](../configuration.md#credential-pool-strategies) for the same provider. This applies to every job, pinned or not.
- **Fall back to an alternate provider** from `fallback_providers` (or the legacy `fallback_model`) in `config.yaml` — **unpinned jobs only**. That covers a failure while resolving credentials before the run starts and a provider error mid-run.
## Run failures (`last_error`)
## Missed scheduled fires (`last_fire_error`)
- `cronjob_manage` tool → `action: "list"` — the `last_fire_error` field
- `hermes cron list`: a red missed-fire warning under the job
- `hermes cron doctor`: a per-job missed-fire finding that makes the command exit `1`
- The dashboard job view
### Local missed-run policy
### Misfire catch-up
## Schedule formats
### Relative delays (one-shot)
### Intervals (recurring)
### Natural day/time schedules (recurring)
### Cron expressions
### ISO timestamps
## Repeat behavior
| Schedule type | Default repeat | Behavior |
|--------------|----------------|----------|
| One-shot (`in 30m`, timestamp) | 1 | Runs once |
| Interval (`every 2h`) | forever | Runs until removed |
| Cron expression | forever | Runs until removed |
## Managing jobs programmatically
### Manual runs are asynchronous
## Toolsets available to cron jobs
### Skipping the agent entirely: `wakeAgent`
#### Recipes: cheap pre-run gates
### Chaining jobs: `context_from`
## Job storage
## Self-contained prompts still matter
## Security

## delegation.md
# Subagent Delegation
## Completion delivery
## Background process lifetime
## Approvals from background children
## Single Task
## Parallel Batch
## Structured Output (`output_schema`)
## How Subagent Context Works
### Forwarding Images to a Subagent
- **Vision-capable child model** — the images arrive as native multimodal content on the child's first turn: local files are embedded as data URLs (subject to the same read guard as every other file read), remote and `data:` URLs pass through verbatim. The child sees the actual pixels.
- **Non-vision child model** — the goal gains `[Image attached at: <path>]` hint lines and the child is told to inspect them with `vision_analyze`.
## Practical Examples
### Parallel Research
### Code Review + Fix
### Multi-File Refactoring
## Batch Mode Details
### Independent completions (opt-in)
- The schema marks `group` as required on every task, so the model states the delivery choice in each call instead of falling into per-task delivery by leaving a field out.
- The same `group` string on every task is the choice when the parent will respond only once all of them are done: comparison, synthesis, or one coordinated decision. The group returns **one** consolidated message after all its tasks finish. Even independently executable tasks belong in one group when their results inform the same decision.
- A task gets its own `group` only when the parent will act on that result as soon as it lands. Each such result re-enters alone and costs the parent a turn.
- Different groups report independently and can share one call. A task that arrives without a `group` (an older transcript, a model that ignores `required`) is still accepted and reports alone, as before.
- **Maximum concurrency:** 10 tasks by default (configurable via `delegation.max_concurrent_children` or the `DELEGATION_MAX_CONCURRENT_CHILDREN` env var; floor of 1, no hard ceiling). Batches larger than the limit return a tool error rather than being silently truncated.
- **Thread pool:** Uses `ThreadPoolExecutor` with the configured concurrency limit as max workers
- **Progress display:** In CLI mode, a tree-view shows tool calls from each subagent in real-time with per-task completion lines. In gateway mode, progress is batched and relayed to the parent's progress callback. CLI and TUI completion notices use task-first titles such as `Subagent Task Completed: Review changes`; multi-task groups use the group 
- **Result ordering:** Within a unit, results are sorted by task index to match input order regardless of completion order; `TASK i/N` labels index the whole call
- **Cancellation:** Follow-up messages and stopping the current turn (Ctrl+C, Esc, the Desktop/TUI Stop button, an ACP cancel) do not cancel a top-level background batch. The `/stop` command (gateway or CLI) or closing/resetting the owning session ends its background children and every synchronous descendant beneath them; each stopped child still r
### Durable background completions
### Child background-process notifications
### Handing a process to the parent
- **wait** — `process_manage(action="wait", session_id=...)` and report the result itself;
- **kill** — `process_manage(action="kill", ...)`;
- **hand off** — `process_manage(action="handoff", session_id=..., data="<one sentence: what it is for>")`. The runtime transfers ownership to the parent under the registry lock (up to 3 per child; only a running process the child owns is accepted, anything else is a tool error). The parent's completion notice then arrives in the parent chat with `
## Model Override
### Cost strategy: frontier planner, inexpensive workers
### Delegation strategies (`delegation.backend`)
- a **profile per subagent** (`profile_routes`): each child runs as one of your Hermes profiles, with that profile's model, credentials, skills and persona;
- a **named model map** (`model_map`): you list titled options (`fast`, `deep`, ...) with a description and a provider/model each, and the model picks one per task.
## The `/review` Command
### Review model
## Inherited Tool Access
- `delegate_task` — blocked for leaf subagents (the default). Retained for `role="orchestrator"` children, bounded by `max_spawn_depth` — see [Depth Limit and Nested Orchestration](#depth-limit-and-nested-orchestration) below.
- `clarify` — subagents cannot interact with the user
- `memory` — no writes to shared persistent memory
- `send_message` — no cross-platform side effects
- `cronjob` — no scheduling more work in the parent's name
## Max Iterations
## Child Timeout
## Failure Visibility
- **CLI**: the delegation tree prints a one-line reason: `⚠️ Subagent failed — "your goal": HTTP 404: model not found (after 12s)`. Batch runs append the reason to the per-task `✗` completion line.
- **Gateway platforms** (Telegram, Discord, Slack, ...): the same clean line is delivered as a standalone chat notice, **even when `tool_progress` is off** for that platform.
- **Parent agent**: the tool result entry carries `status: "failed"` plus the full `error` text, so the model can react (retry, re-route, report).
## Stall Detection for Background Subagents
## Monitoring Running Subagents (`/agents`)
- Live tree view of running and recently-finished subagents, grouped by parent
- Per-branch cost, token, and file-touched rollups
- Kill and pause controls — cancel a specific subagent mid-flight without interrupting its siblings
- Post-hoc review: step through each subagent's turn-by-turn history even after they've returned to the parent
### Live activity above the composer
| Surface | Expand and inspect | Control a selected worker |
|---|---|---|
| Classic CLI | **Ctrl+T** (or **F6**) opens the full-screen live roster; arrows select, **Enter** opens the transcript tail, **PgUp/PgDn** scroll | **s** opens a separate steering input; **x**, then **y** requests stop |
| TUI | **Ctrl+T** or `/agents` opens the full-height tree; **Enter/t** opens the live transcript tail; **d** opens rich detail (archived/replay Enter still opens detail) | **e** opens steering; **x** stops the selected worker; **X** stops its subtree |
| Desktop | Expand **Subagents** above the composer, then select a worker to inspect its activity and details | **Steer** queues guidance; **Stop** requests interruption for that worker |
## Steering a Running Subagent
### From the parent agent (model-facing)
- **`list`** returns the conversation's live children: `subagent_id`, goal, status, `running_seconds`, `accepting_steer`, and the live transcript path. Ids also come back in the spawn dispatch response as `subagent_ids`.
- **`steer`** queues a course correction into a running child without stopping it (delivery semantics below).
- **`stop`** ends a child early at its next iteration boundary; the partial result still re-enters the conversation as a normal completion message.
| `resume_mode` | Typical cause | What continuing needs |
|---|---|---|
| `AUTO_AT` | usage/rate limit with a reset time, transient provider errors | wait until `failure_resets_at` |
| `AUTO_ON_BOOT` | gateway shutdown, crash or hard kill | nothing; check git state and the last tool before re-running side effects |
| `ON_FIX` | credits, auth, model access, account policy, request-shape errors | you fix the cause first |
| `REPAIR_THEN_CONTINUE` | the conversation no longer fits, or an image/reasoning part is rejected | the transcript needs repairing first |
| `HANDOFF` | a provider safety refusal of this conversation | another model, or the parent takes the work over |
### Automatic resume (`delegation.auto_resume`, opt-in)
- **What resumes:** `AUTO_AT` children once their limit's reset time has passed, and `AUTO_ON_BOOT` children (gateway shutdown, crash, hard kill) once the gateway is back up.
- **What never resumes:** `ON_FIX`, `REPAIR_THEN_CONTINUE` and `HANDOFF` children (credits, auth, model not found, refusals, deterministic request errors), children you or the parent stopped, children whose stop did not promise a resume (stopped while `auto_resume` was off, or under a parent no gateway serves, such as a classic local CLI: those get
- **Silent while it waits.** A stop that is promised an automatic resume sends the parent no completion. `delegate_task(action='list')` shows the child as `waiting_to_resume` with a note such as "waiting to resume at 14:05 (attempt 1 of 2); gives up at 19:40". In a batch where only some children wait, the completion lists the waiting ones with that
- **One report at the end.** The parent hears once per child: the resumed run's own result, or ONE terminal failure when the scheduler gives up. It gives up when 2 automatic attempts are used, when `delegation.auto_resume_deadline_minutes` (default 360) has passed since the child's first stop, when the limit's reset is known to land after that dead
- **Explicit cancel only.** Only these cancel a waiting resume (the record becomes `superseded`, or `stopped` for `stop`; it can still be continued by hand until its TTL):
- **No duplicate of a waiting child.** A spawn whose goal is exactly (ignoring surrounding whitespace) the goal of one of the parent's children that is waiting to resume is refused with a tool error naming that `subagent_id`, when it resumes, and the two ways forward: wait for it, or `delegate_task(action='stop', subagent_id=...)` first and then re
- **Unconfirmed side effects.** The resume message tells the child: if its last tool call has no recorded result and it cannot establish whether that call ran, it must not repeat it and must finish with a result that starts with `BLOCKED:` naming that call.
- **Bounded:** at most 2 automatic attempts per child, counted before each attempt runs, so a child whose resume crashes the gateway every time stops after two boots; the record says why. A resume that hits the limit again goes back to waiting with its attempt counted. Resumes stay within the profile's own `delegation.max_concurrent_children` (coun
- **Cost:** a resumed child spends tokens with nobody watching, and so does the parent turn its result starts. Leave it off where unattended spend is not wanted.
### From the TUI / gateway (session-facing)
## Live Transcripts
## Depth Limit and Nested Orchestration
- `role="leaf"` (default): child cannot delegate further — identical to the flat-delegation behavior.
- `role="orchestrator"`: child retains the `delegation` toolset. Gated by `delegation.max_spawn_depth` (default **1** = flat, so `role="orchestrator"` is a no-op at defaults). Raise `max_spawn_depth` to 2 to allow orchestrator children to spawn leaf grandchildren; 3+ for deeper trees. There is no upper ceiling — cost is the practical limit.
- `delegation.orchestrator_enabled: false`: global kill switch that forces every child to `leaf` regardless of the `role` parameter.
## Finite chat runs
- **Automatic joining:** no opt-in or background-mode override is needed.
- **Per-input policy:** the CLI records finite consumption with the admitted prompt,
- **Existing safeguards:** delegation limits, timeouts, cancellation, and
- **Separate limits:** `--run-budget` remains unsupported by gateway chat; it is not
## Messaging chats without interim messages
- **`auto`:** a messaging chat whose resolved `display.interim_assistant_messages` is off
- **`inline`:** every gateway session joins its children inside the turn.
- **`background`** (alias `async`): every gateway session keeps background delegation.
## Lifetime and Durability
- Normal follow-up messages do not cancel background children, and neither does stopping the current turn: Ctrl+C, Esc, the Desktop/TUI Stop button and an ACP cancel end only the turn. The `/stop` command (gateway `/stop` — also when the session is idle after the dispatching turn ended — or CLI `/stop`) ends the session's running background delegat
- Explicit session close/reset interrupts that session's background children. In the CLI and TUI, the gateway owns the session and the client only views it: when the last viewer exits or runs `/new` (a TUI resume switch counts too), the gateway interrupts that session's background children at once and refuses new ones for it until a viewer opens th
- A background child's final result (including an interrupted one) or a `notify_on_complete` process exit for a CLI, TUI, Desktop or web session runs as the parent's turn whether or not a viewer is attached, so the work carries on while your laptop sleeps, the connection drops or the tab is closed. Results for a chat you closed (exit, `/new`, or a 
- A Hermes process restart does **not** resume a running child. Its attempt becomes `unknown` because Hermes cannot prove which side effects happened.
- A child that completed before restart but whose result was not delivered is restored and routed back through the owning session's normal checks.
- Stopped children return a structured result (`status="interrupted"`, `exit_reason="interrupted"`) whose `summary` is the last text the child produced before the stop (the interrupt placeholder moves to `error`). The stop recurses down the spawn tree — an orchestrator child's synchronous workers are interrupted first and their partial results roll
- `cronjob` (action=`create`) — schedules a separate agent run; immune to parent-turn interrupts.
- `terminal(background=True, notify_on_complete=True)` — long-running shell commands that keep running while the agent does other things.
## Key Properties
- Each subagent gets its **own terminal session** (separate from the parent)
- Subagents inherit the parent's enabled toolsets; the model cannot select or widen them per call
- **Nested delegation is opt-in** — only `role="orchestrator"` children can delegate further, and only when `max_spawn_depth` is raised from its default of 1 (flat). Disable globally with `orchestrator_enabled: false`.
- Leaf subagents **cannot** call: `delegate_task`, `clarify`, `memory`, `send_message`, `cronjob`. Orchestrator subagents retain `delegate_task` but keep the other blocks. Both roles retain `execute_code` (programmatic tool calling) so children can batch mechanical work instead of burning reasoning iterations.
- **Cancellation follows ownership** — `/stop` or closing/resetting the owning session ends its background children and the synchronous descendants beneath them; each returns an interrupted completion carrying its partial output
- Only the final summary enters the parent's context, keeping token usage efficient
- Subagents inherit the parent's **API key, provider configuration, and credential pool** (enabling key rotation on rate limits)
## Worktree Isolation
- Each child starts its terminal in `<repo>/.worktrees/subagent-<id>` on its
- The parent's checkout stays untouched; children can't clobber each other's
- When a child finishes, its result entry gains a `worktree` field reporting
- A worktree left with **no commits and a clean tree is pruned automatically**
- Pruning requires proof. If a git inspection probe fails — or finalization
## Delegation vs execute_code
| Factor | delegate_task | execute_code |
|--------|--------------|-------------|
| **Reasoning** | Full LLM reasoning loop | Just Python code execution |
| **Context** | Fresh isolated conversation | No conversation, just script |
| **Tool access** | All non-blocked tools with reasoning | 7 tools via RPC, no reasoning |
| **Parallelism** | 10 concurrent subagents by default (configurable) | Single script |
| **Best for** | Complex tasks needing judgment | Mechanical multi-step pipelines |
| **Token cost** | Higher (full LLM loop) | Lower (only stdout returned) |
| **User interaction** | None (subagents can't clarify) | None |
## Configuration

## mcp.md
# MCP (Model Context Protocol)
## What MCP gives you
- Access to external tool ecosystems without writing a native Hermes tool first
- Local stdio servers and remote HTTP MCP servers in the same config
- Automatic tool discovery and registration at startup
- Utility wrappers for MCP resources and prompts when supported by the server
- Per-server filtering so you can expose only the MCP tools you actually want Hermes to see
## Quick start
## Catalog: one-click install for Nous-approved MCPs
- **API key** — Hermes prompts at install time and writes the value to
- **OAuth** (remote MCP) — written as `auth: oauth` in your config; the MCP
- **OAuth** (third-party provider like Google/GitHub) — Hermes points you at
### n8n's official MCP server
### Tool selection at install time
### Trust model
### Manifest version compatibility
### Runtime `${ENV_VAR}` substitution
### Entries that need your own OAuth app (no DCR)
### Updating tool selection later
### Updating the catalog manifest
### Suggestion metadata (`suggest:`)
## Two kinds of MCP servers
### Stdio servers
- the server is installed locally
- you want low-latency access to local resources
- you are following MCP server docs that show `command`, `args`, and `env`
### HTTP servers
- the MCP server is hosted elsewhere
- your organization exposes internal MCP endpoints
- you do not want Hermes spawning a local subprocess for that integration
### OAuth-authenticated HTTP servers
- **Hermes Desktop (automatic):** when you run the OAuth sign-in from the Desktop app's MCP setup UI against a remote backend, Desktop hosts the callback listener on *your* machine and relays the authorization back to the gateway automatically — no tunnel, paste, or proxy needed. Requires both the Desktop app and the backend to be up to date.
- **Paste-back (no setup):** on an interactive terminal Hermes prints "Or paste the redirect URL here…" alongside the authorize URL. Open the URL in your browser, approve, copy the full URL the browser ends up on (the redirect will show a connection error — that's expected), paste it at the prompt. Bare `?code=…&state=…` query strings work too.
- **Device-code login (no callback at all):** if the server's authorization server advertises a device authorization endpoint, run `hermes mcp login <server> --flow device` on the machine running Hermes. It prints a verification URL and a short code; open the URL on any device, enter the code, and Hermes polls for approval. No browser is launched o
- **SSH port forward:** `ssh -N -L <port>:127.0.0.1:<port> user@host` in a separate terminal, then let the redirect flow normally.
- **Proxied callback (`redirect_uri`):** when a public HTTPS endpoint forwards to the host (e.g. a Tailscale Funnel or reverse proxy pointed at the callback port), set `oauth.redirect_uri` and the browser redirect reaches Hermes on its own — no tunnel or paste needed:
## mTLS / client certificates
- **A single combined PEM path** — one file holding both the certificate and the private key:
- **A `[cert, key]` 2-tuple** — certificate and key in separate files (equivalent to setting `client_cert` + `client_key`):
- **A `[cert, key, password]` 3-tuple** — when the private key is encrypted, the third element is the key passphrase:
## Per-user identity header
- `value_from: static` sends the literal `value` from config.yaml.
- `value_from: profile` sends the active Hermes profile name, resolved once at connect time — useful when multiple profiles on one machine talk to the same server and it needs to tell them apart.
## Basic configuration reference
### Common keys
| Key | Type | Meaning |
|---|---|---|
| `command` | string | Executable for a stdio MCP server |
| `args` | list | Arguments for the stdio server |
| `env` | mapping | Environment variables passed to the stdio server |
| `cwd` | string | Working directory for the stdio server process. Default: the session working directory when one is pinned (ACP/gateway sessions, `terminal.cwd`), else the Hermes process directory |
| `url` | string | HTTP MCP endpoint |
| `headers` | mapping | HTTP headers for remote servers |
| `client_cert` | string \| list | Client certificate for mTLS — a combined PEM path, or `[cert, key]` / `[cert, key, password]` |
| `client_key` | string | Client private-key PEM path (when separate from `client_cert`) |
| `identity_header` | mapping | Optional per-user identity header for HTTP/SSE servers — `{name, value_from: static\|profile, value}` |
| `timeout` | number | Tool call timeout |
| `connect_timeout` | number | Initial connection timeout (also bounds the MCP `initialize` handshake) |
| `lazy` | bool | If `true`, register the server's tools from the schema cache at startup and only start/connect it on the first tool call (default `false`). Needs one prior live connect to fill the cache. |
| `idle_timeout_seconds` | number | Recycle a stdio server after this many seconds without a tool call (`0` = never, default). The server restarts transparently on the next tool call. |
| `max_lifetime_seconds` | number | Recycle a stdio server after this total age (`0` = never, default). Restarts transparently on next use. |
| `enabled` | bool | If `false`, Hermes skips the server entirely |
| `supports_parallel_tool_calls` | bool | If `true`, tools from this server may run concurrently |
| `events` | bool | If `true`, register the experimental MCP events tools for this server (default `false`). See [MCP Events](#mcp-events-experimental) |
| `tools` | mapping | Per-server tool filtering and utility policy |
### Minimal stdio example
### Recycling memory-heavy stdio servers
### Minimal HTTP example
## Built-in presets
| Preset | What it wires up |
|---|---|
| `codex` | The Codex CLI's MCP server (`codex mcp-server` over stdio). Requires the `codex` CLI on PATH. |
## How Hermes registers MCP tools
| Server | MCP tool | Registered name |
|---|---|---|
| `filesystem` | `read_file` | `mcp_filesystem_read_file` |
| `github` | `create-issue` | `mcp_github_create_issue` |
| `my-api` | `query.data` | `mcp_my_api_query_data` |
### Tool-result sanitization and `_meta`
- **Invisible Unicode TAG characters are stripped.** Characters in the U+E0000–U+E007F range render as nothing in terminals and chat UIs but are fully visible to the model — a classic prompt-injection smuggling channel for a malicious or compromised server. Hermes strips them from tool results, resource content, and tool descriptions. Legitimate em
- **Vendor `_meta` is surfaced; protocol-reserved keys are not.** When a server attaches a `_meta` mapping to a tool result (vendor namespaces like `com.example/handoff`), Hermes passes it through to the model alongside the result content. Keys under protocol-reserved prefixes — a `modelcontextprotocol` or `mcp` label followed by another label, e.g
### Image results
## MCP utility tools
- `list_resources`
- `read_resource`
- `list_prompts`
- `get_prompt`
- `mcp_github_list_resources`
- `mcp_github_get_prompt`
### Important
- Hermes only registers resource utilities if the MCP session actually supports resource operations
- Hermes only registers prompt utilities if the MCP session actually supports prompt operations
## Per-server filtering
### Disable a server entirely
### Whitelist server tools
### Blacklist server tools
### Glob patterns
### Precedence rule
### Filter utility tools too
- `tools.resources: false` disables `list_resources` and `read_resource`
- `tools.prompts: false` disables `list_prompts` and `get_prompt`
### Full example
## What happens if everything is filtered out?
## Runtime behavior
### Discovery time
### Lazy start
### Dynamic Tool Discovery
### Reloading
### Toolsets
## Security model
### Stdio env filtering
### Config-level exposure control
- disable dangerous tools you do not want the model to see
- expose only a minimal whitelist for a sensitive server
- disable resource/prompt wrappers when you do not want that surface exposed
## Example use cases
### GitHub server with a minimal issue-management surface
### Stripe server with dangerous actions removed
### Filesystem server for a single project root
## Troubleshooting
### MCP server not connecting
### Remote (HTTP) server rejects the connection
### A server's response is too large
### Tools not appearing
- the server failed to connect
- discovery failed
- your filter config excluded the tools
- the utility capability does not exist on that server
- the server is disabled with `enabled: false`
### Why didn't resource or prompt utilities appear?
## Parallel Tool Calls
## MCP Sampling Support
## Tool costs and spend limits
- The approval prompt names the cost: `Costs about 20 credits.` or `Costs 5-20 credits (per animation).`, plus `This chat has used about 40 credits on 'renders' (limit 100).` once the chat has spent something.
- Hermes keeps a running total per chat and per server, counting the declared figure (the top of a range) for every costed call it approved and started. Declined or blocked calls add nothing; a call whose server dies before receiving it still counts, which errs toward asking sooner. The total lives in memory: it is a budget for one conversation, no
- The total follows the conversation when context compression or `/branch` continues it under a new session id. Starting a new chat (`/new` in the CLI, a new Desktop chat) starts a new total; on messaging platforms the total belongs to the chat, so it survives `/new` there. Each cron run has its own total. Restarting Hermes resets every total.
- With `spend_limit`, a costed call that would take the chat past the limit asks first, even on a `trust: full` server that otherwise never prompts, and every further costed call in that chat asks again: that prompt offers only Allow Once and Deny, and an earlier Session or Always answer for the tool does not silence it. Where nobody can answer (cr
- Below the limit, a `trust: full` server still runs costed tools without asking; on a `trust: untrusted` server every write-capable call already asks and the prompt now carries the cost. Tools annotated `readOnlyHint: true` stay unprompted unless the budget requires it.
- [YOLO](../security.md#yolo-mode) (`/yolo`, `--yolo`) and `approvals.mode: off` skip the `trust: untrusted` prompt (`/yolo` for that chat, `--yolo` for that process, `approvals.mode: off` for that profile), but not the spend limit: below the limit a costed call runs without asking, and a call that would cross it still asks (or is refused where nob
- A malformed `tool_costs`, `spend_limit` or `cost_unit` skips the server with an error naming the key, so a budget you declared is never silently dropped. A `tool_costs` key the server does not offer (a typo, or Hermes' `mcp_<server>_` display prefix copied in) is named in a warning at connect time: use the server's own tool names.
## MCP Elicitation Support
## MCP Events (experimental)
- `list_event_types`: the event types the server offers, and your current subscriptions to them
- `subscribe_event`: subscribe with the event's arguments, what to do when it arrives
- `unsubscribe_event`: cancel a subscription
## Running Hermes as an MCP server
## Related docs
- [Use MCP with Hermes](../../guides/use-mcp-with-hermes.md)
- [CLI Commands](../../reference/cli-commands.md)
- [Slash Commands](../../reference/slash-commands.md)
- [FAQ](../../reference/faq.md)

## plugins.md
# Plugins
## Quick overview
### Minimal working example
### Project-local plugins (`./.hermes/plugins/`) {#project-local-plugins}
## What plugins can do
| Capability | How |
|-----------|-----|
| Add tools | `ctx.register_tool(name=..., toolset=..., schema=..., handler=...)` |
| Add hooks | `ctx.register_hook("post_tool_call", callback)` |
| Add slash commands | `ctx.register_command(name, handler, description)` — adds `/name` in CLI and gateway sessions |
| Dispatch tools from commands | `ctx.dispatch_tool(name, args)` — invokes a registered tool with parent-agent context auto-wired |
| Add CLI commands | `ctx.register_cli_command(name, help, setup_fn, handler_fn)` — adds `hermes <plugin> <subcommand>` |
| Inject messages | `ctx.inject_message(content, role="user", session_key=... \| origin=...)` - see [Injecting Messages](#injecting-messages) |
| Post and edit a status message | `handle = ctx.post_message(text)`, then `handle.edit(text)` — see [Posting a live message](#posting-a-live-message) |
| Ship data files | `Path(__file__).parent / "data" / "file.yaml"` |
| Bundle skills | `ctx.register_skill(name, path)` — namespaced as `plugin:skill`, loaded via `skill_view("plugin:skill")` |
| Gate on env vars | `requires_env: [API_KEY]` in plugin.yaml — prompted during `hermes plugins install` |
| Distribute via pip | `[project.entry-points."hermes_agent.plugins"]` |
| Register a gateway platform (Discord, Telegram, IRC, …) | `ctx.register_platform(name, label, adapter_factory, check_fn, ...)` — see [Adding Platform Adapters](../../developer-guide/adding-platform-adapters.md) |
| Register an image-generation backend | `ctx.register_image_gen_provider(provider)` — see [Image Generation Provider Plugins](../../developer-guide/image-gen-provider-plugin.md) |
| Register a delegation strategy (how each `delegate_task` subagent is routed) | `ctx.register_delegation_provider(provider)` — see [Delegation Provider Plugins](../../developer-guide/delegation-provider-plugin.md) |
| Register a video-generation backend | `ctx.register_video_gen_provider(provider)` — see [Video Generation Provider Plugins](../../developer-guide/video-gen-provider-plugin.md) |
| Register a context-compression engine | `ctx.register_context_engine(engine)` — see [Context Engine Plugins](../../developer-guide/context-engine-plugin.md) |
| Register a terminal execution backend (cloud sandbox) | `ctx.register_terminal_environment_provider(provider)` — see [Terminal Environment Plugins](../../developer-guide/terminal-environment-plugin.md) |
| Register a computer-use driver | `ctx.register_computer_use_provider(provider)` — selected by `computer_use.backend`; see [Computer-use backend plugins](../../developer-guide/plugins/index.md#computer-use-backend-plugins) |
| Route human approval prompts | `ctx.register_approval_transport(name, present_fn)` — see [Approval transports](#approval-transports) |
| Register a memory backend | Subclass `MemoryProvider` in `plugins/memory/<name>/__init__.py` — see [Memory Provider Plugins](../../developer-guide/memory-provider-plugin.md) (uses a separate discovery system) |
| Run a host-owned LLM call | `ctx.llm.complete(...)` / `ctx.llm.complete_structured(...)` — borrow the user's active model + auth for a one-shot completion with optional JSON schema validation. See [Plugin LLM Access](../../developer-guide/plugin-llm-access.md) |
| Call an MCP tool (capability-gated) | `ctx.call_mcp(server, tool, arguments, timeout=30)` — see [Calling MCP servers from plugins](#calling-mcp-servers-from-plugins) |
| Register an inference backend (LLM provider) | `register_provider(ProviderProfile(...))` in `plugins/model-providers/<name>/__init__.py` — see [Model Provider Plugins](../../developer-guide/model-provider-plugin.md) (uses a separate discovery system) |
- **Return its text** instead of calling `print()`. Printed output goes to the gateway's stdout, not to the user's chat.
- **Read the session's working directory with `agent.runtime_cwd.resolve_agent_cwd()`**, not `os.getcwd()`, which is the gateway's own directory. The gateway pins the session's directory while the handler runs.
- **Take the calling session as handler keywords**: declare `session_id`, `session_key`, `platform` or `source` (the sender's `SessionSource`) and Hermes passes them; one-argument handlers keep working unchanged. The same values are bound as `HERMES_SESSION_*` around the handler (read them with `from gateway.session_context import get_session_env`,
## Plugin discovery
| Source | Path | Use case |
|--------|------|----------|
| Bundled | `<repo>/plugins/` | Ships with Hermes — see [Built-in Plugins](./built-in-plugins.md) |
| User | `~/.hermes/plugins/` | Personal plugins |
| Project | `.hermes/plugins/` | Not loaded by the gateway that runs sessions; see [Project-local plugins](#project-local-plugins) |
| pip | `hermes_agent.plugins` entry_points | Distributed packages |
| Nix | `services.hermes-agent.extraPlugins` / `extraPythonPackages` | NixOS declarative installs — see [Nix Setup](../../getting-started/nix-setup.md#plugins) |
### Plugin sub-categories
| Sub-directory | What it holds | Discovery system |
|---|---|---|
| `plugins/` (root) | General plugins — tools, hooks, slash commands, CLI commands, bundled skills | `PluginManager` (kind: `standalone` or `backend`) |
| `plugins/platforms/<name>/` | Gateway channel adapters (`ctx.register_platform()`) | `PluginManager` (kind: `platform`, one level deeper) |
| `plugins/image_gen/<name>/` | Image-generation backends (`ctx.register_image_gen_provider()`) | `PluginManager` (kind: `backend`, one level deeper) |
| `plugins/delegation/<name>/` | Delegation strategies (`ctx.register_delegation_provider()`); `default` is today's routing | `PluginManager` (kind: `backend`, one level deeper) |
| `plugins/memory/<name>/` | Memory providers (subclass `MemoryProvider`) | **Own loader** in `plugins/memory/__init__.py` (kind: `exclusive` — one active at a time) |
| `plugins/context_engine/<name>/` | Context-compression engines (`ctx.register_context_engine()`) | **Own loader** in `plugins/context_engine/__init__.py` (one active at a time) |
| `plugins/model-providers/<name>/` | LLM provider profiles (`register_provider(ProviderProfile(...))`) | **Own loader** in `providers/__init__.py` (lazily scanned on first `get_provider_profile()` call) |
## Plugins are opt-in (with a few exceptions)
### Installing from a private repository
### What the allow-list does NOT gate
| Plugin kind | How it's activated instead |
|---|---|
| **Bundled platform plugins** (IRC, Teams, etc. under `plugins/platforms/`) | Auto-loaded so every shipped gateway channel is available. The actual channel turns on via `gateway.platforms.<name>.enabled` in `config.yaml`. |
| **Bundled backends** (image-gen providers under `plugins/image_gen/`, etc.) | Auto-loaded so the default backend "just works". Selection happens via `<category>.provider` in `config.yaml` (e.g. `image_gen.provider: openai`). |
| **Memory providers** (`plugins/memory/`) | All discovered; exactly one is active, chosen by `memory.provider` in `config.yaml`. |
| **Context engines** (`plugins/context_engine/`) | All discovered; one is active, chosen by `context.engine` in `config.yaml`. |
| **Model providers** (`plugins/model-providers/`) | All bundled providers under `plugins/model-providers/` discover and register at the first `get_provider_profile()` call. The user picks one at a time via `--provider` or `config.yaml`. |
| **Pip-installed `backend` plugins** | Opt-in via `plugins.enabled` (same as general plugins). |
| **User-installed platforms** (under `~/.hermes/plugins/platforms/`) | Opt-in via `plugins.enabled` — third-party gateway adapters need explicit consent. |
### Approval transports
### Migration for existing users
## Available hooks
| Descriptive category | Shipped hooks |
|---|---|
| **Directive/control** | `pre_tool_call`, `pre_llm_call`, `pre_verify`, `pre_gateway_dispatch` |
| **Transform** | `transform_tool_result`, `transform_terminal_output`, `transform_llm_output`, `pre_transcription` |
| **Observer** | `post_tool_call`, `post_llm_call`, `pre_api_request`, `post_api_request`, `api_request_error`, `pre_auxiliary_call`, `post_auxiliary_call`, `on_stream_start`, `on_stream_delta`, `on_stream_end`, `on_interim_message`, `on_session_start`, `on_session_end`, `on_session_finalize`, `on_session_reset`, `post_compaction`, `post_curator_ru
## Plugin types
| Type | What it does | Selection | Location |
|------|-------------|-----------|----------|
| **General plugins** | Add tools, hooks, slash commands, CLI commands | Multi-select (enable/disable) | `~/.hermes/plugins/` |
| **Memory providers** | Replace or augment built-in memory | Single-select (one active) | `plugins/memory/` |
| **Context engines** | Replace the built-in context compressor | Single-select (one active) | `plugins/context_engine/` |
| **Model providers** | Declare an inference backend (OpenRouter, Anthropic, …) | Multi-register, picked by `--provider` / `config.yaml` | `plugins/model-providers/` |
## Pluggable interfaces — where to go for each
| Want to add… | How | Authoring guide |
|---|---|---|
| A **tool** the LLM can call | Python plugin — `ctx.register_tool()` | [Build a Hermes Plugin](../../developer-guide/plugins/index.md) · [Adding Tools](../../developer-guide/adding-tools.md) |
| A **lifecycle hook** (pre/post LLM, session start/end, tool filter) | Python plugin — `ctx.register_hook()` | [Hooks reference](./hooks.md) · [Build a Hermes Plugin](../../developer-guide/plugins/index.md) |
| A **slash command** for the CLI / gateway | Python plugin — `ctx.register_command()` | [Build a Hermes Plugin](../../developer-guide/plugins/index.md) · [Extending the CLI](../../developer-guide/extending-the-cli.md) |
| A **subcommand** for `hermes <thing>` | Python plugin — `ctx.register_cli_command()` | [Extending the CLI](../../developer-guide/extending-the-cli.md) |
| A bundled **skill** that your plugin ships | Python plugin — `ctx.register_skill()` | [Creating Skills](../../developer-guide/creating-skills.md) |
| An **inference backend** (LLM provider: OpenAI-compat, Codex, Anthropic-Messages, Bedrock) | Provider plugin — `register_provider(ProviderProfile(...))` in `plugins/model-providers/<name>/` | **[Model Provider Plugins](../../developer-guide/model-provider-plugin.md)** · [Adding Providers](../../developer-guide/adding-providers.md) |
| A **gateway channel** (Discord / Telegram / IRC / Teams / etc.) | Platform plugin — `ctx.register_platform()` in `plugins/platforms/<name>/` | [Adding Platform Adapters](../../developer-guide/adding-platform-adapters.md) |
| A **memory backend** (Honcho, Mem0, Supermemory, …) | Memory plugin — subclass `MemoryProvider` in `plugins/memory/<name>/` | [Memory Provider Plugins](../../developer-guide/memory-provider-plugin.md) |
| A **context-compression strategy** | Context-engine plugin — `ctx.register_context_engine()` | [Context Engine Plugins](../../developer-guide/context-engine-plugin.md) |
| An **image-generation backend** (DALL·E, SDXL, …) | Backend plugin — `ctx.register_image_gen_provider()` | [Image Generation Provider Plugins](../../developer-guide/image-gen-provider-plugin.md) |
| A **delegation strategy** (a profile or named model per subagent) | Backend plugin — `ctx.register_delegation_provider()`, selected by `delegation.backend` | [Delegation Provider Plugins](../../developer-guide/delegation-provider-plugin.md) |
| A **video-generation backend** (Veo, Kling, Pixverse, Grok-Imagine, Runway, …) | Backend plugin — `ctx.register_video_gen_provider()` | [Video Generation Provider Plugins](../../developer-guide/video-gen-provider-plugin.md) |
| A **computer-use driver** (desktop control behind the `computer_use` tool) | Single-select provider plugin — `ctx.register_computer_use_provider()`, selected by `computer_use.backend` | [Computer-use backend plugins](../../developer-guide/plugins/index.md#computer-use-backend-plugins) |
| A **TTS backend** (any CLI — Piper, VoxCPM, Kokoro, xtts, voice-cloning scripts, …) | Config-driven (recommended) — declare under `tts.providers.<name>` with `type: command` in `config.yaml`. OR Python backend plugin — `ctx.register_tts_provider()` for Python-SDK / streaming engines that need more than a shell template. | [TTS Setup](./tts.md#cus
| An **STT backend** (any CLI — whisper.cpp, custom whisper binary, local ASR CLI) | Config-driven (recommended) — declare under `stt.providers.<name>` with `type: command` in `config.yaml`, or set `HERMES_LOCAL_STT_COMMAND` for the legacy single-command escape hatch. OR Python backend plugin — `ctx.register_transcription_provider()` for Python-SDK
| **External tools via MCP** (filesystem, GitHub, Linear, Notion, any MCP server) | Config-driven — declare `mcp_servers.<name>` with `command:` / `url:` in `config.yaml`. Hermes auto-discovers the server's tools and registers them alongside built-ins. | [MCP](./mcp.md) |
| **Additional skill sources** (custom GitHub repos, private skill indexes) | CLI — `hermes skills tap add <repo>` | [Skills Hub](./skills.md#skills-hub) · [Publishing a custom tap](./skills.md#publishing-a-custom-skill-tap) |
| **Gateway event hooks** (fire on `gateway:startup`, `session:start`, `agent:end`, `command:*`) | Drop `HOOK.yaml` + `handler.py` into `~/.hermes/hooks/<name>/` | [Event Hooks](./hooks.md#gateway-event-hooks) |
| **Shell hooks** (run a shell command on events — notifications, audit logs, desktop alerts) | Config-driven — declare under `hooks:` in `config.yaml` | [Shell Hooks](./hooks.md#shell-hooks) |
## NixOS declarative plugins
## Managing plugins
### Update checks and provenance
| Surface | Update all | Auto-update |
|---|---|---|
| CLI | `hermes plugins update` | `hermes plugins auto-update [on\|off]` |
| Chat (classic CLI and TUI) | `/plugins update` | `/plugins auto-update [on\|off]` |
| Desktop app | Plugins tab, **Update all** | Plugins tab, **Auto-update** switch |
| Web dashboard | Plugins page, **Update all** | Plugins page, **Auto-update** switch |
### Dependency preparation and preservation
### One-click install links (Desktop)
### Plugin capabilities and consent
| Capability | Legacy key (`plugins.entries.<id>.…`) |
|---|---|
| `tools.override` | `allow_tool_override` |
| `llm.provider_override` | `llm.allow_provider_override` |
| `llm.model_override` | `llm.allow_model_override` |
| `llm.agent_id_override` | `llm.allow_agent_id_override` |
| `llm.profile_override` | `llm.allow_profile_override` |
| `llm.task_override` | `llm.allow_task_override` |
| `gateway.platform_actions` | `allow_platform_actions` |
### Platform actions
### Discovering plugins — the Hermes plugin catalog
### Plugin packs
### Install-time security scanning
| Verdict | Behavior |
|---|---|
| **safe** | Installs normally, no extra output |
| **caution** | Findings are shown; you confirm `Install anyway? [y/N]` (or pass `--force`) |
| **dangerous** | Blocked. `--force` does **not** override |
- **Documentation prose** (`README.md`, `AGENTS.md`, `docs/**/*.md`, `.txt`,
- **Test trees and fixtures** (`tests/`, `test/`, `testing/`, `spec/`,
- **Whole-line comments and `CHANGELOG.md`** describe a defense; they score as
- **Base64 that decodes to a media header** (PNG/JPEG/GIF/WOFF/PDF … in a
### Running plugins out of process (`plugins.isolation`)
- **No shared interpreter.** A plugin's module never enters the Hermes process, so it cannot read
- **Crashes stay contained.** A plugin that crashes or exits kills its host, not Hermes; the call in
- **A few surfaces need in-process code** and fail that plugin with a clear reason instead of loading:
- **No cookies.** A hosted plugin's API answers are served from the gateway's own address, so Hermes
### Interactive UI
- **General Plugins section** — checkboxes, toggle with SPACE. A row opens checked when the plugin is active right now: listed in `plugins.enabled`, or a bundled platform, backend or model provider (on without a list entry), or the selected provider of a category. Only rows you flip are written on exit: unticking adds the plugin to `plugins.disable
- **Provider Plugins section** — shows current selection. Press ENTER to drill into a radio picker where you choose one active provider.
- Bundled plugins appear in the same list with a `[bundled]` tag.
### Enabled vs. disabled vs. neither
| State | Meaning | In `plugins.enabled`? | In `plugins.disabled`? |
|---|---|---|---|
| `enabled` | Loaded on next session | Yes | No |
| `disabled` | Explicitly off — won't load even if also in `enabled` | (irrelevant) | Yes |
| `not enabled` | Discovered but never opted in | No | No |
## Posting a live message
| Surface | `post_message` | `edit` |
|---|---|---|
| Messaging platforms that can edit (Telegram, Discord, Slack, Matrix, ...) | the adapter's normal `send` into the chat/thread | edits the same message in place |
| Platforms without edit support (SMS, email, plugin adapters that do not implement `edit_message`) | `send` | sends a new message; the handle follows it (`edited: False, fallback: True`) |
| Desktop and the Ink TUI | a keyed notice to the session's viewers (Desktop toast, TUI status line) | replaces that notice in place |
| Classic CLI (`hermes chat`) | queued as a notice line, printed at the end of the next turn (the classic CLI has no live notice slot) | each edit queues another line |
## Injecting Messages
- If the target session is **idle**, the message starts a new turn.
- If it is **mid-turn**, the message waits in that session's queue and runs as the next turn after the current one finishes; the running tool is not interrupted.
- For non-`"user"` roles, the content is prefixed with `[role]` (e.g. `[system] ...`).
- Returns `True` when a host accepted the message (see the per-host rules below).
- `session_key` must identify an existing gateway session. It is the stable routing key, not the CLI session ID.
- Hermes reuses that session's stored platform, chat, thread, profile, and conversation history. Plugins cannot change that route through `session_key`.
- The session must belong to the plugin's own profile: the key of another profile's chat or local session gets `False` right away, with the reason in the gateway log.
- Hermes rechecks the stored route against the gateway's current authorisation rules before dispatch.
- Routes that relied only on an adapter-time or upstream authorisation decision are rejected unless Hermes can revalidate them from current core allowlists, pairing, or explicit allow-all configuration.
- Injected text is always conversational input. It cannot invoke slash commands, approve tools, or resolve pending confirmation and clarification prompts.
- The route and conversation are pinned while dispatch is pending. Hermes drops the request if topic recovery changes the route or the session rotates before handling starts.
- The request enters the platform adapter's normal message path. Active sessions use the existing busy-session queue rather than starting a competing turn.
- Returns `True` when the live gateway accepts the request for asynchronous dispatch. This does not confirm that the agent turn or platform delivery has completed.
- Returns `False` when both `session_key` and `origin` are given, neither is given and no chat is chosen (above), the permission is not granted, or no live host can accept the request. Unknown or unroutable session keys discovered after asynchronous acceptance are written to the gateway log.
- `origin` uses the same shape as a stored session origin (`SessionSource.to_dict()`): `platform` and `chat_id` are required; `chat_type`, `thread_id`, `user_id`, `user_name`, `chat_name` and the other origin fields are optional. Use it when no session exists yet — for example, a bot-authored forum post, which platforms like Discord never deliver b
- The session is created in **the plugin's own profile** (the profile whose plugin manager loaded it) through the normal session path, with a fresh prompt and an empty history, and the reply is delivered to that chat/thread. If the chat already has a session in that profile, the text joins it exactly like a human message would (the busy-session que
- Under the gateway's session authority (every gateway-owned session), the plugin turn is committed to that session's durable FIFO and borrows the connector provenance of a human message already admitted there. An `origin` whose chat has **no human message admitted yet** (no session, or a session with no committed human turn) is therefore refused: 
- A plugin can never target another profile. `False` is returned (with the reason in the gateway log) when the origin names a different `profile`, when the gateway's own profile routing sends that chat to a different profile, when the plugin's profile is not served by this gateway, or when that profile has no connected adapter for `platform` — anot
- Authorisation is not bypassed: the origin's user must pass the gateway's current allowlists, pairing, or allow-all configuration for that profile, the same check as the `session_key` path. Choose a `user_id` that is authorised, or the request is dropped at dispatch and logged.
- The same text rules apply: no slash commands, approvals, or prompt resolution. The permission below is read from the plugin's own profile `config.yaml`.
## Plugin code inside the gateway
- `os.getcwd()` is the gateway's own directory, shared by every session; Hermes never changes it. Use `ctx.session_cwd()` for the working directory of the session the hook or tool runs for: the workspace a CLI/TUI/Desktop session was started in, or the profile's terminal cwd for a chat (`""` when there is none). Pass it as `cwd=` to subprocesses. I
- `ctx.session_surface()` names the calling session's client: `"cli"`, `"tui"`, `"desktop"`, `"acp"`, or the messaging platform (`"telegram"`, …); `""` outside a session.
- `hermes_cli.plugin_session.session_client()` returns the client that created the calling local session, as it named itself on `session.create` (`{"name": "my-client", "version": "1.2.0"}`; `version` only when sent), or `None`: outside a session, for a messaging chat, and for a client that sent no name (the TUI, Desktop, the classic CLI and ACP se
- `print()` output goes to the gateway's `gateway-stdio.log`, not to your terminal. Use `logging` instead.
- `input()` gets end-of-file: the gateway has no terminal. Ask through a tool result or a slash command.
## A plugin with its own chat client
- `hermes_cli.gateway_chat.shell_client_env()` returns the allowlisted variables of the shell that ran your command, as a dict. Call it in the launcher, before you change the environment, and send the dict as `client_env` on `session.create`. The session's commands then run with the user's `PATH`, virtual environment and toolchain variables. Names 
- `hermes_cli.chat_launch.hermes_command(args)` returns the argv that runs this Hermes install with `args`. It needs no `PYTHONPATH`; the child serves the profile named by the inherited `HERMES_HOME`. Give it to your frontend for `hermes gateway attach`, instead of `hermes` from `PATH` (which may be another install) or `sys.executable` (which an up
- `hermes_cli.chat_launch.print_exit_summary(session_id, command=...)` prints the lines Hermes shows when a chat ends: how to resume the session, its id, title, message count and tokens. Call it after your frontend exits, with your own command name. It prints nothing for a session with no messages. The lines name two ways to resume, `--resume <id>`
### `-w`: run the session in a fresh worktree
- `create()` before the client starts. It makes a new git worktree of the repository the user is in, under `<repo>/.worktrees/`, on its own `hermes/` branch. It returns `{"path", "branch", "repo_root", "base"}`, or `None` when no tree could be made (it has already printed why). Send `path` as the `cwd` of `session.create`.
- `restore(session_id)` before the client attaches to a resumed session. Pass the full session id. When that session ran in a `-w` tree that an earlier quit removed, it makes the tree again at the same path and returns the path. Otherwise it returns `None`. Without it, the gateway refuses every turn of that session with `workspace_unavailable`.
- `finish(info, session_id, ask=...)` after the client exits, in a `finally` block, so a client that fails to start does not leave the tree behind. Pass `ask=True` after a clean exit: a tree with uncommitted or unpushed work then gets a keep / archive / discard question on the terminal. Call it only after your client has given the terminal back. A 
## Calling MCP servers from plugins
### Security: default-off, per-server allowlist
- Calling a server not in the list raises `PermissionError` naming the exact config key to set.
- The grant is per-server and per-plugin — never ambient authority over every configured server, and `"*"` wildcards are not honored.
- Every call has an enforced timeout (default 30 s) so a hung MCP server cannot stall the hook or tool pipeline that invoked it.
- MCP servers return untrusted content. Treat `result` as data, not instructions — don't feed it into privileged decisions (approvals, command execution) without validation.

## tools.md
# Tools & Toolsets
## Available Tools
| Category | Examples | Description |
|----------|----------|-------------|
| **Web** | `web_search`, `web_extract` | Search the web and extract page content. |
| **X Search** | `x_search` | Search X (Twitter) posts and threads via xAI's built-in `x_search` Responses tool — gated on xAI credentials (SuperGrok OAuth or `XAI_API_KEY`); off by default, opt in via `hermes tools` → 🐦 X (Twitter) Search. |
| **Terminal & Files** | `terminal`, `process`, `read_file`, `patch` | Execute commands and manipulate files. |
| **Browser** | `browser_exec`, `browser_cdp`, `browser_dialog`, `browser_vault` | Browser automation: `browser_exec` drives Chromium with Python over CDP (screenshots included). The Camofox browser is a separate catalog plugin (`hermes plugins install camofox`). |
| **Media** | `vision_analyze`, `image_generate`, `text_to_speech` | Multimodal analysis and generation. |
| **Agent orchestration** | `todo`, `clarify`, `execute_code`, `delegate_task` | Planning, clarification, code execution, and subagent delegation. |
| **Memory & recall** | `memory`, `session_search` | Persistent memory and session search. |
| **Automation** | `cronjob` | Scheduled tasks with create/list/update/pause/resume/run/remove actions. Outbound delivery is handled by cron's own delivery, the `hermes send` CLI, and the gateway notifier — not by an agent-callable tool. |
| **Integrations** | MCP server tools, plugin tools | MCP and plugin integrations, e.g. Home Assistant (`ha_*`, from the [`homeassistant` catalog plugin](../messaging/homeassistant.md)). |
## Using Toolsets
## Tool result annotations
- **Signal deaths are explained.** When a terminal command is killed by a signal, the result carries a human-readable note instead of a bare numeric code — e.g. exit `-9`/`137` becomes "terminated by signal 9: SIGKILL — often the kernel OOM killer on memory exhaustion, or an explicit kill -9", and segfaults, aborts, SIGTERM, broken pipes, and CPU/f
- **UTF-16 text files are transcoded, not refused.** `read_file` detects UTF-16 (BOM or byte-pattern heuristic, either endianness — common for Windows Notepad files and PowerShell `>` redirects) and transcodes it to UTF-8 for display instead of flagging the file as binary. The result includes a hint disclosing the conversion; edits via `patch`/`wri
## Terminal Backends
| Backend | Description | Use Case |
|---------|-------------|----------|
| `local` | Run on your machine (default) | Development, trusted tasks |
| `docker` | Isolated containers | Security, reproducibility |
| `ssh` | Remote server | Sandboxing, keep agent away from its own code |
| `singularity` (plugin) | HPC containers | Cluster computing, rootless |
| `sprites` (plugin) | Stateful Fly.io cloud sandbox | Persistent cloud environments with checkpoint & restore |
| `modal` | Cloud execution | Serverless, scale |
| `daytona` | Cloud sandbox workspace | Persistent remote dev environments |
| `vercel_sandbox` | Vercel Sandbox cloud microVM | Cloud execution with snapshot-backed filesystem persistence |
### Configuration
### Shell startup files and non-interactive commands
- **Slow init (`nvm`, version managers, network-touching prompts):** the classic `nvm.sh` sourcing adds noticeable latency to *every* shell start, and the agent starts many shells. Multi-second rc files turn a quick `git status` into a timeout risk.
- **TTY-expecting blocks:** anything in `.bashrc`/`.zshrc` that prompts, runs `tmux`/`screen` attach, calls `read`, or prints a menu will hang a non-interactive shell — the command appears to run forever and then times out.
- **Unconditional output:** rc files that `echo` banners pollute every command's output the agent has to parse.
### Docker Backend
### SSH Backend
### Singularity/Apptainer
### Sprites
### Modal (Serverless Cloud)
### Vercel Sandbox
### Container Resources
### Container Security
- Read-only root filesystem (Docker)
- All Linux capabilities dropped
- No privilege escalation
- PID limits (256 processes)
- Full namespace isolation
- Persistent workspace via volumes, not writable root layer
## Background Process Management
### Repeated waits on a quiet process
## Sudo Support

## skills.md
# Skills System
- [Bundled Skills Catalog](../../reference/skills-catalog.md)
- [Official Optional Skills Catalog](../../reference/optional-skills-catalog.md)
## Install from the website
## Starting with a blank slate
## Using Skills
### Stacking multiple skills in one command
## Learning a skill from sources (`/learn`)
### Large sources become knowledge-base skills
## Progressive Disclosure
### Skills added during a conversation
## SKILL.md Format
### Platform-Specific Skills
| Value | Matches |
|-------|---------|
| `macos` | macOS (Darwin) |
| `linux` | Linux |
| `windows` | Windows |
## Skill output and media delivery
### Forcing document-style delivery: `[[as_document]]`
- You produce screenshots or charts the user needs as files (for editing in another tool, archiving, sharing intact).
- The default lossy preview would obscure detail (small text, pixel-accurate diagrams, color-sensitive renders).
### Conditional Activation (Fallback Skills)
| Field | Behavior |
|-------|----------|
| `fallback_for_toolsets` | Skill is **hidden** when the listed toolsets are available. Shown when they're missing. |
| `fallback_for_tools` | Same, but checks individual tools instead of toolsets. |
| `requires_toolsets` | Skill is **hidden** when the listed toolsets are unavailable. Shown when they're present. |
| `requires_tools` | Same, but checks individual tools. |
## Secure Setup on Load
### Skill Config Settings
## Skill Directory Structure
### Advisory SkillEvaluator scan
## External Skill Directories
### How it works
- **Create locally, update in place**: New agent-created skills are written to `~/.hermes/skills/` (or `skills.create_dir` when configured — see below). Existing skills are modified where they are found, including skills under `external_dirs`, when the agent uses `skill_manage` actions such as `patch` (targeted or full rewrite), `write_file`, `remo
- **External dirs are not a write-protection boundary**: If an external skill directory is writable by the Hermes process, agent-managed skill updates can change files in that directory. Use filesystem permissions or a separate profile/toolset setup if shared external skills must stay read-only.
- **Precedence**: If the same skill name exists in more than one directory, the higher-precedence directory wins everywhere — skill index, `skills_list`, `hermes skills list`, `skill_view`, slash commands, `-s` preload and cron: `project → local (~/.hermes/skills/) → skills.create_dir → external_dirs`. The hidden copy is logged as shadowed. See [Du
- **Full integration**: External skills appear in the system prompt index, `skills_list`, `skill_view`, and as `/skill-name` slash commands — no different from local skills.
- **Non-existent paths are silently skipped**: If a configured directory doesn't exist, Hermes ignores it without errors. Useful for optional shared directories that may not be present on every machine.
### Example
## Redirecting Skill Creation (`skills.create_dir`)
- **`skill_manage` create writes there.** New skills (including category subdirectories) are created under `create_dir` instead of the local skills dir. The directory is created on first write if it doesn't exist.
- **The agent's instructions follow the config.** Every agent-facing instruction that names the skill-creation path — the `skill_manage` tool description and related prompt text — dynamically renders the configured directory, so the agent is told to create skills there. No system-prompt overrides or filesystem tricks needed.
- **The directory is fully integrated.** Skills under `create_dir` are scanned alongside the local dir: they appear in the skill index, `skills_list`, `skill_view`, slash commands, and can be patched or deleted like any local skill.
- **Everything else stays local.** Existing skills are still modified in place wherever they live; bundled skill sync, the hub, and the curator keep operating on the profile-local dir.
## Project-Local Skills
### Trusting a project
### Precedence
### Duplicate skill names
- **Across directories**, the higher-precedence directory wins (`project → local → skills.create_dir → external_dirs`), including for the same relative path (`productivity/xdup` local and external loads the local copy). The shadowed copy is hidden and a warning is logged.
- **Inside one directory**, two *different* skills that share a name are never guessed between. Both are listed under their exact relative path (for example `a/one` and `b/two`), and loading the bare name fails with `Ambiguous skill name dup-demo: use one of a/one, b/two`. That message is also what `hermes -s dup-demo` and cron jobs report. Identic
### Scan-time quarantine
### Non-interactive surfaces (cron, API, ACP)
## Skill Bundles
### Quick example
### YAML schema
- `name` (optional — defaults to the filename stem) — the bundle's display name. Normalized to a hyphen slug for the slash command (`Backend Dev` → `/backend-dev`).
- `description` (optional) — short text shown in `/bundles` and `hermes bundles list`.
- `skills` (required, non-empty list) — skill names or paths relative to your skills directory. Use the same identifier you'd pass to `/<skill-name>`.
- `instruction` (optional) — extra guidance prepended to the loaded skill content. Useful for codifying "how we always use these together."
### Managing bundles
### Behavior
- **Bundles take precedence over individual skills** when slugs collide. If you name a bundle `research` and you also have a skill called `research`, `/research` invokes the bundle. This is intentional — you opted into the bundle by naming it.
- **Missing skills are skipped, not fatal.** If a bundle lists `skill-foo` and you haven't installed it, the bundle still loads the skills that do resolve, and the agent gets a note listing what was skipped.
- **Bundles work in every surface** — interactive CLI, TUI, dashboard chat, and every gateway platform (Telegram, Discord, Slack, …) — because dispatch is centralized in the same place as individual skill commands.
- **Bundles do not invalidate the prompt cache.** They generate a fresh user message at invocation time, the same way `/<skill-name>` does — no system prompt mutation.
### When bundles beat installing each skill manually
- You always pair the same skills for a recurring task (`/backend-dev`, `/release-prep`, `/incident-response`).
- You want a one-character-shorter mental model than typing several `/skill` invocations in a row.
- You want to ship a team-wide "task profile" by checking the bundle YAML into a shared dotfiles repo and symlinking it into `~/.hermes/skill-bundles/`.
## Agent-Managed Skills (skill_manage tool)
### When the Agent Creates Skills
- When it worked out a multi-step workflow worth repeating
- When it hit errors or dead ends and found the working path
- When the user corrected its approach
### What a skill entry looks like
### Actions
| Action | Use for | Key params |
|--------|---------|------------|
| `create` | New skill from scratch | `name`, `content` (full SKILL.md), optional `category` |
| `patch` | Targeted fixes (preferred) | `name`, `old_string`, `new_string` |
| `patch` with `content` | Major structural rewrites (replaces the whole SKILL.md; `edit` is the legacy alias) | `name`, `content` |
| `delete` | Remove a skill entirely | `name` |
| `write_file` | Add/update supporting files | `name`, `file_path`, `file_content` |
| `remove_file` | Remove a supporting file | `name`, `file_path` |
### Gating agent skill writes (`skills.write_approval`)
## Skills Hub
### Common commands
### Supported hub sources
| Source | Example | Notes |
|--------|---------|-------|
| `official` | `official/security/1password` | Optional skills shipped with Hermes. |
| `skills-sh` | `skills-sh/vercel-labs/agent-skills/vercel-react-best-practices` | Searchable via `hermes skills search <query> --source skills-sh`. Hermes resolves alias-style skills when the skills.sh slug differs from the repo folder. |
| `well-known` | `well-known:https://mintlify.com/docs/.well-known/skills/mintlify` | Skills served directly from `/.well-known/skills/index.json` on a website. Search using the site or docs URL. |
| `url` | `https://sharethis.chat/SKILL.md` | Direct HTTP(S) URL to `SKILL.md` plus explicitly referenced support files. Name resolution: frontmatter → URL slug → interactive prompt → `--name` flag. |
| `github` | `openai/skills/k8s` | Direct GitHub repo/path installs and custom taps. |
| `clawhub`, `lobehub`, `browse-sh` | Source-specific identifiers | Community or marketplace integrations. |
### Integrated hubs and registries
#### 1. Official optional skills (`official`)
- Catalog: [Official Optional Skills Catalog](../../reference/optional-skills-catalog)
- Source in repo: `optional-skills/`
- Example:
#### 2. skills.sh (`skills-sh`)
- Directory: [skills.sh](https://skills.sh/)
- CLI/tooling repo: [vercel-labs/skills](https://github.com/vercel-labs/skills)
- Official Vercel skills repo: [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills)
- Example:
#### 3. Well-known skill endpoints (`well-known`)
- Example live endpoint: [Mintlify docs skills index](https://mintlify.com/docs/.well-known/skills/index.json)
- Reference server implementation: [vercel-labs/skills-handler](https://github.com/vercel-labs/skills-handler)
- Example:
#### 4. Direct GitHub skills (`github`)
- [openai/skills](https://github.com/openai/skills)
- [anthropics/skills](https://github.com/anthropics/skills)
- [huggingface/skills](https://github.com/huggingface/skills)
- [NVIDIA/skills](https://github.com/NVIDIA/skills) — NVIDIA-verified skills (signed `skill.oms.sig` + governance `skill-card.md`)
- [wandb/skills](https://github.com/wandb/skills), [qdrant/skills](https://github.com/qdrant/skills), [pinecone-io/skills](https://github.com/pinecone-io/skills), [makenotion/skills](https://github.com/makenotion/skills), [Airtable/skills](https://github.com/Airtable/skills), [Shopify/Shopify-AI-Toolkit](https://github.com/Shopify/Shopify-AI-Toolki
- [garrytan/gstack](https://github.com/garrytan/gstack)
- [K-Dense-AI/scientific-agent-skills](https://github.com/K-Dense-AI/scientific-agent-skills) and [synthetic-sciences/openscience](https://github.com/synthetic-sciences/openscience) — ~480 scientific research skills (bioinformatics, chemistry, physics, ML training, scholarly tooling), grouped under one `science` category. Community trust: every ins
- Example:
#### 5. ClawHub (`clawhub`)
- Site: [clawhub.ai](https://clawhub.ai/)
- Hermes source id: `clawhub`
#### 6. LobeHub (`lobehub`)
- Site: [LobeHub](https://lobehub.com/)
- Public agents index: [chat-agents.lobehub.com](https://chat-agents.lobehub.com/)
- Backing repo: [lobehub/lobe-chat-agents](https://github.com/lobehub/lobe-chat-agents)
- Hermes source id: `lobehub`
#### 7. browse.sh (`browse-sh`)
- Site: [browse.sh](https://browse.sh/)
- Catalog API: `https://browse.sh/api/skills`
- Hermes source id: `browse-sh`
- Trust level: `community`
#### 8. Direct URL (`url`)
- Hermes source id: `url`
- Identifier: the URL itself (no prefix needed)
- Scope: `SKILL.md` plus exact referenced support files in the allowlisted directories. Hermes does not enumerate or copy unrelated files from the host.
### Security scanning and `--force`
- repo URL
- skills.sh detail page URL
- install command
- weekly installs
- upstream security audit statuses
- well-known index/endpoint URLs
- `--force` can override policy blocks for caution/warn-style findings.
- `--force` does **not** override a `dangerous` scan verdict.
- Official optional skills (`official/...`) are treated as built-in trust and do not show the third-party warning panel.
### Trust levels
| Level | Source | Policy |
|-------|--------|--------|
| `builtin` | Ships with Hermes | Always trusted |
| `official` | `optional-skills/` in the repo | Built-in trust, no third-party warning |
| `trusted` | Trusted registries/repos such as `openai/skills`, `anthropics/skills`, `huggingface/skills`, `NVIDIA/skills`, `wandb/skills`, `qdrant/skills`, `pinecone-io/skills`, `makenotion/skills`, `Airtable/skills`, `Shopify/Shopify-AI-Toolkit` | More permissive policy than community sources |
| `community` | Everything else (`skills.sh`, well-known endpoints, custom GitHub repos, most marketplaces) | Non-dangerous findings can be overridden with `--force`; `dangerous` verdicts stay blocked |
### Update lifecycle
### Publishing a custom skill tap
#### Repo layout
- Each skill lives in its own directory under the tap's root path (default `skills/`).
- The directory name becomes the skill's install slug.
- Each skill directory must contain a `SKILL.md` with standard [SKILL.md frontmatter](#skillmd-format) (`name`, `description`, plus optional `metadata.hermes.tags`, `version`, `author`, `platforms`, `metadata.hermes.config`).
- Subdirectories like `references/`, `templates/`, `scripts/`, `assets/` are downloaded alongside `SKILL.md` at install time.
- Skills whose directory name starts with `.` or `_` are ignored.
#### Minimal tap example
#### Non-default paths
#### Installing individual skills directly (without adding a tap)
#### Trust levels for taps
#### Tap management
## Bundled skill updates (`hermes skills reset`)
- **Unchanged** → safe to pull upstream changes, copy the new bundled version in, record the new origin hash.
- **Changed** → treated as **user-modified** and skipped forever, so your edits never get stomped.
### Slash commands (inside chat)