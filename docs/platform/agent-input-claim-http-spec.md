# Spesifikasi: klaim dan permintaan input lewat HTTP, dengan identitas per klien

Status: **draf revisi 3, menunggu review Opus 5.5 lalu persetujuan pemilik.** Belum ada kode.

Urutan kerja (bagian 13): prasyarat store P1 sampai P3 boleh dikerjakan setelah revisi ini disetujui, karena tidak bergantung pada PR #30. Kode autentikasi dan token menunggu merge PR #30.

Basis kode: `ao/input-required/root` di commit `2885b46` (di atas `main` `04ccf7f`). Rujukan `berkas:baris` di dokumen ini mengacu ke `2885b46`, kecuali `tokens.rs` yang mengacu ke PR #30 (`ao/cli-141/mcp-multi-token`, `e19a9bb`).

Cabang PR tetap berbasis `main` dan hanya berisi dokumen ini. Menarget cabang ke `2885b46` akan memasukkan kode input-required kembali ke diff PR #32.

## 1. Tujuan dan batas

Worker yang berjalan sebagai proses terpisah bisa mengklaim pesan, menyelesaikan atau melepasnya, dan meminta input dari pengirim, lewat HTTP di loopback. Setiap klien bertindak atas identitasnya sendiri.

Di luar cakupan: jaringan non-loopback, perpanjangan lease, ACL yang bisa dikonfigurasi per label, dan penyapu (sweeper) untuk task yang tertahan.

## 2. Kondisi sekarang (terverifikasi di `2885b46`)

- Rute di `route()` (`agent_http.rs:25-50`): `POST /message:send`, `GET /tasks/{from}/{id}`, `POST /tasks/{from}/{id}:cancel`, `POST /tasks/{from}/{id}:input`, `GET /.well-known/agent.json`.
- Rute `GET /tasks/{from}/{id}/events` (SSE) **tidak** lewat `route()`. Ia ditangani di `agent_serve.rs` `handle()` (`:129-133`) dan `stream_task` (`:155`), dan tidak memeriksa label. Setelah autentikasi token, siapa pun yang punya token bisa membaca state dan prompt task pengirim mana pun.
- Autentikasi: satu token `PLAESY_AGENT_TOKEN` (`agent_serve.rs`, `TOKEN_ENV`). Tidak ada identitas. `from` dan `to` dideklarasikan sendiri.
- `claim_next(recipient, lease_seconds)` (`relay.rs:510-561`) tidak menerima owner. Store membuat UUID v4 sendiri dan menyimpannya di `lease_owner`, lalu mengembalikannya dalam `Message`.
- `acknowledge_claim(id, recipient, owner)` (`:564`) mengembalikan `Ok(false)` bila lease tidak cocok, bukan error. `release_claim(...)` (`:593`) mengembalikan `Ok(None)` untuk kondisi yang sama. Tidak ada `LeaseConflict` dari kedua fungsi ini.
- `request_input(id, recipient, owner, question)` (`:319`) mengembalikan `LeaseConflict` bila UPDATE tidak mengenai baris (`:338-343`). Ia sudah memeriksa transisi task di dalam transaksi (`:353`). Lease tetap dipegang bila transisi gagal.
- `provide_input(sender, id, reply)` (`:366`) mengembalikan `TaskNotFound` untuk `(sender, id)` yang tidak ada (`:380-381`).
- Batas: body HTTP 64 KiB (`agent_http.rs`), `question` dan `reply` 1..=65536 byte (`relay.rs:329`, `374`), `error` maksimal 1024 byte (`relay.rs:603`), `id` 1..=128 byte tercetak (`relay.rs:838`), lease 1..=86400 detik (`relay.rs:516`), `MAX_ATTEMPTS = 5` (`relay.rs:21`).
- Karena body HTTP dibatasi 64 KiB dan JSON menambah overhead, `question` dan `reply` 65536 byte tidak bisa penuh lewat HTTP. Batas efektif HTTP dihitung di bagian 8.
- Skema: `relay_messages.id TEXT PRIMARY KEY` global, dengan `UNIQUE(sender, id)` (`relay.rs:167-174`). `relay_tasks` berkunci `(sender, id)` (`:178-185`).
- Status pesan diturunkan di SQL (`relay.rs:741-750`): `selesai`, `gagal`, `menunggu-input`, `diambil`, `menunggu`.
- Dokumen yang ada (`docs/platform/agent-protocol-design.md:54-55`) menyatakan bahwa tugas `canceled` tetap bisa diklaim dan di-ack. Dokumen itu juga menyatakan bahwa lease terakhir yang kedaluwarsa tidak menulis ke tabel. Spec ini mengubah butir pertama (lihat §3 P1 dan §11).
- CLI lokal (`plaesy relay`) mencetak `lease_owner` pada klaim (`plaesy/tests/relay_cli.rs:44`, `124`), dan menghapusnya dari inbox dan pending (`cli/relay.rs:76-99`).
- PR #30 (`tokens.rs`): `TokenSet::authorize` memberi `Allowed(label)` untuk token env (label `default`) dan token berkas. `validate_label` hanya dipanggil dari `add` (`:216`), tidak dari `load` (`:310-327`). `TokenAuth::new` mengembalikan `Ok(None)` bila tidak ada sumber (`:430-432`). `check_at` mengembalikan `Verdict::Open` bila `required` belum pernah aktif (`:464-466`).

