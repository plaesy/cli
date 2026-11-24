# Prioritas kemampuan lintas produk

Tanggal: 2026-11-23. Sumber: 13 inventaris fitur di folder ini (`agent-orchestrator`, `craft-anything`, `deckcraft`, `deepseek-harness`, `figcraft`, `herald-os`, `hermes-agent`, `nimbalyst`, `openbot`, `openproject`, `orca`, `paperclip`, `wordcraft`). `orchestrator-candidates.md` tidak dihitung karena bukan inventaris fitur.

Status: analisis, bukan keputusan pemilik. Urutan ini hanya usulan untuk memilih kemampuan yang dikaji lebih lanjut.

## Ringkasan

- Baris yang dianalisis: 4.885 (`belum` 4.742, `sebagian` 143). Dua baris `ada` tidak dipakai.
- Kandidat kemampuan yang diuji: 49. Yang masuk tabel hanya yang lintas minimal 5 produk: 40 kemampuan.
- Kemampuan dengan skor tertinggi: komentar dan mention (12 produk), lalu akun dan autentikasi, redaksi dan penyimpanan rahasia, tema dan token desain, serta instalasi dan pembaruan otomatis.
- Plaesy sudah punya kode atau modul untuk sebagian besar kemampuan teratas. Yang sudah berstatus `sebagian` ada di 143 baris, tersebar di kemampuan memori, MCP, orkestrasi, persetujuan, dan relay.
- Angka dalam tabel adalah perkiraan. Klasifikasi memakai kata kunci pada teks fitur, dan presisinya belum merata (lihat bagian Metode dan Batasan). Lima kemampuan teratas perlu diverifikasi manual sebelum dijadikan dasar rencana.

## Metode

1. Pengurai: semua tabel fitur dibaca dengan pemisah `|` yang tidak di-escape. Status diambil dari kolom terakhir dan backtick dibuang. Baris `belum` dan `sebagian` dipakai. Status `sebagian` tidak selalu punya tabel bukti; untuk 55 baris, bukti hanya ada di catatan produk.
2. Pengelompokan: setiap kemampuan didefinisikan dengan ekspresi kata kunci (Indonesia dan Inggris) pada kolom fitur. Satu baris bisa masuk lebih dari satu kemampuan.
3. Hitungan: produk = jumlah produk yang punya minimal satu baris cocok. Baris = jumlah baris cocok dari semua produk.
4. Kedekatan dengan kode yang ada, dinilai dari `crates/` (cek nama dan isi modul):
   - 2: ada modul di `crates/` yang memiliki domain ini dan bisa dikembangkan.
   - 1: hanya ada di `docs/platform/` atau spesifikasi, tanpa modul pemilik.
   - 0: tidak ada apa pun di plaesy.
5. Skor = jumlah produk x kedekatan. Urutan: skor, lalu jumlah produk, lalu jumlah baris `sebagian`.
6. Status plaesy: `sebagian` jika ada baris `sebagian` yang cocok, selain itu `belum`. Modul relevan diambil dari bukti di inventaris, atau dari modul yang tercantum di kolom kedekatan.

## Tabel prioritas

Kolom "Produk (baris)" memakai kode: AOR agent-orchestrator, CRA craft-anything, DCK deckcraft, DSH deepseek-harness, FGC figcraft, HLD herald-os, HRM hermes-agent, NMB nimbalyst, OBT openbot, OPN openproject, ORC orca, PPC paperclip, WRD wordcraft. ID baris tidak dicantumkan untuk setiap produk karena tabel akan terlalu lebar; ID contoh untuk lima teratas ada di bagian berikutnya.

