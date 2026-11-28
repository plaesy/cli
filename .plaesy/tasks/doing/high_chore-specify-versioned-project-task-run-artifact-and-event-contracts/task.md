---
spec: plaesy-agent-platform
topic: chore-specify-versioned-project-task-run-artifact-and-event-contracts
title: Specify versioned project, task, run, artifact, and event contracts
phase: implement
status: doing
---

# Specify versioned project, task, run, artifact, and event contracts

## Goal

Definisikan kontrak domain versioned untuk project/task/run/artifact/event dan transisi lifecycle workflow terpilih.

## Scope

- Tidak membekukan semua-product API atau memilih transport/database tanpa keputusan owner.

## Non-goals

- Tidak membekukan semua-product API atau memilih transport/database tanpa keputusan owner.

- Do not infer parity or mark work complete without execution evidence.

## Deliverables

- Schema/ADR kontrak, lifecycle state machine, compatibility dan error semantics.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` where decisions affect scope; document UNKNOWN blockers.

- Relevant audit/baseline tasks and approved spec/contract; do not begin dependent implementation before these are satisfied.

## Acceptance criteria

- [ ] Identitas stabil, ownership/scope, valid transitions, terminal states, cancellation, retry dan unknown outcomes didefinisikan.

- [ ] Schema versioning, compatibility, idempotency/correlation, event ordering serta error model tertulis.
- [ ] Setiap kontrak memiliki contoh valid/invalid dan skenario acceptance; unresolved behavior tetap UNKNOWN.
