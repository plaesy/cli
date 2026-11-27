# Spesifikasi: klaim dan permintaan input lewat HTTP, dengan identitas per klien

Status: **draf revisi 5, menunggu review Opus 5.5 lalu persetujuan pemilik.** Belum ada kode.

Urutan kerja (bagian 13): prasyarat store P1 sampai P3 boleh dikerjakan setelah revisi ini disetujui, karena tidak bergantung pada PR #30. Kode autentikasi dan token menunggu merge PR #30.

Basis kode: `ao/input-required/root` di commit `2885b46` (di atas `main` `04ccf7f`). Rujukan `berkas:baris` mengacu ke `2885b46`, kecuali `tokens.rs` yang mengacu ke PR #30 (`ao/cli-141/mcp-multi-token`, `e19a9bb`).

Cabang PR tetap berbasis `main` dan hanya berisi dokumen ini. Menarget cabang ke `2885b46` akan memasukkan kode input-required kembali ke diff PR #32.

## 1. Tujuan dan batas

Worker yang berjalan sebagai proses terpisah bisa mengklaim pesan, menyelesaikan atau melepasnya, dan meminta input dari pengirim, lewat HTTP di loopback. Setiap klien bertindak atas identitasnya sendiri.

Di luar cakupan: jaringan non-loopback, perpanjangan lease, ACL yang bisa dikonfigurasi per label, dan penyapu (sweeper) untuk task yang tertahan.

## 2. Kondisi sekarang (terverifikasi di `2885b46`)

- Rute di `route()` (`agent_http.rs:25-50`): `POST /message:send`, `GET /tasks/{from}/{id}`, `POST /tasks/{from}/{id}:cancel`, `POST /tasks/{from}/{id}:input`, `GET /.well-known/agent.json`.
- Rute `GET /tasks/{from}/{id}/events` (SSE) **tidak** lewat `route()`. Ia ditangani di `agent_serve.rs` `handle()` (`:129-133`) dan `stream_task` (`:166`), dan tidak memeriksa label. Setelah autentikasi token, siapa pun yang punya token bisa membaca state dan prompt task pengirim mana pun.
- Autentikasi: satu token `PLAESY_AGENT_TOKEN` (`agent_serve.rs`, `TOKEN_ENV`). Tidak ada identitas. `from` dan `to` dideklarasikan sendiri.
- `send_task` (`relay.rs:221`) memeriksa duplikat dengan `WHERE id=?1` (`:236`).
- `claim_next(recipient, lease_seconds)` (`relay.rs:510-561`) tidak menerima owner. Store membuat UUID v4 sendiri (`:520`) dan menyimpannya di `lease_owner`, lalu mengembalikannya dalam `Message`. `Message` memuat `sender`.
- `acknowledge_claim(id, recipient, owner)` (`:564`) mengembalikan `Ok(false)` bila lease tidak cocok, bukan error. `release_claim(id, recipient, owner, error)` (`:593`) mengembalikan `Ok(None)` untuk kondisi yang sama. Tidak ada `LeaseConflict` dari kedua fungsi ini.
- `request_input(id, recipient, owner, question)` (`:319`) mengembalikan `LeaseConflict` bila UPDATE tidak mengenai baris (`:338-343`). Cek transisi task (`:353`) baru berjalan setelah itu.
- `provide_input(sender, id, reply)` (`:366`) mengembalikan `TaskNotFound` untuk `(sender, id)` yang tidak ada (`:380-381`).
- `set_task_state(sender, id, next)` (`:282`) menulis state task tanpa menyentuh pesan. Pembatalan tidak menyentuh lease atau status pesan.
- `acknowledge(id, recipient)` (legacy, `:645`) hanya menerima pesan dengan `attempts=0` dan `awaiting_input=0`. **Tidak** memeriksa `failed_at_epoch_ms` (`:654`).
- Batas: body HTTP 64 KiB (`agent_http.rs`), `question` dan `reply` 1..=65536 byte (`relay.rs:329`, `374`), `error` maksimal 1024 byte (`relay.rs:603`), `id` 1..=128 byte tercetak (`relay.rs:838`), lease 1..=86400 detik (`relay.rs:516`), `MAX_ATTEMPTS = 5` (`relay.rs:21`).
- Skema: `relay_messages.id TEXT PRIMARY KEY` global, dengan `UNIQUE(sender, id)` (`relay.rs:167-174`). `relay_tasks` berkunci `(sender, id)` (`:178-185`). `CREATE TABLE IF NOT EXISTS` (`:167`) tidak mengubah tabel yang sudah ada, dan `migrate_columns` hanya menambah kolom.
- Status pesan diturunkan di SQL (`relay.rs:741-750`): `selesai`, `gagal`, `menunggu-input`, `diambil`, `menunggu`. Inbox (`:763`) menyaring pesan yang `failed_at` terisi.
- `agent-protocol-design.md` baris 54 (lease terakhir yang kedaluwarsa, dipertahankan) dan baris 55 (tugas `canceled` tetap bisa diklaim dan di-ack, dibalik oleh §3).
- CLI lokal (`plaesy relay`, `cli/relay.rs`) punya perintah `Send`, `Inbox`, `Wait`, `Claim`, dan `Ack` (`:18-65`). Pemanggil store: `claim_next` (`:120`), `acknowledge_claim` (`:129`), dan `acknowledge` (`:131`). CLI tidak punya perintah `release`. Output klaim mencetak `lease_owner` (`plaesy/tests/relay_cli.rs:44`, `124`), dan inbox serta pending menghapusnya (`cli/relay.rs:76-99`).
- PR #30 (`tokens.rs`): `TokenSet::authorize` memberi `Allowed(label)` untuk token env (label `default`) dan token berkas. `validate_label` hanya dipanggil dari `add` (`:216`), tidak dari `load` (`:310-327`). `TokenAuth::new` mengembalikan `Ok(None)` bila tidak ada sumber (`:430-432`). `check_at` mengembalikan `Verdict::Open` bila `required` belum pernah aktif (`:464-466`).

