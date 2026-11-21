---
spec: plaesy-agent-platform
topic: chore-define-measurable-workflow-nfrs-and-benchmark-thresholds
title: Define measurable workflow NFRs and benchmark thresholds
phase: implement
status: backlog
---

# Define measurable workflow NFRs and benchmark thresholds

## Goal

Tetapkan NFR dan benchmark yang dapat diukur untuk workflow/profile terpilih berdasarkan kebutuhan owner dan hardware terukur.

## Scope

- Measure the delegated first-slice thresholds recorded in `docs/platform/decisions.md`: 99.9% control-plane availability over 28 days, read latency p95 below 200 ms, run-state RPO at most 1 hour and RTO at most 4 hours, and monthly restore drill.
- Use the delegated cohort of 3–5 people already running agent workflows on the same project, with one operator as primary; do not expose production data or external side effects during synthetic tests.
- Keep hardware/resource sizing and production capacity separate from this development-host evidence. Do not claim the SLOs are met until measured in the target deployment profile.

## Non-goals

- Do not infer production capacity from a development VM.
- Do not infer parity or mark work complete without execution evidence.
- Do not redefine delegated thresholds; they are revocable and recorded in the decision log.

## Deliverables

- NFR table with the delegated metric/threshold, workload, environment, reproducible measurement protocol, observed results and limitations.
- A synthetic workload plan for 3–5 users; a capacity baseline and clear UNKNOWNs wherever the target environment cannot be measured.

## Dependencies

- Cohort, success, baseline and initial NFR thresholds are delegated and revocable in `docs/platform/decisions.md`; selected provider and deployment profile details remain UNKNOWN.
- Link measured hardware capacity and the approved first-slice spec. Do not begin production qualification until a target deployment environment exists.

## Acceptance criteria

- [ ] Record availability measurement window and denominator; 99.9% over 28 days is the delegated target, not a result claim.
- [ ] Measure read latency at p95 with a declared workload, sample count and repeatable commands; target is below 200 ms.
- [ ] Demonstrate run-state RPO ≤1 hour, RTO ≤4 hours and monthly restore-drill procedure on disposable data; keep target-profile qualification separate.
- [ ] Baseline records host/software, warm/cold runs, samples, variation and commands; distinguish observed results from thresholds.
- [ ] Accelerator, performance or production capacity not actually measured remains UNKNOWN; no synthetic numbers are presented as observed.
- [ ] Delegated cohort/success/NFR choices are labeled revocable and linked to `docs/platform/decisions.md`.
