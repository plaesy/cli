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
- **Pesan**: `POST /message:send`. Bentuk yang diimplementasikan: `{ "id", "from", "to", "text" }`, bukan `parts` seperti rancangan awal. Balasan `201` berisi `{ "id", "from", "to", "state" }`.
- **Tugas**: `GET /tasks/{from}/{id}` mengembalikan `{ "id", "from", "state" }`. Tugas diidentifikasi oleh pengirim dan ID, bukan ID saja, karena ID hanya unik per pengirim.
- **Push**: `GET /tasks/{from}/{id}/events` sebagai SSE, dengan event `task` setiap state berubah. Stream ditutup saat state terminal, dan `event: error` dikirim jika tugas tidak ada.
- **Batal**: `POST /tasks/{from}/{id}:cancel`. Mengembalikan `409` jika state tidak boleh berpindah ke `canceled`.

Yang masih desain, belum ada di kode: `GET /.well-known/agent.json` (Agent Card), artefak pada balasan tugas, dan bagian `parts`.

## Status implementasi

Yang sudah ada dan teruji:

- Server: `plaesy agent serve` (default port 8765, hanya loopback). Jika `PLAESY_AGENT_TOKEN` diisi, setiap permintaan wajib `Authorization: Bearer <token>`.
- Tugas disimpan di tabel `relay_tasks` di database relay yang sama dengan pesan. Pesan dan tugas dibuat dalam satu transaksi, sehingga pesan tidak pernah ada tanpa tugasnya.
- Pengiriman ulang dengan ID dan isi yang sama bersifat idempoten: tidak ada pesan ganda, dan balasannya berisi state tugas saat ini (`201`). Isi yang berbeda untuk ID yang sama ditolak dengan `409`.

Lifecycle tugas yang diterapkan oleh relay:

| Peristiwa di relay | Tugas |
| --- | --- |
| Pesan dikirim | `submitted` |
| Penerima mengklaim pesan (lease) | `working` |
| Penerima mengakui pesan (ack) | `completed`; lewat `working` bila masih `submitted` |
| Klaim dilepas dan masih ada percobaan | tetap `working` |
| Klaim dilepas pada percobaan terakhir | `failed` |
| Pengirim membatalkan | `canceled` |

Batasan yang perlu diketahui:

- Ack berarti penerima sudah menerima dan menangani pesan. Ack tidak berarti pekerjaan yang diminta berhasil.
- Lease terakhir yang kedaluwarsa membuat pesan berstatus `gagal` tanpa menulis ke tabel, sehingga tugasnya tetap `working`. Tugas hanya menjadi `failed` lewat pelepasan klaim eksplisit.
- Tugas yang sudah `canceled` tetap bisa diklaim dan di-ack. Pembatalan tidak mencegah pengiriman, dan state tetap `canceled`.
- Listener chat membaca inbox tanpa mengklaim pesan. Akibatnya, tugas yang dikirim dari chat tetap `submitted` sampai pesannya di-ack.
- `input-required` ada di model state, tetapi belum ada transisi yang memakainya.
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

Yang masih terbuka: Agent Card, artefak, `input-required`, dan retensi (`plaesy agent gc`).

## Keputusan default (didelegasikan, bisa dicabut pemilik)

1. Server berjalan satu per repo, bukan per sesi chat.
2. Pesan dan riwayat tugas disimpan 30 hari, lalu dihapus saat `plaesy agent gc`.
3. Perintah baru `plaesy agent serve` dibuat, bukan memakai `plaesy remote serve`, karena `remote` hanya read-only.

Implementasi bertahap: model tugas dan transisi state lebih dulu, dengan tes unit. Endpoint HTTP dan SSE setelahnya.
