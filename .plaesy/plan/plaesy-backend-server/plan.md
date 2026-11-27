# Plan: backend server Plaesy (untuk klien web cli-125)

Status: draft tahap 1 (plan dan threat model saja, belum ada kode).
Basis: `origin/main` = `9db81a5`.
Threat model: [threat-model.md](threat-model.md).

## Tujuan

Backend HTTP `/v1` yang bisa diakses klien web di browser lewat WireGuard atau internet, dengan standar enterprise: bind aman, autentikasi per perangkat, otorisasi deny-by-default per tenant, penyimpanan SQLite yang aman untuk dua penulis, audit log, observabilitas, dan deployment yang terkunci.

## Keadaan saat ini (terverifikasi di kode)

- `crates/plaesy/interface/http` hanya berisi `router(store)`. Tidak ada listener, tidak ada `main`, dan tidak ada pemanggil di binary. Backend belum bisa dijalankan.
- Rute yang ada: `GET /v1/health`, `GET /v1/documents/{kind}`, `GET|PUT|DELETE /v1/documents/{kind}/{id}`, `POST /v1/validate`, `POST /v1/openapi`, `POST /v1/codegen/flow/{lang}`.
- `DocumentStore` (`crates/plaesy/service/src/port/mod.rs`) sinkron. Handler axum memanggilnya langsung, jadi memblokir worker tokio.
- `DocumentStore` diimplementasikan di `adapter/sqlite` dan di tes `service/src/documents_test.rs`. Tidak ada revisi atau ETag.
- `SqliteStore::open` memakai `Mutex<Connection>`, migrasi lewat `PRAGMA user_version`, dan tidak mengatur WAL atau `busy_timeout`.
- Dokumen dikunci `(kind, id)` tanpa tenant atau project. Tidak ada ruang nama tenant.
- `domain/identity` (identity, authz, secrets) hanya in-memory. `AuditLog` berupa `Vec`. Tidak ada persistensi.
- `authz::Action` hanya berisi aksi agen (`Submit`, `Start`, `ToolRequest`, `MessageSend`, `Cancel`). Belum ada aksi dokumen.
- Pola yang bisa dipakai ulang: `adapter/relay/src/relay/transport.rs` menolak bind non-loopback kecuali TLS diakui terminasi di proxy hulu. `adapter/memory/src/mcp/mod.rs` memakai bearer token (minimal 32 byte, dibandingkan dengan `constant_time_eq`).
- `interface/client/src/adapter/http/mod.rs` adalah kontrak klien saat ini. Hanya `Request`, `Response`, `Transport`, dan `document_path`. Tidak ada header auth dan tidak ada ETag.
- `plaesy-domain-*` tidak boleh bergantung ke tokio, axum, rusqlite, dan crate I/O lain (dicek `plaesy/tests/architecture_test.rs`). Menurut ADR 0003, `cli` adalah satu-satunya perakit adapter, jadi composition root listener ada di `interface/cli`.
- Dependensi: `axum 0.8.9`, `tokio 1.54`, `tower 0.5`, `tracing`, `tracing-subscriber`, `sha2`, `subtle`, dan `tokio-tungstenite 0.26` sudah ada di workspace. `tower-http` hanya transitif di lock.

## Aturan lock (berlaku untuk semua slice)

Semua cek memakai `--locked`. Agen tidak menjalankan Cargo, jadi `Cargo.lock` tidak boleh berubah tanpa regenerasi oleh cli-113 atau keputusan owner.

- Mengaktifkan fitur crate yang sudah terkunci (misalnya `axum/ws`, `tower/timeout`) biasanya tidak mengubah lock. Tetap diverifikasi oleh cli-113.
- Menambah edge dependensi (crate `X` memakai `tower-http`) pasti mengubah lock. Kolom "Lock" di bawah menandainya.
- Lebih disukai middleware tulisan sendiri di atas axum/tower yang sudah ada, daripada menambah dependensi.

## Keputusan yang perlu owner (D1 sampai D6)

