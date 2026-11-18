---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Enforce isolated concurrent agent changes
phase: implement
status: backlog
---

# Enforce isolated concurrent agent changes

## Goal

Prevent unsafe overlapping file changes for the selected workflow using isolated workspaces or a verified enforced conflict boundary.

## Scope

- Follow child spec and current `spec/files-realtime-collaboration/spec.md`. Prefer disjoint worktrees if sufficient; if shared write coordination is required, define fencing/CAS and out-of-band writer behavior. Advisory claims alone are not write locks.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Selected isolation/conflict mechanism and process-level concurrent edit tests.

## Dependencies

- Child spec; owner-selected deployment profile.

## Acceptance criteria

[ ] Two agents editing overlapping files cannot silently overwrite each other.

- [ ] Out-of-band edit/stale base is detected and original changes preserved.

- [ ] Coordination failure has specified fail-safe behavior.
- [ ] Worktree/branch/index side effects are isolated or serialized.

- [ ] If concurrency is outside first workflow, defer with rationale.
