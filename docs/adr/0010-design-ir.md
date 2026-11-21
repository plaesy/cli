# ADR 0010: Design IR dan konversi desain-kode (design-ir/1)

- Status: diterima lewat delegasi owner (2026-11-21, dapat dicabut). Bagian yang diwarisi dari ADR 0001 sudah diterima lewat delegasi owner (2026-11-18). Format token DTCG sudah diterapkan ke schema dan contoh; korpus 20 fixture sintetis ada (lihat Konfirmasi).
- Tanggal: 2026-11-21
- Pengambil keputusan: owner (belum ada persetujuan)
- Berlaku untuk: `design-ir/1` dan konversi desain, kode, dan IR
- Menggantikan: ADR 0001 (desain-kode IR), ADR 0007 (design IR)
- Digantikan oleh: -
- Terkait: ADR 0006, 0008, 0009

## Ringkasan

- IR desain gabungan yang menggantikan ADR 0001 dan 0007: node, komponen dan variant, token.
- Kebijakan edit B (guarded merge) dan fidelity struktural 100% pada 20 fixture berasal dari ADR 0001 dan berstatus diterima lewat delegasi.
- Format token dan kind node masih usulan.

## Konteks

Plaesy harus mengubah desain menjadi kode dan kode menjadi desain. Dua ADR sebelumnya mendefinisikan dua IR desain yang berbeda: ADR 0001 dengan kind `frame, layout, component, variant, text, vector, image, token_ref, opaque`, dan ADR 0007 dengan kind `frame, rect, text, image, vector, instance`. ADR 0007 juga mengklaim berlaku untuk seluruh desain. Satu IR desain harus ada, dan ADR ini menggabungkan keduanya.

## Dorongan keputusan

- Satu IR desain, satu converter per format (bukan per pasangan format).
- Token adalah sumber nilai visual. Nilai literal hanya dipakai bila tidak ada token yang cocok.
- Komponen dan variant dinyatakan eksplisit. Instance merujuk komponen dan mengisi nilai variant.
- Layout auto dinyatakan sebagai arah, jarak, dan perataan, bukan posisi absolut.
- Netral terhadap renderer: tidak ada field yang menyebut Figma, Dioxus, CSS, atau platform.
- Konstruksi yang tidak didukung dicatat, tidak dibuang (ADR 0009, bagian 4).

## Keputusan

### Format

- Kanonik: JSON (UTF-8). Validasi bentuk: `schemas/design-ir-1.schema.json`. Contoh: `examples/design-ir-1/`.
- Konvensi lintas-IR (versi, `extensions`, ID, laporan kehilangan, batas input) mengikuti ADR 0009.

### Node

Kind node: `frame`, `rect`, `text`, `image`, `vector`, `instance`, dan `opaque`. Posisi `x`, `y`, `width`, `height` opsional dan hanya dipakai bila layout bukan `auto`.

| Kind | Isi wajib | Catatan |
|---|---|---|
| `frame` | `id`, `children` | Kontainer; boleh punya `layout` |
| `rect` | `id` | Bentuk dasar |
| `text` | `id`, `text` | Nilai teks tidak boleh kosong |
| `image` | `id`, `src` | `src` adalah rujukan aset |
| `vector` | `id` | Path disimpan sebagai data opaque bila tidak dipetakan |
| `instance` | `id`, `component`, `variant` | `component` harus merujuk `cp_` yang ada |
| `opaque` | `id`, `construct`, `source` | Konstruksi yang tidak dipetakan, disimpan dengan lokasi sumber |

- `layout.mode`: `none` atau `auto`. `auto` wajib punya `direction`, dengan `gap`, `padding`, `align`, dan `wrap` opsional.
- `style`: `fill`, `stroke`, `radius`, `shadow`, `opacity`, `font`, dan `text_color`. Setiap nilai berupa rujukan token atau literal.

### Komponen dan variant

- `components`: daftar komponen dengan `id` (`cp_`), `name`, `root` (satu node), dan `variants` (nama variant dan daftar nilainya).
- Komponen tidak boleh merujuk dirinya sendiri, langsung atau lewat instance di dalamnya.
- Nilai `variant` pada instance harus berasal dari daftar variant komponen.

### Token

- `tokens`: kelompok `color`, `space`, `radius`, `font`, dan `shadow`. Nama berformat `kategori.nama`, misalnya `color.primary`.
- Rujukan token harus ada dan kategorinya harus cocok dengan tempat pemakaian (misalnya `color.*` tidak boleh dipakai sebagai `radius`).
- Keputusan (delegasi, 2026-11-21, sudah diterapkan): format token mengikuti W3C DTCG 2025.10 (`$value`, `$type`, alias dengan `{...}`). Schema dan contoh `design-ir/1` sudah diubah ke bentuk DTCG. Contoh lama dalam bentuk internal sudah dikonversi.

### Aturan semantik (domain/design)

- `id` node dan komponen unik dalam satu desain.
- Setiap node punya paling banyak satu induk.
- `instance.component` dan `instance.variant` harus valid (lihat di atas).
- Rujukan token harus valid (lihat di atas).
- `layout.mode = auto` wajib punya `direction`.
- Node `text` wajib punya teks; node `image` wajib punya `src`.

### Rujukan dari IR lain

