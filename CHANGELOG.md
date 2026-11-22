# Changelog

Format: [Keep a Changelog](https://keepachangelog.com). Versions follow SemVer once 0.1.0 is tagged; nothing is tagged yet.

## [Unreleased]

### Added

- Add a read-only `plaesy memory curate` review across hygiene, unanswered searches, exact duplicates,
  unlinked notes, and knowledge folders that may need an index; expose it through MCP and scheduled jobs.
- Consolidate the Plaesy CLI and memory engine into one `plaesy` binary, with memory, validation,
  task, install, and development reload commands.
- Add cross-platform install lifecycle coverage and CI/release workflows.
- Add a synthetic secret-scanner regression corpus that reports precision and recall without using real credentials.

### Fixed

- `plaesy tasks complete` now prints that a manual completion has no checklist verification when the task has no checklist items.

- Allow `memory curate` and scheduled curation to read the daemon-maintained index without attempting
  a conflicting vault sync.
- Detect additional provider-formatted credentials during memory hygiene without exposing matched values.

### Changed

- Move first-run setup to `plaesy onboard`; `plaesy memory onboard` now points to the new command.
- Replace the memory TOML config with `settings.json`: user settings at `~/.plaesy/settings.json` (the same on every OS)
  (written by `plaesy onboard`) and project settings at `.plaesy/settings.json`. Both use the same
  nested schema. `memory.toml` and `~/.hermes/memory.toml` are no longer read.
- The schedule snapshot is now `<config dir>/plaesy/schedule/settings.json`.

### Removed

- Remove the unused Spec-Kit commands and standalone Hermes Python plugin; Hermes can use the
  built-in JSON service or MCP integration.