## 3. Prasyarat perubahan store

Empat perbaikan ini harus selesai sebelum rute klaim dibuka. Setiap perbaikan punya tes sendiri. P1 sampai P3 tidak bergantung pada PR #30.

### P1. Pembatalan dan kegagalan task menutup pesan

**(a) Penutupan.** Saat `set_task_state` memindahkan task ke `canceled` **atau** `failed`, dalam transaksi yang sama, setiap pesan milik task itu yang belum di-ack dan belum gagal ditandai gagal:

- `failed_at_epoch_ms` diisi, `last_error` = `"task canceled"` atau `"task failed"`, dan `lease_owner` serta `lease_expiry_epoch_ms` dikosongkan.
- Hasilnya: status pesan `gagal`, tidak bisa diklaim, dan tidak muncul di inbox (filter `failed_at` di `:763`).

Pemicu `failed` ikut karena alasan yang sama: task yang sudah gagal tidak boleh meninggalkan pesan hidup di inbox dan di `relay wait`. Ini perluasan dari keputusan default 7 di §11.

**(b) Klaim menolak task non-aktif.** `claim_next` melewati pesan yang task-nya berstate `canceled`, `failed`, `completed`, atau `input-required`. Pemeriksaan memakai **LEFT JOIN** ke `relay_tasks` (bukan inner join). Pesan tanpa baris task (dari `relay send`) tetap bisa diklaim. Ini penjagaan untuk baris lama yang tidak ditutup oleh (a).

Tes: pesan tanpa baris task tetap bisa diklaim; pembatalan dan kegagalan menandai pesan `gagal` dan melepas lease; pesan task `canceled` atau `failed` tidak bisa diklaim dan tidak muncul di inbox.

### P2. Selesai, lepas, dan minta input hanya pada task `working`

Aturan yang berlaku untuk tiga fungsi store `acknowledge_claim`, `release_claim`, dan `request_input`:

- Sebelum UPDATE lease, fungsi memeriksa state task dalam transaksi yang sama. **Bila task ada dan state-nya bukan `working`, fungsi mengembalikan `TaskTransition`** dan pesan tidak berubah. Pemeriksaan ini mendahului pemeriksaan lease, jadi task yang sudah dibatalkan selalu menghasilkan `TaskTransition`, bukan `LeaseConflict`.
- Pesan tanpa baris task tetap diizinkan (tidak ada state yang diperiksa).
- `request_input` juga memeriksa `working` sebelum UPDATE. Cek transisi `input-required` yang ada (`:353`) tetap berlaku sesudahnya.

