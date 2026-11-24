# Desain fitur chat: MCP, hook, bot/roles, agent-to-agent, plugin

Status: desain, belum diimplementasikan. Dokumen ini menjelaskan bentuk yang disarankan untuk lima
fitur di `plaesy chat`, dengan prinsip yang sama untuk semuanya. Setiap bagian menyebut apa yang
sudah ada di kode, supaya tidak ada klaim kemampuan yang belum terbukti.

## Prinsip bersama

1. **Satu sumber konfigurasi.** Semua pengaturan ada di `settings.json`, dengan dua cakupan:
   - repo: `<repo>/.plaesy/settings.json`, untuk pengaturan proyek yang ikut di-commit.
   - user: `~/.plaesy/settings.json`, untuk preferensi pribadi (bahasa, lokasi skill).
   Urutan prioritas: flag CLI, lalu variabel lingkungan, lalu repo, lalu user, lalu default.
   Nilai rahasia tidak pernah disimpan di file. File hanya menyebut nama variabel lingkungan,
   seperti `api_key_env` yang sudah dipakai provider.
2. **Izin eksplisit.** Tool, hook, dan plugin yang bisa mengubah file atau menjalankan perintah
   harus diizinkan lewat konfigurasi. Default-nya menolak. Ini sejalan dengan `PermissionRequest`
   yang sudah dipakai di Claude Code dan dengan `policy/approvals` di `workflow`.
3. **Tidak ada jalur jaringan diam-diam.** Sesuai aturan jaringan di `CLAUDE.md` (2026-11-23),
   setiap jalur jaringan harus eksplisit, aktif lewat konfigurasi, dan dicatat di
   `docs/platform/decisions.md`.
4. **Gagal tertutup.** Server MCP, hook, atau plugin yang gagal dimuat tidak menghentikan sesi.
   Kegagalan dicatat dan ditampilkan, lalu sesi berjalan tanpa komponen itu.
5. **Setiap fitur punya tes unit sendiri** di file `*_test.rs` di sebelah modulnya, dan
   menjalankan `cargo test -p <crate>` saja. Tidak ada uji workspace penuh sampai pemilik memintanya.

Kondisi kode saat ini, sebagai acuan:

- Provider: trait `Provider` di `support/loop`, konfigurasi `delegate.providers` di
  `.plaesy/settings.json`.
- Skill: `plaesy skills` membaca direktori skill. `chat` belum memuat skill.
- Keputusan dan pertanyaan terbuka: `/decision`, `/ask`, `/pending`, `/decisions`, `/pick`.
- Tugas: `/tasks` membaca `.plaesy/tasks/<status>/<id>/`.
- `plaesy mcp` sudah ada sebagai **server** MCP untuk folder `.plaesy/`. Chat belum menjadi
  **klien** MCP.
- Hook: hanya `loop stop-hook` (untuk loop otomatis). Chat belum punya hook.

---

## 1. MCP (klien)

**Tujuan.** Chat dapat memakai tool dari server MCP yang dikonfigurasi, misalnya server
dokumentasi atau basis data, tanpa mengubah kode chat.

**Konfigurasi** (`settings.json`):

```json
{
  "mcp": {
    "servers": [
      {
        "name": "docs",
        "transport": "stdio",
        "command": "docs-mcp",
        "args": ["--read-only"],
        "env_from": ["DOCS_TOKEN"],
        "enabled": true,
        "allow_tools": ["search", "read_page"]
      }
    ]
  }
}
```

- `allow_tools` wajib diisi. Tanpa daftar itu, tidak ada tool yang tersedia. Ini mencegah server
  menambah kemampuan baru diam-diam.
- `env_from` hanya meneruskan nama variabel yang disebut. Nilainya dibaca dari lingkungan saat
  server dijalankan.

**Perilaku.**

- Server dijalankan saat sesi pertama butuh tool, lalu dihentikan saat `/exit`.
- Daftar tool diambil lewat `tools/list`. Hanya tool dalam `allow_tools` yang dikirim ke model.
- Pemanggilan tool dicatat (nama, argumen dipotong, hasil dipotong). Argumen dan hasil yang
  mengandung nilai dari `env_from` disamarkan.
