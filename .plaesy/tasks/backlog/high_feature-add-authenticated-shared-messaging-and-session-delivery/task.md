---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Add authenticated shared messaging and session delivery
phase: implement
status: backlog
---

# Add authenticated shared messaging and session delivery

## Goal

If the selected workflow needs inter-agent communication, implement project-authorized durable delivery with truthful acknowledgements across isolated processes/hosts.

## Scope

- Only implement if child spec requires multiple agents or hosts. Reuse existing Relay concepts; define authenticated transport, principal/project ACL, idempotency, queue/lease/ack/retry and receiver adapter semantics. Do not label polling as push/realtime or claim idle-session wake without receiver evidence.

## Non-goals

- Do not silently broaden beyond the selected first workflow.

- Do not change upstream source code.
- Do not claim completion without linked evidence and focused test results.

## Deliverables

- Authenticated service/client contract, delivery implementation and multi-process/host acceptance evidence, or documented deferral if not required.

## Dependencies

- Child spec; approved identity model and runtime adapter.

## Acceptance criteria

[ ] Unauthorized project/recipient cannot read or send messages.

- [ ] Restart, duplicate send, lost ack, offline receiver and retry produce documented states.

- [ ] Queued, runtime-accepted and acknowledged are distinct and evidence-backed.
- [ ] No credentials or lease tokens leak in ordinary output/logs.

- [ ] If not in first workflow, record explicit deferment and do not expand scope.
