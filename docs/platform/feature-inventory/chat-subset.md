# Subset chat: ukuran celah `plaesy chat`

Tanggal: 2026-11-25. Dasar: tiga inventaris (`hermes-agent.md`, `deepseek-harness.md`, `orca.md`). Dokumen ini hanya menghitung baris yang masuk subset chat, dari kolom Kategori. Belum ada audit baris per baris terhadap kode.

## Batas subset

- Hermes: kategori `slash-command`, `tool`, `skill`, `model`, `agen`, `memori`, `konfigurasi`, `keamanan`, `otomasi`, `CLI`.
- DeepSeek: kategori `Agen & tool`, `Model & kredensial`, `Sesi`, `Keamanan`, `CLI`, `Otomasi`.
- Orca: kategori `agen`, `CLI`, `orkestrasi agen`, `konfigurasi`.
- Di luar subset: platform pesan, browser, editor, remote, UI, git, terminal, Web UI, desktop, dan SDK.

## Jumlah saat ini

| Inventaris | Baris subset | `belum` | `sebagian` | `ada` |
|---|---|---|---|---|
| Hermes | 498 | 473 | 25 | 0 |
| DeepSeek | 103 | 99 | 4 | 0 |
| Orca | 126 | 115 | 11 | 0 |
| **Total** | **727** | **687** | **40** | **0** |

Angka `sebagian` masih berasal dari penilaian lama dan belum diperiksa ulang terhadap slice 1–8 dan tool chat yang sudah masuk.

## Yang belum dikerjakan

- Audit tiap baris subset terhadap kode dan tes sekarang. Ini langkah berikutnya dan belum dimulai.
- Status `di luar batas` belum diberikan ke baris mana pun; keputusannya perlu persetujuan pemilik.
- Daftar `belum` berurut prioritas belum disusun.
