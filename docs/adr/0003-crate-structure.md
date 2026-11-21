# ADR 0003: Struktur folder dan crate Rust (backend)

- Status: diusulkan (belum disetujui). Bagian struktur klien dipindah ke ADR 0011.
- Tanggal: 2026-11-21
- Pengambil keputusan: owner (belum ada persetujuan)
- Berlaku untuk: crate Rust backend, `domain/*`, `service`, `adapter/*`, `interface/*`
- Menggantikan: -
- Digantikan oleh: -
- Terkait: ADR 0002, 0004, 0005, 0006, 0008, 0009, 0010, 0011

Cara membaca dokumen ini: bagian "Keputusan" berisi keputusan yang akan diimplementasi. Bagian "Considered options" menjelaskan alternatif yang ditolak dan alasannya. Bagian "Confirmation" menjelaskan cara memeriksa bahwa implementasi mengikuti keputusan ini.

## Ringkasan

- Backend dibagi per lapisan di bawah `crates/plaesy/`: `kernel`, `domain/<konteks>`, `dto`, `service`, `adapter/<nama>`, `interface/*`, `plugin`.
- `domain/*` tidak boleh bergantung pada I/O. Ini diperiksa dengan `cargo tree` setelah crate ada.
- Struktur klien dipindah ke ADR 0011.
- Status: diusulkan.

## Konteks

Plaesy menggantikan banyak produk (Hermes Agent, Paperclip, DeepSeek Harness, Nimbalyst, Orca, Craft Anything, FigCraft, DeckCraft, WordCraft, Herald OS, OpenProject) dan menjadi satu kesatuan. Cakupannya:

- desain visual (Figma),
- flow dan logika tombol atau endpoint (Windmill),
- pembuatan aplikasi dari layar dan komponen (FlutterFlow),
- visual untuk data: skema tabel, relasi, dan query,
- konversi dua arah antara visual dan kode.

Struktur harus memisahkan aturan bisnis, penyimpanan, transport, DTO, dan klien, serta mencegah dependensi yang melanggar arah.

Repo saat ini: satu package Rust di `src/` (355 file) dan crate skeleton flow editor di `crates/editor/flow` (nama paket `flow-editor`). Crate ini akan dipindah ke `crates/plaesy/interface/client/` sesuai ADR 0011.

## Dorongan keputusan

- Satu model domain dan satu identitas untuk semua kapabilitas.
- Visual dan kode adalah dua tampilan dari satu model, lewat IR.
- Aturan dependensi ditegakkan compiler. Aturan di dalam crate klien ada di ADR 0011.
- Semua crate berada di bawah `crates/plaesy/`, dengan nama paket diawali `plaesy-`.
- Pengelompokan per lapisan dan per konteks domain, bukan per produk.
- DTO tidak berada di bawah transport tertentu.
- Struktur klien berbasis fitur, dengan ukuran besar sejak awal agar tidak perlu dipindah.

## Keputusan: struktur level atas

```text
cli/
├── Cargo.toml                       virtual manifest: [workspace], [workspace.dependencies], [workspace.lints]
└── crates/
    └── plaesy/
        ├── kernel/                  plaesy-kernel
        ├── domain/<konteks>/        plaesy-domain-<konteks>
        ├── dto/                     plaesy-dto
        ├── service/                 plaesy-service
        ├── adapter/<nama>/          plaesy-adapter-<nama>
        ├── interface/
        │   ├── cli/                 plaesy-interface-cli
        │   ├── http/                plaesy-interface-http
        │   ├── mcp/                 plaesy-interface-mcp
        │   ├── ws/                  plaesy-interface-ws
        │   ├── acp/                 plaesy-interface-acp
        │   └── client/              plaesy-interface-client (satu crate; lihat bagian klien)
        ├── plugin/
        │   └── sdk/                 plaesy-plugin-sdk
        └── plaesy/                  plaesy (facade)
```

### Konteks domain

