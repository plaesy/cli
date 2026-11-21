# ADR 0011: Struktur klien (plaesy-interface-client)

- Status: diusulkan (belum disetujui).
- Tanggal: 2026-11-21
- Pengambil keputusan: owner (belum ada persetujuan)
- Berlaku untuk: crate klien `plaesy-interface-client` (fitur `web`, `desktop`, `mobile`)
- Menggantikan: -
- Digantikan oleh: -
- Terkait: ADR 0003, 0009, 0010

## Ringkasan

- Klien adalah satu crate `plaesy-interface-client` dengan fitur `web`, `desktop`, dan `mobile`.
- Struktur dibagi `shell`, `ui`, `state`, `adapter`, `shared`, `features`, dan `platform`, dengan aturan dependensi yang jelas.
- Status: diusulkan.

## Konteks

ADR 0003 memuat struktur klien dalam satu bagian yang panjang. Bagian itu memakai keputusan yang berbeda dari crate backend (fitur Cargo, transport WebSocket, pemisahan platform) dan punya open items sendiri. Dipisah agar setiap ADR punya satu keputusan utama.

## Keputusan

Klien adalah satu crate `plaesy-interface-client` dengan fitur Cargo `web`, `desktop`, dan `mobile`. Struktur di bawah berlaku sejak awal. Fitur baru ditambahkan di tempat yang sudah disediakan.

Klien adalah satu crate dengan fitur Cargo `web`, `desktop`, dan `mobile`. Titik masuk per platform berada di `platform/`. Struktur di bawah ini berlaku sejak awal, dan fitur baru ditambahkan di tempat yang sudah disediakan.

```text
interface/client/
├── Cargo.toml                    fitur: web, desktop, mobile
└── src/
    ├── lib.rs
    ├── shell/                    routing, layout, navigasi, error boundary, guard sesi
    │   ├── router/               registrasi route dari setiap fitur
    │   ├── layout/               sidebar, topbar, panel, breadcrumb
    │   ├── nav/                  menu dan pencarian global
    │   └── guard/                izin per route (identity)
    ├── state/                    state global bersama
    │   ├── session/
    │   ├── preferences/          tema, bahasa, layout
    │   ├── realtime/             bus event dari ws, dipakai semua fitur
    │   └── notifications/
    ├── adapter/                  adapter ke server dan transport; tidak tahu fitur apa pun
    │   ├── http/                 klien HTTP, satu modul per konteks dto
    │   │   ├── client/           base URL, retry, header auth
    │   │   ├── workflow/
    │   │   ├── flow/
    │   │   ├── app/
    │   │   ├── data/
    │   │   ├── design/
    │   │   ├── memory/
    │   │   ├── collaboration/
    │   │   ├── identity/
    │   │   ├── messaging/
    │   │   ├── automation/
    │   │   └── documents/
    │   └── ws/                   koneksi realtime; protokol dan transport dipisah
    │       ├── port.rs           trait Transport: connect, send, recv, close (tanpa tipe platform)
    │       ├── protocol/         logika murni, tanpa I/O
    │       │   ├── v1/           envelope, seq, heartbeat, kebijakan backoff
    │       │   └── negotiate.rs  versi dan subprotokol (plaesy.v1, plaesy.v2)
    │       ├── session/          tiket sekali pakai, auth, penyimpanan last_seq (port storage)
    │       ├── transport/
    │       │   ├── web/          implementasi Transport di browser (cfg web)
    │       │   └── native/       implementasi Transport di desktop dan mobile (cfg desktop, mobile)
    │       └── tests/            Transport palsu untuk uji reconnect dan urutan, tanpa jaringan
    ├── ui/                       sistem desain; tidak tahu fitur apa pun
    │   ├── primitives/           button, input, badge, icon
    │   ├── forms/                field, validasi, select, date
    │   ├── data_display/         tabel, daftar, kartu, tree
    │   ├── feedback/             toast, alert, skeleton, empty state
    │   ├── overlay/              dialog, drawer, popover, menu
    │   ├── layout/               stack, grid, split, scroll
    │   ├── theme/                token dari design-ir, mode gelap dan terang
    │   └── a11y/                 fokus, aria, navigasi keyboard
    ├── shared/                   mesin non-produk yang dipakai beberapa fitur; tidak tahu fitur apa pun
    │   ├── canvas/               mesin graf untuk flow, app, data, dan design
    │   │   ├── nodes/
    │   │   ├── edges/
    │   │   ├── viewport/         zoom, pan, minimap
    │   │   ├── selection/
    │   │   ├── history/          undo dan redo
    │   │   └── layout_engine/    auto-layout graf
    │   └── diff/                 mesin diff: IR (flow, data, api, app, design) dan teks; menghasilkan daftar perubahan
    │       ├── ir/               diff per IR, tahu struktur tiap IR
    │       ├── text/             diff teks biasa
    │       └── changeset/        bentuk perubahan yang dipakai view dan merge
    ├── features/
    │   ├── flow/
    │   │   ├── pages/            daftar flow, editor flow, riwayat run
    │   │   ├── editor/           palette node, inspector, validasi inline
    │   │   ├── components/
    │   │   ├── hooks/
    │   │   ├── state/            store editor, selector, reducer
    │   │   ├── services/         pemanggilan api khusus flow
    │   │   ├── model/            view model, bukan DTO
    │   │   ├── i18n/
    │   │   └── tests/
    │   ├── design/               struktur sama: pages, editor, components, hooks, state, services, model, i18n, tests
    │   ├── app/                  struktur sama
    │   ├── data/                 struktur sama, plus erd/ untuk diagram relasi
    │   ├── workflow/             struktur sama
    │   ├── memory/               struktur sama
    │   ├── collaboration/        struktur sama, plus presence/ dan lease/
    │   ├── identity/             struktur sama, plus roles/ dan approvals/
    │   ├── messaging/            struktur sama
    │   ├── automation/           struktur sama
    │   └── documents/            struktur sama, plus editor/ untuk teks dan presentasi
    └── platform/
        ├── web/                  main, asset, integrasi browser; mengaktifkan fitur web
        ├── desktop/              main, tray, notifikasi OS, akses file lokal; mengaktifkan fitur desktop
        └── mobile/               main, kamera, notifikasi push; mengaktifkan fitur mobile
```

