# Spesifikasi: klaim dan permintaan input lewat HTTP, dengan identitas per klien

Status: **draf, menunggu persetujuan pemilik.** Belum ada kode. Implementasi dimulai setelah spesifikasi ini disetujui dan PR #30 (`ao/cli-141/mcp-multi-token`, token per klien) sudah di-merge.

Basis: `ao/input-required/root` di commit `2885b46` (di atas `main` `04ccf7f`). Commit `993c86b` di remote adalah versi awal dengan subjek yang sama; `2885b46` menggantikannya dan memperbaiki pengiriman ulang SSE (bila pertanyaan berubah pada state yang sama, event dikirim ulang). Tidak ada perilaku yang hilang.

## Tujuan

Worker yang berjalan sebagai proses terpisah bisa mengklaim pesan, menyelesaikannya atau melepasnya, dan meminta input dari pengirim, semuanya lewat HTTP di loopback. Setiap klien memakai identitas sendiri, sehingga klien tidak bisa bertindak sebagai klien lain.

Di luar cakupan: jaringan non-loopback, pembaruan lease (renewal), dan ACL yang bisa dikonfigurasi per label. Semuanya bisa ditambahkan kemudian.

## Kondisi sekarang

- Rute HTTP yang ada: `POST /message:send`, `GET /tasks/{from}/{id}`, `GET /tasks/{from}/{id}/events` (SSE), `POST /tasks/{from}/{id}:cancel`, `POST /tasks/{from}/{id}:input`, `GET /.well-known/agent.json`. Klaim tidak ada di HTTP (`agent_http.rs` baris 11-12; `agent-protocol-design.md` baris 59).
- Autentikasi: satu token bersama `PLAESY_AGENT_TOKEN` (`agent_serve.rs`). Tidak ada identitas. `from` dan `to` dideklarasikan sendiri oleh pemanggil.
- Store sudah menyediakan operasi lease: `claim_next(recipient, owner, lease_seconds)`, `acknowledge_claim(id, recipient, owner)`, `release_claim(id, recipient, owner, error)`, `request_input(id, recipient, owner, question)`, dan `provide_input(sender, id, reply)`. Lease dicek lewat `lease_owner` dan `lease_expiry_epoch_ms`.
- PR #30 menambahkan token per label (`plsy_<id>_<secret>`, hanya hash yang disimpan, rotasi dengan masa tenggang, revokasi) di `crates/plaesy/adapter/memory/src/mcp/tokens.rs`. Saat ini middleware MCP membuang label (`Verdict::Allowed(_)`).
- `plaesy-interface-cli` sudah bergantung pada `plaesy-adapter-memory` dan `plaesy-adapter-relay`, jadi memakai `tokens.rs` tidak melanggar aturan lapisan.

## Model identitas per klien

Label adalah identitas klien yang sudah diautentikasi. Sumber label bergantung pada konfigurasi server:

| Kondisi | Label | Perilaku |
|---|---|---|
| Tidak ada token sama sekali (tidak ada env, tidak ada file) | `open` | Mode kompatibel. Identitas dideklarasikan sendiri, persis seperti sekarang. Pemeriksaan ACL dilewati. |
| `PLAESY_AGENT_TOKEN` diisi | `default` | Satu identitas untuk semua pemegang token. Sama seperti sekarang, tetapi sudah tercatat sebagai label. |
| File token agen (`PLAESY_AGENT_TOKENS_FILE`) diisi | label dari file | Satu label per klien. Token tidak sah ditolak `401`. |

Keputusan:

- **File token agen terpisah** dari `PLAESY_MCP_TOKENS_FILE`. Dengan begitu merevokasi token memori tidak memutus worker, dan sebaliknya. Path default mengikuti pola PR #30 dan tetap di luar direktori yang diawasi (`ensure_outside`).
- **Format token dan `TokenSet` dipakai ulang** dari `adapter/memory`. Dipindahkan ke crate bersama hanya sebagai langkah terpisah bila sudah ada pengguna ketiga.
- **Mode aktif sekali.** Setelah file token pernah terlihat, autentikasi tetap wajib. File kosong atau hilang menolak semua permintaan. Perilaku ini mengikuti PR #30.

## Otorisasi per label

Aturan default tanpa ACL konfigurasi: **label hanya boleh bertindak atas namanya sendiri.** Label adalah nama agen.