Jalur legacy `acknowledge(sender, id, recipient)` (`:645`): tambahkan `AND failed_at_epoch_ms IS NULL` di `:654`. Tanpa ini, pesan yang gagal karena pembatalan dan belum pernah diklaim (`attempts=0`) bisa di-ack tanpa token, dan statusnya berubah menjadi `selesai` sementara task tetap `canceled`. Tes wajib: `acknowledge` pada pesan gagal mengembalikan `false`.

Tes yang sudah ada yang berubah karena perilaku ini (disengaja):

- `relay_test.rs:519` `canceled_task_stays_canceled_when_its_message_is_claimed_and_acked`: klaim setelah batal sekarang `None`. Assertion diubah.
- `relay_test.rs:698` `request_input_is_refused_for_a_canceled_task_and_keeps_the_lease`: sekarang `TaskTransition`, dan ack setelahnya `false`.
- `relay_test.rs:747` `canceling_an_input_required_task_drops_the_question_...`: klaim setelah batal sekarang `None`.

### P3. Id per pengirim

`relay_messages.id` adalah `PRIMARY KEY` global. Akibatnya pengirim A bisa mendeteksi id milik pengirim B lewat `409 idempotency_conflict`, dan dua pengirim tidak bisa memakai id yang sama.

**Perubahan skema.** `PRIMARY KEY` menjadi `(sender, id)`. SQLite tidak bisa mengubah primary key dengan `ALTER`, jadi migrasi memakai rebuild: buat `relay_messages_v2` dengan kunci baru, salin semua baris, hapus tabel lama, ganti nama. `CREATE TABLE IF NOT EXISTS` di `:167` diubah agar cocok. Migrasi dijalankan saat basis data lama dibuka. Ada tes yang membuka basis data versi sebelumnya dan memeriksa bahwa semua baris tersalin.

**Signature store.** Sender menjadi argumen eksplisit. Tidak ada fungsi yang mencarinya dari id saja, karena id tidak unik lintas pengirim.

| Fungsi | Signature baru |
|---|---|
| `find(sender, id)` | pengganti `find(id)` (`:494`). Dipakai untuk tes dan tampilan lokal. |
| `find_for_recipient(recipient, sender, id)` | baru. Mengembalikan pesan hanya bila penerima dan pengirim cocok. Dipakai rute HTTP. |
| `acknowledge_claim(sender, id, recipient, owner)` | `:564` |
| `release_claim(sender, id, recipient, owner, error)` | `:593` |
| `request_input(sender, id, recipient, owner, question)` | `:319` |
| `acknowledge(sender, id, recipient)` | legacy, `:645` |
| `claim_next(recipient, lease_seconds)` | **tidak berubah.** `Message` sudah memuat `sender`. |

**Query yang berubah.** Setiap query yang hanya memakai `id` harus memakai `sender` atau `recipient` juga. Daftar dari `2885b46`:

| Baris | Fungsi | Perubahan |
|---|---|---|
| `:236` | `send_task` (cek duplikat) | `WHERE sender=?1 AND id=?2` |
| `:339` | `request_input` (UPDATE) | `sender=?` di WHERE. Sender dari argumen. |
| `:347` | `request_input` (SELECT sender) | Dihapus. Sender dari argumen. |
| `:441` | `send` (cek idempoten) | `WHERE sender=? AND id=?` |
| `:500` | `find` | `find(sender, id)` |
| `:528` | `claim_next` (SELECT kandidat) | `SELECT id, sender` lalu `:540` dan `:548` memakai keduanya. |
| `:540`, `:580`, `:614` | claim, ack, release (UPDATE) | `sender=?` di WHERE. |
| `:548` | `claim_next` (ambil ulang) | `WHERE sender=? AND id=?` |
| `:623` | `release_claim` (SELECT sender) | Dihapus. Sender dari argumen. |
| `:654` | `acknowledge` (legacy) | `WHERE sender=? AND id=? AND recipient=?`, plus `failed_at_epoch_ms IS NULL` (P2). |
| `:723` dan `:732` | `backfill_acked_at_epoch` (SELECT dan UPDATE) | SELECT ambil `id, sender`. UPDATE `WHERE sender=? AND id=?`. |
| `:830` | `complete_task` (SELECT sender) | Sender dikirim sebagai argumen, tanpa SELECT. |