| Konteks | Isi |
|---|---|
| `workflow` | task dan run |
| `flow` | IR flow, node, edge, aturan semantik, inti interpreter (tanpa I/O) |
| `app` | layar, komponen, binding event (tombol → flow) |
| `api` | endpoint: method, path, DTO request dan response, auth, flow yang menjadi isinya |
| `data` | skema: tabel, kolom, relasi; IR query |
| `design` | frame, komponen, variant, token |
| `memory` | pengetahuan, pencarian percakapan |
| `collaboration` | agent, sesi, pesan, lease, usulan perubahan, kehadiran pengguna |
| `identity` | pengguna, peran, izin, persetujuan, anggaran, audit |
| `messaging` | notifikasi, kanal |
| `automation` | rutinitas, jadwal, heartbeat |
| `documents` | teks dan presentasi |

Konteks `flow`, `app`, `api`, `data`, dan `design` mengikuti pola yang sama: IR netral, dengan visual dan kode sebagai dua tampilan.

| Konteks | IR | ADR | Skema dan contoh |
|---|---|---|---|
| `flow` | `flow-ir/1` | 0002 | `schemas/flow-ir-1.schema.json`, `examples/flow-ir-1/` |
| `data` | `data-ir/1` | 0004 | `schemas/data-ir-1.schema.json`, `examples/data-ir-1/` |
| `api` | `api-ir/1` | 0005 | `schemas/api-ir-1.schema.json`, `examples/api-ir-1/` |
| `app` | `app-ir/1` | 0006 | `schemas/app-ir-1.schema.json`, `examples/app-ir-1/` |
| `design` | `design-ir/1` | 0007 | `schemas/design-ir-1.schema.json`, `examples/design-ir-1/` |
| impor kode ke visual | (memakai IR di atas) | 0008 | belum ada skema |

Validasi semantik untuk kelima IR: `tools/validate-ir.js` (prototipe Node).

### Adapter

| Adapter | Fungsi |
|---|---|
| `sqlite` | penyimpanan utama, model persistence privat |
| `runtime` | proses lokal, supervisor, worktree, menjalankan langkah yang memanggil proses |
| `http-client` | langkah flow yang memanggil HTTP |
| `relay` | transport pesan antar-agent |
| `sync` | sinkronisasi real-time untuk kolaborasi multi-pengguna |
| `git` | git dan diff |
| `providers` | penyedia model AI |
| `integrations` | GitHub, Linear, OpenProject, dll. |
| `plugin-host` | memuat plugin dengan batas izin |
| `codegen-rust`, `codegen-go`, `codegen-js` | IR flow dan app menjadi kode, satu adapter per bahasa |
| `codegen-sql` | IR data menjadi DDL dan query SQL |
| `import-sql` | skema database yang ada menjadi IR data (kode ke visual) |
| `import-code-<bahasa>` | kode sumber menjadi IR (kode ke visual), ditambahkan per bahasa (ADR 0008) |

## Keputusan: struktur di dalam crate

Aturan umum: satu modul per konteks, dan nama modul sama di setiap crate yang menyentuh konteks itu.

### domain/flow

```text
domain/flow/src/
├── lib.rs
├── ir/          node, edge, input, output, tipe (versi flow-ir/1)
├── semantic/    aturan eksekusi: urutan, retry, batas loop, outcome kegagalan
├── eval/        inti interpreter murni: kondisi, transformasi, tanpa I/O
├── port/        trait untuk langkah yang butuh I/O (dipanggil dari service)
└── error.rs
```

Inti `eval/` tidak memakai I/O, sehingga bisa dikompilasi ke WebAssembly dan dijalankan di klien maupun server.

### domain/app dan domain/api

```text
domain/app/src/
├── lib.rs
├── screen/       layar, navigasi
├── component/    komponen, properti, variant
└── binding/      event → flow, data → komponen

domain/api/src/
├── lib.rs
├── endpoint/     method, path, auth
├── schema/       referensi DTO request dan response
└── handler/      referensi flow yang menjadi isi endpoint
```

### domain/data

```text
domain/data/src/
├── lib.rs
├── schema/       tabel, kolom, tipe, kunci
├── relation/     relasi antar-tabel: satu-ke-banyak, banyak-ke-banyak
├── query/        IR query: sumber, filter, join, proyeksi, urutan
└── error.rs
```

### dto