- Hasil tool dibatasi ukurannya (misalnya 16 KiB) sebelum masuk ke riwayat.

**Keamanan.** Server stdio dijalankan sebagai proses pengguna. Ini sama dengan menjalankan
perintah, jadi `command` harus berupa path yang sudah ada di konfigurasi repo atau user, bukan
hasil dari input model. Transport HTTP ditunda sampai ada keputusan tentang jaringan dan
autentikasi.

**Gagal tertutup.** Server yang gagal start atau timeout dicatat sebagai `unavailable`. Chat
tetap jalan. `/mcp` (rencana) menampilkan status tiap server.

**Uji.** Klien MCP diuji dengan server tiruan di proses yang sama (stdio), mencakup: tool
diizinkan, tool tidak diizinkan tidak terlihat, server gagal start, timeout, dan redaksi nilai
rahasia.

---

## 2. Hook

**Tujuan.** Perintah yang dijalankan pada titik tertentu di sesi, misalnya setelah `/decision`
disimpan, atau sebelum pesan dikirim ke provider.

**Titik hook** (fase pertama, sengaja sedikit):

- `session.start`, `session.end`
- `message.before_send`, `message.after_reply`
- `decision.saved`, `task.changed`

**Konfigurasi:**

```json
{
  "hooks": [
    {
      "event": "decision.saved",
      "command": "scripts/notify.sh",
      "args": ["{id}"],
      "timeout": "10s",
      "on_failure": "warn"
    }
  ]
}
```

- `args` memakai placeholder dari data event, bukan string yang disusun bebas. Nilai selalu
  dikirim sebagai argumen terpisah, tanpa shell, untuk menghindari injeksi.
- `timeout` wajib. Hook yang melewati batas dihentikan.
- `on_failure`: `warn` (default) atau `block`. `block` hanya untuk `message.before_send`, dan
  hanya jika pemilik mengaturnya sendiri.

**Keamanan.** Hook hanya dari `settings.json` repo atau user. Hook dari repo yang belum
dipercaya tidak dijalankan sampai pengguna menyetujuinya sekali (catatan persetujuan disimpan di
user, dengan hash konten hook). Ini mencegah repo baru menjalankan perintah saat dibuka.

**Uji.** Hook diuji dengan perintah tiruan (`true`, `false`, perintah yang lama). Kasus yang harus
dicakup: timeout, gagal dengan `warn` dan `block`, argumen dengan spasi atau tanda kutip, dan hook
dari repo yang belum disetujui.

---

## 3. Bot/roles (profil)

**Tujuan.** Peran yang dipilih per sesi, misalnya `reviewer`, `penulis`, atau `peneliti`. Yang
berbeda antar peran hanya profilnya: system prompt, provider default, dan daftar skill. Skill,
memori, dan plugin dipakai bersama semua peran.

**Konfigurasi:**

```json
{
  "roles": {
    "reviewer": {
      "system": "Tinjau perubahan kode. Jangan menulis kode baru.",
      "provider": "claude-cli",
      "skills": ["code-review"],
      "tools": ["read"]
    }
  }
}
```

- `plaesy chat --role reviewer`, atau `/role reviewer` di tengah sesi.
- Peran tidak punya akses lebih dari yang diberikan pengguna. `tools` hanya mempersempit, tidak
  menambah.
- Peran `default` disediakan. Tanpa `--role`, sesi memakai `default`.

**Perilaku.** Peran menentukan prompt awal. Pergantian peran di tengah sesi tidak menghapus
riwayat, tetapi system prompt baru berlaku untuk pesan berikutnya, dan pergantian dicatat di
transkrip.

**Uji.** Pemilihan peran, peran tidak dikenal, peran yang meminta `provider` yang tidak
diaktifkan, dan `tools` yang hanya mempersempit.

---

## 4. Agent-to-agent

