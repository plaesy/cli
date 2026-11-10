# Operations: running the second brain day to day

Examples use `<vault>` for the folder that holds your notes. Replace it with your own path. Commands are shown for Windows PowerShell; memory operations use `plaesy memory`.

## 1. What is where

| Item | Location | Set by |
|---|---|---|
| Vault (your notes, source of truth) | `<vault>`, any folder you choose | `MEMORY_WATCH_DIR` |
| Index database | `%LOCALAPPDATA%\plaesy\memory\<vault-id>.db` (+ `.hnsw` snapshot) | `plaesy memory` selects a per-vault file based on vault path; `MEMORY_DB_PATH` overrides |
| Embedding model | `%LOCALAPPDATA%\plaesy\models\<name>\` | default; `MEMORY_MODEL_DIR` overrides |
| Datasets (evaluation only) | `%LOCALAPPDATA%\plaesy\datasets\` | convention |
| Exclusions | folders or file names that are never indexed | `MEMORY_EXCLUDE_DIRS` |

The index is a derived local store: deleting the selected database and re-indexing rebuilds it from the notes. The database lives outside the vault, so it is not synced between devices; each device builds its own local index. The notes remain the canonical source.

## 2. Writing notes so they can be trusted

The engine reads optional frontmatter. Notes without it still work, but cannot declare themselves.

```yaml
---
tier: semantic            # core | episodic | semantic | procedural | archive (inferred from folder if omitted)
confidence: high         # high | medium | low: how sure you are of the claims
last_verified: 2026-10-26
status: active           # superseded | deprecated | archived are hidden unless asked for
valid_from: 2026-01-01   # optional: the period the facts hold
valid_to:                # optional
---
```

Rules that match the engine's behaviour:

- Keep credentials out of notes. The index stores the text, and `memory_search` can return it to a model. `plaesy memory hygiene` reports likely API keys, passwords, bearer/access tokens, client secrets, private-key headers, and recognizable AWS, GitHub, Slack, Stripe, Google, and OpenAI token formats by file and line; it never includes matched values. It ignores fenced code examples and remains a heuristic, not a complete secret scanner. See the secret-scan item in [assessment](assessment.md).

To inspect only likely credential findings in JSON:

```powershell
plaesy memory --vault "<vault>" hygiene "<vault>" --json | ConvertFrom-Json | Select-Object -ExpandProperty findings | Where-Object kind -eq possible_secret
```

The scanner is read-only and heuristic. Remove or rotate a credential outside the memory tool; do not paste the value into bug reports or agent prompts.

- A note older than 180 days (`last_verified`) is flagged STALE and its score is multiplied by 0.85. It is down-weighted, not hidden.
- Use `[[wikilinks]]` for relations. They power `plaesy memory links` and dangling-link checks.
- Folder names are tiers by default: put episodic logs in an `episodic` folder, not in `semantic`.

## 3. Index and search

```powershell
plaesy memory --vault "<vault>" index "<vault>"                 # first run, or after large restructures
plaesy memory --vault "<vault>" search "how do we deploy"       # hybrid by default
plaesy memory --vault "<vault>" search "deploy" --mode keyword # exact terms
plaesy memory --vault "<vault>" search "deploy" --tier procedural --min-trust high
plaesy memory --vault "<vault>" links runbook                   # who links to it
plaesy memory --vault "<vault>" stats                           # documents, chunks, vector search on/off
```

Select the vault before the memory command. A positional directory passed to `index` or `hygiene`
sets the files to scan; it does not select which vault's local database to use. `index` picks up
changes on the next run. The watch mode keeps the index current while the machine is on.

## 4. Changing the embedding model

The index records the model it was built with. Changing `MEMORY_MODEL_NAME` (for example to `multilingual-e5-small`) re-embeds every document on the next sync. Expect about an hour for a large vault with e5. Keyword search keeps working during the rebuild; semantic results are incomplete until it finishes.

Do not mix models in one database. To compare, use a separate `MEMORY_DB_PATH` per model.

## 5. Connecting an AI

**Claude Code (MCP):**

```powershell
claude mcp add plaesy-memory -e MEMORY_WATCH_DIR=<vault> -- plaesy memory serve --mcp
```

The server exposes eight tools: six read-only (`memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`, `memory_curate`, `memory_stats`) and two bounded write tools (`memory_remember`, `memory_resolve_gap`). `memory_curate` is a bounded review report; it does not edit notes. The write tools create a new note or close a query gap; neither replaces or deletes a note. Details: [MCP](mcp.md).

**Hermes:** connect through its MCP client support using the generated configuration from `plaesy memory --vault "<vault>" config mcp --app hermes`. The same `plaesy memory serve` command is used; there is no separate `memory` executable or plugin binary.

## 6. Routine maintenance

| When | Command | What to look at |
|---|---|---|
| Weekly | `plaesy memory --vault "<vault>" curate --limit 50` | confirmed and inferred hygiene findings, unanswered searches, exact duplicates, unlinked notes, topic folders that may need an index |
| Before CI or a strict gate | `plaesy memory --vault "<vault>" hygiene "<vault>" --strict` | fail on any hygiene finding, including inferred findings |
| Monthly | review `confidence: low` and STALE notes | update, demote to `archive`, or delete |
| After restructuring the vault | `plaesy memory --vault "<vault>" index "<vault>"` | the index holds the old paths otherwise |
| After a model change | `plaesy memory --vault "<vault>" stats` shows the new model | see section 4 |

`hygiene --strict` exits with a non-zero code when it finds problems, so it can gate a pre-commit hook or a scheduled task.

### Scheduled jobs

`plaesy memory --vault "<vault>" schedule once` runs the selected vault's due jobs and exits.
`plaesy memory --vault "<vault>" schedule run` keeps running until Ctrl-C.

- Jobs are set in the config under `[[schedule.jobs]]`: `name`, `every` (`30m`, `6h`, `1d`), `action` (`index`, `hygiene`, `missing` or `curate`). With no jobs configured, `index` runs every hour.
- `hygiene`, `missing` and `curate` do not run unless listed. A vault with known findings would otherwise add a line to the log every hour.
- Each run appends one line per job to `<root>\Plaesy\schedule.log`. A failing job is logged and the rest still run.
- Last-run times are kept in the database (`schedule_runs`). A second `once` within the interval does nothing.
Install or remove a user-level OS task with the CLI. The command saves a private snapshot of the effective memory configuration, including the selected vault and database, so scheduled runs use the same settings even when the original terminal environment is gone:

```sh
plaesy memory --vault "<vault>" schedule install
plaesy memory schedule uninstall
```

Run `plaesy memory --vault "<vault>" schedule once` manually before installation to check the jobs and inspect `<vault>/Plaesy/schedule.log`. The OS task runs every 30 minutes; internal job intervals decide what is due. No administrator/root privileges are used. On headless Linux servers, keep a systemd user manager alive after logout with `sudo loginctl enable-linger "$USER"` so the timer runs without an interactive session. Windows Task Scheduler and macOS LaunchAgents run in the user's logged-in session.

Manual OS registration is also available if automatic setup fails. These examples poll every 30 minutes, the shortest supported job interval.

**Windows PowerShell (current user, while signed in):**

```powershell
$exe = (Get-Command plaesy).Source
$taskRun = "`"$exe`" memory schedule once"
schtasks.exe /Create /F /TN "Plaesy Memory Schedule" /TR $taskRun /SC MINUTE /MO 30 /IT
schtasks.exe /Run /TN "Plaesy Memory Schedule"
schtasks.exe /Query /TN "Plaesy Memory Schedule" /V /FO LIST
```