- `app-ir/1` merujuk desain dengan `design_ref` berbentuk `dsg_<id>` (seluruh desain) atau `dsg_<id>#cp_<id>` (komponen di dalam desain). Pemeriksaan lintas-file (rujukan ada di desain yang dimuat) belum ada.

### Konversi (lowering contracts)

| Arah | Masukan | Keluaran | Kehilangan |
|---|---|---|---|
| desain → IR | format desain Plaesy (JSON) | design-ir/1 | konstruksi yang tidak dipetakan sebagai `opaque` |
| IR → desain | design-ir/1 | format desain Plaesy | konstruksi yang tidak bisa diekspresikan target |
| IR → kode | design-ir/1 | kode UI (framework belum dipilih) | konstruksi yang tidak bisa diekspresikan target |
| kode → IR | kode dari target yang sama | design-ir/1 | kode di luar daerah yang dikelola |

### Kebijakan edit: B (guarded merge)

- Edit di dalam daerah yang dikelola dibaca kembali lewat ID sumber (source map).
- Edit di luar daerah itu dipertahankan dan tidak diurai.
- Tidak ada sinkronisasi dua arah penuh (opsi C ditolak).

### Fidelity

- Round trip struktural harus 100% pada korpus minimal 20 fixture. Yang diperiksa: ID node, hierarki, dan nilai token.
- Fidelity piksel tidak diklaim.
- Hasil dilaporkan per fixture dan agregat. Satu kegagalan membuat run gagal.
- Isi korpus (20 fixture) belum ada dan menjadi tanggung jawab owner dengan QA.

### Sumber desain dan target kode

- Sumber desain pertama dan target kode pertama belum dipilih owner. Keduanya tidak boleh dipilih di ADR ini.

## Warisan dari ADR 0001

Bagian berikut berasal dari ADR 0001 yang diterima lewat delegasi owner pada 2026-11-18, dan tetap berlaku:

- Node ID stabil; ID yang sudah pensiun tidak dipakai ulang (sekarang di ADR 0009).
- Konstruksi tidak didukung menjadi `opaque`, tidak dibuang.
- Kebijakan edit B.
- Fidelity struktural 100% pada korpus minimal 20 fixture.
- Batas 5 MB dan kedalaman 64 (sekarang di ADR 0009).

## Validasi yang sudah dijalankan

- `examples/design-ir-1/order-desk-ui.json` dan `dashboard-card.json` lolos validasi bentuk dan semantik (`docs/adr/tools/check-ir.js`, `tools/validate-ir.js`).
- Fixture negatif `examples-negative/design-ir-1/unknown-node-type.json` ditolak.
- Konversi (desain ↔ IR ↔ kode) belum diimplementasi.

## Opsi yang dipertimbangkan

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. Dua IR desain (ADR 0001 dan 0007 terpisah) | Tidak perlu menggabungkan | Dua sumber kebenaran; hubungan belum jelas | Ditolak |
| B. Satu IR desain, menggabungkan kind dari kedua ADR | Satu sumber; konsisten dengan ADR 0006 | Migrasi contoh dan skema | Dipilih |
| C. Format desain proprietari dari satu alat | Cepat dipakai | Mengunci vendor | Ditolak |

Format token DTCG tidak dimasukkan sebagai keputusan di sini. Lihat Open items.

## Konfirmasi

- `docs/adr/tools/check-ir.js` lulus untuk contoh positif dan fixture negatif design.
- `tools/validate-ir.js` lulus untuk contoh design.
- `tools/check-ir.js` juga memeriksa 20 fixture korpus (`corpus/design-ir-1/`): bentuk, semantik, dan round trip serialisasi kanonik (ID dan token tidak berubah). Ini bukan round trip ke kode.
- Fixture negatif semantik (`examples-negative-semantic/design-ir-1/`) menguji aturan kategori token, token yang tidak ada, dan nilai dimension yang salah.
- Round trip struktural ke kode (desain → IR → kode → IR) belum diuji: adapter codegen belum ada (Fase 3).

## Konsekuensi

- Positif: satu IR desain untuk app (ADR 0006) dan konversi kode.
- Negatif: contoh dan skema dari ADR 0007 perlu penyesuaian kecil: kind `opaque` ditambahkan dan field `construct` serta `source` dipakai di node itu. Tidak ada kind yang dihapus.
- Netral: tidak ada renderer yang dipilih.

## Kriteria penerimaan

1. `domain/design::validate` mengimplementasikan aturan semantik di atas dan lulus fixture negatif per aturan.
2. Round trip struktural lulus pada korpus minimal 20 fixture.
3. Keputusan format token (DTCG atau bentuk internal) dicatat dengan persetujuan owner.
4. Pemeriksaan lintas-file untuk `design_ref` dari app ada di CI.

## Open items

- Keputusan delegasi (dapat dicabut): format token DTCG; daftar kind node di bagian "Node".
- Keputusan delegasi (dapat dicabut): sumber desain pertama adalah format JSON desain Plaesy sendiri (`design-ir/1`), tanpa alat desain pihak ketiga; target kode pertama adalah Rust dengan komponen Dioxus, sejalan dengan adapter codegen pertama di ADR 0002.
- Korpus saat ini sintetis (`tools/gen-design-corpus.js`). Korpus dari desain nyata dan QA yang memverifikasinya masih terbuka.
- Impor dari format desain lain: ADR tersendiri.
