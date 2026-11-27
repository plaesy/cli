# Rencana: paritas klien Plaesy (web dulu, desktop dan mobile menyusul)

Status: draf. Sumber: ADR 0011 (`docs/adr/0011-client-structure.md`), `docs/platform/product.md`, `docs/platform/remaining-gaps.md`, `docs/platform/decisions.md` (bagian "Keputusan klien (2026-11-27)"), `docs/platform/feature-inventory/cross-product-priorities.md`, dan kode `crates/plaesy/interface/client`.

Satu slice = satu area berkas, satu atau beberapa commit kecil, tes murni di `<nama>_test.rs`, dan satu bullet CHANGELOG. Verifikasi dijalankan cli-113 per SHA, digabung per beberapa slice. Sesi cli-125 tidak menjalankan Cargo, dx, build wasm, atau Playwright.

## Keputusan yang dipakai (pemilik, 2026-11-27)

1. **Renderer desktop dan mobile: Dioxus.** Sama dengan web. Tercatat di `decisions.md`.
2. **Backend di server, bukan loopback.** Dibangun oleh cli-137 (`ao/backend-server/root`). Klien tidak membuat server dokumen dan tidak mengubah `interface/http`.
3. **Autentikasi token per perangkat.** `Authorization: Bearer <token>`. Token dan base URL adalah konfigurasi, tidak di-hard-code.

Kontrak dari cli-137 (usulan, aditif, belum final menunggu persetujuan pemilik): rute tenant `/v1/projects/{project}/documents/{kind}/{id}` (D1 disetujui; rute lama mati saat autentikasi aktif), ETag dan If-Match (412 konflik, 428 bila If-Match wajib), galat 401, 403, 412, 413 (body > 1 MiB), 422 (IR tidak valid), 429 (Retry-After), `GET /v1/ready`, spec OpenAPI di `/v1/spec`, pairing kode sekali pakai (TTL 5 menit) menjadi token, dan CORS hanya untuk origin di allowlist. WebSocket ditunda sampai setelah slice 7 backend.

## Inventaris (terverifikasi ke kode kecuali ditandai dugaan)

| Area | Item | Status | Bukti |
|---|---|---|---|
| Shell | Route dan guard (murni) | ada; guard belum terhubung ke sesi | `shell/mod.rs` |
| Shell | Layout, sidebar, breadcrumb | sebagian (topbar dan tab Flow/Data) | `platform/web/mod.rs` |
| Shell | Error boundary | belum | - |
| State | Session (login, pairing) | ada murni; UI belum | `state/session.rs` |
| State | Preferences (tema, bahasa, layout) | ada murni; UI belum | `state/preferences.rs` |
| State | Realtime bus | ada murni; transport web belum | `state/realtime.rs`, `adapter/ws/port.rs` |
| State | Notifikasi | ada murni; UI belum | `state/notifications.rs` |
| Adapter | HTTP (builder, execute) | ada; belum mengikuti kontrak cli-137 (rute tenant, ETag, galat) | `adapter/http/mod.rs` |
| Adapter | WS (protokol) | ada; transport web belum | `adapter/ws/protocol/mod.rs` |
| Flow | Model, layout, reducer, document, inspect, changes | ada | `features/flow/*` |
| Flow | Editor visual (lokal, memori sesi) | sebagian | `platform/web/mod.rs` |
| Flow | Connect input di inspector | ada (S1) | `features/flow/inspect.rs` |
| Flow | Validasi lokal dan tanda di kanvas | ada (S1) | `features/flow/inspect.rs` |
| Flow | Validasi lewat server (`/v1/validate`) | belum | - |
| Flow | Riwayat run dan codegen | belum | - |
| Dokumen | Store sesi (`MemoryStore`) | ada; tidak persisten | `features/store.rs` |
| Dokumen | Simpan dan muat lewat server | belum (menunggu kontrak dan server) | - |
| Data | Baca dokumen `data-ir/1`, daftar, editor baca-saja | ada (S5) | `features/data/document.rs`, `platform/web/mod.rs` |
| Data | Edit tabel, kolom, relasi lewat reducer; simpan | belum | `features/data/reducer.rs` |
| Design | Daftar atau editor | belum (validator ada di `adapter/design-ir`) | - |
| App | Daftar atau editor | belum (IR di `domain/app`) | - |
| Workflow | Daftar run, kebijakan | belum (`domain/workflow`) | - |
| Memori | Pencarian dan catatan | belum UI (`adapter/memory`, MCP) | - |
| Kolaborasi | Presence, lease | belum | - |
| Identity | Peran, persetujuan | belum (`domain/identity`) | - |
| Messaging | Pesan antar-agen | belum (`adapter/relay`) | - |
| Automation | Jadwal | belum (`adapter/memory/schedule`) | - |
| Canvas | Graf, viewport, history, layout | ada murni; editor belum memakainya | `shared/canvas/*` |
| Diff | Perubahan lokal (S8) | ada | `features/flow/changes.rs` |
| UI | Komponen FigCraft | ada; belum dipakai halaman | `ui/components.rs` |
| Platform | Web | sebagian | `platform/web/mod.rs` |
| Platform | Desktop | belum (fitur `desktop` kosong, tanpa folder) | `Cargo.toml` |
| Platform | Mobile | belum (fitur `mobile` kosong, tanpa folder) | `Cargo.toml` |

