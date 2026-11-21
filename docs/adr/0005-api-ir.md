# ADR 0005: API IR (api-ir/1)

- Status: diusulkan (belum disetujui).
- Tanggal: 2026-11-21
- Pengambil keputusan: owner (belum ada persetujuan)
- Berlaku untuk: `api-ir/1`: endpoint, DTO, dan auth
- Menggantikan: -
- Digantikan oleh: -
- Terkait: ADR 0002 (flow), 0003 (crate), 0004 (data), 0009 (konvensi IR)

## Ringkasan

- Endpoint API disimpan sebagai JSON dengan validasi JSON Schema. Logika endpoint adalah flow yang dirujuk, bukan kode.
- Keluaran yang direncanakan: OpenAPI 3.1 (usulan).
- Status: diusulkan.

## Konteks

Pengguna perlu mendefinisikan endpoint API secara visual: method, path, tipe request dan response, auth, dan logika yang dijalankan. Logika endpoint adalah flow (ADR 0002). Tipe request dan response menjadi DTO di `dto/` dan dipakai server maupun klien.

## Dorongan keputusan

- Satu sumber untuk kontrak API: dari sini dibentuk OpenAPI, tipe DTO, dan route di `interface/http`.
- Logika endpoint hanya merujuk flow yang ada dengan versi yang jelas. Endpoint tidak menyimpan kode.
- Tipe menggunakan grammar yang sama dengan DTO, tanpa tipe bahasa tertentu.
- Auth dinyatakan sebagai scope, konsisten dengan scope flow.

## Keputusan

### Format

- **Kanonik: JSON** (UTF-8), konsisten dengan `flow-ir/1` dan `data-ir/1`.
- **Validasi bentuk: JSON Schema** draft 2020-12 (`schemas/api-ir-1.schema.json`).
- **Contoh:** `examples/api-ir-1/orders.json`.

### Isi dokumen

- `base_path`: prefiks path, misalnya `/v1/orders`.
- `types`: definisi tipe bernama dengan nama huruf besar. Bentuknya:
  - primitif (`string`, `number`, `integer`, `boolean`, `null`),
  - `enum`,
  - `object` dengan `properties` dan `required`,
  - `array` dengan tipe elemen.
  Referensi ke tipe lain memakai `{ "$ref": "NamaTipe" }`.
- `endpoints`: daftar endpoint dengan:
  - `method`: `GET`, `POST`, `PUT`, `PATCH`, atau `DELETE`,
  - `path` relatif terhadap `base_path`, dengan parameter `{nama}`,
  - `path_params`, `query_params`, dan `body` (tipe),
  - `auth.scope`: daftar scope yang dibutuhkan,
  - `handler`: `flow_id` dan `flow_version`,
  - `responses`: kode status ke tipe, minimal satu,
  - `rate_limit_per_minute` opsional.

### Aturan semantik (domain/api)

- Setiap parameter di `path` harus punya deklarasi di `path_params`, dan sebaliknya.
- Kombinasi `method` dan `path` harus unik dalam satu API.
- `$ref` harus merujuk tipe yang ada di `types`. Tidak boleh ada siklus tipe tanpa elemen `array` atau `object` di antaranya.
- `handler.flow_id` dan `flow_version` harus ada. Input flow harus cocok dengan `path_params`, `query_params`, dan `body`.
- Output flow harus cocok dengan salah satu `responses`.
- Scope endpoint harus termasuk dalam scope yang diizinkan untuk flow handler.
- Method `GET` dan `DELETE` tidak boleh punya `body`.
- Kode status harus berupa tiga digit, dan kode 2xx dan 4xx/5xx harus punya tipe yang sesuai.

### Hubungan dengan IR lain

- Handler adalah flow (`flow-ir/1`). Endpoint tidak menyimpan logika sendiri.
- Tipe yang mengambil data dari tabel bisa diturunkan dari query (`data-ir/1`), tapi hubungan itu belum didefinisikan di sini.
- Route dinamis di `interface/http` membaca dokumen ini dan meneruskan request ke `service/api`.

## Validasi yang sudah dijalankan

- `examples/api-ir-1/orders.json` lolos `ajv validate --spec=draft2020` terhadap `api-ir-1.schema.json`.
- Uji negatif (method `FETCH` dan `responses` kosong) ditolak dengan benar.
- Aturan semantik di atas belum diimplementasi.

## Opsi yang dipertimbangkan

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. OpenAPI YAML sebagai sumber utama | Standar luas, banyak tooling | Tidak terhubung ke flow (logika endpoint); sulit diedit visual | Ditolak sebagai sumber; dipakai sebagai keluaran |
| B. api-ir/1 JSON, OpenAPI dihasilkan darinya | Satu sumber; terhubung ke flow dan DTO | Perlu generator OpenAPI | Dipilih |
| C. Protobuf/gRPC | Kontrak kuat | Klien web tidak nyaman; tidak cocok dengan endpoint buatan pengguna | Ditolak |

Riset: OpenAPI 3.1 sejajar dengan JSON Schema draft 2020-12, dan skema di `api-ir-1` memakai draft yang sama, jadi tipe dan validasinya bisa dipetakan langsung ([OpenAPI 3.0 to 3.1 upgrade guide](https://learn.openapis.org/upgrading/v3.0-to-v3.1.html)). Perubahan yang perlu diperhatikan saat menghasilkan OpenAPI 3.1: `nullable` diganti tipe `null` dalam array `type`, `example` diganti `examples`, dan payload biner memakai `contentEncoding`/`contentMediaType`. Versi yang dihasilkan: OpenAPI 3.1.x (3.1.1 disebut sebagai patch terbaru di panduan migrasi; cek halaman rilis resmi sebelum menetapkan versi).

Keputusan: keluaran utama adalah OpenAPI 3.1. OpenAPI 3.0 tidak dihasilkan.

## Konfirmasi

- Setiap endpoint di `examples/api-ir-1/` menghasilkan dokumen OpenAPI yang valid menurut validator OpenAPI yang dipilih.
- Setiap `handler.flow_id` harus ada dan inputnya cocok dengan parameter endpoint. Diperiksa oleh `domain/api`.
- Route dinamis di `interface/http` hanya meneruskan request ke `service/api`. Diperiksa dengan uji yang memastikan tidak ada logika bisnis di handler.

## Kriteria penerimaan (implementasi `api-ir/1`)

1. `domain/api::validate` mengimplementasikan semua aturan di bagian "Aturan semantik" dan lulus uji negatif.
2. Contoh `orders.json` dan `auth-and-errors.json` menghasilkan dokumen OpenAPI 3.1 yang lolos validator OpenAPI.
3. Setiap `handler.flow_id` yang dirujuk harus ada. Uji memastikan endpoint dengan flow yang tidak ada ditolak.
4. Route dinamis di `interface/http` hanya meneruskan request ke `service/api`. Uji memastikan tidak ada logika bisnis di handler.

## Konsekuensi

- Positif: kontrak API, tipe DTO, dan route dibentuk dari satu sumber.
- Negatif: setiap perubahan tipe di `types` bisa mengubah banyak endpoint. Versi API perlu dikelola (`/v1`, `/v2`).
- Netral: tidak ada generator OpenAPI yang dipilih; itu bagian implementasi.

## Open items

- Owner menyetujui format dan aturan semantik.
- Versi API: apakah setiap perubahan tidak kompatibel membuat `base_path` baru.
- Hubungan tipe dengan query `data-ir/1`.
