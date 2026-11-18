---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Implement authenticated agent identity and policy boundary
phase: implement
status: backlog
---

# Implement authenticated agent identity and policy boundary

## Goal

Enforce least-privilege agent/task/project/tool authorization for the selected workflow, including revocation.

## Scope

- Implement only after the identity hierarchy and threat boundary are approved. Authenticate agent principal; scope grants to project/task/runtime tools; protect secrets; audit allow/deny; implement revocation at execution boundary. Use synthetic principals and credentials in tests.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Implemented identity/policy path with focused allow/deny/revocation/security tests and threat notes.

## Dependencies

- Owner decisions; child workflow spec.

## Acceptance criteria

[ ] Unauthenticated and cross-project requests are denied.

- [ ] Allowed scoped action succeeds; out-of-scope tool/action is denied before side effect.

- [ ] Revoked principal cannot perform subsequent action.
- [ ] Synthetic secret does not appear in unauthorized logs/errors/exports.

- [ ] Focused security tests pass.