Dugaan (tidak diverifikasi di kode): editor kanvas belum punya interaksi drag dan panel properti (`remaining-gaps.md`). Plan ini memakai interaksi yang sudah ada.

## Slice (urutan kerja)

**S0. Verifikasi.** Cli-113 memverifikasi HEAD terbaru per SHA (gabungan). Kriteria: fmt, clippy `--all-targets`, test client, check dan clippy `--features web`, dan `dx build`. Menunggu hasil untuk `bb271b0`, `60dcb56`, dan `53de09c` (perbaikan E0503). Tidak ada area berkas.

**S1. Editor di sesi.** Selesai secara kode: Connect, tanda kanvas, panel perubahan (S8), perbaikan E0503 (`53de09c`). Menunggu S0.

**S4a. Shell dan layout tanpa jaringan.** Area: `platform/web/mod.rs`, `shell/`. Kriteria: sidebar navigasi, breadcrumb, halaman error boundary untuk panic tampilan, dan halaman "Akses ditolak" memakai `Guard`. Tes: `shell_test.rs` untuk breadcrumb murni. Tidak butuh backend.

**S7. Notifikasi dan preferensi (UI).** Area: `state/`, `ui/`, `platform/web/mod.rs`. Kriteria: toast dari `state/notifications`, pengaturan tema dan bahasa lewat `state/preferences::reduce`. Tes: reducer tema dan bahasa sudah ada; tambah tes untuk pemetaan toast. Sesi (login) hanya formulir kode pairing dan token, belum memanggil server.

**S2c. Adapter HTTP sesuai kontrak (murni).** Area: `adapter/http/` (berkas baru `contract.rs`, `contract_test.rs`). Kriteria: konfigurasi klien (base URL, project, token) sebagai nilai biasa; pembentuk path `/v1/projects/{project}/documents/{kind}/{id}`; header Authorization dan If-Match; pemetaan galat 401, 403, 412, 413, 422, 429 (dengan Retry-After), dan 428. Tes tanpa jaringan. Nama rute final mengikuti persetujuan cli-137; sampai itu, ditandai provisional.

**S2b. Transport HTTP web dan simpan/muat.** Area: `adapter/http/` (transport web), `platform/web/mod.rs`. Kriteria: Save draft dan buka flow lewat server dengan `fetch` async, dengan fallback `MemoryStore` dan status yang jelas bila server tidak ada. Catatan: butuh dependensi baru (`web-sys` atau `gloo-net`); `Cargo.lock` diperbarui oleh cli-113. Bergantung pada S2c dan server cli-137 slice autentikasi (sebelum itu, hanya fallback).

**S5b. Edit data lewat reducer dan simpan.** Area: `features/data/` (document, reducer), `platform/web/mod.rs`. Kriteria: tambah tabel, kolom, dan relasi lewat `reducer::apply`; simpan dokumen data dengan If-Match. Bergantung pada S2b.

**S3. Validasi lewat server.** Area: `adapter/http/`, `platform/web/`. Kriteria: Validate memanggil `/v1/validate` bila server dapat dijangkau, dan memakai validasi lokal bila tidak. Bergantung pada S2b.

**S4b. URL browser sinkron dengan shell.** Area: `shell/`, `platform/web/`. Kriteria: `shell::route` dipakai untuk membaca dan menulis URL. Butuh `web-sys`, jadi dikerjakan bersama S2b (lewat cli-113).