## 3. Prasyarat perubahan store

Empat perbaikan ini harus selesai sebelum rute klaim dibuka. Masing-masing punya tes sendiri. P1 sampai P3 tidak bergantung pada PR #30.

### P1. Klaim pada task yang sudah dibatalkan

`claim_next` (`relay.rs:510`) memilih pesan tanpa memeriksa `relay_tasks.state`. `advance_task` diam saat transisi tidak valid (`:816-818`). Akibatnya pesan dari task `canceled` tetap bisa diklaim dan dikerjakan.

Perbaikan: `claim_next` melewati pesan yang task-nya berstate `canceled`, `failed`, `completed`, atau `input-required`. Pemeriksaan memakai **LEFT JOIN** ke `relay_tasks` (atau `NOT EXISTS`), **bukan** inner join. Pesan yang dikirim lewat CLI `relay send` tidak punya baris task. Inner join akan membuat pesan itu tidak bisa diklaim selamanya.

Tes regresi: pesan tanpa baris task tetap bisa diklaim. Pesan dari task `canceled` tidak bisa diklaim.

### P2. Selesai atau lepas pada task yang bukan `working`

- `acknowledge_claim` (`:564`) mengubah pesan menjadi selesai lalu memanggil `complete_task`, yang diam saat transisi tidak valid (`:835`, `:816-818`). Hasilnya: `:complete` pada task `canceled` tetap sukses.
- `release_claim` (`:593`) tidak memeriksa state task sama sekali.
- `request_input` sudah memeriksa transisi (`:353`), jadi tidak termasuk perbaikan ini.

Perbaikan: `acknowledge_claim` dan `release_claim` memeriksa state task di transaksi yang sama. Transisi tidak valid mengembalikan `TaskTransition` dan tidak mengubah pesan.

Keputusan untuk pesan dari task `canceled` (lihat §11): pesan itu tidak bisa diklaim (P1) dan tidak bisa di-release (P2). Lease yang dipegang saat pembatalan terjadi habis sendiri. Pesan kemudian menjadi `gagal` setelah percobaan habis. Ini mengubah perilaku yang tertulis di `agent-protocol-design.md:54-55`.

### P3. Id per pengirim

`relay_messages.id` adalah `PRIMARY KEY` global. Akibatnya pengirim A bisa mendeteksi id milik pengirim B lewat `409 idempotency_conflict`, dan dua pengirim tidak bisa memakai id yang sama.

Perbaikan skema: `PRIMARY KEY` menjadi `(sender, id)`. Migrasi dijalankan saat basis data lama dibuka, dan ada tes yang membuka basis data versi sebelumnya (aturan skema di `CLAUDE.md`).

Setelah P3, setiap query yang hanya memakai `id` harus memakai `sender` atau `recipient` juga. Daftarnya dari `2885b46`:

| Baris | Fungsi | Perubahan |
|---|---|---|
| `relay.rs:236` | `send_task` (cek duplikat) | `WHERE sender=?` |
| `relay.rs:347` | `request_input` (cari sender) | ambil `sender` dari `(recipient, id)` yang sudah lolos cek lease |
| `relay.rs:388` | `provide_input` | sudah `sender=?2 AND id=?3`, tidak berubah |
| `relay.rs:441` | `find(id)` | ubah jadi `find(sender, id)`, atau `find_for_recipient(recipient, id)` untuk rute klaim |
| `relay.rs:500` | `find` lain | sama seperti `:441` |
| `relay.rs:540`, `:580`, `:614` | claim, ack, release | sudah memakai `recipient`. Tambahkan `sender` di klausa WHERE. |
| `relay.rs:548` | `claim_next` (ambil ulang) | `WHERE sender=? AND id=?` |
| `relay.rs:623` | `release_claim` (cari sender) | ikut parameter `sender` |
| `relay.rs:654` | `acknowledge` lama | `WHERE sender=? AND id=? AND recipient=?` |
| `relay.rs:732` | purge | `WHERE sender=? AND id=?` |
| `relay.rs:830` | `complete_task` (cari sender) | ambil `sender` dari argumen, bukan dari `SELECT ... WHERE id=?` |

Rute klaim karena itu memuat pengirim: `/claims/{from}/{id}...`. Tanpa `from`, dua pengirim dengan id sama ke penerima yang sama tidak bisa dibedakan.

### P4. Validasi label di loader token (di luar store, tetapi wajib sebelum rute)

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

Alasan: tanpa validasi saat load, berkas yang diedit tangan bisa membuat label `default` dan menyamar sebagai identitas token env.

Reload setiap 2 detik memakai loader agen sendiri, bukan `TokenAuth` dari PR #30. `TokenAuth` mengembalikan `Open` saat `required` belum pernah aktif, dan itu bertentangan dengan aturan fail-closed di bawah.

### Mode ditentukan saat start

| Konfigurasi saat start | Mode | Catatan |
|---|---|---|
| Tidak ada env dan tidak ada berkas | `open` | Sama seperti sekarang. |
| Env atau berkas diisi | `token` | Autentikasi wajib. |
| Berkas diisi tetapi hilang atau gagal validasi saat start | server **menolak start** | Fail closed. |

- Berkas dihapus saat server berjalan: loader agen menolak semua permintaan yang memakai token dari berkas. Token env tetap berlaku. Ini **bukan** perilaku PR #30, yang kembali ke token env saja. Ini perbedaan yang disengaja.
- Berkas dihapus saat server mati: start berikutnya gagal, karena konfigurasi masih menunjuk ke berkas itu.
- Rotasi dan revokasi berlaku tanpa restart, dengan pemeriksaan ulang berkas tiap 2 detik.

## 5. Aturan penerima per mode (tegas)

Label adalah nama agen. Tabel ini berlaku untuk setiap rute, termasuk SSE.

| Mode | `/claims/*` | `POST /message:send` | `GET /tasks/{from}/{id}`, `GET .../events`, `:cancel`, `:input` |
|---|---|---|---|
| `open` | **404, tidak tersedia.** | Perilaku lama: `from` dan `to` bebas. | Perilaku lama, tanpa pemeriksaan label. |
| `token` dengan env (`default`) | Hanya pesan dengan `recipient = default`. | `from` harus `default`. `to` bebas. Selain itu `403`. | Hanya bila `from` = `default`. Selain itu `404`. |
| `token` dengan berkas (label `L`) | Hanya pesan dengan `recipient = L`. | `from` harus `L`. `to` bebas. Selain itu `403`. | Hanya bila `from` = `L`. Selain itu `404`. |

- **SSE (`/tasks/{from}/{id}/events`):** pemeriksaan label dipasang di `agent_serve.rs` `handle()` sebelum `stream_task`, dengan aturan yang sama dengan `GET /tasks/{from}/{id}`. Tes: label lain mendapat `404`, dan tidak ada byte event yang dikirim.
- Pemanggil `token` yang sebelumnya memakai `from` selain labelnya akan mendapat `403` (kirim) atau `404` (baca, SSE, jawab). Ini perubahan yang disengaja. Perlu dicatat di CHANGELOG dan `decisions.md` saat implementasi.

## 6. Rute klaim

Semua rute memerlukan token pada mode `token`. Penerima selalu label. Pengirim wajib ada di jalur karena id hanya unik per pengirim (P3).

