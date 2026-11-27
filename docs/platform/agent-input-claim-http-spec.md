# Spesifikasi: klaim dan permintaan input lewat HTTP, dengan identitas per klien

Status: **draf revisi 2, menunggu persetujuan pemilik.** Belum ada kode. Implementasi dimulai setelah spesifikasi ini disetujui, PR #30 (`ao/cli-141/mcp-multi-token`, token per klien) sudah di-merge, dan prasyarat store di bagian 3 sudah dijadwalkan.

Basis kode: `ao/input-required/root` di commit `2885b46` (di atas `main` `04ccf7f`). Semua rujukan `berkas:baris` di dokumen ini mengacu ke `2885b46`. Commit `993c86b` di remote adalah versi awal; `2885b46` menggantikannya dan memperbaiki pengiriman ulang SSE.

Cabang PR tetap berbasis `main` dan hanya berisi dokumen ini. Menarget cabang ke `2885b46` akan memasukkan kode input-required kembali ke diff PR #32, yang sudah dipisahkan.

## 1. Tujuan dan batas

Worker yang berjalan sebagai proses terpisah bisa mengklaim pesan, menyelesaikan atau melepasnya, dan meminta input dari pengirim, lewat HTTP di loopback. Setiap klien bertindak atas identitasnya sendiri.

Di luar cakupan: jaringan non-loopback, perpanjangan lease, ACL yang bisa dikonfigurasi per label, dan penyapu (sweeper) untuk task yang tertahan.

## 2. Kondisi sekarang (terverifikasi di `2885b46`)

- Rute HTTP: `POST /message:send`, `GET /tasks/{from}/{id}`, `GET /tasks/{from}/{id}/events`, `POST /tasks/{from}/{id}:cancel`, `POST /tasks/{from}/{id}:input`, `GET /.well-known/agent.json` (`agent_http.rs:25-50`). Klaim tidak ada di HTTP (`agent_http.rs:11-12`).
- Autentikasi: satu token `PLAESY_AGENT_TOKEN` (`agent_serve.rs`, `TOKEN_ENV`). Tidak ada identitas. `from` dan `to` dideklarasikan sendiri.
- `claim_next(recipient, lease_seconds)` tidak menerima owner. Store membuat token lease sendiri (UUID v4) dan menyimpannya di `lease_owner` (`relay.rs:510-561`). Token itu dikembalikan dalam struct `Message`.
- `acknowledge_claim`, `release_claim`, `request_input` menerima `owner` (`relay.rs:564`, `593`, `319`). `request_input` dan `provide_input` memakai `TaskNotFound` untuk `(sender, id)` yang tidak ada (`relay.rs:282-313`).
- Batas: body HTTP 64 KiB (`agent_http.rs`), `question` dan `reply` 1..=65536 byte (`relay.rs:335`, `372`), `error` maksimal 1024 byte (`relay.rs:18`, `595`), `id` 1..=128 karakter tercetak (`relay.rs:838`), lease 1..=86400 detik (`relay.rs:516`), `MAX_ATTEMPTS = 5` (`relay.rs:21`).
- Tabel `relay_messages`: `id TEXT PRIMARY KEY` secara global, dengan `UNIQUE(sender, id)` (`relay.rs:167-174`). `relay_tasks` berkunci `(sender, id)` (`relay.rs:178-185`).
- Status pesan diturunkan di SQL (`relay.rs:741-750`): `selesai`, `gagal`, `menunggu-input`, `diambil`, `menunggu`.
- CLI lokal (`plaesy relay`) sudah mencetak `lease_owner` pada klaim (`plaesy/tests/relay_cli.rs:44`, `124`) dan menghapusnya dari inbox dan pending (`cli/relay.rs:76-99`).
- PR #30 (`ao/cli-141/mcp-multi-token`, `adapter/memory/src/mcp/tokens.rs`): `TokenSet::authorize` memberi `Allowed(label)` untuk token env (label `default`) dan token file. `TokenAuth::new` mengembalikan mode `Open` bila tidak ada sumber token, dan menandai `required` hanya setelah sumber pernah terlihat.

