# ADR Plaesy

Folder ini berisi keputusan arsitektur untuk IR (representasi antara), struktur crate, klien, dan konversi desain ↔ kode. Ini tempat untuk mencari **apa yang sudah diputuskan**. Untuk **apa yang akan dikerjakan dan urutannya**, lihat [implementation-plan.md](implementation-plan.md).

## Mulai dari sini

1. [ADR 0009](0009-ir-conventions.md): aturan yang berlaku untuk semua IR (versi, field tambahan, ID, laporan kehilangan, batas input, validasi).
2. [ADR 0002](0002-visual-flow-ir.md): flow.
3. [ADR 0010](0010-design-ir.md): desain dan konversi desain ↔ kode.
4. [ADR 0003](0003-crate-structure.md) dan [ADR 0011](0011-client-structure.md): struktur crate backend dan klien.
5. ADR 0004, 0005, 0006, 0008: IR data, API, app, dan impor kode.

## Status: artinya apa

| Status | Arti |
|---|---|
| `diusulkan` (proposed) | Belum diterima. Boleh dibaca dan dikritik, belum mengikat. |
| `diterima` (accepted) | Diterima. Mengikat sampai digantikan atau dicabut. |
| `diterima lewat delegasi` | Diterima oleh asisten atas delegasi owner (2026-11-19 dan 2026-11-21). Setiap keputusan bisa dicabut oleh owner; daftarnya di `docs/platform/decisions.md`. |
| `diterima sebagian` | Sebagian bagian diterima; bagian lain masih terbuka. Setiap ADR menyebut bagian mana di Open items. |
| `digantikan` (superseded) | Tidak berlaku. Ada penggantinya di header. Isi asli ada di riwayat git. |

## Daftar

| ADR | Judul | Status | Yang diterima | Yang masih terbuka |
|---|---|---|---|---|
| [0001](0001-design-code-ir.md) | Canonical IR desain-kode | digantikan (0009, 0010) | - | - |
| [0002](0002-visual-flow-ir.md) | Visual flow IR (flow-ir/1) | diterima (katalog dan keputusan di chat; format lewat delegasi) | Katalog node, CEL, retry, batas loop, kanal approval, Rust sebagai adapter pertama, format JSON | Verifikasi ulang klaim paritas dengan Windmill; codegen Rust |
| [0003](0003-crate-structure.md) | Struktur crate backend | diterima lewat delegasi | Lapisan crate dan aturan dependensi | Implementasi `domain/*`; pemeriksaan `use` |
| [0004](0004-data-ir.md) | Data IR (data-ir/1) | diterima lewat delegasi | Format JSON, tabel, relasi, query sebagai pohon | Dialek SQL pertama; subquery dan window function |
| [0005](0005-api-ir.md) | API IR (api-ir/1) | diterima lewat delegasi | Format JSON, endpoint merujuk flow | Generator OpenAPI 3.1 dan pin versinya; hubungan tipe dengan data |
| [0006](0006-app-ir.md) | App IR (app-ir/1) | diterima lewat delegasi | Layar, komponen, binding, aksi merujuk flow | Daftar tipe komponen; validasi lintas-file di CI |
| [0007](0007-design-ir.md) | Design IR (design-ir/1) | digantikan (0010) | - | - |
| [0008](0008-code-to-visual-import.md) | Impor kode ke visual | diterima lewat delegasi | Arah impor, laporan pemetaan dan kehilangan | Bahasa pertama; parser; urutan impor database |
| [0009](0009-ir-conventions.md) | Konvensi bersama untuk semua IR | diterima lewat delegasi | Versi, `extensions`, skema tertutup, ID, laporan kehilangan, batas input, pemisahan validasi | Codegen Dioxus; CI; korpus dari desain nyata |
| [0010](0010-design-ir.md) | Design IR dan konversi desain-kode | diterima lewat delegasi | Format DTCG (sudah diterapkan), kind node, sumber desain JSON Plaesy, target Rust/Dioxus, edit B, korpus sintetis 20 fixture | Codegen Dioxus `rsx!` (sekarang modul Rust tabel konstanta); korpus dari desain nyata |
| [0011](0011-client-structure.md) | Struktur klien | diterima lewat delegasi | Struktur fitur, aturan dependensi | Library WebSocket; format diff; sinkronisasi real-time |

## Istilah