`/IT` runs in the interactive user session and avoids storing a password. Remove it with `schtasks.exe /Delete /F /TN "Plaesy Memory Schedule"`. See [Microsoft schtasks](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-create).

**Linux systemd user units:** create `~/.config/systemd/user/plaesy-memory.service`:

```ini
[Unit]
Description=Run due Plaesy memory jobs
[Service]
Type=oneshot
ExecStart=%h/.local/bin/plaesy memory schedule once
```

Create `~/.config/systemd/user/plaesy-memory.timer`:

```ini
[Unit]
Description=Schedule Plaesy memory jobs
[Timer]
OnCalendar=*-*-* *:0/30:00
Persistent=true
Unit=plaesy-memory.service
[Install]
WantedBy=timers.target
```

Enable with `systemctl --user daemon-reload && systemctl --user enable --now plaesy-memory.timer`; inspect with `systemctl --user list-timers plaesy-memory.timer` and `journalctl --user -u plaesy-memory.service`. On a headless VPS, enable lingering so the user manager starts without login: `sudo loginctl enable-linger "$USER"`. See [systemd.timer](https://www.freedesktop.org/software/systemd/man/latest/systemd.timer.html).

**macOS LaunchAgent:** save as `~/Library/LaunchAgents/com.plaesy.memory-schedule.plist`; replace paths with absolute paths for the user:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.plaesy.memory-schedule</string>
  <key>ProgramArguments</key><array>
    <string>/Users/your-name/.local/bin/plaesy</string>
    <string>memory</string><string>schedule</string><string>once</string>
  </array>
  <key>StartInterval</key><integer>1800</integer>
  <key>StandardOutPath</key><string>/Users/your-name/Library/Logs/plaesy-memory-schedule.log</string>
  <key>StandardErrorPath</key><string>/Users/your-name/Library/Logs/plaesy-memory-schedule.log</string>
</dict></plist>
```

Create the log directory, then load in the signed-in user's GUI domain with `launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.plaesy.memory-schedule.plist"`; inspect with `launchctl print "gui/$(id -u)/com.plaesy.memory-schedule"`, unload with `launchctl bootout "gui/$(id -u)/com.plaesy.memory-schedule"`. See [Apple LaunchAgents](https://developer.apple.com/documentation/servicemanagement/) and [launchd timed jobs](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html).

Run `plaesy memory schedule once` manually before enabling the task and inspect its log. Scheduled execution inherits the selected user's vault configuration and file access.

## 7. Updating a Linux server from CI

Deploy the Linux artifact from a successful CI run instead of rebuilding on the server. This keeps
the server binary tied to a tested commit and avoids compiling alongside memory-intensive services.
The CI artifact is retained for a limited time; use a published release for long-term downloads.

On a machine with GitHub CLI, download and verify the artifact, then send its extracted binary to
the server's persistent Plaesy state directory. Replace the placeholders with the full commit SHA,
run ID, and SSH destination:

```sh
commit=<full-commit-sha>
run_id=<successful-ci-run-id>
server=<user>@<server>
stage="${XDG_STATE_HOME:-$HOME/.local/state}/plaesy/deploy/$commit"
mkdir -p "$stage"
gh run download "$run_id" --repo plaesy/cli \
  --name "plaesy-linux-x86_64-$commit" --dir "$stage"
(cd "$stage" && sha256sum -c SHA256SUMS)
tar -xzf "$stage/plaesy-linux-x86_64.tar.gz" -C "$stage"
remote_state="$(ssh "$server" 'printf %s "${XDG_STATE_HOME:-$HOME/.local/state}/plaesy"')"
remote_stage="$remote_state/deploy/$commit"
ssh "$server" "mkdir -p '$remote_stage'"
scp "$stage/plaesy" "$server:$remote_stage/plaesy"
sha256sum "$stage/plaesy"
ssh "$server" "sha256sum '$remote_stage/plaesy'"
```

Confirm the two binary hashes match. Then run this on the server, replacing the commit placeholder:

```sh
commit=<full-commit-sha>
state="${XDG_STATE_HOME:-$HOME/.local/state}/plaesy"
binary="$(bash -lc 'command -v plaesy')"
candidate="$state/deploy/$commit/plaesy"
old_sha="$(sha256sum "$binary" | cut -d' ' -f1)"
backup="$state/backups/plaesy-${old_sha:0:16}"
mkdir -p "$state/backups"
if [ ! -e "$backup" ]; then
  cp -p "$binary" "$backup"
else
  test "$(sha256sum "$backup" | cut -d' ' -f1)" = "$old_sha"
fi
install -m 0755 "$candidate" "$binary.new"
cmp -s "$candidate" "$binary.new"
mv -f "$binary.new" "$binary"
sync -f "$binary"
bash -lc 'command -v plaesy && plaesy --version'
plaesy memory archive --help >/dev/null
printf 'backup=%s\n' "$backup"
```

The candidate and replacement are placed on the same filesystem so the final rename is atomic.
Keep the checksum-named backup until the new binary has been verified. To roll back, install that
backup beside the active binary and rename it over the active path using the same steps. If the
server also has a source checkout, fast-forward it separately with `git pull --ff-only`; inspect and
preserve untracked datasets and benchmark output, and do not use `git clean`.

## 8. Backup and recovery

- The notes are the backup. Keep the vault in git or in a sync folder with history.
- The index is rebuildable. Back it up only if re-indexing a large vault is slow for you.
- Models are a cache. `scripts/install-model.sh` downloads them again.

## 9. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `stats` shows `documents: 0` | database is new or points to the wrong file | check `MEMORY_DB_PATH`, run `plaesy memory --vault "<vault>" index "<vault>"` |
| `vector search: off` | model not found in the models folder | run `scripts/install-model.sh`, or set `PLAESY_MODEL_DIR` |
| Results cite old paths | notes moved after indexing | `plaesy memory --vault "<vault>" index "<vault>"` again |
| Hermes uses an unexpected database | process was started before the path change | restart the MCP client and check `plaesy memory --vault "<vault>" stats` |
| Search is slow on first run | model load and first embedding pass | normal; later runs use the embedding cache |

## 10. Checklist for a new machine

1. Install: use the Windows PowerShell or macOS/Linux release installer in the README.
2. Models: run `bash scripts/install-model.sh` on macOS/Linux (add `--rerank` for optional reranking).
3. Index: `plaesy memory --vault "<vault>" index "<vault>"`.
4. Connect: configure an MCP client as in section 5.
5. Check: `plaesy memory --vault "<vault>" stats` shows `vector search: on` and the expected document count.