| No | Kemampuan | Jumlah produk | Baris (belum + sebagian) | Status plaesy | Modul di crates (kedekatan) | Kedekatan | Skor | Produk (baris) |
|---|---|---|---|---|---|---|---|---|
| 1 | Komentar dan mention | 12 | 90 | sebagian (1: PPC-184) | `crates/plaesy/interface/cli/src/cli/tasks_issue.rs` | 2 | 24 | NMB 15, OPN 11, WRD 11, ORC 10, PPC 10, AOR 6, HRM 6, OBT 6, DCK 5, HLD 5, CRA 4, FGC 1 |
| 2 | Akun dan autentikasi pengguna | 11 | 64 | belum | `domain/identity/src/identity` | 2 | 22 | HLD 15, HRM 14, OPN 9, PPC 6, WRD 5, AOR 4, NMB 4, ORC 4, DCK 1, DSH 1, OBT 1 |
| 3 | Redaksi dan penyimpanan rahasia | 10 | 68 | sebagian (4) | `domain/identity/src/secrets`; `adapter/memory/src/hygiene/secrets.rs` | 2 | 20 | HRM 20, PPC 15, DSH 9, HLD 5, OBT 4, OPN 4, AOR 3, CRA 3, ORC 3, NMB 2 |
| 4 | Tema dan token desain | 10 | 48 | sebagian (1: CRA-108) | `domain/design/src/tokens.rs`; `support/designlint/src/designlint/tokens.rs` | 2 | 20 | DCK 11, HLD 11, NMB 9, OBT 4, ORC 4, WRD 4, HRM 2, CRA 1, FGC 1, OPN 1 |
| 5 | Instalasi dan pembaruan otomatis | 10 | 27 | sebagian (1: OPN-315) | `cli/src/cli/install.rs`; `support/common/src/installer` | 2 | 20 | OBT 6, HLD 4, AOR 3, PPC 3, DSH 2, HRM 2, NMB 2, OPN 2, ORC 2, CRA 1 |
| 6 | Integrasi Git: PR, cabang, commit | 10 | 129 | belum | `support/common/src/common/git.rs` | 2 | 20 | ORC 40, NMB 30, AOR 21, HRM 9, OPN 7, PPC 7, DSH 5, HLD 5, OBT 4, CRA 1 |
| 7 | Memori persisten lintas sesi | 9 | 70 | sebagian (11) | `adapter/memory` (MCP, hygiene, search) | 2 | 18 | HRM 32, HLD 15, OBT 11, NMB 5, ORC 2, PPC 2, CRA 1, DSH 1, FGC 1 |
| 8 | Dukungan MCP (klien/server) | 9 | 49 | sebagian (5) | `cli/src/cli/mcp.rs`; `support/common/src/mcpserver.rs` | 2 | 18 | HRM 19, NMB 16, CRA 6, DSH 3, FGC 1, HLD 1, OBT 1, OPN 1, ORC 1 |
| 9 | Penjadwalan tugas berulang | 9 | 53 | sebagian (2) | `adapter/memory/src/schedule/mod.rs` (hanya interval) | 2 | 18 | HRM 19, PPC 10, OBT 6, AOR 4, NMB 4, OPN 4, DSH 2, HLD 2, ORC 2 |
| 10 | Label tugas | 9 | 21 | sebagian (1) | `support/tasks` (issue.json) | 2 | 18 | DCK 4, PPC 3, WRD 3, AOR 2, FGC 2, NMB 2, OPN 2, ORC 2, HRM 1 |
| 11 | Orkestrasi multi-agen dan delegasi subagen | 8 | 77 | sebagian (5) | `cli/src/cli/delegate.rs`; `support/loop/src/delegate.rs` | 2 | 16 | ORC 22, HRM 21, AOR 12, DSH 8, OBT 6, PPC 4, NMB 3, CRA 1 |
| 12 | Gerbang persetujuan manusia untuk tindakan berefek | 8 | 24 | sebagian (3) | `domain/workflow/src/policy/approvals.rs`; `cli/src/cli/approvals.rs` (belum di-commit) | 2 | 16 | HRM 7, HLD 6, CRA 3, DSH 2, ORC 2, PPC 2, NMB 1, OBT 1 |
| 13 | Pencatatan biaya, token, dan anggaran | 8 | 40 | sebagian (2) | `adapter/runtime/src/runtime/usage.rs`; `support/loop/src/delegate/usage.rs` | 2 | 16 | OPN 10, PPC 8, HRM 7, HLD 4, ORC 4, NMB 3, CRA 2, DSH 2 |
| 14 | Hierarki induk-anak tugas (subtugas) | 8 | 36 | sebagian (2) | `cli/src/cli/tasks_create.rs`; `tasks_issue.rs` | 2 | 16 | HRM 9, OPN 9, PPC 6, FGC 5, DSH 4, DCK 1, NMB 1, ORC 1 |
| 15 | Diagnostik kesehatan (doctor/healthz) | 8 | 22 | sebagian (1) | `cli/src/cli/doctor.rs` | 2 | 16 | HRM 6, AOR 3, HLD 3, OBT 3, ORC 3, PPC 2, CRA 1, DSH 1 |
| 16 | Notifikasi ke pengguna | 8 | 68 | sebagian (1) | `interface/client/src/state/notifications.rs` | 2 | 16 | HLD 18, OPN 14, HRM 9, ORC 9, NMB 8, OBT 6, AOR 3, PPC 1 |
| 17 | Impor berkas, data, atau percakapan | 8 | 11 | belum | `adapter/import-*` | 2 | 16 | OBT 3, AOR 2, FGC 1, NMB 1, OPN 1, ORC 1, PPC 1, WRD 1 |
| 18 | Multi-proyek dan workspace | 8 | 77 | belum | `support/common/src/projects.rs` | 2 | 16 | ORC 38, DSH 10, OBT 7, PPC 7, AOR 6, HRM 4, OPN 3, NMB 2 |
| 19 | Antrean pesan antar-agen (relay) | 7 | 30 | sebagian (5) | `adapter/relay/src/relay.rs` | 2 | 14 | OBT 11, HRM 7, ORC 4, AOR 3, DSH 2, PPC 2, NMB 1 |
| 20 | Workflow DAG dan pipeline | 6 | 18 | belum | `domain/flow`; `domain/workflow` | 2 | 12 | OBT 7, OPN 4, HRM 3, DSH 2, ORC 1, PPC 1 |
| 21 | Editor kanvas visual | 6 | 35 | belum | `interface/client` (kerangka editor visual) | 2 | 12 | HLD 14, FGC 11, NMB 7, DCK 1, OBT 1, WRD 1 |
| 22 | Ekspor dokumen dan laporan | 11 | 75 | sebagian (2) | tidak ada modul ekspor dokumen | 1 | 11 | FGC 20, OPN 14, DCK 10, CRA 9, NMB 5, HRM 4, PPC 4, HLD 3, DSH 2, OBT 2, WRD 2 |
| 23 | Riwayat versi dan undo/rollback | 10 | 50 | sebagian (2) | tidak ada; hanya dokumen | 1 | 10 | HLD 10, DCK 7, FGC 7, HRM 6, NMB 5, WRD 5, AOR 4, ORC 4, CRA 1, PPC 1 |
| 24 | Kolaborasi dan berbagi | 10 | 39 | sebagian (1) | tidak ada; hanya dokumen | 1 | 10 | NMB 14, FGC 7, OPN 4, PPC 4, HRM 3, HLD 2, ORC 2, AOR 1, CRA 1, WRD 1 |
| 25 | Tugas/issue: buat, ubah, daftar, tampilkan | 5 | 153 | sebagian (5) | `cli/src/cli/tasks_issue.rs`; `tasks_create.rs` | 2 | 10 | OPN 118, ORC 17, NMB 8, AOR 5, PPC 5 |
| 26 | Log audit | 5 | 12 | sebagian (3) | `domain/workflow/src/runstore`; `adapter/runstore` | 2 | 10 | HRM 4, PPC 4, NMB 2, HLD 1, OBT 1 |
| 27 | Rantai provider dan fallback model | 5 | 8 | sebagian (1) | `support/loop/src/delegate/providers.rs` | 2 | 10 | HRM 3, AOR 2, CRA 1, DCK 1, OBT 1 |
| 28 | Input dan output suara | 8 | 71 | sebagian (1) | tidak ada; hanya dokumen | 1 | 8 | HLD 23, NMB 19, HRM 18, DCK 3, ORC 3, OBT 2, WRD 2, OPN 1 |
| 29 | Akses mobile atau remote | 8 | 107 | belum | tidak ada; hanya dokumen | 1 | 8 | NMB 50, ORC 16, CRA 14, OPN 10, OBT 8, AOR 6, FGC 2, HRM 1 |
| 30 | Terminal dan sesi PTY | 7 | 95 | belum | `adapter/runtime` (menjalankan proses, tanpa PTY) | 1 | 7 | ORC 50, HLD 12, HRM 12, NMB 10, AOR 9, CRA 1, DSH 1 |
| 31 | Telemetri dan analitik | 7 | 20 | belum | tidak ada; hanya dokumen | 1 | 7 | NMB 6, ORC 6, PPC 3, DSH 2, AOR 1, FGC 1, OBT 1 |
| 32 | Retry dan timeout | 6 | 19 | sebagian (3) | `support/loop/src/delegate` (sebagian) | 1 | 6 | HRM 5, DSH 4, ORC 4, OBT 3, PPC 2, AOR 1 |
| 33 | Sistem skill atau plugin ekstensi | 6 | 13 | belum | tidak ada; hanya dokumen | 1 | 6 | OBT 5, CRA 3, HRM 2, DSH 1, NMB 1, ORC 1 |
| 34 | Pembaruan realtime lewat stream/SSE | 6 | 14 | belum | tidak ada; hanya dokumen | 1 | 6 | NMB 6, OPN 3, AOR 2, FGC 1, HLD 1, ORC 1 |
| 35 | Webhook masuk dan keluar | 6 | 27 | belum | tidak ada; hanya dokumen | 1 | 6 | OBT 10, HRM 8, DSH 3, OPN 3, PPC 2, AOR 1 |
| 36 | Integrasi chat/pesan (Slack, Telegram, dll.) | 6 | 44 | belum | tidak ada; hanya dokumen | 1 | 6 | HRM 31, OBT 8, PPC 2, AOR 1, HLD 1, NMB 1 |
| 37 | Kanban board | 5 | 44 | sebagian (3) | status folder tugas (`.plaesy/tasks/`), tanpa papan | 1 | 5 | HRM 36, NMB 5, AOR 1, OPN 1, ORC 1 |
| 38 | Dashboard dan metrik | 5 | 17 | sebagian (1) | tidak ada; hanya dokumen | 1 | 5 | ORC 6, HRM 4, OPN 4, PPC 2, NMB 1 |
| 39 | Batas laju dan kuota | 5 | 9 | sebagian (1) | `support/loop/src/delegate/providers.rs` (sebagian) | 1 | 5 | HRM 3, ORC 2, PPC 2, HLD 1, NMB 1 |
| 40 | Aksesibilitas | 5 | 12 | belum | tidak ada | 0 | 0 | HLD 3, ORC 3, WRD 3, DCK 2, OBT 1 |

