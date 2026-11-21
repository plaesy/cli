# ADR 0004: Data IR (data-ir/1)

- Status: diusulkan (belum disetujui).
- Tanggal: 2026-11-21
- Pengambil keputusan: owner (belum ada persetujuan)
- Berlaku untuk: `data-ir/1`: skema database dan query
- Menggantikan: -
- Digantikan oleh: -
- Terkait: ADR 0002 (flow), 0003 (crate), 0009 (konvensi IR)

## Ringkasan

- Skema database disimpan sebagai JSON dengan validasi JSON Schema. Tabel dan relasi dinyatakan eksplisit.
- Query disusun sebagai pohon JSON, lalu dikompilasi ke SQL berparameter. Tidak ada SQL mentah di IR.
- Dialek SQL dan subset query belum dipilih.

## Konteks

Pengguna perlu melihat dan mengubah skema database secara visual, memetakan relasi antar-tabel, dan menyusun query secara visual. Hasilnya harus bisa diubah ke kode (DDL dan SQL) dan sebaliknya (impor dari database yang sudah ada). Seperti flow, data disimpan sekali sebagai IR netral, lalu diproyeksikan ke tampilan visual dan ke kode.

## Dorongan keputusan

- Tipe bersifat netral database. Pemetaan ke dialek SQL dilakukan oleh adapter `codegen-sql`.
- Relasi dinyatakan eksplisit (satu-ke-banyak, satu-ke-satu, banyak-ke-banyak), supaya bisa digambar sebagai diagram dan dibuat sebagai kunci asing.
- Query disusun dari struktur, bukan dari SQL mentah, supaya bisa dibuat secara visual dan divalidasi.
- Tidak ada field yang menyebut database tertentu.

## Keputusan

### Format

- **Kanonik: JSON** (UTF-8), konsisten dengan `flow-ir/1`.
- **Validasi bentuk: JSON Schema** draft 2020-12 (`schemas/data-ir-1.schema.json`).
- **Contoh:** `examples/data-ir-1/crm.json`.

### Tabel

Setiap tabel memiliki `name`, `columns`, `primary_key` (wajib, minimal satu kolom), dan `indexes` opsional.

Tipe kolom:

- Primitif: `text`, `integer`, `boolean`, `timestamp`, `date`, `uuid`, `json`, `binary`.
- `decimal` dengan `precision` (1–38) dan `scale`.
- `enum` dengan daftar nilai string yang unik.

Nilai default berupa string, angka, boolean, atau `null`.

### Relasi

Relasi memiliki `id` (`rel_...`) dan `kind`:

- `one_to_many` dan `one_to_one`: `parent` dan `child` (tabel dan kolom), serta `on_delete` (`restrict`, `cascade`, atau `set_null`).
- `many_to_many`: `left`, `right`, dan `junction` (tabel perantara beserta kolom kirinya dan kanannya).

Relasi tidak disimpan di dalam tabel, sehingga satu relasi bisa dipakai oleh query dan diagram sekaligus.

### Query

Query memiliki `id` (`qry_...`), `name`, `from`, dan `select` (wajib).

- `params`: parameter yang diisi saat query dijalankan, dengan tipe.
- `joins`: hanya boleh memakai relasi yang sudah didefinisikan. Tipe join `inner` atau `left`.
- `select`: kolom (`table` dan `column`) atau agregat (`count`, `sum`, `avg`, `min`, `max`) dengan `alias`.
- `where`: predikat berbentuk pohon (`and`, `or`, `not`, `cmp`). Operator perbandingan: `eq`, `ne`, `lt`, `lte`, `gt`, `gte`, `in`, `like`, `is_null`, `is_not_null`. Operand berupa kolom, parameter, atau literal.
- `group_by`, `order_by` (berdasarkan alias), dan `limit` (maksimum 1000).

Query tidak berisi SQL mentah. Dialek SQL dibentuk oleh `codegen-sql`.

## Validasi semantik (domain/data)

Skema hanya memeriksa bentuk. Aturan berikut dicek di `domain/data`:

- Nama tabel, kolom, relasi, dan query unik dalam skema masing-masing.
- Setiap `primary_key` merujuk kolom yang ada.
- Setiap referensi kolom dalam relasi dan query merujuk tabel dan kolom yang ada.
- Kolom pada sisi relasi kompatibel tipenya (misalnya `orders.customer_id` harus sama tipenya dengan `customers.id`).
- Kolom pada `junction` merujuk tabel perantara yang ada.
- `join` hanya memakai relasi yang kedua tabelnya termasuk dalam `from` atau join sebelumnya.
- Di query dengan `group_by`, setiap kolom yang tidak diagregasi harus ada di `group_by`.
- `order_by` hanya merujuk alias yang ada di `select`.
- Setiap `param` yang dipakai harus dideklarasikan di `params`, dan tipenya cocok dengan operand lawannya.
- `on_delete: cascade` pada relasi yang bisa membentuk siklus harus ditolak.
- Impor dari database yang ada mencatat setiap konstruksi yang tidak dipetakan sebagai laporan kehilangan, tidak dilewati diam-diam.

## Validasi yang sudah dijalankan

- `examples/data-ir-1/crm.json` lolos `ajv validate --spec=draft2020` terhadap `data-ir-1.schema.json`.
- Uji negatif (`on_delete` tidak dikenal, `limit` di atas 1000, dan `primary_key` kosong) ditolak dengan benar.
- Validasi semantik belum ada.

## Opsi yang dipertimbangkan

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. SQL mentah sebagai IR | Sederhana, langsung dijalankan | Tidak bisa divisualkan atau diperiksa semantik; mengunci dialek | Ditolak |
| B. Pohon JSON (query) yang dikompilasi ke SQL berparameter | Bisa divisualkan; bisa diperiksa; dialek ditentukan di kompilasi | Subset SQL terbatas; kompiler harus dibuat | Dipilih |
| C. Visual langsung ke SQL satu dialek | Cepat untuk satu database | Mengunci dialek; sulit berpindah | Ditolak |

Riset: pola yang umum adalah UI menghasilkan pohon JSON, lalu kompiler mengubahnya menjadi SQL dengan parameter terikat, bukan penyambungan string nilai. [react-querybuilder: export](https://react-querybuilder.js.org/docs/utils/export) menunjukkan format parameter terpisah dari SQL. 

## Konfirmasi

- `codegen-sql` tidak pernah menyambung nilai ke string SQL. Setiap nilai menjadi parameter. Diperiksa dengan uji yang mencari string literal di hasil kompilasi.
- Setiap contoh di `examples/data-ir-1/` dikompilasi ke minimal dua dialek (SQLite dan PostgreSQL), lalu dijalankan pada data uji. Hasilnya harus sama.
- Bentuk diperiksa oleh `docs/adr/tools/check-ir.js` (ajv, versi tetap). Semantik diperiksa oleh `tools/validate-ir.js`. Keduanya dijalankan secara lokal; belum ada di CI (lihat rencana implementasi).

## Kriteria penerimaan (implementasi `data-ir/1`)

1. `domain/data::validate` mengimplementasikan semua aturan di bagian "Validasi semantik" dan lulus uji negatif untuk setiap aturan.
2. `codegen-sql` menghasilkan query berparameter untuk SQLite dan PostgreSQL; uji memastikan tidak ada nilai yang tersambung ke string SQL.
3. Contoh `crm.json` dan `inventory.json` menghasilkan hasil query yang sama di kedua dialek pada data uji.
4. `import-sql` mengimpor skema contoh, lalu `codegen-sql` mengekspornya kembali. IR yang dihasilkan sama dengan IR asli.

## Konsekuensi

- Positif: satu IR untuk diagram, query, dan DDL. Relasi eksplisit bisa diperiksa.
- Negatif: query visual terbatas pada subset SQL yang dinyatakan di atas. Fitur seperti subquery, window function, dan CTE belum tercakup.
- Netral: dialek SQL belum dipilih oleh ADR ini.

## Open items

- Owner menyetujui format dan subset query.
- Apakah query perlu subquery dan window function, dan kapan.
- Dialek pertama untuk `codegen-sql` (misalnya PostgreSQL atau SQLite).
- Impor dari database yang sudah ada: adapter `import-sql` untuk dialek mana.