| Rute | Body | Memanggil | Respons sukses |
|---|---|---|---|
| `POST /claims:next` | `{"lease_seconds"?: 300}` | `claim_next(label, n)` | `200`: `{id, from, text, body_format, attempts, lease: {token, expires_at_epoch_ms}, state: "working", input_reply?}`. Tidak ada pesan: `204` tanpa body. |
| `GET /claims/{from}/{id}` | - | `find_for_recipient(label, from, id)` | `200`: `{id, from, state, status}`. Tanpa token lease. Dipakai untuk memeriksa hasil setelah balasan hilang. |
| `POST /claims/{from}/{id}:complete` | `{"lease"}` | `acknowledge_claim(id, label, token)` (dengan `sender` setelah P3) | `200`: `{id, state}` dengan `state` dibaca dari `task_state`. |
| `POST /claims/{from}/{id}:release` | `{"lease", "error"?}` | `release_claim(id, label, token, error)` | `200`: `{id, status, state}`. `status` `menunggu` atau `gagal`. `state` dari `task_state`. |
| `POST /claims/{from}/{id}:request-input` | `{"lease", "question"}` | `request_input(id, label, token, question)` | `200`: `{id, state: "input-required"}` dengan `state` dari `task_state`. |

- `text` diambil dari body JSON pesan (`{"depth", "text"}`, `agent_http.rs` `send_message`). Bila body tidak bisa di-parse, `text` kosong dan `body_format` bernilai `"raw"` dengan body mentah.
- `lease_seconds` default 300. Di luar `1..=86400`: `400 invalid_lease`.
- `question` 1..=65536 byte. `error` 0..=1024 byte, boleh kosong.
- `input_reply` hanya muncul bila pengirim sudah menjawab.
- Perpanjangan lease tidak ada.

## 7. Token lease

- Token adalah UUID v4 yang dibuat `claim_next` (122 bit acak). Tidak ada perubahan signature. Format `<label>.<nonce>` dan klaim 128-bit dari revisi sebelumnya dihapus.
- Pengikatan ke label lewat store: server memanggil dengan `recipient = label`, dan store mencocokkan `recipient`, `id`, `sender` (setelah P3), dan `lease_owner` (`relay.rs:564-587`).
- Token dikembalikan sekali di respons klaim. Tidak ada di `GET`, SSE, Agent Card, inbox, log, atau respons error.
- Jaminan kebocoran:
  - Respons HTTP selain klaim tidak memuat `lease_owner`. `find` dan `task_view` mengembalikan `Message` lengkap, jadi server membuang field itu sendiri.
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
| `409` | `lease_conflict` | Lease tidak dipegang, habis, atau token salah. Hanya bila pesan memang milik label. |
| `409` | `task_transition` | State task tidak mengizinkan aksi (misalnya `:input` pada task yang bukan `input-required`). |
| `409` | `idempotency_conflict` | Id yang sama dengan isi berbeda, dari pengirim yang sama. |
| `413` | `body_too_large` | Body di atas 64 KiB. Dicek sebelum parse. |

`write_reply` (`agent_serve.rs:292-303`) perlu ditambah 204 (tanpa body) dan 403. Saat ini belum ada.

### Pemetaan dari store

- `acknowledge_claim` mengembalikan `Ok(false)`: `409 lease_conflict` bila pesan milik label, `404 not_found` bila tidak. Pengecekan kepemilikan dilakukan lewat `find_for_recipient` sebelum panggilan store.
- `release_claim` mengembalikan `Ok(None)`: sama seperti di atas.
- `request_input` mengembalikan `LeaseConflict`: `404` bila `find` tidak menemukan pesan milik label, `409 lease_conflict` bila ya.
- `TaskNotFound`: `404 not_found`.
- `TaskTransition`: `409 task_transition`.
- `IdempotencyConflict`: `409 idempotency_conflict`.
- `InvalidLease`: `400 invalid_lease`.
- `Invalid(msg)`: `400 invalid_*` sesuai field.

Batas byte efektif lewat HTTP: body total 64 KiB. `question` dan `reply` dibatasi juga oleh `MAX_BODY_BYTES` store (65536). Jadi batas efektif adalah yang lebih kecil, yaitu sisa 64 KiB setelah overhead JSON. Tes memastikan teks 65536 byte ditolak dengan `413` atau `400`, bukan terpotong.

