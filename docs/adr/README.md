# ADR Plaesy

Folder ini berisi keputusan arsitektur untuk IR (representasi antara), struktur crate, dan konversi desain ↔ kode. Ini tempat yang benar untuk mencari **apa yang sudah diputuskan**. Untuk **apa yang akan dikerjakan dan urutannya**, lihat [implementation-plan.md](implementation-plan.md).

## Mulai dari sini

1. [ADR 0009](0009-ir-conventions.md): aturan yang berlaku untuk semua IR (versi, field tambahan, ID, laporan kehilangan, batas input).
2. [ADR 0002](0002-visual-flow-ir.md): flow. Bagian keputusan sudah diterima; format JSON-nya masih usulan.
3. [ADR 0010](0010-design-ir.md): desain dan konversi desain ↔ kode.
4. [ADR 0003](0003-crate-structure.md) dan [ADR 0011](0011-client-structure.md): struktur crate backend dan klien.
5. ADR 0004, 0005, 0006, 0008: IR data, API, app, dan impor kode. Semuanya masih usulan.

## Status: artinya apa

| Status | Arti |
|---|---|
| `diusulkan` (proposed) | Belum disetujui owner. Boleh dibaca dan dikritik, belum mengikat. |
| `diterima` (accepted) | Disetujui owner. Mengikat sampai digantikan. |
| `diterima sebagian` | Sebagian bagian sudah diterima; bagian lain masih usulan. Setiap bagian menyebut statusnya sendiri. |
| `digantikan` (superseded) | Tidak berlaku. Ada penggantinya di header. Isi asli tetap ada di riwayat git. |

## Daftar

| ADR | Judul | Status | Yang sudah diterima | Yang masih usulan |
|---|---|---|---|---|
| [0001](0001-design-code-ir.md) | Canonical IR desain-kode | digantikan (0009, 0010) | - | - |
| [0002](0002-visual-flow-ir.md) | Visual flow IR (flow-ir/1) | diterima | Katalog node, CEL, rumus retry, batas loop, kanal approval, Rust sebagai adapter pertama, bentuk JSON dan aturan validasi (delegasi) | Verifikasi ulang klaim paritas dengan Windmill |
| [0003](0003-crate-structure.md) | Struktur crate backend | diusulkan | - | Seluruh struktur crate |
| [0004](0004-data-ir.md) | Data IR (data-ir/1) | diusulkan | - | Seluruh isi |
| [0005](0005-api-ir.md) | API IR (api-ir/1) | diusulkan | - | Seluruh isi; OpenAPI 3.1 sebagai keluaran |
| [0006](0006-app-ir.md) | App IR (app-ir/1) | diusulkan | - | Seluruh isi |
| [0007](0007-design-ir.md) | Design IR (design-ir/1) | digantikan (0010) | - | - |
| [0008](0008-code-to-visual-import.md) | Impor kode ke visual | diusulkan | - | Bahasa pertama, parser |
| [0009](0009-ir-conventions.md) | Konvensi bersama untuk semua IR | diterima (delegasi) | Versi, `extensions`, ID, laporan kehilangan, batas input, validasi | Mode lenient (belum diimplementasi) |
| [0010](0010-design-ir.md) | Design IR dan konversi desain-kode | diterima (delegasi) | Format token DTCG, kind node, sumber desain (JSON Plaesy), target kode (Rust/Dioxus), edit B, fidelity 100% pada 20 fixture | Implementasi DTCG dan korpus 20 fixture (Fase 3) |
| [0011](0011-client-structure.md) | Struktur klien | diusulkan | - | Seluruh isi |

## Istilah

- **IR** (intermediate representation): representasi netral yang disimpan sekali, lalu diproyeksikan ke visual dan ke kode.
- **Adapter**: modul yang mengubah IR ke format target (`codegen-*`) atau sebaliknya (`import-*`).
- **Kehilangan (loss)**: konstruksi yang tidak bisa dipetakan. Dicatat di laporan, tidak dibuang diam-diam.
- **Opaque**: node yang menyimpan konstruksi tak terpetakan beserta lokasi sumbernya, supaya ekspor ulang bisa mengembalikannya.
- **MAJOR / MINOR**: versi IR. MAJOR berubah bila arti berubah. MINOR berubah bila hanya menambah.
- **`extensions`**: tempat data tambahan yang tidak dikenal versi ini (ADR 0009).
- **Round trip**: impor lalu ekspor (atau sebaliknya) menghasilkan IR yang sama.
- **Fixture negatif**: contoh yang sengaja salah dan harus ditolak validator.

## Yang sudah dan belum bisa diperiksa

Dijalankan dari `docs/adr/tools/` (`npm ci` dulu):

```sh
npm run check                              # bentuk: contoh harus VALID, fixture negatif harus INVALID
node validate-ir.js ../examples/*/*.json   # semantik (prototipe Node)
```

Dari root repo:

```sh
plaesy validate markdown docs/adr --no-baseline
```

Belum ada langkah CI untuk pemeriksaan di atas. Ditunda sesuai keputusan owner; lihat Fase 0 di rencana.

## Struktur folder

- `0001`-`0011`: ADR. Nomor berikutnya: `0012`.
- `schemas/`: JSON Schema draft 2020-12, satu per IR.
- `examples/`: contoh positif. Harus VALID.
- `examples-negative/`: fixture negatif. Harus INVALID.
- `tools/`: pemeriksa bentuk (`check-ir.js`) dan prototipe semantik (`validate-ir.js`).
- `implementation-plan.md`: rencana fase dan keputusan owner yang dibutuhkan.
- `template.md`: templat ADR baru. Salin ke `NNNN-judul.md` dan isi semua bagiannya.

## Menulis atau mengganti ADR

1. Ambil nomor berikutnya. Nomor tidak dipakai ulang.
2. Isi header dengan urutan yang sama seperti ADR lain: `Status`, `Tanggal`, `Pengambil keputusan`, `Berlaku untuk`, `Menggantikan`, `Digantikan oleh`, `Terkait`.
3. Bagian wajib: Konteks, Dorongan keputusan, Opsi yang dipertimbangkan, Keputusan, Konsekuensi, Konfirmasi, Open items.
4. Tulis klaim verifikasi hanya jika bisa dijalankan dari repo. Jika belum, tulis "belum diperiksa".
5. Aturan lintas-IR merujuk ADR 0009, jangan disalin.
6. Untuk mengganti keputusan: ubah status lama menjadi `digantikan` dan tunjuk penggantinya. Jangan menghapus keputusan lama diam-diam.
