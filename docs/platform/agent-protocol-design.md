# Protokol agent internal: desain dan status implementasi

Status: sebagian sudah diimplementasikan (lihat "Status implementasi"). Bagian yang belum ada ditandai secara eksplisit sebagai desain.

## Tujuan

Agent di Plaesy mengirim pesan dan menerima hasil secara realtime, tanpa menunggu dependensi pihak ketiga. Tidak ada broker, tidak ada library A2A, tidak ada layanan terpisah yang harus dipasang.

## Yang sudah ada dan dipakai ulang

- **Relay lokal** (`plaesy relay`): antrean SQLite per tujuan. Pesan tahan lama dan harus di-ack.
- **Listener berbasis perubahan berkas** di `plaesy chat` dan `plaesy gateway serve`: bangun saat database relay berubah.
- **Server HTTP tanpa dependensi** (`plaesy remote serve`): sudah menangani loopback, token bearer untuk bind non-loopback, dan SSE.

## Bentuk protokol

Nama dan bentuk mengikuti pola yang sama dengan A2A supaya pemetaan nanti mudah. Ini bukan klaim kepatuhan terhadap A2A.

- **Agent Card**: `GET /.well-known/agent.json`, berisi `name`, `description`, `version`, `capabilities`, dan daftar `skills`.
- **Pesan**: `POST /message:send`. Bentuk utama: `{ "id", "from", "to", "text" }`. Bentuk alternatif: `{ "id", "from", "to", "parts" }` dengan `parts` berisi item `{ "kind": "text", "text" }` atau `{ "kind": "data", "data": {...} }`. Balasan `201` berisi `{ "id", "from", "to", "state" }`.
- **Tugas**: `GET /tasks/{from}/{id}` mengembalikan `{ "id", "from", "state" }`. Tugas diidentifikasi oleh pengirim dan ID, bukan ID saja, karena ID hanya unik per pengirim.
- **Push**: `GET /tasks/{from}/{id}/events` sebagai SSE, dengan event `task` setiap state berubah. Stream ditutup saat state terminal, dan `event: error` dikirim jika tugas tidak ada.
- **Batal**: `POST /tasks/{from}/{id}:cancel`. Mengembalikan `409` jika state tidak boleh berpindah ke `canceled`.
- **Jawaban input**: `POST /tasks/{from}/{id}:input` dengan `{ "text" }`. Hanya tugas `input-required` yang menerimanya; state lain mengembalikan `409`, teks kosong `400`, tugas tidak ada `404`. Balasan `200` berisi `{ "id", "from", "state" }` dengan state `working`.
- **Pertanyaan**: `GET` dan event SSE menyertakan `prompt` selama tugas `input-required`. Event SSE dikirim ulang bila seluruh isinya berubah, termasuk pertanyaan baru pada state yang sama.

Yang masih desain, belum ada di kode: artefak pada balasan tugas, dan part `file`.

## Status implementasi

Yang sudah ada dan teruji:

- Server: `plaesy agent serve` (default port 8765, hanya loopback). Jika `PLAESY_AGENT_TOKEN` diisi, setiap permintaan wajib `Authorization: Bearer <token>`.
- Tugas disimpan di tabel `relay_tasks` di database relay yang sama dengan pesan. Pesan dan tugas dibuat dalam satu transaksi, sehingga pesan tidak pernah ada tanpa tugasnya.
- `parts` (paling banyak 16 item) diterima sebagai alternatif `text`, tidak boleh keduanya. Pesan wajib punya minimal satu part `text` yang tidak kosong, supaya pembaca lama tetap bisa menampilkannya. Isi yang disimpan adalah `{"depth":0,"text":<gabungan part text dipisah baris baru>,"parts":[...]}`. Kunci objek `data` diurutkan, sehingga pengiriman ulang dengan urutan kunci berbeda tetap idempoten. Pengiriman `text` menyimpan byte yang sama seperti sebelumnya.
- Pengiriman ulang dengan ID dan isi yang sama bersifat idempoten: tidak ada pesan ganda, dan balasannya berisi state tugas saat ini (`201`). Isi yang berbeda untuk ID yang sama ditolak dengan `409`.
- Agent Card di `GET /.well-known/agent.json` berisi `name`, `description`, `version` (dari `CARGO_PKG_VERSION`), `capabilities`, dan `skills`. Kartu tidak memuat token, path database, atau path repositori. Kartu berada di balik aturan token yang sama dengan route lain: bila `PLAESY_AGENT_TOKEN` diisi, permintaan tanpa token yang benar ditolak `401`. `inputRequired` bernilai `true`. Kapabilitas yang belum ada (`pushNotifications`, `artifacts`) dinyatakan `false`.

Lifecycle tugas yang diterapkan oleh relay:

| Peristiwa di relay | Tugas |
| --- | --- |
| Pesan dikirim | `submitted` |
| Penerima mengklaim pesan (lease) | `working` |
| Penerima mengakui pesan (ack) | `completed`; lewat `working` bila masih `submitted` |
| Klaim dilepas dan masih ada percobaan | tetap `working` |
| Klaim dilepas pada percobaan terakhir | `failed` |
| Pengirim membatalkan | `canceled` |
| Penerima meminta input lewat lease yang masih berlaku | `input-required` |
| Pengirim menjawab (`:input`) | `working`; pesan bisa diklaim lagi dan jawabannya ikut di klaim berikutnya |

