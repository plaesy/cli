# ADR 0006: App IR (app-ir/1)

- Status: proposed. Belum disetujui owner.
- Tanggal: 2026-11-21
- Approver: owner (belum ada persetujuan).
- Version: app-ir/1 (usulan)
- Related: ADR 0002 (flow-ir/1, aksi tombol), ADR 0003 (struktur folder), ADR 0004 (data-ir/1, sumber data), ADR 0005 (api-ir/1), ADR 0009 (konvensi IR), ADR 0010 (design IR, menggantikan 0007)

## Context

Pengguna membangun aplikasi secara visual: layar, komponen, data yang ditampilkan, dan aksi yang dijalankan ketika tombol diklik. Aksi adalah flow (ADR 0002), data berasal dari query (ADR 0004), dan tampilan komponen merujuk desain (ADR 0001 dan konteks `design`).

## Decision drivers

- Aksi tombol dan perubahan input hanya merujuk flow dan query yang sudah ada.
- Tampilan dan logika dipisah: komponen merujuk desain, bukan menyimpan gaya sendiri.
- Aplikasi bisa dijalankan di klien (pratinjau) dan diekspor ke kode, dengan IR yang sama.

## Decision

### Format

- **Kanonik: JSON** (UTF-8), konsisten dengan IR lain.
- **Validasi bentuk: JSON Schema** draft 2020-12 (`schemas/app-ir-1.schema.json`).
- **Contoh:** `examples/app-ir-1/order-desk.json`.

### Isi dokumen

- `screens`: daftar layar dengan `id`, `route` (mendukung parameter `:nama`), `params` bertipe, `queries` yang dipakai, dan `root`.
- `root` dan `children`: pohon komponen. Setiap komponen punya `id`, `type`, `design_ref` (rujukan ke desain), `props`, `events`, `children`, dan `visible_when` opsional.
- `props`: nilai berupa salah satu dari:
  - `literal`,
  - `param` (parameter layar),
  - `query` dan `column` (hasil query),
  - `flow_output` dan `flow_id` (output flow).
- `events`: `on` (`click`, `change`, `submit`, atau `load`) dan `action`.
- `action`: `flow_id`, `flow_version` opsional, `inputs` (nilai dari `valueSource`), `on_success` dan `on_failure` berupa navigasi ke layar lain.

### Aturan semantik (domain/app)

- `id` layar dan komponen unik dalam satu aplikasi.
- `start_screen` harus merujuk layar yang ada.
- `route` unik dalam satu aplikasi, dan parameter di `route` harus dideklarasikan di `params` layar.
- `param` yang dipakai di komponen atau aksi harus dideklarasikan di `params` layar tersebut.
- `query` harus ada di `queries` layar, dan `column` harus ada di hasil query.
- `flow_id` dan `flow_version` harus ada. Input aksi harus cocok dengan input flow, dan `flow_output` harus ada di output flow.
- `navigation.screen` harus merujuk layar yang ada, dan `params` navigasi harus cocok dengan `params` layar tujuan.
- `design_ref` harus merujuk `dsg_<id>` yang ada. Bila berbentuk `dsg_<id>#cp_<id>`, komponen `cp_<id>` harus ada di desain itu.
- `visible_when` dan ekspresi lain diparse dengan CEL, sama seperti di flow.
- Tipe yang tidak cocok (misalnya `flow_output` berisi objek ke prop yang hanya menerima string) ditolak, atau diberi peringatan jika konversi eksplisit.

### Hubungan dengan IR lain

- Aksi adalah `flow-ir/1`. Tombol tidak menyimpan logika sendiri.
- Data adalah query `data-ir/1`, dan tidak ada SQL mentah di aplikasi.
- Gaya dan tampilan berasal dari desain. Komponen aplikasi hanya membawa `design_ref`.

## Validasi yang sudah dijalankan

- `examples/app-ir-1/order-desk.json` lolos `ajv validate --spec=draft2020` terhadap `app-ir-1.schema.json`.
- Uji negatif (route tanpa `/`, event `hover` yang tidak dikenal, dan tipe parameter yang salah) ditolak dengan benar.
- Aturan semantik di atas belum diimplementasi.

## Considered options

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. app-ir/1 deklaratif (layar, komponen, binding) | Bisa diedit visual; bisa diekspor ke beberapa framework | Perlu runtime interpreter atau generator | Dipilih |
| B. Kode framework (Dioxus) sebagai sumber | Fleksibel; tidak perlu IR | Tidak bisa diedit visual; sulit diperiksa | Ditolak |
| C. Format proprietari dari satu produk builder | Cepat dipakai | Mengunci vendor; bertentangan dengan tujuan satu kesatuan | Ditolak |

Riset: belum ada standar terbuka yang diakui badan standar untuk app builder. Yang ada adalah format spesifik proyek, dan beberapa di antaranya bisa dijadikan referensi desain: [A2UI](https://chenhui9392.github.io/a2ui-vue-engine/en/guide/json-schema.html) (binding dengan tipe path, literal, atau expression; aksi dengan nama event, tipe aksi, dan payload), [json-render](https://github.com/vercel-labs/json-render) (katalog komponen dan aksi, aksi tidak dijalankan oleh komposer), dan [OpenTiny GenUI](https://gitcode.com/opentiny/genui-sdk/blob/dev/docs/src/en/schema/protocol.md) (pohon komponen dengan props dan event).

Keputusan: `app-ir/1` adalah format milik Plaesy, karena tidak ada standar yang bisa diadopsi. Bentuk binding dan aksi mengikuti pola A2UI (event, tipe aksi, payload). Prinsip dari json-render dipakai juga: komposer tidak menjalankan aksi; aksi dijalankan oleh interpreter flow.

## Confirmation

- Setiap event (`click`, `change`, `submit`, `load`) di contoh merujuk flow yang ada. Diperiksa oleh `tools/validate-ir.js`.
- Setiap komponen `design_ref` merujuk komponen desain yang ada (lintas-file; belum diperiksa, perlu validator gabungan).
- Pratinjau di `client/*` menjalankan app-ir yang sama dengan hasil ekspor kode. Diuji dengan contoh yang sama.

## Acceptance criteria (implementasi `app-ir/1`)

1. `domain/app::validate` mengimplementasikan semua aturan di bagian "Aturan semantik" dan lulus uji negatif.
2. Contoh `order-desk.json` dan `login-and-stock.json` dijalankan di pratinjau klien. Setiap aksi memanggil flow yang benar, diuji dengan Transport palsu.
3. Hasil ekspor kode untuk contoh yang sama menghasilkan perilaku yang sama dengan pratinjau pada fixture uji.
4. Validasi gabungan lintas-file (`design_ref` terhadap `design-ir/1`) belum ada (lihat rencana implementasi).

## Consequences

- Positif: satu model untuk layar, data, dan aksi. Pratinjau dan ekspor memakai IR yang sama.
- Negatif: aplikasi bergantung pada flow, query, dan desain yang sudah ada. Urutan pembuatan menjadi penting.
- Netral: framework UI untuk ekspor belum dipilih.

## Open items

- Owner menyetujui format dan aturan semantik.
- Daftar tipe komponen yang didukung (misalnya `page`, `table`, `button`) dan propertinya.
- Aturan konversi tipe antara `flow_output` dan prop.
- Framework UI untuk ekspor kode.