```text
dto/src/
├── lib.rs
├── common.rs
├── flow.rs       bentuk flow untuk editor
├── app.rs        bentuk layar dan komponen
├── api.rs        bentuk endpoint
├── data.rs       bentuk skema dan query
├── design.rs     bentuk desain
├── workflow.rs
├── memory.rs
├── collaboration.rs
├── identity.rs
├── messaging.rs
├── automation.rs
├── documents.rs
├── event.rs      event ws
└── message.rs    pesan antar-agent dan envelope relay
```

### service

```text
service/src/
├── flow/         use case: jalankan flow, validasi, orkestrasi I/O
├── app/          use case: simpan dan muat aplikasi, generate
├── api/          use case: dispatch request ke handler flow
├── data/         use case: introspeksi, validasi relasi, eksekusi query
├── design/
├── workflow/
├── memory/
├── collaboration/
├── identity/
├── messaging/
├── automation/
├── documents/
├── port/         trait: storage, relay, runtime, http-client, sync, codegen
├── plugin/       kontrak plugin
└── error.rs
```

### interface/http

```text
interface/http/src/
├── lib.rs
├── app.rs
├── routes/
│   ├── mod.rs
│   ├── system.rs
│   ├── workflow.rs
│   ├── flow.rs        /v1/flows
│   ├── app.rs         /v1/apps
│   ├── data.rs        /v1/data
│   ├── design.rs      /v1/designs
│   ├── memory.rs
│   ├── collaboration.rs
│   ├── identity.rs
│   ├── messaging.rs
│   └── automation.rs
├── dynamic/           /v1/endpoints/{app}/{path}: diteruskan ke service/api
├── handlers/
├── extract/
├── error.rs
├── middleware/
├── openapi.rs
└── tests/
```

Route dinamis untuk endpoint yang dibuat pengguna hanya meneruskan request ke `service/api`. Tidak ada logika bisnis di sana.

### interface/ws, mcp, acp, cli

Pola sama dengan `interface/http`: `lib.rs`, folder per konteks atau per tipe pesan, `error.rs`, dan `tests/`. `cli` tetap satu-satunya perakit adapter.

### interface/client

Dipindah ke ADR 0011 (struktur klien). Crate `plaesy-interface-client` mengikuti aturan di sana.

### plugin/sdk dan plaesy (facade)

Pola sama seperti sebelumnya.

## Opsi yang dipertimbangkan

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. Satu crate besar dengan modul | Build cepat, tanpa koordinasi antar-crate | Aturan dependensi hanya konvensi | Ditolak: aturan lapisan harus ditegakkan |
| B. Flat `crates/plaesy-*` (rekomendasi matklad) | Sederhana, nama folder sama dengan nama crate | Tidak mengelompokkan lapisan; banyak crate sejajar | Ditolak: lapisan dan konteks tidak terlihat |
| C. Berkelompok per lapisan di bawah `crates/plaesy/` (dipilih) | Lapisan dan konteks terlihat; nama paket `plaesy-*` tetap unik di crates.io | Path lebih dalam; crate lebih banyak | Dipilih |
| D. Dioxus fullstack (satu framework untuk server dan klien) | Satu bahasa dan satu toolchain | Klien terikat ke runtime server; `service` mudah bocor ke klien | Ditolak |
| E. Klien sebagai repo terpisah | Rilis klien independen | Kontrak API harus disinkronkan di dua repo | Ditunda: klien tetap di monorepo sampai ada alasan rilis terpisah |

Riset yang mendasari pilihan: batas crate sebagai penegak dependensi oleh compiler (lihat bagian "Dorongan keputusan"); flat vs berkelompok ([matklad: Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)).

## Konfirmasi

Cara memeriksa implementasi mengikuti ADR ini:

- `cargo tree -p plaesy-domain-<konteks>` tidak memuat `tokio`, `axum`, `rusqlite`, atau crate I/O lain. Belum bisa diperiksa: crate `domain/*` belum ada.
- `cargo metadata` menunjukkan setiap crate berada di bawah `crates/plaesy/` dan namanya diawali `plaesy-`.
- Validasi IR: `ajv validate` untuk skema bentuk, dan `tools/validate-ir.js` untuk semantik (prototipe, akan digantikan `domain/*`).