`purge_completed` (`:667`) tidak memakai id dan tidak berubah.

**Dampak ke CLI.** Perubahan signature ini memengaruhi pemanggil CLI berikut:

- `relay ack`: `acknowledge_claim` (`:129`) dan `acknowledge` (`:131`) sekarang butuh `sender`. Perintah `ack` menerima `--from <sender>` yang wajib.
- `relay claim`: tidak berubah. Output tetap menyertakan `sender` dan `lease_owner`, jadi pengguna punya nilai `--from` yang dibutuhkan `ack`.
- CLI tidak punya perintah `release`. Pemanggil `release_claim` hanya ada di store dan HTTP (§6). Tidak ada perubahan CLI untuk `release`.

Perubahan ini terlihat pengguna. Dicatat di CHANGELOG.

### P4. Validasi label di loader token (di luar store)

Dikerjakan bersama autentikasi, setelah PR #30 merge (lihat §4).

## 4. Identitas, prioritas sumber, dan status mode

### Sumber token

- `PLAESY_AGENT_TOKEN`: label `default`.
- `PLAESY_AGENT_TOKENS_FILE`: berkas terpisah dari `PLAESY_MCP_TOKENS_FILE`, agar revokasi token memori tidak memutus worker.
- Keduanya boleh diisi bersamaan. Keduanya aktif.

### Loader berkas agen

Loader agen memakai `load` dan `TokenSet` dari PR #30 dan **menambahkan validasi saat load**, yang tidak ada di PR #30:

- Format label dicek dengan `validate_label`.
- Label `default` ditolak dari berkas.
- Label ganda ditolak. Berkas yang gagal validasi membuat server menolak start.

Loader agen membungkus error `MemoryError` dari PR #30 dengan pesannya sendiri, supaya pesan error tidak menyebut "MCP". Reload setiap 2 detik memakai loader agen sendiri, bukan `TokenAuth` dari PR #30, karena `TokenAuth` mengembalikan `Open` saat `required` belum pernah aktif.

### Mode ditentukan saat start

| Konfigurasi saat start | Mode | Catatan |
|---|---|---|
| Tidak ada env dan tidak ada berkas | `open` | Sama seperti sekarang. |
| Env atau berkas diisi | `token` | Autentikasi wajib. |
| Berkas diisi tetapi hilang atau gagal validasi saat start | server **menolak start** | Fail closed. |

- Berkas dihapus saat server berjalan: loader agen menolak semua permintaan yang memakai token dari berkas. Token env tetap berlaku. Ini **bukan** perilaku PR #30, yang kembali ke token env saja. Perbedaan ini disengaja.
- Berkas dihapus saat server mati: start berikutnya gagal, karena konfigurasi masih menunjuk ke berkas itu.
- Rotasi dan revokasi berlaku tanpa restart, dengan pemeriksaan ulang berkas tiap 2 detik.

## 5. Aturan penerima per mode (tegas)

Label adalah nama agen. Tabel ini berlaku untuk setiap rute, termasuk SSE.

| Mode | `/claims/*` | `POST /message:send` | `GET /tasks/{from}/{id}`, `GET .../events`, `:cancel`, `:input` |
|---|---|---|---|
| `open` | **404, tidak tersedia.** | Perilaku lama: `from` dan `to` bebas. | Perilaku lama, tanpa pemeriksaan label. |
| `token` dengan env (`default`) | Hanya pesan dengan `recipient = default`. | `from` harus `default`. `to` bebas. Selain itu `403`. | Hanya bila `from` = `default`. Selain itu `404`. |
| `token` dengan berkas (label `L`) | Hanya pesan dengan `recipient = L`. | `from` harus `L`. `to` bebas. Selain itu `403`. | Hanya bila `from` = `L`. Selain itu `404`. |

- **SSE:** pemeriksaan label dipasang di `agent_serve.rs` `handle()` sebelum `stream_task`, dengan aturan yang sama dengan `GET /tasks/{from}/{id}`. Tes: label lain mendapat `404`, dan tidak ada byte event yang dikirim.
- Pemanggil `token` yang sebelumnya memakai `from` selain labelnya akan mendapat `403` (kirim) atau `404` (baca, SSE, jawab). Ini perubahan yang disengaja. Dicatat di CHANGELOG dan `decisions.md` saat implementasi.

