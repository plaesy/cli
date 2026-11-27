# Rencana: paritas klien Plaesy (web dulu, desktop dan mobile menyusul)

Sumber: ADR 0011 (`docs/adr/0011-client-structure.md`), `docs/platform/product.md` (Web dan Desktop), `docs/platform/remaining-gaps.md`, `docs/platform/decisions.md` (bagian keputusan teknis 2026-11-22), `docs/platform/feature-inventory/cross-product-priorities.md` (baris 16, 21), dan kode `crates/plaesy/interface/client`.

Status per slice dicatat di bagian Status. Satu slice = satu area berkas, satu atau beberapa commit kecil, dan satu bullet CHANGELOG. Setiap slice diverifikasi oleh cli-113 per SHA. Sesi cli-125 tidak menjalankan Cargo, dx, build wasm, atau Playwright.

## Inventaris (terverifikasi ke kode kecuali ditandai dugaan)

| Area | Item | Status | Bukti |
|---|---|---|---|
| Shell | Route dan guard (murni) | ada, belum terhubung ke sesi | `shell/mod.rs` |
| Shell | Layout, sidebar, breadcrumb | sebagian (topbar dan tab Flow/Data) | `platform/web/mod.rs` |
| Shell | Error boundary | belum | - |
| State | Session (login) | ada murni, belum ada UI | `state/session.rs` |
| State | Preferences (tema, bahasa, layout) | ada murni, belum ada UI | `state/preferences.rs` |
| State | Realtime bus | ada murni, belum ada transport web | `state/realtime.rs`, `adapter/ws/port.rs` |
| State | Notifikasi | ada murni, belum ada UI | `state/notifications.rs` |
| Adapter | HTTP (builder, execute) | ada; transport web belum | `adapter/http/mod.rs` |
| Adapter | WS (protokol) | ada; transport web dan native belum | `adapter/ws/protocol/mod.rs` |
| Flow | Model, layout, reducer, document | ada | `features/flow/*` |
| Flow | Editor visual | sebagian: edit lokal di memori sesi | `platform/web/mod.rs` |
| Flow | Hubungkan input (Connect) di inspector | belum (reducer ada) | `features/flow/reducer.rs` |
| Flow | Validasi lokal | ada; tampil di panel Masalah | `features/flow/model.rs` |
| Flow | Validasi lewat server (`/v1/validate`) | belum | `interface/http` |
| Flow | Riwayat run dan codegen | belum (server `/v1/codegen` ada) | `interface/http` |
| Dokumen | Store sesi (`MemoryStore`) | ada; tidak persisten | `features/store.rs` |
| Dokumen | Simpan dan muat lewat `/v1/documents` | belum (server belum dijalankan oleh perintah apa pun) | `interface/http/src/lib.rs`, `adapter/sqlite` |
| Data | Model dan reducer | ada; UI belum | `features/data/*` |
| Design | Editor atau daftar | belum (validator ada di `adapter/design-ir`) | - |
| App | Editor atau daftar | belum (IR ada di `domain/app`) | - |
| Workflow | Daftar run, kebijakan | belum (runstore ada di `domain/workflow`) | - |
| Memori | Pencarian dan catatan | belum UI (`adapter/memory`, MCP) | - |
| Kolaborasi | Presence, lease | belum | - |
| Identity | Peran, persetujuan | belum (domain ada) | - |
| Messaging | Pesan antar-agen | belum (relay ada) | - |
| Automation | Jadwal | belum (`adapter/memory/schedule`) | - |
| Canvas | Graf, viewport, history, layout | ada murni; editor belum memakainya | `shared/canvas/*` |
| Diff | Diff JSON, teks, changeset | ada murni; UI belum | `shared/diff/*` |
| UI | Komponen FigCraft | ada; belum dipakai halaman | `ui/components.rs` |
| UI | `flow_view` (HTML string) | ada; hanya dipakai tes | `ui/flow_view.rs` |
| Platform | Web | sebagian | `platform/web/mod.rs` |
| Platform | Desktop | belum (fitur `desktop` kosong, tidak ada folder) | `Cargo.toml` |
| Platform | Mobile | belum (fitur `mobile` kosong, tidak ada folder) | `Cargo.toml` |