| ID | Pertanyaan | Default yang saya usulkan | Dampak |
|---|---|---|---|
| D1 | Model tenant: rute baru `/v1/projects/{project}/documents/...` (aditif) atau prefiks kunci? | Rute aditif. Rute lama dimatikan saat auth aktif. | Kontrak klien, perlu disetujui cli-125 |
| D2 | Autentikasi | Sudah diputuskan user: token per perangkat (pairing, rotasi, revokasi, hash SHA-256, cakupan per perangkat, audit). OIDC opsional, ditunda. | Slice 3 |
| D3 | TLS | Terminasi di Caddy (sudah ada di host). Rustls di proses ditunda. HSTS dikirim Caddy. | Slice 9 |
| D4 | Izin mengubah `Cargo.lock` per slice | Cli-113 meregenerasi lock atas permintaan, satu commit lock per slice | Semua slice yang menambah edge |
| D5 | Realtime WebSocket | Ditunda ke setelah slice 7. Sumber event (perubahan dokumen) butuh desain. | Slice 8 |
| D6 | Port default | `127.0.0.1:8790` (8787 sudah dipakai `plaesy-messages`). | Slice 1 dan 9 |

## Slice berurutan

Setiap slice: satu commit lokal, tes di `<name>_test.rs`, satu bullet di `CHANGELOG.md`, lint Markdown untuk `.md` yang diubah. Tidak ada push atau PR.

| # | Slice | Kriteria selesai | Cek scoped (dijalankan cli-113) | Lock |
|---|---|---|---|---|
| 0 | Plan, threat model, entri `decisions.md` untuk jalur jaringan | Dokumen ada dan lolos lint; entri decisions berisi jalur jaringan (bind, TLS di Caddy, CORS default kosong) | `plaesy validate markdown .plaesy/plan/plaesy-backend-server docs/platform --no-baseline` | Tidak |
| 1 | Entry server dan bind gate: `plaesy serve` (perakit di `interface/cli`: membuka `SqliteStore` dari path di config lalu memasangnya sebagai `DocumentStore` di belakang router), config bertingkat (default < berkas < env `PLAESY_*` < flag), CORS allowlist dari config (default kosong), default loopback, non-loopback hanya dengan `allow_non_loopback` plus `tls_terminated_upstream`, graceful shutdown, tracing JSON ke stderr | Tes: default loopback; non-loopback ditolak tanpa opt-in; shutdown menutup listener; stdout kosong saat `serve` | `cargo clippy -p plaesy-interface-http -p plaesy-interface-cli --all-targets --locked -- -D warnings`; `cargo test -p plaesy-interface-http`; `cargo test -p plaesy-interface-cli serve` | Mungkin (edge cli ke http, tracing) |
| 2 | Batas async dan request: `spawn_blocking` di sekitar `DocumentStore`, body limit (default 1 MiB), timeout, request id (`x-request-id`), galat JSON tanpa detail internal, `/v1/ready` (cek DB dan skema) | Tes: body besar ditolak 413; timeout menghasilkan 503/408; galat backend tidak membocorkan path; `/v1/ready` 503 saat DB gagal | `cargo test -p plaesy-interface-http`; `cargo clippy -p plaesy-interface-http --all-targets --locked -- -D warnings` | Tidak, bila memakai tower dan axum yang sudah ada |
| 3 | Pairing dan token perangkat (keputusan user): `plaesy device pair` membuat kode pairing sekali pakai (TTL 5 menit, hanya hash disimpan); perangkat menukar kode menjadi token acak 256 bit; token hanya hash SHA-256 di DB; rotasi (`device rotate`: token baru, token lama tetap berlaku sampai grace window habis); pencabutan (`device revoke`, berlaku pada request berikutnya); `device ls`; cakupan per perangkat (project + aksi) disimpan di domain identity dan dipakai authz (slice 4); setiap pairing, rotasi, dan pencabutan masuk audit (slice 6). Tabel `devices` dan `device_pairings` lewat migrasi | Tes: token dan kode pairing tidak tersimpan plaintext; kode pairing kedaluwarsa dan sekali pakai; token cabut dan token lama setelah grace window ditolak; rotasi tidak memutus token baru; DB versi lama terbuka dan termigrasi | `cargo test -p plaesy-domain-identity`; `cargo test -p plaesy-adapter-sqlite`; `cargo clippy` untuk kedua crate | Ya (sha2, sumber entropi) |
| 4 | Authz deny-by-default: middleware bearer → `Principal`, aksi dokumen (`DocumentRead`, `DocumentWrite`, `DocumentList`, `DocumentDelete`) per project, rute tenant sesuai D1, setiap keputusan masuk audit in-memory | Tes: tanpa token 401; token project A ke project B 403; aksi tanpa grant 403; rute lama dimatikan saat auth aktif | `cargo test -p plaesy-domain-identity authz`; `cargo test -p plaesy-interface-http`; clippy | Tidak |
| 5 | Konkurensi dan WAL: `PRAGMA journal_mode=WAL`, `busy_timeout`, kolom `revision` lewat migrasi, metode `DocumentStore` bertipe default (`get_versioned`, `put_if_match`) agar `documents_test.rs` tidak rusak, `If-Match`/`ETag`, 412 saat konflik, 428 saat `If-Match` wajib tetapi tidak ada | Tes: dua penulis, satu dapat 412; DB versi lama termigrasi dan data tetap utuh; WAL aktif | `cargo test -p plaesy-adapter-sqlite`; `cargo test -p plaesy-service`; `cargo test -p plaesy-interface-http`; clippy untuk tiga crate | Tidak |
| 6 | Audit persisten dan observabilitas: tabel `audit_events` append-only (trigger SQLite `RAISE(ABORT)` untuk UPDATE dan DELETE), counter metrik di `/v1/metrics` (hanya loopback atau bearer admin), `request_id` masuk log dan audit | Tes: UPDATE dan DELETE audit ditolak; metrik naik setelah request; request id sama di log dan audit | `cargo test -p plaesy-adapter-sqlite`; `cargo test -p plaesy-interface-http`; clippy | Tidak |
| 7 | Browser hardening: CORS allowlist eksplisit (default kosong; tanpa `*` dengan kredensial), preflight, header keamanan (`nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Cache-Control: no-store` untuk `/v1`), rate limit token bucket per principal dan per IP (IP dari `X-Forwarded-For` hanya bila peer loopback, dikonfigurasi `trusted_proxies`), validasi IR pada PUT (`validate_document` menolak error dengan 422) | Tes: origin asing tidak mendapat header CORS; rate limit 429 dengan `Retry-After`; PUT tidak valid 422 dan tidak tersimpan; X-Forwarded-For dari peer non-loopback diabaikan | `cargo test -p plaesy-interface-http`; `cargo test -p plaesy-service`; clippy | Tidak |
| 8 | WebSocket realtime untuk event dokumen (ditunda, tunggu D5): tiket sekali pakai dari POST yang sudah diautentikasi (TTL 60 detik), cek `Origin` saat upgrade, langganan per project (event `document.put` dan `document.deleted` dengan revisi, hanya untuk project yang diizinkan), batas koneksi per principal dan global, idle timeout, batas ukuran pesan. Memakai `interface/ws` yang sudah ada | Tes: upgrade tanpa tiket 401; origin asing 403; koneksi melebihi batas ditolak | `cargo test -p plaesy-interface-ws`; `cargo test -p plaesy-interface-http`; clippy | Mungkin (axum/ws) |
| 9 | Kontrak OpenAPI `/v1` yang dikonsumsi klien web: spec server di `/v1/spec` (bukan `/v1/openapi`, yang sudah dipakai untuk api-ir), disusun dari daftar rute dan skema yang sama dengan router; tes kontrak memastikan setiap rute dan kode galat di spec cocok dengan perilaku router dan dengan `interface/client`; `plaesy serve` menerima CORS allowlist dan batas body dari config (sudah dari slice 1), `backup` dan `restore` lewat `VACUUM INTO` plus `PRAGMA integrity_check`, unit systemd dengan hardening di `deploy/systemd/` (tidak dipasang), snippet Caddy di `deploy/caddy/` (tidak diterapkan), baris release-checklist | Tes: spec memuat semua rute; restore dari backup utuh; unit lolos `systemd-analyze verify` di sisi owner (tidak dijalankan di sini) | `cargo test -p plaesy-interface-http`; `cargo test -p plaesy-interface-cli backup`; clippy; Markdown lint | Tidak |