## 6. Rute klaim

Semua rute memerlukan token pada mode `token`. Penerima selalu label. Pengirim wajib ada di jalur karena id hanya unik per pengirim (P3). Setiap rute memanggil `find_for_recipient` lebih dulu dan membalas `404` bila pesan tidak ditemukan.

| Rute | Body | Memanggil | Respons sukses |
|---|---|---|---|
| `POST /claims:next` | `{"lease_seconds"?: 300}` | `claim_next(label, n)` | `200`: `{id, from, text, body_format, attempts, lease: {token, expires_at_epoch_ms}, state: "working", input_reply?}`. Tidak ada pesan: `204` tanpa body. |
| `GET /claims/{from}/{id}` | - | `find_for_recipient(label, from, id)` | `200`: `{id, from, state, status}`. Tanpa token lease. Dipakai untuk memeriksa hasil setelah balasan hilang. |
| `POST /claims/{from}/{id}:complete` | `{"lease"}` | `acknowledge_claim(from, id, label, token)` | `200`: `{id, state}` dengan `state` dari `task_state(from, id)`. |
| `POST /claims/{from}/{id}:release` | `{"lease", "error"?}` | `release_claim(from, id, label, token, error)` | `200`: `{id, status, state}`. `status` `menunggu` atau `gagal`. `state` dari `task_state`. |
| `POST /claims/{from}/{id}:request-input` | `{"lease", "question"}` | `request_input(from, id, label, token, question)` | `200`: `{id, state: "input-required"}` dengan `state` dari `task_state`. |

- `text` diambil dari body JSON pesan (`{"depth", "text"}`, `agent_http.rs` `send_message`). Bila body tidak bisa di-parse, `text` kosong dan `body_format` bernilai `"raw"` dengan body mentah.
- `lease_seconds` default 300. Di luar `1..=86400`: `400 invalid_lease`.
- `question` 1..=65536 byte. `error` 0..=1024 byte, boleh kosong.
- `input_reply` hanya muncul bila pengirim sudah menjawab.
- Perpanjangan lease tidak ada.

## 7. Token lease

- Token adalah UUID v4 yang dibuat `claim_next`. Tidak ada perubahan signature. Format `<label>.<nonce>` dan klaim 128-bit dari revisi sebelumnya dihapus.
- Pengikatan ke label lewat store: server memanggil dengan `recipient = label`, dan store mencocokkan `recipient`, `sender`, `id`, dan `lease_owner` (`relay.rs:564-587`).
- Token dikembalikan sekali di respons klaim. Tidak ada di `GET`, SSE, Agent Card, inbox, log, atau respons error.
- Jaminan kebocoran:
  - Respons HTTP selain klaim tidak memuat `lease_owner`. `find_for_recipient` dan `task_view` mengembalikan `Message` lengkap, jadi server membuang field itu sendiri.
  - Log `agent serve` tidak mencetak header `Authorization`, body permintaan, atau token lease.
  - Tes: token tidak muncul di `GET /claims/...`, `GET /tasks/...`, SSE, Agent Card, stderr saat klaim, atau body error.
- CLI lokal `plaesy relay claim` tetap mencetak `lease_owner` (keputusan default, lihat §11).

## 8. Pemetaan error, status, dan retry

### Status dan kode

| Kode | `code` | Kapan |
|---|---|---|
| `200` | - | Berhasil. |
| `201` | - | Pesan dikirim (tidak berubah). |
| `204` | - | Tidak ada pesan untuk diklaim. Tanpa body. `write_reply` perlu dukungan untuk ini. |
| `400` | `invalid_body`, `invalid_id`, `invalid_lease`, `invalid_question`, `invalid_reply`, `invalid_error` | Body bukan JSON, field tidak valid, atau melewati batas byte. |
| `401` | `unauthorized` | Token hilang atau salah pada mode `token`. |
| `403` | `forbidden` | Hanya pada `send` ketika `from` bukan label. |
| `404` | `not_found` | Id tidak ada, bukan milik label, pengirim tidak cocok, atau `/claims` pada mode `open`. Body identik untuk semua kasus ini. |
| `409` | `lease_conflict` | Lease tidak dipegang, habis, atau token salah. Hanya bila pesan milik label dan task-nya `working`. |
| `409` | `task_transition` | Task bukan `working` (misalnya setelah pembatalan atau kegagalan, atau `:input` pada task yang bukan `input-required`). Berlaku untuk `:complete`, `:release`, dan `:request-input`. |
| `409` | `idempotency_conflict` | Id yang sama dengan isi berbeda, dari pengirim yang sama. |
| `413` | `body_too_large` | Body di atas 64 KiB. Dicek sebelum parse. |