| Aksi | Syarat label |
|---|---|
| Kirim pesan (`POST /message:send`) | `from` harus sama dengan label. `to` bebas. |
| Klaim, selesai, lepas, minta input (rute `/claims`) | Penerima adalah label. Tidak ada parameter penerima di jalur. |
| Baca task dan event SSE (`GET /tasks/{from}/{id}`) | Label harus sama dengan `from` (pengirim), seperti sekarang. |
| Jawab input (`POST /tasks/{from}/{id}:input`) | Label harus sama dengan `from` (pengirim task). |
| Batalkan (`:cancel`) | Label harus sama dengan `from`, seperti sekarang. |

Mode `open` tidak menjalankan pemeriksaan ini, agar tidak ada perubahan perilaku bagi pemanggil yang ada.

## Lease owner: token per klaim

Label tidak dipakai langsung sebagai `lease_owner`. Dua worker yang memakai token yang sama tidak boleh saling melepas atau menyelesaikan lease satu sama lain.

- Saat klaim berhasil, server membuat token lease acak (minimal 128 bit, hex) dan menyimpannya di `lease_owner` dalam bentuk `<label>.<nonce>`.
- Token dikembalikan **sekali** dalam respons klaim. Token tidak pernah muncul di `GET`, SSE, atau Agent Card.
- Setiap panggilan lease (`:complete`, `:release`, `:request-input`) memeriksa **dua hal**: label dari autentikasi cocok dengan awalan `<label>.`, dan seluruh string `lease_owner` sama dengan token yang dikirim. Salah satu gagal berarti `409 lease_conflict`.
- Panjang token dibatasi 128 karakter, sesuai `validate_id` di relay.

## Rute baru

Semua rute di bawah memerlukan autentikasi sesuai mode di atas. `recipient` selalu label.

| Rute | Body | Memanggil | Respons sukses |
|---|---|---|---|
| `POST /claims:next` | `{"lease_seconds"?: 300}` | `claim_next` | `200` dengan `{id, from, body, attempt, lease: {token, expires_at_epoch_ms}, input_reply?}`. Tidak ada pesan: `204`. |
| `POST /claims/{id}:complete` | `{"lease"}` | `acknowledge_claim` | `200` dengan `{id, state: "completed"}` |
| `POST /claims/{id}:release` | `{"lease", "error"}` | `release_claim` | `200` dengan `{id, status}` (`menunggu` atau `gagal`) |
| `POST /claims/{id}:request-input` | `{"lease", "question"}` | `request_input` | `200` dengan `{id, state: "input-required"}` |

Aturan tambahan:

- `lease_seconds` default `300`. Rentang `1..=86400`, sesuai batas store. Di luar rentang: `400`.
- `question` dan `error` tidak boleh kosong setelah dipangkas: `400`.
- `input_reply` hanya ada bila pengirim sudah menjawab. Field ini membawa jawaban ke klaim berikutnya, sesuai perilaku store yang sudah ada.
- **Pembaruan lease (renewal) tidak ada.** Store belum punya fungsi perpanjangan. Worker harus selesai sebelum lease habis, atau melepas lease. Renewal menjadi fungsi store baru dalam langkah terpisah.

## Status code dan format error

| Kode | Arti |
|---|---|
| `200` | Berhasil |
| `201` | Pesan dikirim (tidak berubah) |
| `204` | Tidak ada pesan untuk diklaim (bukan error) |
| `400` | Body tidak valid, field kosong, atau `lease_seconds` di luar rentang |
| `401` | Token hilang atau salah (saat autentikasi aktif) |
| `403` | Label terautentikasi tidak diizinkan untuk aksi ini |
| `404` | Task atau klaim tidak ditemukan |
| `409` | Konflik, dengan `code` yang bisa dibaca mesin |
| `413` | Body terlalu besar (tidak berubah) |

Kode `409` yang sekarang semuanya menjadi satu teks bebas. Setelah perubahan, body error memuat `error` (teks, seperti sekarang) dan `code`:

- `lease_conflict`: `MessageError::LeaseConflict`. Lease tidak dipegang lagi, sudah habis, atau token salah.
- `task_transition`: `MessageError::TaskTransition`. Misalnya `:input` pada task yang bukan `input-required`.
- `idempotency_conflict`: `MessageError::IdempotencyConflict`. ID yang sama dengan isi berbeda.

