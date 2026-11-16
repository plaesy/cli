# CLI and configuration

**Status:** proposed command contract. Existing `plaesy` commands remain authoritative for current releases; no command below is implemented merely because it is specified here. Before implementation, reconcile names/options with `plaesy --help`, preserve existing workflows, and publish a migration/deprecation policy.

## UX principles

- One executable, predictable noun/verb grammar, actionable help, shell completion, stable exit codes and machine-readable JSON.
- Local operation is the safe default. A remote target is explicitly named or selected and echoed before mutations.
- Destructive commands do not infer confirmation from interactive defaults; CI can use explicit `--yes` plus narrowly scoped safeguards.
- `--dry-run` previews supported changes; `--json` output is versioned and excludes secrets.
- Use `--` to delimit passthrough arguments. Avoid hidden network calls and implicit remote deployment.

## Command domains (target namespace)

```text
plaesy [global options] <command>
  status | doctor | version | completion
  init | config | profile
  desktop [install|open|status|update|uninstall]
  web [open|install|run|status|stop|connect|logs|uninstall|deploy]
  web deploy [plan|apply|status|update|rollback|remove]
  instance [list|create|show|select|connect|disconnect|remove|doctor]
  module [list|enable|disable|configure|status|upgrade]
  auth [login|logout|whoami|token|provider]
  backup [create|list|verify|restore|export]
  plugin [list|install|inspect|enable|disable|remove]
  completion <shell>
```

This is a complete conceptual command map, not an implementation commitment for each subcommand. Exact grammar, verbs, and hierarchy require review against present commands; avoid overloading `install` for both app installation and remote provisioning without clear help.

## Command semantics

- `plaesy web` selects the configured local instance by default, starts it if needed, and opens its Web URL; `--no-open` prints only. If there is no local install, explain `plaesy web install` / `plaesy web run` as appropriate.
- `plaesy web run` is foreground, interruptible, and suitable for development; it must not silently create a persistent service.
- `plaesy web install` provisions a local managed runtime; it clearly separates app binary, service/container artifacts and user data.
- `plaesy web deploy plan` previews and `plaesy web deploy apply` provisions a remote Web instance only after target, host, domain/TLS, storage, credentials, firewall and data-impact preflight is shown. First deployment and updates are distinguishable. `plaesy web` without the `deploy` verb opens the selected Web interface; it does not deploy.
- `plaesy web connect` stores an instance reference, not a plaintext reusable credential. `instance select` changes the target used by future commands; every remote mutation prints the selected instance.
- `web uninstall` differentiates removing client/runtime from removing remote instance and deleting its data. A separate explicit purge action is needed for irreversible data erasure.
- `backup restore` requires target instance, backup identity, validation result and overwrite/merge semantics; default must not overwrite current data.

## Global options

Specify consistent options: `--version`, `--help`, `--config <path>`, `--profile <name>`, `--target <id>`, `--output human|json`, `--no-color`, `--quiet`, `--verbose`, `--timeout`, `--dry-run`, `--yes`. Not every flag applies to every command. Define precedence: explicit flag > environment variable > selected profile/config > documented default. Never put credentials in argv, shell history or process listings.

## Profiles and configuration

- Separate profile (user preferences and target selection), instance configuration (service operation), and secret material (OS/server secret store).
- Discover config using platform conventions; provide `plaesy config path`, `show` with redaction, `validate`, `get` and controlled `set`. Do not make secret values readable through `show`.
- Support environment overrides for automation, document names and precedence, and avoid silent config rewriting.
- Include schema version, migration behavior, permissions, comments preservation policy, backup-before-migration and rollback behavior.
- Remote target configuration includes stable ID, display name, endpoint, pinned identity/fingerprint where applicable, compatible version/capabilities and credential reference; never embed tokens in exported profile by default.

## Exit codes and errors

Define stable categories: success; usage/invalid config; unsupported platform; unavailable dependency; authentication/authorization; network/TLS/identity failure; incompatible version; operation timeout; conflict; disk/resource limit; data-integrity/migration; user cancellation; internal failure. Include a concise action, safe diagnostic ID, retryability and relevant target. JSON errors have a versioned envelope. Logs go to stderr; stdout remains parseable when JSON output is requested.

## Automation and idempotency

Commands document whether repeated execution is idempotent, convergent or rejected. Provision/update operations accept idempotency keys or use stable deployment IDs. Long operations report operation ID, phases and final state; provide status/resume/cancel where safe. Noninteractive execution never waits for hidden prompts. CI credentials are scoped, short lived and loaded from protected environment/secret mechanisms, never command-line values.

## Compatibility

Keep `plaesy --help` as the source of truth. Add commands without changing current memory subcommands unexpectedly. Deprecations include warnings, replacement, a sunset period, and migration support. Use API capability negotiation before invoking remote operations. Unsupported target/CLI combinations fail before mutation. Shell completion and command reference are generated from the same command definitions when possible.
