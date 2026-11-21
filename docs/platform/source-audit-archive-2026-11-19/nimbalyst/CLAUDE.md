# CLAUDE.md

Guidance for agents working in this repository. Each rule is one line; the linked file has the detail. Most hard rules are also enforced by the git hooks and by `pnpm test:prepush-gate`, so a rule you did not read still fails the push. Past incidents behind these rules live in the linked rule files, not here.

## Hard rules

### Git and commits

- Never commit, push, release, restart the app, or run `pnpm run dev` unless asked. A commit request is the only time you edit `CHANGELOG.md`; see [/commit](./.claude/commands/commit.md).
- One-sentence commit subject. One CHANGELOG bullet per feature, user-facing, no internal names. `Fixes #123` and `Fixes NIM-123` trailers close issues and tracker items.
- The only push is `git push origin main`. Land contributions with [/land-contribution](./.claude/commands/land-contribution.md). Never push to a contributor branch, approve or merge a PR on GitHub, force-push, or use `--no-verify`. The pre-push hook refuses other destinations, test-fixture authors, oversized commits, and local-only files.
- No `git stash`, `git reset`, `git add -A`, or amend without asking. No `Co-Authored-By` lines.
- Never commit `nimbalyst-local/` or `agent-mistakes.md`. The pre-commit hook refuses them.

### Tests and the gate

- Every runtime behavior change ships with a unit test. For sync, main-process init, IPC, and restart-to-verify bugs the failing test comes first; see [end-to-end-verification](./.claude/rules/end-to-end-verification.md).
- The gate is `pnpm typecheck && pnpm test:prepush`. The orchestrating session runs it once per batch; slices never run it. The pre-push hook runs it too.
- Never run the suite twice to see what failed, and never pipe it through `tail`. Read `pnpm run test:last`.
- A failing test on main is yours to fix. "Pre-existing" is not a reason to push.
- Tests catch what a reader cannot see: no presentation-only tests, no asserting on component source text, no injecting CSS and asserting on it, no `expect(getBy*()).toBeTruthy()`.
- Mock the narrowest module and import deep paths. Never mock or import the `@nimbalyst/runtime` barrel in a test. When you move an import, grep the tests for the old specifier in the same commit.
- Put `// @vitest-environment node` first in any test that never touches the DOM. Extend an existing test file before creating one.
- E2E: one spec per `pnpm exec playwright test` invocation, never in parallel. See [E2E_TESTING.md](./docs/E2E_TESTING.md).

### Data and safety

- Both PGLite and better-sqlite3 are live. `data->'key'` is an object on PGLite and a string on SQLite; parse defensively. Read [DATABASE.md](./packages/electron/DATABASE.md) before touching schema, queries, or JSON columns.
- Never open the database files from another process. Query through `mcp__nimbalyst-extension-dev__database_query`.
- Local database state is not server state. Shared trackers and documents live in Durable Objects in the `nimbalyst-collab` repository. Verify the server side with `wrangler tail` or the wrangler-backed collab E2E specs before closing a sync bug.
- Any path that renames, moves, truncates, overwrites, or deletes user data retries first, verifies real damage, emits its event before acting, and leaves a recoverable artifact. See [destructive-data-paths](./.claude/rules/destructive-data-paths.md).
- API keys come only from Nimbalyst settings. Never read them from `process.env`.
- The personal JWT is for personal and mobile sync only. The team JWT authorizes all team collaboration. Use the branded types in `packages/runtime/src/auth/jwtScopes.ts`. See [IDENTITY_AUTH_AND_ROOMS.md](./docs/IDENTITY_AUTH_AND_ROOMS.md).
- Never `app.exit()`. Never `localStorage` in the renderer. Use `safeHandle` and `safeOn` in main, never raw `ipcMain`.

### Verification and observation

- Do not say "fixed" or "verified" until you observed it: a test that went red to green, a screenshot, or a log line showing the failing step succeed. End the task with that evidence, or say plainly that it is unverified.
- Run your own observation commands. Logs: `get_main_process_logs` and `get_renderer_debug_logs`. HTTP, `wrangler tail`, and `gh` run from Bash. Live renderer state: `renderer_eval`. Never ask the user to run a command or paste output. See [DEBUGGING_LOGS.md](./docs/DEBUGGING_LOGS.md).
- A `try { } catch { console.error }` around your change can swallow it. Grep the log for the catch message after exercising the path.
- Check dev mode with `get_environment_info` before changing code. Ask before `restart_nimbalyst`.

### Parallel sessions

- Slices own disjoint file sets, including `CHANGELOG.md`, `package.json`, barrels, registries, and `docs/FEATURE_INVENTORY.md`. If two slices need one file, they are one slice. No whole-file `Write` on a file you did not create. See [parallel-sessions](./.claude/rules/parallel-sessions.md).

