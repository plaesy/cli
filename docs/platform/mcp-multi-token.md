# Token MCP per klien

Status: diimplementasikan di cabang `ao/cli-141/mcp-multi-token`. Belum di-deploy. Pilihan teknis di sini adalah default yang didelegasikan dan dapat dicabut pemilik (lihat `decisions.md`).

## Masalah

`plaesy memory serve --mcp --transport http` memeriksa satu nilai bearer (`PLAESY_MCP_TOKEN` atau `--auth-token`). Semua klien berbagi satu rahasia, sehingga mencabut satu klien (misalnya laptop) berarti merotasi rahasia untuk semua klien, termasuk server.

## Tujuan

- Banyak token bernama (label), satu per klien.
- Setiap label dapat dirotasi dengan masa tenggang dan dicabut, tanpa mengganggu label lain.
- Server tidak menyimpan token dalam bentuk teks biasa.
- Rotasi dan pencabutan berlaku tanpa restart layanan.
- Kompatibel mundur: `PLAESY_MCP_TOKEN` tunggal tetap bekerja.

## Format dan penyimpanan

Format token: `plsy_<id>_<secret>`.

- `id`: 12 karakter hex dari UUID v4. Bukan rahasia; dipakai sebagai kunci pencarian.
- `secret`: 64 karakter hex dari dua UUID v4 (sekitar 244 bit acak). Dibuat dengan crate `uuid` yang sudah ada, jadi tidak ada dependensi baru.

Berkas penyimpanan: `~/.config/plaesy/mcp-tokens.toml`, dapat diganti dengan `PLAESY_MCP_TOKENS_FILE`.

```toml
[[token]]
label = "laptop"
enabled = true
created = 1795000000          # unix detik
id = "a1b2c3d4e5f6"           # non-rahasia
salt = "<hex 32 byte>"
hash = "<hex sha256(salt || token)>"
# Hanya ada selama masa tenggang rotasi:
previous_id = "..."
previous_salt = "..."
previous_hash = "..."
previous_expires = 1795086400
```

- Berkas hanya menyimpan hash. Token plaintext hanya tercetak ke stdout sekali saat `add` atau `rotate`.
- Salt per entri membuat hash tidak bisa dibandingkan antar entri.
- Label: `^[a-z0-9-]{1,32}$`. Label `default` dicadangkan untuk `PLAESY_MCP_TOKEN`.

## Verifikasi

1. Ambil `id` dari token yang dikirim. Jika format tidak cocok, tolak.
2. Cari entri (current atau previous) dengan `id` itu. Jika tidak ada, tetap hitung hash dummy agar waktu respons seragam.
3. Hitung `sha256(salt || token)` dan bandingkan dengan `hash` memakai perbandingan waktu-konstan. Slot previous hanya dicek jika `previous_expires` belum lewat terhadap waktu sekarang saat permintaan.
4. Entri dengan `enabled = false` ditolak.

Perbandingan selalu menjalankan seluruh panjang hash, tanpa keluar lebih awal.

## Mode autentikasi dan fail-closed

- Autentikasi wajib sejak sumber token pertama terlihat: `PLAESY_MCP_TOKEN` atau `--auth-token` saat startup, atau berkas token yang muncul (dan dimuat ulang paling lama 2 detik kemudian). Setelah itu wajib selama proses berjalan.
- Jika berkas token menjadi kosong atau dihapus setelah pernah terlihat, semua permintaan ditolak (401). Mode tidak pernah kembali ke tanpa autentikasi.
- Akibatnya, server yang dimulai tanpa token terbuka sampai `plaesy memory token add` pertama kali membuat berkas. Permintaan dalam jendela 2 detik itu masih lolos.
- Jika berkas gagal diuraikan saat dimuat ulang, server tetap memakai himpunan token terakhir yang valid dan mencatat galat ke stderr tanpa isi berkas.
- Jika tidak ada `PLAESY_MCP_TOKEN` dan berkas token tidak ada, perilaku tetap seperti sekarang: hanya peringatan. Ini kasus kompatibilitas.
- `PLAESY_MCP_TOKEN` tetap memakai aturan 32 byte dan tanpa spasi di ujung.

## Hot reload

- Setiap permintaan autentikasi memeriksa metadata berkas (mtime dan ukuran) paling sering setiap 2 detik. Jika berubah, berkas dimuat ulang.
- Tidak memakai SIGHUP (klien laptop berjalan di Windows) dan tidak memakai watcher `notify` pada berkas, karena rename atomik memutus watcher.
- Perintah CLI menulis berkas secara atomik: berkas sementara di direktori yang sama, lalu rename. Di Unix berkas dibuat dengan mode 0600.
- Rollout pertama memerlukan satu restart untuk memuat biner baru. Setelah itu `add`, `rotate`, dan `revoke` tidak perlu restart.

## Perintah CLI

Perintah berada di bawah `plaesy memory`, karena server yang melayani adalah `memory serve`. `plaesy mcp` tetap berarti server proyek (stdio).

```text
plaesy memory token add <label>
plaesy memory token rotate <label> [--grace-hours 24]
plaesy memory token revoke <label>
plaesy memory token list
```

`add` dan `rotate` mencetak token ke stdout sekali. Pesan lain ke stderr. Berkas ditulis lewat `PLAESY_MCP_TOKENS_FILE` atau lokasi default.

## Rotasi dan pencabutan

- `rotate <label> [--grace-hours N]` (default 24): token baru menjadi `current`. Token lama pindah ke `previous` dan tetap valid sampai `previous_expires`. Rotasi kedua menghapus `previous` yang lama.
- `revoke <label>`: `enabled = false` dan `previous` dihapus. Entri tetap ada agar `list` menampilkan label yang dicabut.
- `add <label>`: membuat entri baru. Menolak label yang sudah ada.
- `list`: menampilkan label, status, `created`, dan `previous_expires` jika ada. Tidak pernah mencetak hash, salt, atau token.

## Log dan batas percobaan

- Log sukses: label, path berkas, dan waktu (timestamp tracing). Tidak ada token, hash, atau isi catatan.
- Log gagal: tanpa label dan tanpa token. Log gagal dibatasi satu baris per 10 detik, dengan jumlah kegagalan sejak baris terakhir.
- Tidak ada penguncian global. Server berada di belakang tunnel loopback, sehingga batas per alamat IP tidak berarti. Penguncian global bisa mengunci klien yang sah. Risiko: penyerang bisa menghasilkan banyak log, tetapi tidak bisa memblokir token yang valid.
- Stdout hanya untuk protokol MCP dan token yang dicetak CLI. Diagnostik ke stderr.

## Keamanan lokasi

- Berkas token ditolak jika berada di dalam `watch_dir` vault. Vault disinkronkan (ada `.stversions`), dan hash tidak boleh ikut tersinkron ke perangkat lain.
- Di Unix, izin berkas selain 0600 menghasilkan peringatan saat dimuat.

## Non-tujuan

- Tidak ada akun pengguna, peran, atau kedaluwarsa per token selain masa tenggang rotasi.
- Tidak ada pembatasan per alat MCP.
- Tidak ada audit log terpisah di luar log tracing.

## Pengujian

Logika dibuat sebagai fungsi murni dan diuji di `tokens_test.rs`: kompatibilitas env tunggal, dua label dengan satu dicabut, rotasi sebelum dan sesudah masa tenggang, berkas kosong (401 tanpa mematikan autentikasi), berkas rusak (himpunan terakhir dipertahankan), log tanpa token, dan berkas tersimpan tanpa plaintext.
