---
spec: plaesy-agent-platform
topic: feature-implement-durable-run-append-retry-torn-tail-recovery-and-shutdown-drain
title: Implement durable run append, retry, torn-tail recovery, and shutdown drain
phase: implement
status: backlog
---

# Implement durable run append, retry, torn-tail recovery, and shutdown drain

## Goal

Implementasikan durability dan recovery run store sesuai kontrak yang disetujui; jangan mengasumsikan format incumbent.

## Scope

- Perubahan dibatasi ke persistence Rust dan kontrak terpilih

- hindari migrasi produksi.

## Non-goals

- hindari migrasi produksi.

- Do not infer parity or mark work complete without execution evidence.

## Deliverables

- Store/event append semantics, retry queue, recovery, shutdown drain dan fault tests.

## Dependencies

- `high_chore-owner-decisions-for-first-replacement-slice` where decisions affect scope; document UNKNOWN blockers.

- Relevant audit/baseline tasks and approved spec/contract; do not begin dependent implementation before these are satisfied.

## Acceptance criteria

- [ ] Append-vs-flush durability dinyatakan dan dibuktikan dengan fault injection.

- [ ] Retry mempertahankan urutan serta data batch; duplicate/replay berperilaku idempotent sesuai kontrak.
- [ ] Torn-tail recovery mempertahankan record valid dan melaporkan korupsi tanpa klaim sukses palsu.

- [ ] Shutdown drain/cancellation bounded dan teruji; restart memulihkan status persisten secara konsisten.
- [ ] Migrasi schema backward/forward compatibility ditentukan; R14 build/test gates lulus.
