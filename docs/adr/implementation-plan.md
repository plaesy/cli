# Rencana implementasi IR dan struktur crate

- Status: draft untuk persetujuan owner. Belum ada fase yang dimulai.
- Tanggal: 2026-11-21
- Dasar: ADR 0002, 0003, 0004, 0005, 0006, 0008, 0009, 0010, 0011
- Sifat: rencana, bukan klaim bahwa ada yang sudah diimplementasi. Keadaan saat ini: skema dan contoh ada, validasi semantik hanya prototipe Node, `domain/*` dan crate Rust baru belum ada.

## Prinsip

- Setiap fase punya gerbang masuk (entry gate), kriteria keluar (exit), dan keputusan owner yang diperlukan.
- Setiap fase lulus `cargo build`, `cargo test`, `cargo clippy`, dan `cargo fmt --check` sebelum fase berikutnya dimulai (CLAUDE.md).
- Setiap aturan semantik punya minimal satu fixture negatif (ADR 0009, bagian 6).
- Tidak ada klaim "lulus di CI" sebelum langkah CI ada dan berjalan.

## Urutan dan ketergantungan

```text
Fase 0 (tata kelola & CI)
  └─> Fase 1 flow-ir/1  (domain/flow, interpreter murni, codegen Rust)
        └─> Fase 2 data-ir/1  (domain/data, codegen-sql)
        └─> Fase 3 design-ir/1 (domain/design, round trip)
              └─> Fase 4 api-ir/1 (domain/api, OpenAPI)
                    └─> Fase 5 app-ir/1 (domain/app, validasi lintas-file)
                          └─> Fase 6 impor (import-sql, lalu import-code-rust)
                                └─> Fase 7 klien (ADR 0011)
```

Urutan flow → data → design → api → app mengikuti ADR 0003 dan 0006 (app bergantung pada flow, data, api, dan design). Fase 2 dan 3 bisa paralel setelah Fase 1 jika owner setuju.

## Fase 0: tata kelola dan CI

Masuk: ADR 0009–0011 ditulis (selesai).

Kerja:

1. Tambah langkah CI di `.github/workflows/ci.yml`: `npm ci && npm run check` di `docs/adr/tools/`, dan `node validate-ir.js` untuk semua contoh. Hanya saat `docs/adr/**` berubah.
2. Tambah pemeriksaan Markdown untuk `docs/adr/` (`plaesy validate markdown docs/adr --no-baseline`) ke CI.
3. Registrasikan ADR 0002–0011 di `docs/platform/decisions.md`. File itu punya perubahan belum di-commit milik owner; koordinasikan sebelum mengubahnya.
4. Keputusan delegasi 2026-11-21 untuk ADR 0002, 0009, dan 0010 sudah tercatat di `docs/platform/decisions.md`. Owner dapat mencabutnya.

Keluar:

- CI menjalankan pemeriksaan bentuk dan Markdown, dan lulus.
- Status setiap ADR di `docs/platform/decisions.md` sama dengan header ADR.

Keputusan: 0009 dan 0010 diterima lewat delegasi (dapat dicabut). Urutan fase tetap berlaku sampai owner mengubahnya.

## Fase 1: flow-ir/1

Masuk: Fase 0 keluar. ADR 0002 keputusan sudah diterima; bagian format perlu disetujui (lihat Open items 0002).

Kerja:

1. Buat `crates/plaesy/kernel`, `crates/plaesy/dto`, dan `crates/plaesy/domain/flow` dengan `members` eksplisit di `Cargo.toml` (ADR 0003, langkah 2).
2. `domain/flow::ir`: tipe dari skema `flow-ir-1`. Dengan `serde` dan `deny_unknown_fields`, sesuai ADR 0009 bagian 2 (`extensions` untuk field tambahan).
3. `domain/flow::validate`: semua aturan di ADR 0002 bagian "Aturan semantik (domain/flow)". Satu fixture negatif per aturan.
4. `domain/flow::eval`: interpreter murni tanpa I/O. Ekspresi memakai CEL (keputusan ADR 0002).
5. Codegen Rust: satu adapter (`adapter/codegen-rust`). Uji konformansi: output kode dan interpreter menghasilkan hasil sama pada fixture yang sama.
6. Uji retry dan batas loop memakai rumus di ADR 0002 (sudah diverifikasi aritmetikanya).