Setelah pembatalan atau kegagalan, `:complete`, `:release`, dan `:request-input` selalu membalas `409 task_transition`. Tidak ada `404` untuk kasus ini, karena pesan itu masih milik label (pengirim dan penerima tidak berubah). Kode ini konsisten dengan P2.

`write_reply` (`agent_serve.rs:292-303`) perlu ditambah 204 (tanpa body) dan 403. Saat ini belum ada.

### Pemetaan dari store

- `acknowledge_claim` mengembalikan `Ok(false)` (lease tidak cocok, pesan sudah selesai atau tidak dimiliki): `409 lease_conflict` bila `find_for_recipient` menemukan pesan, `404` bila tidak.
- `release_claim` mengembalikan `Ok(None)`: sama seperti di atas.
- `request_input` mengembalikan `LeaseConflict`: sama seperti di atas.
- `TaskTransition`: `409 task_transition`. Dipetakan sebelum cek lease (P2).
- `TaskNotFound`: `404 not_found`.
- `IdempotencyConflict`: `409 idempotency_conflict`.
- `InvalidLease`: `400 invalid_lease`.
- `Invalid(msg)`: `400 invalid_*` sesuai field.

Batas byte efektif lewat HTTP: body total 64 KiB. `question` dan `reply` juga dibatasi oleh `MAX_BODY_BYTES` store (65536). Batas efektif adalah yang lebih kecil, yaitu sisa 64 KiB setelah overhead JSON. Tes memastikan teks 65536 byte ditolak dengan `413` atau `400`, bukan terpotong.

Body error tetap memuat `error` (teks) untuk kompatibilitas. `code` adalah tambahan.

### Retry dan replay

- **Send:** replay dengan id dan isi yang sama mengembalikan state task saat ini. Isi berbeda: `409 idempotency_conflict`.
- **Klaim (`:next`):** tidak idempoten. Setiap klaim yang berhasil menambah satu percobaan. Respons klaim yang hilang di jalan membuat lease tak diketahui, yang habis dan memakan satu percobaan. Ini didokumentasikan.
- **`:complete`, `:release`, `:request-input` yang diulang setelah berhasil:** lease sudah dilepas, jadi balasan kedua `409 lease_conflict`. Klien memeriksa hasilnya lewat `GET /claims/{from}/{id}`. Replay tidak dibuat sukses, karena store tidak menyimpan riwayat lease.
- **Lease habis sebelum selesai:** pesan kembali ke `menunggu`, atau `gagal` bila percobaan sudah `MAX_ATTEMPTS`. Lihat §9.

## 9. Lease habis dan task tertahan

- Lease habis tanpa `:release` tidak mengubah state task. Task tetap `working`.
- Setiap klaim memakan satu percobaan (`relay.rs:540`), termasuk klaim yang lease-nya habis. Ada 5 percobaan.
- Setelah percobaan kelima habis tanpa `:release`, status pesan menjadi `gagal` (dari SQL status) tetapi task tetap `working`. Tidak ada yang mengubah state task. Ini celah yang sudah ada dan tercatat di `agent-protocol-design.md` sebagai butir lease terakhir yang kedaluwarsa. Perbaikannya adalah pekerjaan terpisah (penyapu yang memindahkan task ke `failed`).
- `:release` pada percobaan terakhir mengubah pesan ke `gagal` dan task ke `failed` (`relay.rs:632-638`). Pada percobaan lain, task tetap `working`.

## 10. Kompatibilitas `POST /tasks/{from}/{id}:input`