**S6. Realtime web.** Area: `adapter/ws/transport/web`, `state/realtime`. Kriteria: transport WebSocket lewat trait `Transport`, dengan tes reconnect memakai transport palsu. Bergantung pada server ws cli-137 yang dijadwalkan setelah slice 7 backend. Menunggu jadwal.

**S9. Fitur domain (satu slice per fitur).** Design, App, Workflow, Memori, Kolaborasi, Identity, Messaging, Automation. Kriteria per fitur: daftar read-only dari API atau dokumen yang sudah tersedia. Fitur tanpa API ditunda. Bergantung pada kontrak cli-137 untuk tiap domain.

**S10. Desktop (Dioxus).** Area: `platform/desktop/`, fitur `desktop` di `Cargo.toml`. Dikerjakan setelah S1 sampai S5 dan kontrak final. Token per perangkat dan base URL dikonfigurasi, tidak di-hard-code.

**S11. Mobile (Dioxus).** Area: `platform/mobile/`, fitur `mobile`. Sama dengan S10.

## Slice yang independen dari backend

- Bisa dikerjakan sekarang: S4a, S7, S2c (murni). Semuanya sebelum S2b.
- Bergantung backend: S2b, S3, S4b, S5b, S6, S9, S10, S11.
- Di luar klien (cli-137, bukan worker dari sini): server dokumen, CORS allowlist, perintah `plaesy serve`, autentikasi, WS server, dan `GET /v1/ready`.

## Verifikasi per slice (cli-113)

- `cargo fmt --all --check`
- `cargo clippy -p plaesy-interface-client --all-targets --locked -- -D warnings`
- `cargo test -p plaesy-interface-client --locked`
- `cargo check -p plaesy-interface-client --features web --locked` dan `cargo clippy -p plaesy-interface-client --lib --features web --locked -- -D warnings`
- `dx build --platform web --package plaesy-interface-client --example web --features web` bila disk dan RAM cukup
- Tes visual `tests/web_visual_test.rs` dengan `PLAESY_WEB_URL` bila server dev hidup. Halaman editor harus punya tepat 3 `section`.

## Status

- S0: menunggu hasil cli-113 untuk `bb271b0` (permintaan terkirim) dan `60dcb56`. `53de09c` adalah perbaikan E0503 dari `3ffa70e` dan belum diminta.
- S1: kode selesai, menunggu verifikasi.
- S5 (baca-saja): kode selesai, menunggu verifikasi.
- S8 (perubahan lokal): kode selesai, menunggu verifikasi.
- S2c: kode selesai, sudah sesuai jawaban cli-137 (pairing, rotasi, token_expired, revision_conflict). Commit `39e57a3`, `1cbb703`. Menunggu verifikasi cli-113.
- S4a: kode selesai (breadcrumb di topbar, `shell::breadcrumb`). Commit `f90c6ef`, `424763a`. Sidebar belum dibuat; navigasi memakai tab di topbar.
- S7: kode selesai (panel Pengaturan lewat `state::preferences`; toast dari `state::notifications` di editor). Belum ada: layar pairing (menunggu S2b) dan toast di halaman data. Belum diverifikasi.
- S2b: kode selesai (transport fetch di `adapter/http/web.rs`, pairing, muat dan simpan dengan If-Match di editor, fallback sesi). Commit `cc623b2`, `9c221b7`, `b332314`, `8f50b3a`. Cargo.lock diperbarui oleh cli-113 (belum di-commit di sini). Menunggu verifikasi.
- S3: kode selesai (`validate_document` ke `POST /v1/validate`, fallback lokal). Commit `3a57e18`. Rute belum dikonfirmasi cli-137.
- S5b: kode selesai (edit tabel dan kolom lewat reducer, simpan dengan If-Match). Commit `660666e`. Relasi belum bisa diedit.
- S4b: kode selesai (`shell::path_for`, sinkron alamat, halaman awal dari alamat). Commit `67206b5`. Tombol Back belum ditangani.
- S6, S9, S10, S11: menunggu (sesuai instruksi orkestrator: hentikan dulu, tunggu cek gabungan S2b/S3/S5b/S4b). S3 dan S5b bergantung pada S2b yang sudah ada. S6 menunggu WS cli-137 setelah slice 7.