Keluar:

- Semua aturan validasi flow punya uji positif dan negatif.
- Dua contoh di `examples/flow-ir-1/` menghasilkan output yang sama pada interpreter dan kode hasil codegen.
- `cargo tree -p plaesy-domain-flow` tidak memuat `tokio`, `axum`, atau `rusqlite`.

Keputusan: format flow-ir/1 diterima lewat delegasi (ADR 0002, bagian "Format").

## Fase 2: data-ir/1

Masuk: Fase 1 keluar (pola `domain/*` sudah ada).

Kerja:

1. `domain/data`: schema, relation, query. Aturan semantik di ADR 0004 bagian "Validasi semantik". Satu fixture negatif per aturan.
2. `adapter/codegen-sql`: query berparameter. Tidak ada nilai yang disambung ke string SQL; uji mencari string literal di hasil.
3. Dialek pertama (SQLite atau PostgreSQL) belum dipilih. Tetapkan saat masuk fase ini.
4. Contoh `crm.json` dan `inventory.json` dijalankan di dua dialek dan hasilnya diuji sama.

Keluar: seluruh aturan semantik lulus uji negatif; hasil query sama di dua dialek.

Keputusan owner: dialek pertama; apakah subquery dan window function masuk.

## Fase 3: design-ir/1

Status sekarang (2026-11-21): format DTCG diterapkan; validator semantik dan lenient di Rust (`src/designir/`); codegen ke modul Rust dengan tabel konstanta; round trip struktural ke kode lulus untuk 20 korpus. Yang belum: codegen Dioxus `rsx!`, dan korpus dari desain nyata.

Masuk: Fase 0 keluar. ADR 0010 diterima lewat delegasi.

Kerja:

1. `domain/design` di Rust: node, komponen, variant, token, layout. Aturan di ADR 0010 bagian "Aturan semantik". Validator Node sudah ada sebagai referensi.
2. Format token DTCG: sudah diterapkan. Konversi contoh dilakukan oleh skrip sekali jalan (sudah selesai).
3. Korpus: 20 fixture sintetis sudah ada di `docs/adr/corpus/design-ir-1/` (dibuat `tools/gen-design-corpus.js`). Ganti atau tambah dengan desain nyata saat tersedia.
4. Codegen ke Dioxus 0.7.10 (`src/designir/dioxus_codegen.rs`): ada, dikompilasi dengan feature `designir-dioxus`. Render dan uji runtime belum ada.
5. Round trip struktural ke modul Rust (ID, hierarki, kind, scope, nilai token) pada korpus: 100% (sudah diukur, lulus). Round trip ke komponen Dioxus belum ada.

Keluar: round trip struktural 100% pada korpus 20 fixture melalui adapter; aturan semantik di Rust lulus uji negatif.

Keputusan delegasi: format token DTCG, sumber desain JSON Plaesy, target Rust/Dioxus (lihat `docs/platform/decisions.md`).

## Fase 4: api-ir/1

Masuk: Fase 1 keluar (handler adalah flow).

Kerja:

1. `domain/api`: aturan di ADR 0005 bagian "Aturan semantik", termasuk cek input flow dan output flow terhadap parameter dan respons.
2. Generator OpenAPI 3.1 (keputusan di ADR 0005). Pin versi OpenAPI 3.1.x yang dipakai setelah dicek di halaman rilis resmi.
3. Validasi dokumen OpenAPI yang dihasilkan dengan validator OpenAPI yang dipilih.
4. Uji: endpoint dengan `flow_id` yang tidak ada ditolak.

Keluar: contoh `orders.json` dan `auth-and-errors.json` menghasilkan OpenAPI 3.1 yang lolos validator.

Keputusan owner: versi API (`/v1`, `/v2`) dan pemakaian `base_path` baru untuk perubahan tidak kompatibel.