Urutan boleh digeser hanya bila dependensi memaksa. Slice 4 bergantung pada 3, slice 5 bergantung pada 2, slice 8 bergantung pada 4 dan 7.

## Ditunda (dengan alasan)

- OIDC: belum ada IdP yang dipilih. Keputusan owner.
- ACL per dokumen: butuh model izin baru. Di luar slice ini. Otorisasi di tingkat project lebih dulu.
- Rate limit terdistribusi: hanya satu instance. Cukup token bucket di proses.
- TLS di proses (rustls): Caddy sudah ada di host. Lihat D3.
- Operasi ke server nyata: di luar cakupan. Hanya dengan persetujuan user.

## Kontrak dengan klien web (cli-125)

Semua perubahan kontrak harus aditif atau disepakati dulu. Pesan ke cli-125 memakai prefiks `[from cli-137@windows to cli-125@windows id=...]`. Isinya: header `Authorization: Bearer` (slice 3 ke atas), header `ETag` dan `If-Match` (slice 5), rute tenant (D1), kode galat 401, 403, 412, 413, 422, dan 429, serta `/v1/ready`.

## Kontrak klien (disepakati dengan cli-125, 2026-11-27)

Keputusan ini didelegasikan user dan bisa dicabut. Bentuk final berlaku untuk slice 3 sampai 5.