Batasan yang perlu diketahui:

- Ack berarti penerima sudah menerima dan menangani pesan. Ack tidak berarti pekerjaan yang diminta berhasil.
- Lease terakhir yang kedaluwarsa membuat pesan berstatus `gagal` tanpa menulis ke tabel, sehingga tugasnya tetap `working`. Tugas hanya menjadi `failed` lewat pelepasan klaim eksplisit.
- Tugas yang sudah `canceled` tetap bisa diklaim dan di-ack. Pembatalan tidak mencegah pengiriman, dan state tetap `canceled`.
- Listener chat membaca inbox tanpa mengklaim pesan. Akibatnya, tugas yang dikirim dari chat tetap `submitted` sampai pesannya di-ack.
- Selama `input-required`, pesan berstatus `menunggu-input` dan tidak bisa diklaim atau di-ack. Lease dilepas saat meminta input.
- Ack lewat jalur lama (`acknowledge`) tidak menerima pesan yang sedang menunggu input.
- Penerima meminta input lewat API store (`request_input`), bukan lewat HTTP, karena klaim tidak tersedia di HTTP.
- Retensi 30 hari dan perintah `plaesy agent gc` belum diimplementasikan.

## Penyimpanan dan pengiriman

- Pesan tetap disimpan di relay SQLite. Itu sumber kebenaran, sehingga pesan yang belum terkirim tidak hilang saat proses mati.
- Perubahan state ditulis ke tabel yang sama. Listener SSE membaca lewat perubahan berkas, bukan polling.

## Keamanan

- Default bind ke loopback. Bind non-loopback wajib token bearer, sama seperti `remote serve`.
- Pesan dari agent lain ditampilkan, tidak pernah dijalankan otomatis. Tindakan tetap butuh persetujuan orang.

## Di luar cakupan

- Tidak ada RabbitMQ, Kafka, atau WebSocket.
- Tidak ada adapter jaringan (Telegram, Slack). Itu keputusan terpisah dan belum disetujui.
- Tidak ada klaim kepatuhan A2A. Kalau nanti perlu bicara dengan agent vendor lain, pemetaan dilakukan sebagai pekerjaan terpisah.

## Pertanyaan terbuka

1. Siapa yang menjalankan server: satu proses per repo, atau per sesi chat?
2. Berapa lama pesan dan riwayat tugas disimpan?
3. Apakah `remote serve` dipakai ulang atau dibuat perintah baru, misalnya `plaesy agent serve`?

## Langkah berikutnya setelah disetujui

1. Tipe tugas dan state: sudah ada (`plaesy-adapter-relay::task`), dengan tes unit.
2. Tabel tugas di relay SQLite: sudah ada (`relay_tasks`, dibuat dengan `CREATE TABLE IF NOT EXISTS`). Belum ada migrasi versi.
3. Endpoint `message:send`, `tasks/{from}/{id}`, dan SSE: sudah ada, dengan tes pada socket lokal.
4. `/agent send` dan `@nama`: membuka tugas di database relay yang sama lewat store, bukan lewat HTTP.

Yang masih terbuka: artefak dan retensi (`plaesy agent gc`).

## Keputusan default (didelegasikan, bisa dicabut pemilik)

- Bentuk `parts` hanya `text` dan `data`. Part `file` belum ada, karena cara membawa berkas (inline atau URI) dan batas ukurannya belum diputuskan.
- Pesan dengan `parts` wajib punya minimal satu part `text` non-kosong. Pesan yang hanya `data` ditolak.
- Artefak pada balasan tugas belum dibangun. Siapa yang boleh menghasilkan artefak, lewat route mana, dan disimpan di mana belum diputuskan, jadi `artifacts` pada Agent Card tetap `false`.

1. Server berjalan satu per repo, bukan per sesi chat.
2. Pesan dan riwayat tugas disimpan 30 hari, lalu dihapus saat `plaesy agent gc`.
3. Perintah baru `plaesy agent serve` dibuat, bukan memakai `plaesy remote serve`, karena `remote` hanya read-only.

Keputusan default untuk permintaan input (bisa dicabut pemilik):

1. Lease tidak diperpanjang selama menunggu jawaban. Penerima melepas lease saat meminta input, dan pesan tidak diklaim ulang sampai dijawab. Menunggu manusia tidak menghabiskan percobaan.
2. Percobaan yang meminta input dikembalikan, sehingga pertanyaan tidak mendorong pesan ke `gagal`.
3. Membatalkan atau menggagalkan tugas `input-required` membuat pesannya tetap tertahan (`menunggu-input`): tidak bisa diklaim atau di-ack. Tugas `canceled` yang pesannya sudah dikirim tetap bisa diklaim seperti sebelumnya; aturan itu tidak berubah.
4. Jawaban disimpan di pesan dan dikirim bersama klaim berikutnya, bukan sebagai pesan relay baru.