## 3. Prasyarat perubahan store

Tiga cacat yang ada sekarang harus diperbaiki sebelum rute klaim dibuka. Tiap perbaikan punya tes sendiri.

- **P1. Klaim pada task yang sudah dibatalkan.** `claim_next` memilih pesan tanpa memeriksa `relay_tasks.state`. `advance_task` diam saat transisi tidak valid (`relay.rs:816-818`). Jadi pesan dari task `canceled` tetap bisa diklaim dan dikerjakan. Perbaikan: `claim_next` melewati task dengan state `canceled`, `failed`, `completed`, atau `input-required` (SQL join ke `relay_tasks`).
- **P2. Selesai atau lepas pada task yang bukan `working`.** `acknowledge_claim` mengubah pesan menjadi selesai lalu `complete_task` diam bila transisi tidak valid (`relay.rs:835`). Hasilnya: `:complete` pada task `canceled` tetap mengembalikan sukses. Perbaikan: `acknowledge_claim`, `release_claim`, dan `request_input` memeriksa state task dalam transaksi yang sama. Transisi tidak valid mengembalikan `TaskTransition` dan tidak mengubah apa pun.
- **P3. Id per pengirim.** `PRIMARY KEY` pada `relay_messages.id` membuat id unik secara global. Akibatnya pengirim A bisa mendeteksi id milik pengirim B lewat `409 idempotency_conflict`. Perbaikan: `PRIMARY KEY` menjadi `(sender, id)`, dengan migrasi saat membuka basis data lama dan tes yang membuka basis data versi sebelumnya (sesuai aturan skema di `CLAUDE.md`).

Penyapu task yang tertahan (lihat bagian 9) bukan prasyarat, tetapi dicatat sebagai pekerjaan terpisah.

## 4. Identitas, prioritas sumber, dan status mode

### Sumber token

- `PLAESY_AGENT_TOKEN`: label `default`.
- `PLAESY_AGENT_TOKENS_FILE`: berkas terpisah dari `PLAESY_MCP_TOKENS_FILE`, agar revokasi token memori tidak memutus worker. Format dan `TokenSet` dipakai ulang dari PR #30. Label `default` dilarang di berkas (`validate_label`).
- Keduanya boleh diisi bersamaan. Keduanya aktif. Label dari env selalu `default`.

### Mode ditentukan saat start

Mode ditentukan dari konfigurasi saat server start. Tidak ada state yang disimpan.

| Konfigurasi saat start | Mode | Catatan |
|---|---|---|
| Tidak ada env dan tidak ada berkas | `open` | Sama seperti sekarang. |
| Env atau berkas diisi | `token` | Autentikasi wajib. |
| Berkas diisi tetapi hilang atau gagal dibaca saat start | server **menolak start** | Fail closed. Ini berbeda dari `TokenAuth` PR #30, yang memberi mode `Open` ketika tidak ada sumber yang pernah terlihat. Agent serve memakai loader sendiri. |