## Fase 5: app-ir/1

Masuk: Fase 3 dan Fase 4 keluar (app merujuk desain dan flow).

Kerja:

1. `domain/app`: aturan di ADR 0006 bagian "Aturan semantik".
2. Validasi lintas-file: `design_ref` (`dsg_<id>` atau `dsg_<id>#cp_<id>`) harus ada di desain yang dimuat. Ini menjadi langkah di CI (ADR 0006 acceptance 4).
3. Uji pratinjau: aksi memanggil flow yang benar, dengan transport palsu.

Keluar: contoh `order-desk.json` dan `login-and-stock.json` lolos validasi lintas-file dan uji pratinjau.

Keputusan owner: daftar tipe komponen (`page`, `table`, `button`, dan lainnya) dan propertinya.

## Fase 6: impor

Masuk: Fase 2 keluar untuk `import-sql`; Fase 1 keluar untuk `import-code-rust`.

Kerja:

1. `import-sql`: introspeksi skema database contoh menjadi `data-ir/1`. Uji: impor lalu ekspor ulang menghasilkan IR yang sama (ADR 0008 acceptance 2).
2. `import-code-rust`: parser `syn` atau parser resmi (usulan ADR 0008, belum disetujui). Laporan pemetaan dan kehilangan sesuai ADR 0009 bagian 4.
3. Uji konstruksi tidak didukung: harus muncul di laporan sebagai `opaque`, tidak hilang.
4. Bahasa kedua dimulai hanya setelah bahasa pertama lulus (ADR 0008 acceptance 4).

Keluar: impor-ekspor-impor idempoten untuk contoh yang didukung; tidak ada konstruksi hilang tanpa tercatat.

Keputusan owner: bahasa pertama untuk `import-code-*`; parser; apakah impor database didahulukan.

## Fase 7: klien

Masuk: Fase 1 keluar; ADR 0011 disetujui.

Kerja:

1. Ganti nama `crates/editor/flow` (`flow-editor`) menjadi `plaesy-interface-client`, pindahkan ke `crates/plaesy/interface/client/`. Perbarui referensi di `CLAUDE.md` dan task backlog.
2. Buat struktur di ADR 0011 bertahap: `shell`, `ui`, `state`, `adapter`, `shared`, lalu `features/flow`.
3. Tambah pemeriksaan `use` antar-modul (ADR 0011, bagian "Konfirmasi") sebagai langkah CI. Bentuk pemeriksaan (skrip atau lint) dipilih saat fase ini.
4. Transport WebSocket: pilih library per platform (ADR 0011 open items).

Keluar: `cargo build` untuk fitur `web`, `desktop`, `mobile`; pemeriksaan `use` lulus di CI.

Keputusan owner: struktur `shared/`; library WebSocket; pemecahan fitur ke crate sendiri (kapan).

## Risiko

| Risiko | Dampak | Mitigasi |
|---|---|---|
| Keputusan owner belum ada untuk format token dan flow | Fase 1 dan 3 tertunda | Fase 0 meminta keputusan secara eksplisit; kerja yang tidak bergantung dilanjutkan |
| Schema akar tertutup (ADR 0009) mematahkan dokumen yang sudah ada | Dokumen lama ditolak | Contoh di `examples/` sudah diperbarui dan lulus; migrasi tidak diperlukan selama MAJOR sama |
| Konvensi `extensions` tidak diterapkan di validator Rust | Field tambahan hilang saat tulis ulang | Uji round trip dengan `extensions` wajib di Fase 1 |
| Pemeriksaan CI belum ada sehingga klaim tidak tervalidasi | ADR menjadi klaim kosong | Fase 0 adalah gerbang: tidak ada fase lain yang masuk tanpa CI |
| Workspace glob gagal | Build tidak jalan | `members` eksplisit (ADR 0003, langkah 2) |

## Open items

- Owner menyetujui rencana ini dan urutan fase.
- Owner menyetujui ADR 0009 dan 0010 (lihat masing-masing bagian Open items).
- Setiap keputusan owner di tiap fase dicatat di ADR yang bersangkutan dengan tanggal.
