# ADR 0007: Design IR (design-ir/1)

- Status: superseded oleh ADR 0010 (design IR). Sebelumnya proposed; isinya digabung ke ADR 0010.
- Superseded by: ADR 0010
- Approver: owner (belum ada persetujuan).
- Version: design-ir/1 (usulan)
- Related: ADR 0001 (design-code IR), ADR 0006 (app-ir/1, komponen merujuk desain), ADR 0003 (struktur folder)

## Context

Pengguna mendesain tampilan secara visual: frame, teks, gambar, komponen dengan variant, dan token (warna, jarak, radius, font, bayangan). Desain menjadi sumber tampilan untuk aplikasi (ADR 0006), dan bisa diekspor ke kode. ADR 0001 sudah mencakup desain-ke-kode untuk sebagian kasus. ADR ini mendefinisikan IR desain yang berlaku untuk seluruh desain Plaesy.

## Decision drivers

- Token adalah satu-satunya sumber nilai visual. Nilai literal hanya dipakai bila tidak ada token yang sesuai.
- Komponen punya variant yang dinyatakan eksplisit, sehingga instance bisa dirujuk dengan nilai variant tertentu.
- Layout otomatis (auto layout) dinyatakan sebagai arah, jarak, dan perataan, bukan posisi absolut.
- Netral terhadap renderer: tidak ada field yang menyebut Figma, Dioxus, CSS, atau platform tertentu.

## Decision

### Format

- **Kanonik: JSON** (UTF-8), konsisten dengan IR lain.
- **Validasi bentuk: JSON Schema** draft 2020-12 (`schemas/design-ir-1.schema.json`).
- **Contoh:** `examples/design-ir-1/order-desk-ui.json`.

### Isi dokumen

- `tokens`: kelompok `color`, `space`, `radius`, `font`, dan `shadow`. Nama token berformat `kategori.nama`, misalnya `color.primary`.
- `components`: definisi komponen dengan `root` (satu node), dan `variants` berupa nama variant dengan daftar nilainya.
- `pages`: daftar halaman, masing-masing berisi `frames`.
- `node`: `frame`, `rect`, `text`, `image`, `vector`, atau `instance`. `instance` merujuk komponen dan mengisi nilai variant.
- `layout`: `mode` (`none` atau `auto`), dengan `direction`, `gap`, `padding`, `align`, dan `wrap` bila `mode` adalah `auto`.
- `style`: `fill`, `stroke`, `radius`, `shadow`, `opacity`, `font`, dan `text_color`. Nilainya berupa token atau literal.

### Aturan semantik (domain/design)

- `id` node dan komponen unik dalam satu desain.
- Token yang dirujuk harus ada di `tokens`. Rujukan ke kategori yang tidak sesuai ditolak, misalnya `color.*` dipakai sebagai `radius`.
- `instance.component` harus merujuk komponen yang ada.
- Setiap nilai `variant` pada `instance` harus merujuk nama variant yang ada, dan nilainya harus termasuk dalam daftar variant tersebut.
- Komponen tidak boleh merujuk dirinya sendiri, langsung atau lewat instance di dalamnya.
- `layout.mode = auto` wajib punya `direction`.
- Node `text` wajib punya `text`, dan `image` wajib punya `src`.
- Node yang sama tidak boleh menjadi anak dari dua induk.

### Hubungan dengan IR lain

- `app-ir/1` merujuk desain lewat `design_ref`, dengan format `dsg_...`. Ini merujuk satu desain, bukan node.
- Komponen di aplikasi merujuk komponen desain melalui instance, tidak menyalin gayanya.
- ADR 0001 tetap berlaku untuk desain-ke-kode. ADR ini tidak menggantikannya, dan hubungan keduanya belum didefinisikan.

## Validasi yang sudah dijalankan

- `examples/design-ir-1/order-desk-ui.json` lolos `ajv validate --spec=draft2020` terhadap `design-ir-1.schema.json`.
- Uji negatif (tipe node `video`, `instance` tanpa `component`, dan layout `auto` tanpa `direction`) ditolak dengan benar.
- Aturan semantik di atas belum diimplementasi.

## Considered options

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. Format JSON bebas (saat ini di ADR ini) | Sederhana, sesuai IR lain | Tidak kompatibel dengan ekosistem token | Dipertimbangkan ulang (lihat di bawah) |
| B. Mengikuti format token W3C DTCG (`$value`, `$type`) | Standar komunitas; bisa dipertukarkan dengan alat lain | Perlu penyesuaian contoh dan skema | Diusulkan |
| C. Format file desain proprietari dari satu alat | Cepat dipakai | Mengunci vendor | Ditolak |

Riset: format DTCG 2025.10 adalah laporan final Community Group. Ia mendefinisikan token sebagai JSON dengan `$value` dan `$type`, grup sebagai objek tanpa `$value`, dan alias dengan referensi dalam kurung kurawal ([DTCG Format Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/)). Untuk multi-tema, ada modul resolver ([DTCG Resolver Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-resolver-20251028/)).

Usulan: sejajarkan bagian `tokens` di design-ir/1 dengan DTCG (`$value` dan `$type`), sehingga token bisa dipertukarkan. Ini perubahan pada contoh dan skema, bukan pada struktur folder. Keputusan ini perlu persetujuan owner.

## Confirmation

- Setiap referensi token (`color.primary` dan sejenisnya) harus ada. Diperiksa oleh `tools/validate-ir.js`.
- Jika usulan DTCG disetujui: contoh dan skema harus lolos validasi DTCG, dan token dari satu desain harus bisa dibaca ulang tanpa kehilangan nilai.
- Variant instance hanya boleh memakai nilai yang dideklarasikan di komponen. Diperiksa oleh `tools/validate-ir.js`.

## Acceptance criteria (implementasi `design-ir/1`)

1. `domain/design::validate` mengimplementasikan semua aturan di bagian "Aturan semantik" dan lulus uji negatif.
2. Contoh `order-desk-ui.json` dan `dashboard-card.json` lolos validasi skema dan semantik.
3. Keputusan format token (DTCG atau bentuk saat ini) dicatat di ADR ini sebelum skema token difinalisasi. Jika DTCG dipilih, contoh harus lolos validasi DTCG.
4. Setiap referensi token dan variant di `instance` diperiksa oleh validator.

## Consequences

- Positif: nilai visual terpusat di token, sehingga satu perubahan tema berlaku di seluruh desain.
- Negatif: desain yang sudah ada di luar Plaesy (misalnya file Figma) perlu diimpor. Impor belum didefinisikan.
- Netral: hubungan dengan ADR 0001 belum diputuskan.

## Open items

- Owner menyetujui format dan aturan semantik.
- Hubungan dengan ADR 0001 (apakah ADR 0001 digabung atau digantikan).
- Impor dari format desain lain.
- Daftar tipe font dan bayangan yang didukung.