## Lima kemampuan teratas

Urutan di bawah mengikuti aturan skor. Presisi diperkirakan dari sampel 9 sampai 12 baris per kemampuan, bukan pemeriksaan penuh.

1. Komentar dan mention (skor 24). Dimiliki 12 produk, paling banyak dalam daftar. Plaesy sudah punya modul tugas (`tasks_issue.rs`), tetapi inventarisnya mencatat komentar tidak ada (PPC-184, `sebagian`). Contoh baris: AOR-132, CRA-168, HRM-014, NMB-062, PPC-046, WRD-285. Presisi sampel sekitar 7 dari 12; kata "mention" juga menangkap fitur chat (Slack).
2. Akun dan autentikasi pengguna (skor 22). Dimiliki 11 produk, tetapi presisi sampel rendah (sekitar 3 dari 10). Kata "autentikasi" dan "login" juga muncul pada fitur yang bukan akun pengguna, misalnya kesiapan agen dan penyimpanan kunci API. Perlu diverifikasi ulang sebelum dipakai. Contoh: HRM-163, NMB-472, OPN-173.
3. Redaksi dan penyimpanan rahasia (skor 20). Dimiliki 10 produk dan punya 4 baris `sebagian`. Plaesy sudah memindai pola rahasia di catatan memori (`hygiene/secrets.rs`) dan punya domain `identity/secrets`, Batasnya dicatat di baris PPC-082, PPC-161, dan PPC-162 (redaksi log dan ekspor audit). Contoh: AOR-014, DSH-019, HRM-005. Presisi sampel sekitar 5 dari 10.
4. Tema dan token desain (skor 20). Dimiliki 10 produk. Plaesy punya `domain/design/tokens.rs` dan `designlint` untuk token, jadi kedekatannya tinggi. Contoh: CRA-108, HRM-061, OBT-018, OPN-237, WRD-091. Presisi sampel sekitar 7 dari 10.
5. Instalasi dan pembaruan otomatis (skor 20). Dimiliki 10 produk. Plaesy punya `install.rs` dan modul installer. Baris `sebagian` (OPN-315) menandai batas cakupannya. Contoh: CRA-022, HLD-004, NMB-600, OBT-001, ORC-305. Presisi sampel sekitar 6 dari 10.