### Code conventions

- Static imports in the Electron main process. The build's bundle policy rejects lazy chunks. Read [MAIN_PROCESS_INIT.md](./packages/electron/MAIN_PROCESS_INIT.md) before writing `await import()` there.
- `@floating-ui/react` with `FloatingPortal` for every popover, tooltip, and menu. Never hand-compute `position: fixed`.
- Components never subscribe to IPC. Central listeners in `store/listeners/` update Jotai atoms; components read atoms. See [IPC_LISTENERS.md](./docs/IPC_LISTENERS.md).
- Editors own their content state; the tab host does not. Talk to the host through `EditorHost`. See [EDITOR_STATE.md](./docs/EDITOR_STATE.md).
- `camelCase` on the wire, `snake_case` only in SQL columns. `TIMESTAMPTZ` for every timestamp column.
- Fail fast on missing parameters. Workspace-scoped IPC takes `workspacePath` explicitly. See [ERROR_HANDLING.md](./docs/ERROR_HANDLING.md).
- Every meaningful component root gets a stable kebab-case class. Use `@container` queries, `--nim-*` variables, and `select-text` on content. See [UI_PATTERNS.md](./docs/UI_PATTERNS.md).
- `NIM-###` keys are tracker-scoped. Code comments and log strings cite GitHub `#123`; commit trailers may use both. Strip a contributor's `NIM-###` from a PR before merging.
- Alpha is a label (badge plus "(alpha)" in the CHANGELOG), never a release-channel gate.
- No emojis. No enthusiastic filler. No time or effort estimates. No hard line wraps in markdown.

### Product records

- When a notable capability is added, expanded, or removed, update [FEATURE_INVENTORY.md](./docs/FEATURE_INVENTORY.md) during implementation and report "Feature inventory updated" or "No inventory impact" before handoff. In a parallel batch the integrator owns it. Polish and fixes need no entry.
- Record a decision where it is read. In a project with a Wiki, follow its "How we write this wiki" page. A bug fix needs a bug tracker item before the fix. See [TRACKER_WORKFLOWS.md](./docs/TRACKER_WORKFLOWS.md).
- Finished work is `in-review` and the session is `validating` until the user commits. A commit is their approval; put `Fixes NIM-123` in the message and the item closes itself.
- Shortcut changes update `KeyboardShortcutsDialog.tsx`. PostHog event changes update [POSTHOG_EVENTS.md](./docs/POSTHOG_EVENTS.md).
- Default to no architecture diagram. See [ARCHITECTURE_DIAGRAMS.md](./docs/ARCHITECTURE_DIAGRAMS.md) for the bar.

## The codebase

Nimbalyst is an AI-native workspace: an Electron desktop app over a shared TypeScript runtime, native iOS and Android clients, and a collaboration server on Cloudflare. All editors (Lexical markdown, Monaco, spreadsheets, diagrams, custom React) are provided through the extension system; see [EXTENSION_ARCHITECTURE.md](./docs/EXTENSION_ARCHITECTURE.md).

```
packages/
  electron/        # Desktop app: main process, preload, renderer
  runtime/         # Cross-platform services: AI providers, sync, Lexical editor
  core/            # Shared utilities
  collab-client/   # Collaboration client used by desktop and the web console
  collab-protocol/ # Wire types and golden fixtures shared with iOS and Android
  extension-sdk/   # Extension development kit
  extensions/      # Built-in extensions
  ios/ android/    # Native clients
```

The collaboration server (Workers, Durable Objects, D1) and the web console live in the sibling repository `nimbalyst-collab`; client and server changes go in both. Package docs: [electron](./packages/electron/CLAUDE.md), [runtime](./packages/runtime/CLAUDE.md), [ios](./packages/ios/CLAUDE.md), [android](./packages/android/CLAUDE.md).

Transcripts: `ai_agent_messages` is the only persisted source of truth; canonical events are derived in memory by `TranscriptRuntime`. See [TRANSCRIPT_ARCHITECTURE.md](./docs/TRANSCRIPT_ARCHITECTURE.md).

### Commands

- Install: `corepack enable` once, then `pnpm install`. All pnpm settings (overrides, patches, `allowBuilds`, the 72-hour `minimumReleaseAge`) live in `pnpm-workspace.yaml`. Add a dependency with `pnpm --filter <pkg> add <dep>`; a new dependency with a build script must be listed in `allowBuilds`.
- Dev: the user runs `cd packages/electron && pnpm run dev`. A second instance for collab testing is `pnpm run dev:user2`.
- Unit tests: `pnpm run test:unit`. Last failures: `pnpm run test:last`. Typecheck: `pnpm typecheck`.
- Release: `/release-alpha [patch|minor|major]`; see [RELEASING.md](./RELEASING.md). Marketing captures: [MARKETING_SCREENSHOTS.md](./docs/MARKETING_SCREENSHOTS.md).
- Main process log: `~/Library/Application Support/@nimbalyst/electron/logs/main.log`.

