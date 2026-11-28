# Threat model: backend server Plaesy

Status: draft tahap 1. Dasar: kode di `origin/main` `9db81a5`. Yang belum ada dalam kode ditandai "belum".

## Aset

| Aset | Di mana | Catatan |
|---|---|---|
| Dokumen IR (flow, api, data, app, design) | SQLite (`DocumentStore`) | Sumber kebenaran; satu tenant tidak boleh membaca tenant lain |
| Token perangkat | Hanya hash SHA-256 di DB (slice 3) | Plaintext hanya tampil sekali saat dibuat |
| Audit log | Tabel `audit_events` (slice 6); sekarang `AuditLog` in-memory | Harus append-only |
| Secret server (kunci, bila ada) | `$CREDENTIALS_DIRECTORY`, bukan config | Tidak boleh masuk log |
| Backup DB | Berkas hasil `VACUUM INTO` | Berisi hash token dan dokumen; izin berkas ketat |

## Aktor dan permukaan serang

- Klien browser (cli-125) lewat WireGuard atau internet, melalui Caddy di host.
- Perangkat dengan token yang dicuri atau masih aktif setelah dicabut.
- Pengguna sah dari project lain (lateral movement antar tenant).
- Penyerang jaringan (MITM, baca lalu lintas HTTP tanpa TLS).
- Halaman web asing yang memanggil API dari browser (CORS, CSRF, CSWSH).
- Klien yang memalsukan `X-Forwarded-For` untuk menghindari rate limit.
- Dokumen berbahaya: IR besar, nested dalam, atau payload codegen yang memakan CPU.
- Operator yang salah konfigurasi (bind non-loopback tanpa TLS).

## Analisis STRIDE

| Kategori | Ancaman | Kondisi kode saat ini | Mitigasi | Slice |
|---|---|---|---|---|
| Spoofing | Pemanggil tanpa identitas | Belum ada auth. Semua rute terbuka. Router belum dipasang. | Bearer token perangkat, hash at rest, perbandingan konstan | 3, 4 |
| Spoofing | Token bocor lewat log atau URL | Belum ada token | Token hanya di header `Authorization`; tidak pernah di query; log menyaring header auth | 2, 3 |
| Spoofing | Asal permintaan palsu dari browser | Belum ada CORS | Allowlist origin eksplisit, default kosong; tidak pernah `*` bersama kredensial | 7 |
| Tampering | Dua penulis menimpa satu sama lain (lost update) | `put` menimpa tanpa cek | ETag dan `If-Match`; 412 saat konflik, 428 saat header wajib hilang | 5 |
| Tampering | IR tidak valid tersimpan lalu merusak klien | `put` tidak memvalidasi | `validate_document` pada PUT; 422 bila error | 7 |
| Tampering | Audit dimodifikasi | `AuditLog` in-memory | Tabel append-only dengan trigger penolak UPDATE dan DELETE | 6 |
| Repudiation | Tidak tahu siapa mengubah apa | Tidak ada audit persisten | Audit per keputusan authz dan per tulis, dengan request id | 4, 6 |
| Information disclosure | Lintas tenant: token project A membaca project B | Kunci tidak punya tenant | Authz per project; rute tenant (D1); deny by default | 4 |
| Information disclosure | Galat membocorkan path DB atau stack | `error_response` menampilkan pesan `StoreError` | Galat generik ke klien; detail hanya ke stderr | 2 |
| Information disclosure | Backend terekspos publik tanpa TLS | Belum ada listener | Default loopback; non-loopback hanya dengan dua opt-in; TLS di Caddy | 1, 9 |
| Information disclosure | Backup atau DB bisa dibaca user lain | Belum ada | Izin berkas 0600 untuk DB dan backup; `StateDirectory` systemd | 9 |
| Denial of service | Body besar atau tak terbatas | Belum ada batas | Body limit 1 MiB; timeout per request | 2 |
| Denial of service | Handler sinkron memblokir runtime tokio | Store sinkron di handler | `spawn_blocking`; pool atau mutex sempit | 2 |
| Denial of service | Codegen atau validasi CPU berat | Tanpa batas | Timeout, batas ukuran IR, rate limit | 2, 7 |
| Denial of service | Flood koneksi WebSocket | Belum ada | Batas koneksi per principal dan global; idle timeout | 8 |
| Denial of service | Brute force token | Belum ada | Rate limit per IP; token 256 bit membuat brute force tidak praktis | 7 |
| Spoofing IP | X-Forwarded-For dipalsukan untuk melewati rate limit | Belum ada | Dibaca hanya bila peer adalah loopback dan terdaftar di `trusted_proxies` | 7 |
| Elevation | Route lama tetap aktif saat auth aktif | Belum ada | Saat auth aktif, rute `/v1/documents/...` tanpa tenant dimatikan | 4 |
| Elevation | Token dicabut masih dipakai | Belum ada | Cek status cabut pada setiap request; tes cabut | 3 |
| Cross-site WebSocket hijacking | Halaman asing membuka WS memakai kredensial pengguna | Belum ada | Tiket sekali pakai TTL 60 detik; cek `Origin` saat upgrade | 8 |
| Injection | SQL injection | `rusqlite` dengan parameter (perlu verifikasi di `adapter/sqlite`) | Semua query memakai parameter; audit query saat slice 5 | 5 |
| Injection | Path traversal lewat `kind` atau `id` | `check_key` menolak kosong, >256, dan karakter kontrol | Tambahkan uji untuk `/`, `..`, dan karakter Unicode; `kind` berupa allowlist | 2 |
| Supply chain | Dependensi baru tanpa tinjauan | Lock dikunci `--locked` | Setiap edge baru harus disetujui (kolom Lock di plan) | Semua |
| Konfigurasi | Bind non-loopback tanpa TLS | Relay sudah punya pola ini | Gerbang dua opt-in di config | 1 |
| Konfigurasi | Secret di config plaintext | Belum ada | Secret hanya dari `$CREDENTIALS_DIRECTORY`; config menyimpan nama, bukan nilai | 1, 3 |
| Operasi | Proses mati mendadak | Crash recovery sudah ada untuk runstore | Tes restore dan integrity check pada backup | 9 |
| Operasi | Rollback skema salah | Migrasi saat open | Tes membuka DB versi lama; skema lebih baru ditolak (`UnsupportedSchema`) | 3, 5, 6 |

## Ancaman yang dicatat tetapi ditunda

- Tenant antar-organisasi dan ACL per dokumen: butuh model izin baru (di luar slice).
- Identitas OIDC: menunggu IdP.
- Serangan dari dalam Caddy atau dari host yang sama: di luar batas proses. Ditangani lewat hak akses host dan `DynamicUser`.
- Side-channel waktu pada hashing SHA-256 token: token 256 bit, perbandingan konstan sudah cukup.

## Asumsi

- Caddy di host menghentikan TLS dan mengirim `X-Forwarded-For` dari peer loopback.
- Layanan hanya bind ke `127.0.0.1` dan dijangkau lewat Caddy atau WireGuard.
- Klien tidak menyimpan token di cookie. Bearer di header memakai JS memory, sehingga CSRF tidak berlaku. Bila cli-125 memilih cookie, berlaku SameSite dan token CSRF (perlu keputusan baru).
- Host dapat berjalan dengan `DynamicUser` dan `StateDirectory` (systemd).