- **IR** (intermediate representation): representasi netral yang disimpan sekali, lalu diproyeksikan ke visual dan ke kode.
- **Adapter**: modul yang mengubah IR ke format target (`codegen-*`) atau sebaliknya (`import-*`).
- **Kehilangan (loss)**: konstruksi yang tidak bisa dipetakan. Dicatat di laporan, tidak dibuang diam-diam.
- **Opaque**: node yang menyimpan konstruksi tak terpetakan beserta lokasi sumbernya, supaya ekspor ulang bisa mengembalikannya.
- **MAJOR / MINOR**: versi IR. MAJOR berubah bila arti berubah. MINOR berubah bila hanya menambah.
- **`extensions`**: tempat data tambahan yang tidak dikenal versi ini (ADR 0009).
- **Mode lenient**: pembaca yang memindahkan field tak dikenal ke `extensions` dan memberi peringatan.
- **DTCG**: W3C Design Tokens Community Group, format token yang dipakai di `design-ir/1`.
- **Round trip serialisasi**: JSON diurutkan, diparse ulang, hasilnya sama. Ini belum sama dengan round trip ke kode.
- **Fixture negatif**: contoh yang sengaja salah dan harus ditolak validator.

## Yang bisa dan belum bisa diperiksa

Dari `docs/adr/tools/` (jalankan `npm ci` sekali):

```sh
npm run check     # bentuk (ajv), semantik (validate-ir.js), lenient, dan round trip serialisasi korpus
```

Dari root repo:

```sh
plaesy validate markdown docs/adr --no-baseline
```

Belum ada langkah CI yang menjalankan keduanya. Ditunda sesuai keputusan owner; lihat Fase 0 di rencana.

Kode Rust ada di `src/designir/` (design-ir/1: validasi, codegen, round trip, lenient) dan `src/irvalidate/` (flow, data, api, app: validasi; kelima IR: lenient). Diuji dengan `cargo test --locked --lib designir irvalidate`. Uji paritas Node vs Rust ada (`tools/semantic-expected.json`, dibuat `tools/gen-semantic-expected.js`). Belum ada: codegen Dioxus dan korpus dari desain nyata.

## Struktur folder

- `0001`-`0011`: ADR. `template.md`: templat ADR baru. Nomor berikutnya: `0012`.
- `schemas/`: JSON Schema draft 2020-12, satu per IR. Objek tertutup dengan `unevaluatedProperties: false` dan `extensions`.
- `examples/`: contoh positif. Harus VALID dan lolos semantik.
- `examples-negative/`: fixture negatif bentuk. Harus INVALID.
- `examples-negative-semantic/`: fixture yang lolos bentuk tetapi gagal semantik. Harus gagal `validate-ir.js` dan validator Rust.
- `examples-lenient/`: dokumen dengan field dari versi lebih baru. Harus INVALID dengan skema ketat dan VALID setelah lenient.
- `corpus/design-ir-1/`: 20 fixture desain sintetis, dibuat oleh `tools/gen-design-corpus.js`. Bukan data desain nyata.
- `tools/`: `check-ir.js` (pemeriksa utama), `validate-ir.js` (semantik), `lenient.js` (mode lenient), `gen-design-corpus.js` (generator korpus).
- `src/designir/` (kode Rust, di paket utama): validasi, codegen, round trip, dan lenient untuk design-ir/1.
- `src/irvalidate/` (kode Rust): validator semantik untuk flow, data, api, dan app, serta lenient berbasis skema untuk kelima IR.
- `implementation-plan.md`: rencana fase dan keputusan yang dibutuhkan.

## Menulis atau mengganti ADR

1. Ambil nomor berikutnya. Nomor tidak dipakai ulang.
2. Salin `template.md`. Isi header: `Status`, `Tanggal`, `Pengambil keputusan`, `Berlaku untuk`, `Menggantikan`, `Digantikan oleh`, `Terkait`.
3. Bagian wajib: Ringkasan, Konteks, Dorongan keputusan, Opsi yang dipertimbangkan, Keputusan, Konsekuensi, Konfirmasi, Kriteria penerimaan, Open items.
4. Tulis klaim verifikasi hanya jika bisa dijalankan dari repo. Jika belum, tulis "belum diperiksa".
5. Aturan lintas-IR merujuk ADR 0009, jangan disalin.
6. Untuk mengganti keputusan: ubah status lama menjadi `digantikan` dan tunjuk penggantinya. Jangan menghapus keputusan lama diam-diam.