Body error tetap memuat `error` (teks) untuk kompatibilitas. `code` adalah tambahan.

### Retry dan replay

- **Send:** replay dengan id dan isi yang sama mengembalikan state task saat ini. Isi berbeda: `409 idempotency_conflict`.
- **Klaim (`:next`):** tidak idempoten. Setiap klaim yang berhasil menambah satu percobaan. Respons klaim yang hilang di jalan membuat lease tak diketahui, yang habis dan memakan satu percobaan. Ini didokumentasikan.
- **`:complete`, `:release`, `:request-input` yang diulang setelah berhasil:** lease sudah dilepas, jadi balasan kedua `409 lease_conflict`. Klien memeriksa hasilnya lewat `GET /claims/{from}/{id}`. Replay tidak dibuat sukses, karena store tidak menyimpan riwayat lease.
- **Lease habis sebelum selesai:** pesan kembali ke `menunggu`, atau `gagal` bila percobaan sudah `MAX_ATTEMPTS`. Lihat §9.

## 9. Lease habis dan task tertahan

- Lease habis tanpa `:release` tidak mengubah state task. Task tetap `working`.
- Setiap klaim memakan satu percobaan (`relay.rs:540`), termasuk klaim yang lease-nya habis. Ada 5 percobaan.
- Setelah percobaan kelima habis tanpa `:release`, status pesan menjadi `gagal` (dari SQL status) tetapi task tetap `working`. Tidak ada yang mengubah state task. Ini celah yang sudah ada dan tercatat di `agent-protocol-design.md:55`. Perbaikannya adalah pekerjaan terpisah (penyapu yang memindahkan task ke `failed`).
- `:release` pada percobaan terakhir mengubah pesan ke `gagal` dan task ke `failed` (`relay.rs:632-638`). Pada percobaan lain, task tetap `working`.

## 10. Kompatibilitas `POST /tasks/{from}/{id}:input`

- **Mode `open`:** tidak berubah.
- **Mode `token`:** `from` harus label (lihat §5). Respons sukses tidak berubah.
- Jawaban tetap disimpan di baris pesan dan dikirim lewat klaim berikutnya (`input_reply`).
- Keputusan yang sudah disetujui di `agent-protocol-design.md` (lease dilepas, percobaan dikembalikan, membatalkan task menghapus pertanyaan) tidak berubah.

## 11. Yang digantikan dan pertanyaan terbuka

### Yang digantikan (untuk langkah implementasi)

1. `agent_http.rs:11-12`: "No HTTP route raises `input-required`, because claims are not exposed over HTTP." Diganti dengan rute `/claims`.
2. `docs/platform/agent-protocol-design.md:59`: "Penerima meminta input lewat API store..., bukan lewat HTTP". Diganti dengan `/claims/{from}/{id}:request-input`.
3. `docs/platform/agent-protocol-design.md:54-55`: "Tugas yang sudah `canceled` tetap bisa diklaim dan di-ack..." dan "Lease terakhir yang kedaluwarsa membuat pesan berstatus `gagal` tanpa menulis ke tabel, sehingga tugasnya tetap `working`." Baris pertama dibalik oleh P1 dan P2. Baris kedua tetap berlaku sebagai celah (§9), tetapi dicatat sebagai perilaku yang dipertahankan, bukan dibalik.
4. `agent_serve.rs` `handle()`: pemeriksaan label ditambahkan untuk SSE. `TOKEN_ENV` memakai loader agen (§4).

### Keputusan default (didelegasikan, bisa dicabut pemilik)