- **Project.** Id berbentuk `prj_` diikuti 1 sampai 58 karakter `[a-z0-9]`. Id dipakai sebagai segmen path. `GET /v1/projects` (slice 4) mengembalikan `{"projects": ["prj_..."]}`, yaitu project yang diberikan ke perangkat, bukan semua project. Project yang tidak diberikan dijawab 404, sama dengan project yang tidak ada, supaya tidak ada penghitungan keberadaan.
- **Pairing.** `POST /v1/pairing/exchange` tanpa autentikasi, body `{"code": "..."}`. Balasan `200 {"token": "plt_...", "expires_at": "<RFC 3339 UTC>"}`. Kode berbentuk `PAIR-` diikuti 12 karakter base32 (sekitar 60 bit), berlaku 5 menit, dan sekali pakai. Kode dibuat dari CLI server (`plaesy device pair`). Kode salah, kedaluwarsa, atau sudah dipakai semuanya 401 `{"error": "invalid_code"}`.
- **Masa berlaku token.** Token berlaku 90 hari secara default (`device_token_ttl` di config). Token kedaluwarsa dijawab 401 `{"error": "token_expired"}`. Refresh lewat `POST /v1/devices/self/rotate` dengan token yang masih berlaku. Balasannya token baru dan `expires_at`. Token lama tetap berlaku 5 menit (grace window), lalu ditolak.
- **ETag.** Format kuat dengan tanda kutip, `"rev-N"`, dengan N mulai dari 1 dan bertambah tiap PUT yang berhasil. GET dan PUT mengembalikan header `ETag` yang sama bentuknya.
- **If-Match.** PUT ke dokumen yang sudah ada wajib `If-Match`. Tanpa header itu 428. Jika tidak cocok, 412. Dokumen baru boleh dibuat tanpa `If-Match` dan dijawab 201. DELETE menerima `If-Match` opsional, dengan aturan yang sama bila dikirim.
- **Body 412.** `{"error": "revision_conflict", "current": {"etag": "\"rev-4\"", "document": {...}}}`. Body menyertakan dokumen terbaru supaya klien bisa menggabungkan perubahan tanpa GET tambahan.
- **Galat lain.** 401 `{"error": "unauthorized"}` dengan `WWW-Authenticate: Bearer`. 403 `{"error": "forbidden"}` untuk aksi yang tidak diizinkan pada project yang diberikan. 413 body terlalu besar. 422 `{"errors": [...]}` untuk IR tidak valid. 429 dengan `Retry-After`.
- **Rute lama.** `/v1/documents/...` tanpa project dimatikan begitu autentikasi aktif (slice 4). Sebelum itu tetap ada untuk pengujian lokal.
- **Rute tanpa data tenant.** `POST /v1/validate` tetap di `/v1/validate`, tanpa project, karena hanya memeriksa body IR dan tidak membaca atau menulis dokumen. Setelah autentikasi aktif (slice 4), rute ini tetap butuh Bearer token yang valid, supaya pemanggil anonim tidak bisa membebani CPU. Balasan `{valid, errors}` tidak berubah. Rute `POST /v1/codegen/flow/{lang}` dan `POST /v1/openapi` diperlakukan sama. Hanya `/v1/health`, `/v1/ready`, dan `/v1/pairing/exchange` yang tanpa autentikasi.
- **Config klien.** Base URL dan token dikonfigurasi di klien, tidak di-hard-code. Nama provisional S2c di sisi klien boleh dipakai sampai nama final ditetapkan di sini.

## Cek lintas slice

- `plaesy validate markdown` untuk setiap `.md` yang diubah.
- Tidak ada `unwrap()` atau `expect()` di library (tes dikecualikan).
- Stdout tidak boleh dipakai untuk diagnostik. Semua log ke stderr.
- Tidak ada secret di log, di URL, atau di config plaintext. Secret dibaca dari `$CREDENTIALS_DIRECTORY` (systemd `LoadCredential`).
- Migrasi selalu jalan saat open, dan setiap migrasi punya tes membuka DB versi lama.
- Tidak ada perubahan pada crate `interface/client`.

## Tahap berikutnya

Setelah plan disetujui, mulai slice 1. Setiap slice memakai commit lokal, dan cli-113 diminta cek scoped per SHA.