Klien lama tetap bisa membaca `error`. `code` hanya tambahan.

## Kompatibilitas dengan `POST /tasks/{from}/{id}:input`

- **Mode `open`:** tidak berubah. Rute, status, dan body sama.
- **Dengan autentikasi:** hanya label yang sama dengan `from` yang boleh menjawab. Label lain mendapat `403`. Status lain tidak berubah.
- Jawaban tetap disimpan di baris pesan dan dikirim lewat klaim berikutnya (`input_reply`).
- Perilaku yang sudah disetujui di `agent-protocol-design.md` (lease dilepas, percobaan dikembalikan, membatalkan task menghapus pertanyaan) tidak berubah.

## Yang digantikan dan perlu diubah di langkah 3

Spesifikasi ini membalik dua pernyataan yang ada:

1. `crates/plaesy/interface/cli/src/cli/agent_http.rs` baris 11-12: "No HTTP route raises `input-required`, because claims are not exposed over HTTP." Diganti dengan rute `/claims` di atas.
2. `docs/platform/agent-protocol-design.md` baris 59: "Penerima meminta input lewat API store (`request_input`), bukan lewat HTTP, karena klaim tidak tersedia di HTTP." Diganti dengan rute `/claims:request-input`.

Perubahan lain di langkah 3:

- `agent_serve.rs`: autentikasi menghasilkan label (bukan hanya `bool`), dan label diteruskan ke `route`.
- `agent_http.rs`: `route` menerima label; pemeriksaan otorisasi dan body `code` ditambahkan.
- `CHANGELOG.md` dan `docs/platform/decisions.md`: catat keputusan default di bawah.
- Tidak ada perubahan di `plaesy-domain-*`, dan tidak ada perubahan skema SQLite.

## Postur jaringan

- Server tetap hanya di `127.0.0.1`. Tidak ada bind baru.
- Tidak ada token, konfigurasi, atau server yang diubah oleh spesifikasi ini. Yang disiapkan hanya dokumen.
- Di implementasi, `PLAESY_AGENT_TOKENS_FILE` menjadi konfigurasi baru. Catat di `decisions.md` sebagai keputusan default yang bisa dicabut.

## Keputusan default (didelegasikan, bisa dicabut pemilik)

1. Label adalah nama agen. Tanpa ACL, label hanya bertindak atas namanya sendiri (lihat tabel otorisasi).
2. Mode `open` mempertahankan identitas yang dideklarasikan sendiri, tanpa pemeriksaan.
3. Lease default 300 detik, maksimum 86400 detik. Tanpa renewal.
4. Token lease opaque, dikembalikan sekali, dan diikat ke label.
5. File token agen terpisah: `PLAESY_AGENT_TOKENS_FILE`.
6. Error `409` menyertakan `code` tetap di samping `error`.
7. Lease tidak diperpanjang selama menunggu input. Percobaan yang meminta input dikembalikan (dari keputusan di `agent-protocol-design.md`).

## Pertanyaan terbuka

1. Apakah file token agen harus dipisah dari file token memori (default di atas), atau cukup satu file dengan label berbeda? Keduanya berfungsi. Pemisahan mengurangi dampak revokasi.
2. Apakah `tokens.rs` dipindah ke crate bersama sekarang, atau nanti? Default: nanti.
3. Apakah perlu rute baca untuk penerima (`GET` task oleh penerima)? Default: tidak. Penerima mendapat isi lewat klaim.

## Uji yang diharapkan (langkah 3)

- Klaim tanpa pesan: `204`. Klaim dengan lease tidak valid: `409 lease_conflict`.
- Token lease dari label A tidak bisa dipakai label B: `409`.
- Label tidak sama dengan penerima atau pengirim: `403`.
- `request-input` lalu `:input` lalu klaim: `input_reply` muncul dan percobaan tidak bertambah.
- Mode `open`: semua perilaku lama tetap (regresi).
- Token tidak pernah muncul di `GET`, SSE, atau Agent Card.

## Urutan

1. Setujui spesifikasi ini.
2. Merge PR #30.
3. Implementasi di atas `main` terbaru, dengan tes scoped `-p plaesy-adapter-relay` dan `-p plaesy-interface-cli` (`--locked`).