Dugaan (tidak diverifikasi di kode): fitur mobile dan remote butuh keputusan arsitektur dan jalur jaringan (`remaining-gaps.md`), dan editor kanvas butuh keputusan interaksi (drag, koneksi, panel properti).

## Keputusan yang masih terbuka (perlu pemilik)

1. **Renderer desktop**: `decisions.md` mencatat "UI and Rust boundary beyond decided Dioxus web surface" sebagai UNKNOWN tanpa pilihan yang didelegasikan. Slice desktop menunggu pilihan ini.
2. **Renderer dan jalur jaringan mobile**: `remaining-gaps.md` menyatakan fitur mobile dan remote melanggar aturan offline kecuali disetujui. CLAUDE.md sudah mengizinkan jaringan dengan opt-in per jalur, dicatat di `decisions.md`. Slice mobile menunggu pilihan renderer dan jalur.
3. **Interaksi editor kanvas**: drag, koneksi, dan panel properti belum didefinisikan (`remaining-gaps.md`). Slice S1 memakai interaksi yang sudah ada (palette, inspector, reducer) dan tidak menambah drag.

## Slice (berurutan)

**S0. Verifikasi commit yang sudah ada.** Area: tidak ada. Kriteria: cli-113 melaporkan fmt, clippy `--all-targets`, test, check dan clippy dengan `--features web`, dan `dx build` untuk HEAD. Status: menunggu hasil untuk `3ffa70e` dan turunannya.

**S1. Editor lengkap di sesi (tanpa jaringan).** Area: `platform/web/mod.rs`, `features/flow/model.rs`, `features/flow/reducer.rs`.

- Tambah Connect di inspector: pilih input dan sumber, lalu `Action::Connect`. Selesai bila dua input bisa disambung dan validasi melaporkan sumber yang tidak ada.
- Tanda di kanvas untuk modul yang bermasalah dari `validate`.
- Tes murni untuk pilihan input dan tanda masalah, di `features/flow/connect_test.rs` (atau `model_test.rs`).
- Cek: `cargo test -p plaesy-interface-client --locked`, clippy `--features web`, fmt.

**S2a. Server dokumen (worker terpisah, bisa di-spawn).** Area: `interface/cli` (perintah `plaesy serve http` atau nama yang disetujui), `interface/http` (CORS untuk origin loopback). Kriteria: perintah menjalankan router `plaesy-interface-http` dengan `SqliteStore` di path data lokal, bind hanya ke loopback secara default, CORS hanya untuk origin yang dikonfigurasi, dan tes menyentuh `/v1/health` dan `PUT/GET /v1/documents/flow-ir%2F1/{id}`. Catatan: perubahan jaringan butuh entri di `decisions.md`.

**S2b. Transport HTTP web dan simpan/muat.** Area: `adapter/http` (transport web), `platform/web/mod.rs`. Kriteria: Save draft dan buka flow lewat `/v1/documents`, dengan fallback ke `MemoryStore` bila server tidak ada dan pesan status yang jelas. Catatan: transport butuh `fetch`, yaitu dependensi baru (`web-sys` atau `gloo-net`). Perubahan `Cargo.lock` hanya bisa dibuat oleh cli-113 (tanpa `--locked`). Trait `Transport` yang sinkron tidak cocok dengan fetch yang async, jadi perlu jalur async terpisah di platform web.

**S3. Validasi lewat server.** Area: `adapter/http`, `platform/web`. Kriteria: tombol Validate memanggil `/v1/validate` bila server ada, dan memakai validasi lokal bila tidak. Bergantung pada S2b.

