# Protokol agent internal: desain (belum diimplementasikan)

Status: desain untuk ditinjau. Tidak ada kode yang mengikuti dokumen ini sampai pemilik menyetujuinya.

## Tujuan

Agent di Plaesy mengirim pesan dan menerima hasil secara realtime, tanpa menunggu dependensi pihak ketiga. Tidak ada broker, tidak ada library A2A, tidak ada layanan terpisah yang harus dipasang.

## Yang sudah ada dan dipakai ulang

- **Relay lokal** (`plaesy relay`): antrean SQLite per tujuan. Pesan tahan lama dan harus di-ack.
- **Listener berbasis perubahan berkas** di `plaesy chat` dan `plaesy gateway serve`: bangun saat database relay berubah.
- **Server HTTP tanpa dependensi** (`plaesy remote serve`): sudah menangani loopback, token bearer untuk bind non-loopback, dan SSE.

## Bentuk protokol

Nama dan bentuk mengikuti pola yang sama dengan A2A supaya pemetaan nanti mudah. Ini bukan klaim kepatuhan terhadap A2A.

- **Agent Card**: `GET /.well-known/agent.json`, berisi `name`, `description`, `version`, `capabilities`, dan daftar `skills`.
- **Pesan**: `POST /message:send` dengan `{ "to": "<agent>", "parts": [{ "type": "text", "text": "..." }] }`. Balasannya berupa ID tugas.
- **Tugas**: `GET /tasks/{id}` mengembalikan `state` dan artefak. State: `submitted`, `working`, `input-required`, `completed`, `failed`, `canceled`.
- **Push**: `GET /tasks/{id}/events` sebagai SSE. Klien tetap tersambung, dan setiap perubahan state dikirim seketika.
- **Batal**: `POST /tasks/{id}:cancel`.

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

1. Tipe tugas dan state di `plaesy-interface-cli`, dengan tes unit.
2. Tabel tugas di relay SQLite, dengan migrasi.
3. Endpoint `message:send`, `tasks/{id}`, dan SSE, dengan tes pada socket lokal.
4. Hubungkan `/agent send` ke `message:send`.

## Keputusan default (didelegasikan, bisa dicabut pemilik)

1. Server berjalan satu per repo, bukan per sesi chat.
2. Pesan dan riwayat tugas disimpan 30 hari, lalu dihapus saat `plaesy agent gc`.
3. Perintah baru `plaesy agent serve` dibuat, bukan memakai `plaesy remote serve`, karena `remote` hanya read-only.

Implementasi bertahap: model tugas dan transisi state lebih dulu, dengan tes unit. Endpoint HTTP dan SSE setelahnya.