- **Mode `open`:** tidak berubah.
- **Mode `token`:** `from` harus label (lihat §5). Respons sukses tidak berubah.
- Jawaban tetap disimpan di baris pesan dan dikirim lewat klaim berikutnya (`input_reply`).
- Keputusan yang sudah disetujui di `agent-protocol-design.md` (lease dilepas, percobaan dikembalikan, membatalkan task menghapus pertanyaan) tidak berubah. Membatalkan task yang menunggu input sekarang juga menutup pesannya (P1(a)), yang konsisten dengan keputusan itu.

## 11. Yang digantikan dan pertanyaan terbuka

### Yang digantikan (untuk langkah implementasi)

1. `agent_http.rs:11-12`: "No HTTP route raises `input-required`, because claims are not exposed over HTTP." Diganti dengan rute `/claims`.
2. `docs/platform/agent-protocol-design.md`, butir "Penerima meminta input lewat API store..., bukan lewat HTTP": diganti dengan `/claims/{from}/{id}:request-input`.
3. `docs/platform/agent-protocol-design.md`, butir "Tugas yang sudah `canceled` tetap bisa diklaim dan di-ack...": dibalik oleh P1 dan P2. Butir lease terakhir yang kedaluwarsa (yang menyebut `gagal` tanpa menulis ke tabel) dipertahankan sebagai celah (§9), dan tidak dibalik.
4. `agent_serve.rs` `handle()`: pemeriksaan label ditambahkan untuk SSE. `TOKEN_ENV` memakai loader agen (§4).
5. CLI `plaesy relay ack` menerima `--from` yang wajib (§3 P3). CHANGELOG mencatat perubahan ini.
6. Tiga tes di `relay_test.rs` (`:519`, `:698`, `:747`) berubah perilakunya (§3 P2).

### Keputusan default (didelegasikan, bisa dicabut pemilik)

1. Label adalah nama agen. Setiap label hanya bertindak atas namanya sendiri (§5).
2. Mode `open` tidak punya rute klaim.
3. Mode `token` dengan env melayani inbox `default`. Pemanggil yang memakai `from` lain mendapat `403` atau `404`.
4. Mode ditentukan saat start. Berkas token yang hilang atau gagal validasi saat start menolak start. Berkas yang dihapus saat berjalan menolak token berkas, sementara token env tetap berlaku.
5. Lease default 300 detik, maksimum 86400, tanpa perpanjangan.
6. Token lease adalah UUID v4 dari store, dikembalikan sekali, dan tidak diubah formatnya.
7. **Pembatalan atau kegagalan task menutup pesan (P1(a)).** Pesan yang belum di-ack ditandai `gagal` dengan `last_error` yang sesuai, lease-nya dilepas, dan pesan keluar dari inbox. Alternatifnya, pesan tetap `menunggu` selamanya dan tetap muncul di inbox.
8. **Kode error setelah penutupan: `409 task_transition`** untuk `:complete`, `:release`, dan `:request-input` (opsi A di review). Alternatif `404` ditolak karena pesan masih milik label.
9. Error `404` seragam untuk id yang bukan milik label atau bukan dari pengirim yang disebut di jalur.
10. Replay `:complete`, `:release`, dan `:request-input` yang sudah sukses dibalas `409 lease_conflict`. Pemulihan lewat `GET /claims/{from}/{id}`.
11. Rute klaim memuat `{from}` karena id unik per pengirim setelah P3.
12. CLI `relay ack` menerima `--from` (wajib). CLI tidak punya perintah `release`, dan tidak ditambah dalam spec ini.

### Pertanyaan terbuka

1. CLI lokal `plaesy relay claim` tetap mencetak token lease. Perlu redaksi?
2. Penyapu task yang tertahan (§9): dikerjakan di langkah ini atau terpisah? Default: terpisah.
3. Apakah `tokens.rs` dipindah ke crate bersama sekarang? Default: nanti.

## 12. Uji yang diharapkan

