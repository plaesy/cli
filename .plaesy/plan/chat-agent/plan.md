# Rencana: kemampuan agen untuk `plaesy chat`

Status per slice dicatat di bagian Status. Setiap slice satu commit, lulus `cargo test -p plaesy-interface-cli --locked -j 3` dan clippy `-D warnings` untuk crate yang disentuh.

## Keputusan

- Transport tool: protokol teks. Model menulis blok ```` ```tool ```` berisi JSON, host menjalankannya dan mengirim hasil sebagai pesan berikutnya. Alasannya: berfungsi untuk semua provider, termasuk `claude-cli`, dan bisa diuji dengan provider palsu. Native `tool_use` ditunda.
- Fitur opt-in lewat flag `--tools`. Perilaku chat lama tidak berubah tanpa flag.
- Jalur tool tidak mengimpor `model-anthropic-tools` atau `interface/acp` (aturan dependensi, dan crate itu opt-in).
- Keamanan sejak slice 1: path dikurung di bawah root repo (tolak absolut, `..`, dan escape symlink), keluaran dipotong, jumlah putaran dibatasi.
- Tool penulisan dan perintah shell butuh persetujuan. Persetujuan di TUI butuh penanda kontrol (belum dibuat).

## Slice

1. **Loop tool baca-saja** (`chat_tools.rs`, `chat_tools_test.rs`): `read_file`, `list_dir`, dengan `--tools`. Status: selesai.
2. **Tulis dan jalankan perintah**: `write_file` dan `run_command` dengan `--allow-writes`. Selesai. Persetujuan per panggilan (prompt y/n, di TUI lewat penanda kontrol) belum ada.
3. **Checkpoint dan `/undo`**: snapshot sebelum tulis. Selesai, tapi hanya di memori sesi; belum ada riwayat di disk.
4. **Kompresi yang menyisakan ekor** di `compact_transcript`. Selesai.
5. **Todo dalam sesi**. Belum.
6. **Sesi tersimpan dan pencarian sesi**: cek `cli/sessions.rs` dulu. Belum.
7. **Skill di chat**: rencana lama slice 3. Belum.
8. **Memori otomatis**: cek `memory_remember_session` dulu. Belum.
9. **Cron dan kanban sebagai tool**. Belum.
10. **`/model` dan override per giliran**. Belum.
11. **Subagen di worktree**. Belum.
12. **Web search dan extract**: opt-in di konfigurasi, dicatat di `docs/platform/decisions.md`. Belum.
13. **Vision, TTS, image generation**: ditunda. Butuh dukungan multimodal dan I/O audio. Tidak dibuat stub.

## Status

- Slice 1: selesai (tool baca-saja, `--tools`).
- Slice 2: selesai (`--allow-writes`, tanpa persetujuan per panggilan).
- Slice 3: selesai (`/undo` untuk tulisan sesi ini).
- Slice 4: selesai (`/compact` menyisakan 4 giliran terakhir apa adanya)