## Implementation checklist

Urutan yang disarankan. Setiap langkah harus lulus `cargo build` dan `cargo test` di workspace sebelum lanjut.

1. Setujui ADR ini (owner). Tanpa ini, langkah 2 ke bawah tidak dimulai.
2. Buat root `Cargo.toml` sebagai virtual manifest dengan daftar `members` eksplisit, satu baris per crate. Glob seperti `crates/plaesy/*` tidak dipakai: folder kontainer seperti `domain/` tidak punya `Cargo.toml`, dan `cargo metadata` gagal (diuji di workspace scratch, 2026-11-21). Contoh awal: `members = ["crates/plaesy/kernel", "crates/plaesy/dto", "crates/plaesy/domain/flow"]`.
3. Buat `kernel` dan `dto` kosong yang bisa dibuild.
4. Buat `domain/flow` dengan modul `ir`, `semantic`, `eval`, `port`, dan `error`. Pindahkan validasi dari `tools/validate-ir.js` untuk `flow-ir/1` sebagai uji.
5. Ulangi langkah 4 untuk `data`, `api`, `app`, dan `design`, satu konteks per langkah.
6. Buat `service` dengan modul per konteks dan port.
7. Buat adapter yang dibutuhkan lebih dulu: `adapter/sqlite`, lalu `adapter/http-client`, `adapter/runtime`, `adapter/relay`.
8. Buat `interface/http` dan `interface/cli` dengan satu endpoint sebagai contoh, lalu tambah endpoint lain.
9. Buat klien: `interface/client` dengan fitur Cargo `web`, `desktop`, `mobile`. Mulai dari `shell`, `ui`, dan satu fitur (`features/flow`).
10. Pindahkan kode dari `src/` secara bertahap per modul, dengan build dan test di setiap langkah. Jangan memindahkan semua sekaligus.

Setiap langkah yang melanggar aturan dependensi di bagian "Konfirmasi" harus diperbaiki sebelum lanjut.

## Visual ↔ kode

Setiap arah dibuat sebagai pasangan adapter yang membaca dan menulis IR yang sama:

| Konteks | Visual → kode | Kode → visual |
|---|---|---|
| Flow dan logika | `codegen-*` | `import-code-*` (ADR 0008) |
| App (layar dan komponen) | `codegen-*` (UI) | `import-code-*` (ADR 0008) |
| API (endpoint) | `codegen-*` (server) | `import-code-*` (ADR 0008) |
| Data (tabel dan relasi) | `codegen-sql` | `import-sql` |
| Query | `codegen-sql` | `import-sql` (dari view dan query tersimpan) |

Aturan konversi:

- Konstruksi yang tidak bisa dipetakan ke IR disimpan sebagai blok buram dengan lokasi sumbernya. Tidak ada yang dibuang diam-diam. Ini meneruskan prinsip "explicit loss" di ADR 0002.
- Kode yang diimpor dan kemudian diekspor ulang harus menghasilkan IR yang sama. Ini diuji di uji konformansi.

## Simulasi