**Tujuan.** Satu agen dapat meminta agen lain menyelesaikan bagian pekerjaan dan menerima hasilnya.
Ini sudah ada sebagian sebagai relay lokal (`adapter/relay`), tetapi belum dihubungkan ke chat.

**Model.** Permintaan dikirim sebagai pesan ke antrean relay dengan id tujuan, isi, dan batas
kedalaman. Hasil kembali sebagai pesan balasan dengan id permintaan.

**Aturan yang wajib ada:**

- **Batas kedalaman.** Setiap pesan membawa `depth`. Agen yang menerima pesan dengan `depth`
  melebihi batas (misalnya 3) menolak, sehingga tidak ada rantai tak terbatas.
- **Batas biaya.** Setiap permintaan membawa anggaran token atau waktu. Anggaran habis, permintaan
  ditolak dengan alasan.
- **Identitas.** Pengirim diberi tanda dari konfigurasi, bukan dari isi pesan.
- **Tidak ada eksekusi otomatis lintas repo** tanpa persetujuan. Permintaan yang mengubah file
  harus menunggu persetujuan pengguna di sesi penerima.
- **Idempoten.** Permintaan yang sama (id sama) tidak dijalankan dua kali. Ini mencegah efek ganda
  saat relay mengirim ulang.

**Kegagalan.** Agen tujuan tidak aktif: permintaan tetap di antrean dengan status `waiting`,
dan pengirim diberi tahu. Tidak ada klaim bahwa pesan sudah diproses sebelum ada balasan.

**Uji.** Kedalaman, anggaran, identitas, idempotensi, dan pesan kedaluwarsa. Relay diuji di
crate-nya sendiri; sisi chat diuji dengan antrean tiruan.

---

## 5. Plugin

**Tujuan.** Ekstensi yang menambah perintah `/` atau tool, dipasang dari direktori lokal.
Plugin tidak mengubah kode inti.

**Bentuk.** Direktori dengan `plugin.json`:

```json
{
  "name": "jadwal-rapat",
  "version": "0.1.0",
  "commands": [
    { "name": "rapat", "command": "bin/rapat", "help": "Ringkasan rapat hari ini" }
  ],
  "permissions": ["read:.plaesy/tasks"]
}
```

- `permissions` dideklarasikan dan harus disetujui saat pemasangan. Plugin tidak bisa meminta
  akses di luar yang disetujui.
- Perintah plugin dijalankan seperti hook: argumen terpisah, tanpa shell, dengan batas waktu.
- Plugin dipasang dari path lokal. Sumber jarak jauh (registry atau git) ditunda sampai ada
  keputusan tentang jaringan dan penandatanganan.

**Keamanan.** Plugin berjalan sebagai proses pengguna. Izin dicatat saat pemasangan dengan hash
direktori plugin. Jika hash berubah, plugin dinonaktifkan sampai dipasang ulang dan disetujui.

**Uji.** Manifes yang tidak valid, izin yang tidak disetujui, perintah yang melewati batas waktu,
dan hash yang berubah.

---

## Urutan dan ketergantungan

1. **Rename config ke `settings.json`:** sudah dilakukan.
2. **Hook** (kecil, mandiri, memperkenalkan persetujuan dan titik event yang dipakai fitur lain).
3. **MCP klien** (butuh bagian izin dari hook).
4. **Bot/roles** (butuh provider dan skill yang sudah ada).
5. **Plugin** (butuh izin dan hook).
6. **Agent-to-agent** (butuh relay dan batas yang aman; paling berisiko, dikerjakan terakhir).

Pekerjaan paralel dimungkinkan untuk 2 dan 4, karena keduanya menyentuh bagian chat yang berbeda.
MCP, plugin, dan agent-to-agent harus menunggu persetujuan izin dari hook.

## Yang tidak dicakup dokumen ini

- Transport MCP HTTP dan autentikasi OAuth.
- Registry plugin dan penandatanganan.
- Bentuk akhir agent-to-agent lintas mesin. Versi pertama hanya lokal.

Setiap bagian di atas harus diuji di crate masing-masing, dan klaim "berfungsi" hanya boleh dibuat
setelah uji itu dijalankan dan lulus.
