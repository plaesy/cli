# ADR 0001: Canonical IR desain-kode (digantikan)

- Status: digantikan (superseded)
- Tanggal: 2026-11-18 (diterima lewat delegasi owner); digantikan 2026-11-21
- Pengambil keputusan: owner, lewat delegasi
- Berlaku untuk: - (tidak lagi berlaku)
- Menggantikan: -
- Digantikan oleh: ADR 0009 (aturan lintas-IR) dan ADR 0010 (design IR)
- Terkait: ADR 0007 (juga digantikan)

Dokumen ini tidak lagi berisi keputusan yang berlaku. Isinya dipindahkan sebagai berikut. Teks asli ada di commit `6193be9` (sebelum penggabungan).

| Bagian asli | Dipindah ke |
|---|---|
| Node model `ir/1` dan daftar kind | ADR 0010, bagian "Node" |
| Aturan `id` stabil dan ID yang tidak dipakai ulang | ADR 0009, bagian 3 |
| `extensions` dan batas 5 MB, kedalaman 64 | ADR 0009, bagian 2 dan 5 |
| Versi MAJOR/MINOR | ADR 0009, bagian 1 |
| Tabel lowering (desain ↔ IR ↔ kode) | ADR 0010, bagian "Konversi" |
| Kebijakan edit B (guarded merge) | ADR 0010, bagian "Kebijakan edit" |
| Fidelity round trip 100% pada 20 fixture | ADR 0010, bagian "Fidelity" |
| Pertanyaan terbuka 1–6 | ADR 0010, bagian "Open items" |

Pertanyaan yang masih terbuka sudah dipindah. Yang sudah dijawab: edit policy B, fidelity struktural, dan batas input. Yang belum: sumber desain pertama dan target kode pertama (lihat ADR 0010).