| Kondisi | Hasil |
|---|---|
| Tombol baru dengan aksi panggil dua API berurutan | Lulus: `domain/app` (binding), `domain/flow`, dan `domain/api` yang dipanggil |
| Endpoint baru dibuat dari editor | Lulus: `domain/api`, `service/api`, route dinamis di `interface/http` |
| Ekspor flow ke Rust dan Go | Lulus: `codegen-rust` dan `codegen-go`, memakai IR yang sama |
| Interpreter di klien dan hasil codegen berbeda | Gagal uji konformansi |
| Visual relasi tabel menghasilkan DDL | Lulus: `domain/data` dan `codegen-sql` |
| Skema database yang ada diimpor ke visual | Lulus: `import-sql` ke `domain/data` |
| Domain flow mengimpor HTTP atau database | Gagal build; I/O hanya lewat port |
| Klien memanggil `service` langsung | Gagal build (aturan target; belum diperiksa) |
| Fitur memanggil fitur lain secara langsung | Gagal build (aturan target; belum diperiksa) |
| Renderer aktif di `ui` | Gagal build (aturan target; belum diperiksa) |
| Fitur baru (misalnya inventaris) | Lulus: tambah `features/<nama>` dengan pola yang sama |
| Editor flow membesar | Lulus: `editor/` dan `shared/canvas/` tumbuh tanpa pindah folder |
| Ganti library WebSocket di web (misalnya dari web-sys ke gloo) | Lulus: hanya `adapter/ws/transport/web` berubah |
| Tambah platform baru (misalnya TV dengan transport sendiri) | Lulus: tambah `adapter/ws/transport/<nama>` yang mengimplementasikan `port` |
| Uji reconnect dan urutan event tanpa jaringan | Lulus: `adapter/ws/tests` memakai Transport palsu |
| Protokol v2 dengan format pesan baru | Lulus: tambah `adapter/ws/protocol/v2`, `v1` tetap berjalan selama masa transisi |
| Kebijakan backoff diubah | Lulus: hanya `protocol/v1` (atau versi baru) |
| Fitur data menampilkan diff skema antar-versi | Lulus: `shared/diff/ir` dan `ui/data_display` untuk tampilannya |
| Tabel data besar dengan virtualisasi | Lulus: komponen di `ui/data_display`; logika data tetap di fitur |
| Mesin non-produk baru (misalnya markdown) | Lulus: tambah `shared/<nama>` dengan aturan yang sama |
| Platform baru (misalnya TV) | Lulus: tambah `platform/tv` |
| Fitur dipecah jadi crate sendiri | Lulus: pindahkan `features/<nama>` ke crate sendiri, fitur lain tidak berubah |
| Konstruksi kode tidak didukung saat impor | Disimpan sebagai blok buram, tidak hilang |

## Konsekuensi

- Sekitar 30 crate saat semua konteks dan adapter terisi. Crate baru dibuat hanya saat ada kebutuhan nyata.
- Klien satu crate dengan struktur besar. Ukuran build klien bertambah seiring fitur, dan pemecahan ke crate sendiri disiapkan lewat aturan dependensi.
- Interpreter murni di `domain/flow` menjadi komponen paling kritis. Ia harus punya uji konformansi terhadap codegen.
- Sinkronisasi real-time (`adapter/sync`) dibutuhkan untuk kolaborasi, dan keputusan teknologinya belum dibuat.
- Pengelompokan per konteks mengikuti konsep, bukan produk. Produk lama dipetakan ke konteks ini.
- Topologi deployment dan matriks OS tidak mengubah struktur crate.

## Kriteria penerimaan (implementasi struktur)

1. `cargo build --workspace` dan `cargo test --workspace` lulus pada setiap langkah di "Implementation checklist".
2. `cargo tree` untuk setiap `domain/*` tidak memuat `tokio`, `axum`, `rusqlite`, atau crate I/O lain.
3. Setiap crate berada di bawah `crates/plaesy/` dan namanya diawali `plaesy-`.

## Open items

- Owner menyetujui batas layering di bagian "Keputusan: struktur di dalam crate" dan "Decision: struktur level atas".
- Urutan pengerjaan: flow lebih dulu (ADR 0002 sudah diterima), lalu data, desain, dan aplikasi. Urutan ini belum disetujui owner.
- Nama paket `flow-editor` di-rename menjadi `plaesy-interface-client` (modul `features/flow`). Referensi di `CLAUDE.md` dan task backlog ikut diperbarui saat rename.
- `plugins/hermes-plugin/` di root adalah integrasi Python untuk runtime Hermes. Usulan: tetap di luar pohon crate, dan dipindah ke `integrations/hermes/` hanya jika owner memutuskan.
- Validasi semantik untuk kelima IR sudah ada sebagai prototipe Node. Implementasi di `domain/*` dalam Rust tetap diperlukan.
- IR yang sudah ada dan statusnya: `flow-ir/1` (ADR 0002), `data-ir/1` (ADR 0004), `api-ir/1` (ADR 0005), `app-ir/1` (ADR 0006), dan `design-ir/1` (ADR 0007). Semuanya memiliki skema dan contoh yang lolos ajv, tapi belum disetujui owner.
- Impor kode ke visual (ADR 0008) belum punya skema dan contoh.
