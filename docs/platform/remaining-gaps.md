# Celah yang belum ditutup

Status per 2026-11-23. Dokumen ini mencatat apa yang belum dikerjakan, penyebabnya, dan keputusan yang dibutuhkan.

## Sudah digabung ke `integrate/agent-features`

- Komentar dan mention pada task, redaksi rahasia, token desain, cron, approvals CLI, audit, impor JSON/CSV, relay dengan status dan retry, orkestrasi DAG, biaya dan anggaran, subtugas, diagnostik, notifikasi, multi-proyek, MCP resource, memori sesi, board, dashboard, ekspor, riwayat versi, batas laju, retry dengan backoff, fallback model, git helper, SSE framing.
- Belum diverifikasi penuh: test untuk crate yang berubah sedang berjalan. Test penuh workspace belum dijalankan.

## Belum dikerjakan (perlu keputusan atau akses)

| Kemampuan | Penghalang |
|---|---|
| Editor kanvas visual | Butuh keputusan desain: kerangka editor sudah ada, tetapi interaksi kanvas (drag, koneksi, panel properti) belum didefinisikan. |
| Terminal dan sesi PTY | Butuh dependensi PTY per platform (ConPTY di Windows). Keputusan: dependensi mana dan batas keamanan. |
| Input dan output suara | Butuh model suara dan perangkat audio. Tidak ada di mesin ini, dan model lokal perlu persetujuan unduh. |
| Akses mobile atau remote | Butuh jaringan, autentikasi, dan keputusan arsitektur (relay atau langsung). Melanggar aturan runtime offline kecuali ada persetujuan. |
| Integrasi chat (Slack, Telegram, dan lain-lain) | Butuh akun dan token pihak ketiga, serta jaringan. |
| Plugin dan sistem skill | Butuh keputusan format plugin, sandbox, dan izin. |
| Kolaborasi dan berbagi | Butuh server sinkronisasi atau protokol peer. Tidak ada di repo. |
| Aksesibilitas | Butuh UI untuk diuji. Tampilan CLI dan web dasar belum diperiksa dengan pembaca layar. |

## Keputusan yang dibutuhkan dari pemilik

1. Apakah runtime boleh menyentuh jaringan untuk fitur mobile, chat, dan kolaborasi (mengubah aturan offline di CLAUDE.md).
2. Dependensi PTY dan audio yang diizinkan.
3. Format plugin dan batas izinnya.
4. Apakah kolaborasi lewat server sendiri atau lewat berkas bersama.
