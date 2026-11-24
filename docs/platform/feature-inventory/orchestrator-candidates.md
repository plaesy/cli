# Kandidat produk "orcharstrator.inc"

Tanggal: 2026-11-22. Status: temuan penelusuran, bukan keputusan. Tidak ada keputusan pemilik yang dibuat di sini.

## Hasil pencarian di repo

- Pencarian `orchestrator`, `orch`, dan `Orca` di `docs/`, `.plaesy/`, dan seluruh repo tidak menemukan `orcharstrator` atau `orchestrator.inc` sebagai nama produk.
- Nama yang tercatat di repo adalah **Orca** (`stablyai/orca`), dengan commit pinned `65c84d41101e2e6255f64155985de873fc81c902`:
  - `docs/platform/replacement-source-inventory.md` baris 17 (`SRC-ORCA`), dan baris 434-436 (`DELTA-ORCA-*`, status "DOCUMENTED only").
  - `docs/platform/audits/orca.md`: ledger audit yang menyatakan identitas produk, lisensi, dan SHA belum terverifikasi (UNKNOWN).
  - `docs/platform/source-audit-archive-2026-11-19/orca/` berisi README dan LICENSE (salinan arsip, belum terikat ke SHA pinned).
  - Tugas `.plaesy/tasks/done/high_chore-audit-orca-capability-ledger/`.

## Kandidat 1: Orchestrator.inc (paling cocok dengan ejaan pengguna)

- Nama di halaman: "Orchestrator.inc", dengan keterangan "the new name for Agent Orchestrator (AO)".
- URL resmi: https://orchestrator.inc/
- Repo: https://github.com/OrchestratorInc/agent-orchestrator (organisasi: https://github.com/OrchestratorInc)
- Lisensi: Apache-2.0 (menurut halaman).
- Entitas hukum: data terstruktur di halaman mencantumkan "Untrivial, Inc." (San Francisco, CA). Ini dibaca dari halaman, belum diverifikasi dari sumber independen.
- Bukti kecocokan: sufiks ".inc" pada nama pengguna sama dengan domain dan merek "Orchestrator.inc". Produk ini adalah orkestrator agen koding yang menjalankan 38 agen (termasuk Claude Code dan Codex) di worktree Git terpisah.
- Catatan: sumber pihak ketiga (Augment Code) menyebut nama lama "Agent Orchestrator (Formerly Composio Agent Orchestrator)" dan pemilik repo yang berbeda. Kepemilikan repo belum pasti.
- Sumber: https://orchestrator.inc/compare/best-ai-coding-agent-orchestrators/ ; https://www.augmentcode.com/tools/open-source-agent-orchestrators ; https://github.com/andyrewlee/awesome-agent-orchestrators

## Kandidat 2: Orca (Stably AI)

- Nama di halaman: "Orca", dengan tagline "The AI Orchestrator for 100x builders" (README) dan "The agent development environment" (situs).
- URL resmi: https://orca.dev/ (onorca.dev mengalihkan ke sini, 307).
- Repo: https://github.com/stablyai/orca (SHA pinned di repo: `65c84d41101e2e6255f64155985de873fc81c902`)
- Lisensi: MIT (tabel perbandingan di situs; README juga menyebut MIT).
- Entitas hukum: footer "© 2026 Stably AI. All rights reserved." Tidak ada sufiks "Inc." Sumber Y Combinator menyebut perusahaan "Stably AI" (YC: https://www.ycombinator.com/companies/stably-ai-orca).
- Bukti kecocokan: kata "orchestrator" di README dan kata "Orca" ada di repo ini. Namun tidak ada ".inc" atau "orchestrator.inc" di sumber Orca.
- Catatan: di repo, Orca hanya tercatat sebagai incumbent yang diaudit, bukan sebagai produk yang dipilih pemilik.

## Kesimpulan singkat

- Ejaan "orcharstrator.inc" paling mungkin merujuk ke **Orchestrator.inc** (kandidat 1). Dugaan ini hanya berdasarkan kemiripan nama dan domain.
- Jika yang dimaksud adalah "Orca", repo sudah mencatatnya sebagai incumbent yang belum diverifikasi.
- Pemilihan kandidat adalah keputusan pemilik. Belum ada keputusan yang dicatat di `docs/platform/decisions.md`.
