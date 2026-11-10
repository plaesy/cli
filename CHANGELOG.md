# Changelog

Format: [Keep a Changelog](https://keepachangelog.com). Versions follow SemVer once 0.1.0 is tagged; nothing is tagged yet.

## [Unreleased]

### Added

- Add a read-only `plaesy memory curate` review across hygiene, unanswered searches, exact duplicates,
  unlinked notes, and knowledge folders that may need an index; expose it through MCP and scheduled jobs.
- Consolidate the Plaesy CLI and memory engine into one `plaesy` binary, with memory, validation,
  task, install, and development reload commands.
- Add cross-platform install lifecycle coverage and CI/release workflows.

### Removed

- Remove the unused Spec-Kit commands and standalone Hermes Python plugin; Hermes can use the
  built-in JSON service or MCP integration.