Kemampuan ke-6 dengan skor sama adalah Integrasi Git (10 produk, belum ada baris `sebagian`). Plaesy punya `git.rs` tetapi tanpa worktree atau PR. Kemampuan persetujuan (skor 16, 8 produk, 3 baris `sebagian`) punya kedekatan kuat lewat `approvals.rs`, tetapi `approvals.rs` di CLI belum di-commit.

## Batasan

- Pengelompokan memakai kata kunci. Presisi tidak merata dan belum diperiksa untuk semua kemampuan. Untuk beberapa kemampuan (akun, komentar, redaksi), sampel menunjukkan banyak hit yang bukan fitur yang dimaksud.
- Jumlah produk dan baris adalah batas atas, bukan hitungan fitur tunggal.
- Kedekatan dinilai dari nama dan isi modul, tidak dari menjalankan kode. Modul bertanda 2 belum tentu mencakup perilaku yang sama.
- Kemampuan dengan kata kunci yang terlalu umum (misalnya "sandbox", "pencarian", "browser") tidak lolos ambang atau hasilnya berubah banyak sesudah diperketat; ini dicatat sebagai keterbatasan, bukan penilaian bahwa kemampuan itu tidak penting.
- Tabel tidak memuat ID per produk untuk semua baris. ID contoh hanya untuk lima teratas.
- Status `sebagian` sebagian besar punya bukti di tabel terpisah. Untuk 55 baris, bukti hanya ada di catatan produk, sehingga modul tidak bisa dipetakan seluruhnya.
- `crates/plaesy/interface/cli/src/cli/approvals.rs` belum di-commit. Status ini bisa berubah.