Store (P1 sampai P3, boleh dikerjakan sebelum PR #30 merge), `-p plaesy-adapter-relay --locked`:

- P1(a): pembatalan dan kegagalan menandai pesan yang belum di-ack sebagai `gagal`, melepas lease, dan pesan hilang dari inbox.
- P1(b): `claim_next` tidak mengembalikan pesan dari task `canceled`, `failed`, `completed`, atau `input-required`.
- P1(b): pesan tanpa baris task (dari `relay send`) tetap bisa diklaim.
- P2: `acknowledge_claim`, `release_claim`, dan `request_input` pada task yang bukan `working` mengembalikan `TaskTransition`, dan pesan tidak berubah. Ini termasuk kasus yang sebelumnya `LeaseConflict`.
- P2: pesan tanpa baris task tetap bisa di-ack dan di-release.
- P2: `acknowledge` (legacy) pada pesan yang gagal mengembalikan `false`.
- P3: dua pengirim boleh memakai id yang sama ke penerima yang sama, dan klaim memilih pesan dengan `sender` yang benar.
- P3: basis data versi sebelumnya dibuka dan dimigrasikan (rebuild tabel), dan semua baris tersalin.
- P3: setiap query id-only di tabel §3 sudah memakai `sender` atau `recipient`.
- Tiga tes di `relay_test.rs` (`:519`, `:698`, `:747`) diperbarui sesuai P2, dan tesnya tetap bermakna.

CLI, `-p plaesy-interface-cli --locked`:

- `relay ack` tanpa `--from` ditolak. Dengan `--from` yang benar, berhasil.
- `relay_claim_is_exclusive_and_ack_requires_owner_token` (`relay_cli.rs`): ack yang berhasil diberi `--from`. Tes negatif (token salah) tetap menguji token, bukan kelengkapan flag.
- `relay_wait_returns_pending_message_without_acknowledging_it` tidak berubah, karena `claim` tidak memakai `--from`.
- `relay_wait_exits_after_timeout_when_inbox_is_empty` tidak terdampak.
- Pemanggil di `agent_http_test.rs:144` dan `:224` menyesuaikan signature baru.

HTTP (setelah PR #30 merge), `-p plaesy-interface-cli --locked`:

- Mode `open`: `/claims/*` mengembalikan `404`. Semua perilaku lama tetap (regresi).
- Klaim tanpa pesan: `204` tanpa body.
- SSE: label lain mendapat `404`, dan tidak ada byte event yang dikirim.
- Token lease label A tidak bisa dipakai label B: `404` bila pesan milik B.
- Id milik pengirim lain: `404` dengan body identik dengan id yang tidak ada.
- Replay `:complete` setelah sukses: `409 lease_conflict`, lalu `GET /claims/{from}/{id}` menunjukkan `completed`.
- `:complete`, `:release`, dan `:request-input` setelah pembatalan: `409 task_transition`.
- `:release` pada percobaan terakhir: pesan `gagal`, task `failed`.
- `:request-input` lalu `:input` lalu klaim: `input_reply` muncul dan percobaan tidak bertambah.
- Berkas token hilang saat start: server menolak start.
- Berkas token dengan label `default` atau label ganda: server menolak start.
- Berkas token dihapus saat berjalan: token berkas ditolak, token env tetap diterima.
- Token lease tidak muncul di `GET`, SSE, Agent Card, log, atau respons error.
- Teks 65536 byte di `question` dan `reply`: ditolak, tidak terpotong.
- Body di atas 64 KiB: `413` sebelum parse.

## 13. Urutan dan keputusan PR #30

Keputusan: **implementasi boleh dimulai sebelum PR #30 merge, tetapi hanya untuk P1 sampai P3 di `relay` dan CLI `relay ack`.** P1 sampai P3 tidak menyentuh `tokens.rs`. Semua kode autentikasi, loader berkas, pemeriksaan label, dan rute HTTP menunggu merge PR #30, karena memakai `TokenSet` dan `load` dari sana.

1. Opus 5.5 menyetujui revisi 5.
2. P1 sampai P3 di `main` terbaru, dengan tes `-p plaesy-adapter-relay --locked` dan `-p plaesy-interface-cli --locked`. Migrasi P3 diuji dengan basis data versi sebelumnya.
3. Merge PR #30.
4. Loader token agen, validasi label saat load, pemeriksaan label untuk SSE dan rute, dan rute `/claims`, dengan tes `-p plaesy-interface-cli --locked`.
5. Perbarui `agent-protocol-design.md` (§11 butir 2 dan 3) dan `agent_http.rs:11-12`.