1. Label adalah nama agen. Setiap label hanya bertindak atas namanya sendiri (§5).
2. Mode `open` tidak punya rute klaim.
3. Mode `token` dengan env melayani inbox `default`. Pemanggil yang memakai `from` lain mendapat `403` atau `404`.
4. Mode ditentukan saat start. Berkas token yang hilang atau gagal validasi saat start menolak start. Berkas yang dihapus saat berjalan menolak token berkas, sementara token env tetap berlaku.
5. Lease default 300 detik, maksimum 86400, tanpa perpanjangan.
6. Token lease adalah UUID v4 dari store, dikembalikan sekali, dan tidak diubah formatnya.
7. Pesan dari task `canceled` tidak bisa diklaim dan tidak bisa di-release. Lease yang dipegang habis sendiri. Pesan menjadi `gagal` setelah percobaan habis. Perilaku ini menggantikan `agent-protocol-design.md:54`.
8. Error `404` seragam untuk id yang bukan milik label atau bukan dari pengirim yang disebut di jalur.
9. Replay `:complete`, `:release`, dan `:request-input` yang sudah sukses dibalas `409 lease_conflict`. Pemulihan lewat `GET /claims/{from}/{id}`.
10. Rute klaim memuat `{from}` karena id unik per pengirim setelah P3.

### Pertanyaan terbuka

1. CLI lokal `plaesy relay claim` tetap mencetak token lease. Perlu redaksi?
2. Penyapu task yang tertahan (§9): dikerjakan di langkah ini atau terpisah? Default: terpisah.
3. Apakah `tokens.rs` dipindah ke crate bersama sekarang? Default: nanti.

## 12. Uji yang diharapkan

Store (P1 sampai P3, boleh dikerjakan sebelum PR #30 merge):

- P1: `claim_next` tidak mengembalikan pesan dari task `canceled`, `failed`, `completed`, atau `input-required`.
- P1: pesan tanpa baris task (dari `relay send`) tetap bisa diklaim.
- P2: `acknowledge_claim` dan `release_claim` pada task non-`working` mengembalikan `TaskTransition` dan tidak mengubah pesan.
- P3: dua pengirim boleh memakai id yang sama ke penerima yang sama, dan klaim memilih pesan dengan `from` yang benar.
- P3: basis data versi sebelumnya dibuka dan dimigrasikan.
- P3: setiap query id-only di tabel §3 sudah memakai `sender` atau `recipient`.

HTTP (setelah PR #30 merge):

- Mode `open`: `/claims/*` mengembalikan `404`. Semua perilaku lama tetap (regresi).
- Klaim tanpa pesan: `204` tanpa body.
- SSE: label lain mendapat `404`, dan tidak ada byte event yang dikirim.
- Token lease label A tidak bisa dipakai label B: `404` bila pesan milik B.
- Id milik pengirim lain: `404` dengan body identik dengan id yang tidak ada.
- Replay `:complete` setelah sukses: `409 lease_conflict`, lalu `GET /claims/{from}/{id}` menunjukkan `completed`.
- `:complete` pada task `canceled`: `409 task_transition` (atau pesan tidak bisa diklaim sejak awal, karena P1).
- `:release` pada percobaan terakhir: pesan `gagal`, task `failed`.
- `:request-input` lalu `:input` lalu klaim: `input_reply` muncul dan percobaan tidak bertambah.
- Berkas token hilang saat start: server menolak start.
- Berkas token dengan label `default` atau label ganda: server menolak start.
- Berkas token dihapus saat berjalan: token berkas ditolak, token env tetap diterima.
- Token lease tidak muncul di `GET`, SSE, Agent Card, log, atau respons error.
- Teks 65536 byte di `question` dan `reply`: ditolak, tidak terpotong.
- Body di atas 64 KiB: `413` sebelum parse.

## 13. Urutan dan keputusan PR #30

Keputusan: **implementasi boleh dimulai sebelum PR #30 merge, tetapi hanya untuk P1 sampai P3 di `relay`.** P1 sampai P3 tidak menyentuh `tokens.rs`. Semua kode autentikasi, loader berkas, pemeriksaan label, dan rute HTTP menunggu merge PR #30, karena ia memakai `TokenSet` dan `load` dari sana.

1. Opus 5.5 menyetujui revisi 3.
2. P1 sampai P3 di `main` terbaru, dengan tes `-p plaesy-adapter-relay --locked`. Migrasi P3 diuji dengan basis data versi sebelumnya.
3. Merge PR #30.
4. Loader token agen, validasi label saat load, pemeriksaan label untuk SSE dan rute, dan rute `/claims`, dengan tes `-p plaesy-interface-cli --locked`.
5. Perbarui `agent-protocol-design.md` (§11 butir 2 dan 3) dan `agent_http.rs:11-12`.
