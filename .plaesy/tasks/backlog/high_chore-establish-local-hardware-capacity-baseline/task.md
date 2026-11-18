---
spec: plaesy-agent-platform
topic: plaesy-agent-platform
title: Establish local hardware capacity baseline
phase: verify
status: backlog
---

# establish-local-hardware-capacity-baseline

## Goal

Establish a measured, reproducible local development/test capacity profile on the current host so the replacement plan can distinguish feasible local work from deployment capacity assumptions.

## Scope

- Capture CPU allocation/model, RAM and swap limits, filesystem free space, OS/architecture, accelerator visibility, container/runtime availability, and Rust toolchain in a dated, redacted evidence record.

- Measure representative Rust workspace clean/incremental build, focused test, and container build durations/resource peaks using approved small vertical-slice workloads; record exact commands, revision, tool versions and repeat count.
- Measure only synthetic data and isolated disposable workloads; define safe disk/RAM headroom and cleanup.

- Use results to classify build/test workload feasibility and identify resource constraints; update plan assumptions and benchmark recommendations without generalizing to production or other machines.

## Non-goals

- Do not benchmark or load-test production, use real user data, infer GPU availability from missing tools, or claim multi-zone/HA capability.

- Do not select production NFRs, deployment topology, supported platform matrix, or cutover thresholds for the owner.
- Do not delete unrelated caches/data or consume the final disk/RAM headroom to produce a measurement.

## Deliverables

- Redacted dated capacity evidence linked from platform docs, including hardware/VM allocation and actual usable resources.

- Reproducible benchmark commands/results with workload sizes and limitations.
- Plan update separating this host's development/test capacity from any future deployment envelope.

- Follow-up task or owner decision for any resource-sensitive test or hosting target that cannot be established here.

## Dependencies

- Current host and repository; coordinate test workload with the selected vertical-slice task before benchmarking application behavior.

## Acceptance criteria

- [ ] Evidence records actual host facts and their source; it distinguishes physical/virtual allocation and available cgroup limits where present.

- [ ] Build/test measurements are repeated and include exact revision, Rust toolchain, commands, elapsed time, peak resource data if available, and failures; no result is invented or extrapolated to production.
- [ ] Benchmarks run within explicit safety limits and leave filesystem/RAM headroom; disposable outputs are cleaned without deleting unrelated data.

- [ ] Spec/plan state what this machine can support locally and label deployment capabilities `UNKNOWN` until separately measured on the target profile.
- [ ] Evidence passes focused Markdown validation and is linked in the feature/hardware inventory.