**S4. Shell dan layout.** Area: `shell/`, `platform/web/mod.rs`. Kriteria: sidebar navigasi, breadcrumb, halaman error boundary, halaman "Akses ditolak" dari guard, dan URL browser yang sinkron dengan `shell::route`. Sinkron URL butuh `web-sys` (lihat S2b).

**S5. Halaman data.** Area: `features/data/` (pages, services), `platform/web`. Kriteria: daftar dan editor data memakai `features/data/reducer` dan store. Tes murni untuk reducer yang belum ada.

**S6. Realtime web.** Area: `adapter/ws/transport/web`, `state/realtime`. Kriteria: koneksi WebSocket ke `interface/ws` lewat trait `Transport`, dengan tes uji reconnect lewat transport palsu. Server `interface/ws` belum dijalankan perintah apa pun, jadi butuh perintah server seperti S2a.

**S7. Notifikasi, preferensi, dan sesi.** Area: `state/`, `ui/`, `platform/web`. Kriteria: toast dari `notifications`, pengaturan tema dan bahasa dari `preferences`, dan layar masuk sederhana untuk `session`.

**S8. Tampilan diff.** Area: `shared/diff`, `ui/`. Kriteria: panel "Lihat perubahan" yang menampilkan changeset antara dokumen yang dimuat dan hasil edit. Riwayat versi di server belum ada, jadi cakupannya hanya perubahan lokal.

**S9. Fitur domain lainnya (satu slice per fitur, bergantung pada server).** Design, App, Workflow, Memori, Kolaborasi, Identity, Messaging, Automation. Kriteria per fitur: daftar read-only dari dokumen atau API yang sudah ada. Fitur yang tidak punya API atau dokumen (misalnya kolaborasi, presence) ditunda dan dicatat di `remaining-gaps.md`.

**S10. Desktop.** Area: `platform/desktop/`, fitur `desktop` di `Cargo.toml`. Blokir: keputusan renderer (lihat Keputusan 1). Tidak dimulai sebelum pemilik memutuskan.

**S11. Mobile.** Area: `platform/mobile/`, fitur `mobile`. Blokir: keputusan renderer dan jalur jaringan (lihat Keputusan 2). Tidak dimulai sebelum pemilik memutuskan.

## Urutan dan paralelisme

- S0 → S1 → S2b (di satu berkas `platform/web`, dikerjakan berurutan oleh cli-125).
- S2a independen (interface/cli dan interface/http), bisa di-spawn ke worker.
- S4 dan S5 menyentuh `platform/web` dan `shell`, jadi berurutan setelah S2b.
- S6 bergantung pada S2a-like server ws.
- S10 dan S11 menunggu keputusan pemilik.

## Verifikasi per slice (dijalankan cli-113)

- `cargo fmt --all --check`
- `cargo clippy -p plaesy-interface-client --all-targets --locked -- -D warnings`
- `cargo test -p plaesy-interface-client --locked`
- `cargo clippy -p plaesy-interface-client --lib --features web --locked -- -D warnings`
- `dx build --platform web --package plaesy-interface-client --example web --features web` bila disk dan RAM cukup
- Tes visual `tests/web_visual_test.rs` dengan `PLAESY_WEB_URL` (dijalankan cli-113), bila server dev hidup

## Status

- S0: menunggu hasil cli-113 untuk `3ffa70e` (`235a789` dan `a1e2d80` adalah perbaikan format).
- S1: kode selesai (Connect di inspector, tanda modul di kanvas lewat `features/flow/inspect.rs`, tes di `inspect_test.rs`). Menunggu verifikasi cli-113.
- S8 (dimajukan, tanpa server): kode selesai (`features/flow/changes.rs`, tes di `changes_test.rs`, panel "Perubahan belum disimpan" di inspector). Dibatasi pada perubahan lokal terhadap dokumen yang dimuat. Menunggu verifikasi cli-113.
- S2a–S11: belum dimulai. S10 dan S11 menunggu keputusan pemilik.