- Berkas dihapus saat server berjalan: server tetap menolak semua permintaan (perilaku PR #30). Berkas dihapus saat server mati: start berikutnya gagal, karena konfigurasi masih menunjuk ke berkas itu.
- Rotasi dan revokasi berlaku tanpa restart, dengan pemeriksaan ulang berkas tiap 2 detik (perilaku PR #30).

## 5. Aturan penerima per mode (tegas)

Label adalah nama agen. Aturan di tabel ini berlaku untuk setiap rute.

| Mode | `/claims/*` | `POST /message:send` | `GET /tasks/{from}/{id}`, `:cancel`, `:input` |
|---|---|---|---|
| `open` | **404, tidak tersedia.** Klaim butuh identitas. | Perilaku lama: `from` dan `to` bebas. | Perilaku lama, tanpa pemeriksaan label. |
| `token` dengan env (`default`) | Hanya inbox `default`. Setiap klaim dan lease dicek terhadap `default`. | `from` harus `default`. `to` bebas. Selain itu `403`. | Hanya bila `from` = `default`. Selain itu `404`. |
| `token` dengan berkas (label `L`) | Hanya inbox `L`. | `from` harus `L`. `to` bebas. Selain itu `403`. | Hanya bila `from` = `L`. Selain itu `404`. |

Catatan migrasi: pemanggil `token` yang sebelumnya mengirim `from` selain label-nya akan mendapat `403` (kirim) atau `404` (baca dan jawab). Ini perubahan yang disengaja. Perlu disebut di CHANGELOG dan `decisions.md` saat implementasi.

## 6. Rute klaim

Semua rute memerlukan token pada mode `token`. Penerima selalu label. Tidak ada parameter penerima di jalur.

| Rute | Body | Memanggil | Respons sukses |
|---|---|---|---|
| `POST /claims:next` | `{"lease_seconds"?: 300}` | `claim_next(label, n)` | `200`: `{id, from, text, attempts, lease: {token, expires_at_epoch_ms}, state: "working", input_reply?}`. Tidak ada pesan: `204`. |
| `GET /claims/{id}` | - | `find(id)` lalu cek penerima | `200`: `{id, from, state, status}`. Tanpa token. Dipakai untuk memeriksa hasil setelah balasan hilang. |
| `POST /claims/{id}:complete` | `{"lease"}` | `acknowledge_claim(id, label, token)` | `200`: `{id, state}` dengan `state` dibaca dari `task_state`, bukan asumsi. |
| `POST /claims/{id}:release` | `{"lease", "error"?}` | `release_claim(id, label, token, error)` | `200`: `{id, status, state}`, dengan `status` `menunggu` atau `gagal`, dan `state` dari `task_state`. |
| `POST /claims/{id}:request-input` | `{"lease", "question"}` | `request_input(id, label, token, question)` | `200`: `{id, state: "input-required"}` dengan `state` dari `task_state`. |

- `text` diambil dari body JSON pesan (`{"depth", "text"}`, `relay.rs` dan `agent_http.rs` `send_message`). Bila body tidak bisa di-parse, `text` berisi body mentah dan `body_format: "raw"`.
- `lease_seconds` default 300. Di luar `1..=86400`: `400 invalid_lease`.
- `question` 1..=65536 byte. `error` 0..=1024 byte, boleh kosong.
- `input_reply` hanya muncul bila pengirim sudah menjawab.
- Perpanjangan lease tidak ada. Worker harus selesai sebelum lease habis atau melepasnya.

## 7. Token lease

- Token adalah UUID v4 yang dibuat `claim_next` (122 bit acak). **Tidak ada perubahan signature**, dan format `<label>.<nonce>` dan klaim 128-bit dari revisi sebelumnya dihapus.
- Pengikatan ke label tidak lewat format token. Server selalu memanggil store dengan `recipient = label`, dan store mencocokkan `recipient`, `id`, dan `lease_owner` sekaligus (`relay.rs:564-587`). Token dari label lain tidak cocok dengan penerima ini.
- Token dikembalikan sekali di respons klaim. Tidak ada di `GET`, SSE, Agent Card, inbox, log, atau respons error.
- Jaminan kebocoran (berlaku untuk HTTP dan jalur baru):
  - Respons HTTP selain klaim tidak memuat `lease_owner`. Store mengembalikan `Message` lengkap dari `find`, jadi server harus membuang field itu sendiri.
  - Log `agent serve` tidak mencetak header `Authorization`, body permintaan, atau token lease.
  - Tes: token tidak muncul di `GET /claims/{id}`, `GET /tasks/...`, SSE, Agent Card, dan stderr saat klaim.
- Keputusan CLI lokal: `plaesy relay claim` tetap mencetak `lease_owner`, karena tes yang ada bergantung padanya dan CLI hanya dijalankan pemilik mesin. Ini dicatat sebagai pertanyaan terbuka (bagian 11).

## 8. Pemetaan error, status, dan retry

### Status dan kode

| Kode | `code` | Kapan |
|---|---|---|
| `200` | - | Berhasil. |
| `201` | - | Pesan dikirim (tidak berubah). |
| `204` | - | Tidak ada pesan untuk diklaim. Bukan error. |
| `400` | `invalid_body`, `invalid_id`, `invalid_lease`, `invalid_question`, `invalid_reply`, `invalid_error` | Body bukan JSON, field tidak valid, atau melewati batas byte di bagian 2. |
| `401` | `unauthorized` | Token hilang atau salah pada mode `token`. |
| `403` | `forbidden` | Hanya pada `send` ketika `from` bukan label. Pada jalur lain, kasus ini dibuat `404` agar keberadaan id tidak bocor. |
| `404` | `not_found` | Id tidak ada, bukan milik label ini, atau mode `open` untuk `/claims`. Body dan teks identik untuk semua kasus ini. |
| `409` | `lease_conflict` | Lease tidak dipegang, habis, atau token salah. Hanya dikirim bila id memang milik label ini. |
| `409` | `task_transition` | State task tidak mengizinkan aksi (misalnya `:input` pada task yang bukan `input-required`, atau `:complete` pada task `canceled`, setelah P2). |
| `409` | `idempotency_conflict` | Id yang sama dengan isi berbeda, dari pengirim yang sama. |
| `413` | `body_too_large` | Body di atas 64 KiB. Dicek sebelum parse. |

Pemetaan dari store:

- `LeaseConflict` menjadi `404 not_found` bila `find(id)` tidak ada atau penerimanya bukan label. Menjadi `409 lease_conflict` bila id memang milik label.
- `TaskNotFound` menjadi `404 not_found`.
- `TaskTransition` menjadi `409 task_transition`.
- `IdempotencyConflict` menjadi `409 idempotency_conflict`.
- `InvalidLease` menjadi `400 invalid_lease`.
- `Invalid(msg)` menjadi `400 invalid_*` sesuai field.

Body error tetap memuat `error` (teks) untuk kompatibilitas. `code` adalah tambahan.

### Retry dan replay

- **Send:** replay dengan id dan isi yang sama mengembalikan state task saat ini (perilaku `send_task` yang ada). Isi berbeda: `409 idempotency_conflict`.
- **Klaim (`:next`):** tidak idempoten. Setiap klaim yang berhasil menambah satu percobaan. Jika respons klaim hilang di jalan, lease yang tidak diketahui klien itu akan habis dan memakan satu percobaan. Ini didokumentasikan, bukan disembunyikan.
- **`:complete`, `:release`, `:request-input` yang diulang setelah berhasil:** lease sudah dilepas, jadi balasan kedua adalah `409 lease_conflict`. Klien yang tidak yakin hasil sebelumnya harus memanggil `GET /claims/{id}` lalu membaca `status` dan `state`. Replay tidak dibuat sukses, karena store tidak bisa membedakan replay dari lease yang salah tanpa menyimpan riwayat.
- **Lease habis sebelum selesai:** pesan kembali ke `menunggu` (atau `gagal` bila percobaan sudah `MAX_ATTEMPTS`). Lihat bagian 9.

## 9. Lease habis dan task tertahan

- Lease habis tanpa `:release` tidak mengubah state task. Task tetap `working` sampai ada klaim berikutnya atau sampai ada keputusan lain.
- Setiap klaim memakan satu percobaan (`relay.rs:540`), termasuk klaim yang lease-nya habis. Ada 5 percobaan.
- Setelah percobaan ke-5 habis tanpa `:release`, status pesan menjadi `gagal` (dari SQL status) tetapi task tetap `working`. Penyebabnya: tidak ada yang mengubah state task saat status pesan berubah. Ini celah yang sudah ada dan tidak dikoreksi di spec ini. Perbaikannya adalah pekerjaan terpisah (penyapu yang memindahkan task ke `failed`), dan dicatat di bagian 11.
- `:release` pada percobaan terakhir mengubah pesan ke `gagal` dan task ke `failed` (`relay.rs:632-638`). Pada percobaan lain, task tetap `working`.

## 10. Kompatibilitas `POST /tasks/{from}/{id}:input`

- **Mode `open`:** tidak berubah. Rute, status, dan body sama.
- **Mode `token`:** `from` harus label. Selain itu `404` (lihat bagian 5). Respons sukses tidak berubah.
- Jawaban tetap disimpan di baris pesan dan dikirim lewat klaim berikutnya (`input_reply`).
- Keputusan yang sudah disetujui di `agent-protocol-design.md` (lease dilepas, percobaan dikembalikan, membatalkan task menghapus pertanyaan) tidak berubah.

## 11. Yang digantikan dan pertanyaan terbuka

### Yang digantikan (untuk langkah 3)

1. `agent_http.rs:11-12`: "No HTTP route raises `input-required`, because claims are not exposed over HTTP." Diganti dengan rute `/claims`.
2. `docs/platform/agent-protocol-design.md:59`: "Penerima meminta input lewat API store..., bukan lewat HTTP". Diganti dengan `/claims/{id}:request-input`.
3. `agent_serve.rs` `TOKEN_ENV` dan `handle`: autentikasi menghasilkan label. Loader berkas memakai `TokenSet` PR #30 dan menolak start bila berkas hilang.

### Keputusan default (didelegasikan, bisa dicabut pemilik)

1. Label adalah nama agen. Setiap label hanya bertindak atas namanya sendiri (bagian 5).
2. Mode `open` tidak punya rute klaim. Klaim membutuhkan identitas.
3. Mode `token` dengan env melayani inbox `default`. Pemanggil yang memakai `from` lain mendapat `403` atau `404`.
4. Mode ditentukan saat start, dan berkas token yang hilang saat start menolak start.
5. Lease default 300 detik, maksimum 86400, tanpa perpanjangan.
6. Token lease adalah UUID v4 dari store, dikembalikan sekali, dan tidak diubah formatnya.
7. Error 404 seragam untuk id yang bukan milik label.
8. Lease tidak diperpanjang saat menunggu input. Percobaan yang meminta input dikembalikan.
9. Replay `:complete`, `:release`, dan `:request-input` yang sudah sukses dibalas `409 lease_conflict`. Pemulihan lewat `GET /claims/{id}`.

### Pertanyaan terbuka

1. CLI lokal `plaesy relay claim` tetap mencetak token lease (default di atas). Apakah perlu redaksi di CLI juga?
2. Penyapu task yang tertahan (bagian 9): dikerjakan di langkah ini atau terpisah? Default: terpisah.
3. Apakah `tokens.rs` dipindah ke crate bersama sekarang? Default: nanti, setelah ada pengguna ketiga.

## 12. Uji yang diharapkan (langkah 3)

Store:

- P1: `claim_next` tidak mengembalikan pesan dari task `canceled`, `failed`, `completed`, atau `input-required`.
- P2: `acknowledge_claim`, `release_claim`, dan `request_input` pada task non-`working` mengembalikan `TaskTransition` dan tidak mengubah pesan.
- P3: dua pengirim boleh memakai id yang sama. Basis data versi sebelumnya dibuka dan dimigrasikan.

HTTP:

- Mode `open`: `/claims/*` mengembalikan `404`. Semua perilaku lama tetap (regresi).
- Klaim tanpa pesan: `204`.
- Token lease label A tidak bisa dipakai label B: `404` (bukan `409`) bila id milik B.
- Id milik label lain: `404` dengan body identik dengan id yang tidak ada.
- Replay `:complete` setelah sukses: `409 lease_conflict`, lalu `GET /claims/{id}` menunjukkan `completed`.
- `:complete` pada task `canceled`: `409 task_transition` dan pesan tetap ada.
- `:release` pada percobaan terakhir: pesan `gagal`, task `failed`.
- `:request-input` lalu `:input` lalu klaim: `input_reply` muncul dan percobaan tidak bertambah.
- Berkas token hilang saat start: server menolak start.
- Token lease tidak muncul di `GET`, SSE, Agent Card, log, atau respons error.
- Body di atas 64 KiB: `413` sebelum parse.

## 13. Urutan

1. Setujui spesifikasi ini (revisi 2).
2. Merge PR #30.
3. Kerjakan P1 sampai P3 di `main` terbaru, dengan tes `-p plaesy-adapter-relay --locked`.
4. Implementasi rute dan autentikasi, dengan tes `-p plaesy-interface-cli --locked`.