### Read before working in an area

| Area | Doc |
| --- | --- |
| Extensions, editors, editor-host contract | [EXTENSION_ARCHITECTURE.md](./docs/EXTENSION_ARCHITECTURE.md), [EDITOR_STATE.md](./docs/EDITOR_STATE.md) |
| IPC handlers, `electronAPI`, event listeners | [IPC_GUIDE.md](./docs/IPC_GUIDE.md), [IPC_LISTENERS.md](./docs/IPC_LISTENERS.md) |
| Jotai atoms, persisted state, app load errors | [JOTAI.md](./docs/JOTAI.md), [STATE_PERSISTENCE.md](./docs/STATE_PERSISTENCE.md) |
| Main-process bootstrap, singletons, handler registration | [MAIN_PROCESS_INIT.md](./packages/electron/MAIN_PROCESS_INIT.md) |
| Database tables, shutdown, timestamps, backend divergence | [DATABASE.md](./packages/electron/DATABASE.md) |
| Encryption, key custody, rooms, the two JWTs | [IDENTITY_AUTH_AND_ROOMS.md](./docs/IDENTITY_AUTH_AND_ROOMS.md) |
| Tracker schemas, sharing, numbering, decisions, bugs | [TRACKER_SCHEMA_SHARING.md](./docs/TRACKER_SCHEMA_SHARING.md), [TRACKER_WORKFLOWS.md](./docs/TRACKER_WORKFLOWS.md) |
| AI providers, model selection, context tracking | [AI_PROVIDER_TYPES.md](./docs/AI_PROVIDER_TYPES.md), [CONTEXT_WINDOW_USAGE_TRACKING.md](./docs/CONTEXT_WINDOW_USAGE_TRACKING.md) |
| Transcript rendering, MCP servers, tool widgets | [TRANSCRIPT_ARCHITECTURE.md](./docs/TRANSCRIPT_ARCHITECTURE.md), [INTERNAL_MCP_SERVERS.md](./docs/INTERNAL_MCP_SERVERS.md), [CUSTOM_TOOL_WIDGETS.md](./docs/CUSTOM_TOOL_WIDGETS.md) |
| Interactive prompts, permissions, voice | [INTERACTIVE_PROMPTS.md](./docs/INTERACTIVE_PROMPTS.md), [AGENT_PERMISSIONS.md](./docs/AGENT_PERMISSIONS.md), [VOICE_MODE.md](./docs/VOICE_MODE.md) |
| Sessions, worktrees, session hierarchy | [WORKTREES.md](./docs/WORKTREES.md), [SESSION_HIERARCHY.md](./docs/SESSION_HIERARCHY.md) |
| UI, dialogs, themes, DOM markers, render performance | [UI_PATTERNS.md](./docs/UI_PATTERNS.md), [DIALOGS.md](./docs/DIALOGS.md), [THEMING.md](./packages/electron/THEMING.md), [REACT_DOM_MARKERS.md](./docs/REACT_DOM_MARKERS.md), [RENDER_PERFORMANCE.md](./docs/RENDER_PERFORMANCE.md) |
| Help tooltips, walkthroughs, tips | [HELP_WALKTHROUGHS.md](./docs/HELP_WALKTHROUGHS.md), [WALKTHROUGHS.md](./docs/WALKTHROUGHS.md) |
| File watching, change tracking, diffs | [FILE_WATCHING_AND_CHANGE_TRACKING.md](./docs/FILE_WATCHING_AND_CHANGE_TRACKING.md) |
| E2E tests, debugging with logs | [E2E_TESTING.md](./docs/E2E_TESTING.md), [DEBUGGING_LOGS.md](./docs/DEBUGGING_LOGS.md) |
| Analytics events, weekly dashboard | [ANALYTICS_GUIDE.md](./docs/ANALYTICS_GUIDE.md), [POSTHOG_EVENTS.md](./docs/POSTHOG_EVENTS.md), [WEEKLY_DASHBOARD.md](./docs/WEEKLY_DASHBOARD.md) |
| Releases, marketing captures | [RELEASING.md](./RELEASING.md), [MARKETING_SCREENSHOTS.md](./docs/MARKETING_SCREENSHOTS.md), [ANDROID_MARKETING_SCREENSHOTS.md](./docs/ANDROID_MARKETING_SCREENSHOTS.md) |

Support docs for users live in `support/`.
