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

- Allow `memory curate` and scheduled curation to read the daemon-maintained index without attempting
  a conflicting vault sync.
- Detect additional provider-formatted credentials during memory hygiene without exposing matched values.

### Removed

- Remove the unused Spec-Kit commands and standalone Hermes Python plugin; Hermes can use the
  built-in JSON service or MCP integration.