**Aturan dependensi di dalam klien:**

- `features/*` boleh memakai `ui`, `shared/*`, `adapter`, `state`, `shell`, dan `dto`.
- Fitur tidak memakai fitur lain secara langsung. Komunikasi antar-fitur lewat `state`, `state/realtime`, atau navigasi di `shell/router`.
- `shared/*` tidak memakai `features`, `adapter`, atau `state`. Ia boleh memakai `ui` untuk tampilan, dan `dto` untuk tipe IR. Ia hanya mengekspos API publik yang dipakai fitur.
- `shared/diff` tidak memakai `shared/canvas`, dan sebaliknya.
- `ui` tidak memakai `features`, `shared`, `adapter`, atau `state`.
- Komponen tampilan (tabel, daftar, tampilan diff) berada di `ui/data_display`, bukan di `shared`. `shared` hanya berisi logika mesin.
- `adapter/http` hanya memakai `dto` dan `state/session`.
- `adapter/ws/protocol` tidak memakai I/O, `adapter/ws/transport`, `state`, atau `features`. Ia hanya memakai `dto`.
- `adapter/ws/transport/*` hanya mengimplementasikan `adapter/ws/port`. Transport web dan native tidak saling memakai.
- `adapter/ws/session` boleh memakai `state/session` untuk token, dan tidak memakai `state/realtime`.
- `state/realtime` memakai `adapter/ws` lewat API publik. Ini searah, tidak ada siklus.
- `features/*` tidak memakai `adapter/ws` secara langsung; hanya lewat `state/realtime`.
- `shell` memakai `features/*` hanya lewat registrasi route, bukan impor langsung ke internal fitur.
- `platform/*` hanya merakit `shell` dengan renderer dan integrasi OS masing-masing.
- Pemeriksaan di CI: `use` yang melanggar aturan di atas membuat build gagal.

**Pola di dalam fitur:** `pages/`, `editor/` (bila ada), `components/`, `hooks/`, `state/`, `services/`, `model/`, `i18n/`, dan `tests/`. Subfolder tambahan seperti `erd/`, `presence/`, atau `roles/` hanya ditambahkan di fitur yang membutuhkannya.

**Pemecahan ke crate sendiri:** fitur yang sudah besar bisa dipindah ke crate `plaesy-interface-client-<fitur>` tanpa mengubah fitur lain, karena fitur lain sudah tidak boleh memakainya secara langsung.

## Konfirmasi

- Belum diperiksa. Tidak ada pemeriksaan path di CI; ditambahkan di rencana implementasi.
- Tidak ada `use` dari `features/*` ke `features/*` lain.
- Tidak ada `use` dari `ui` ke `features`, `adapter`, atau `state`.
- Tidak ada `use` dari `features/*` ke `adapter/ws` atau `adapter/http` secara langsung.
- Tidak ada `use` dari `shared/*` ke `features`, `adapter`, atau `state`.

## Open items

- Owner menyetujui struktur di atas, termasuk `shared/` (berisi `canvas/` dan `diff/`) sebagai modul bersama setara dengan `ui/`.
- Transport WebSocket: library per platform belum dipilih. Kandidat untuk web: `web-sys` langsung atau `gloo-net` (perlu dicek). Kandidat untuk native: `tokio-tungstenite`. Pilihan diputuskan saat implementasi, tanpa mengubah `port`.
- Format diff per IR (`shared/diff/ir`) dan bentuk `changeset` belum didefinisikan. Butuh ADR sendiri sebelum implementasi.
- Keputusan teknologi sinkronisasi real-time (`adapter/sync`) belum dibuat.
