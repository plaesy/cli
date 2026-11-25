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
5. **Todo dalam sesi**. Selesai sebagai perintah `/todo`; belum bisa dipakai model sebagai alat.
6. **Sesi tersimpan dan pencarian sesi**: selesai. `cli/sessions.rs` berisi status sesi, bukan riwayat chat, jadi tidak dipakai.
7. **Skill di chat**: selesai lewat `/skill <nama>`. Peran (`roles`) sudah memuat skill sejak awal.
8. **Memori otomatis**: selesai lewat `--learn`. Ringkasan dibuat provider dan disimpan sebagai keputusan. Belum memakai `memory_remember_session`.
9. **Cron dan kanban sebagai tool**: sebagian. Daftar task sudah bisa dibaca model; cron dan pembuatan task belum.
10. **`/model` dan override per giliran**: sebagian. `/model` menampilkan; ganti model belum.
11. **Subagen di worktree**. Belum.
12. **Web search dan extract**: opt-in di konfigurasi, dicatat di `docs/platform/decisions.md`. Belum.
13. **Vision, TTS, image generation**: ditunda. Butuh dukungan multimodal dan I/O audio. Tidak dibuat stub.

## Status

- Slice 1: selesai (tool baca-saja, `--tools`).
- Slice 2: selesai (`--allow-writes`, tanpa persetujuan per panggilan).
- Slice 3: selesai (`/undo` untuk tulisan sesi ini).
- Slice 4: selesai (`/compact` menyisakan 4 giliran terakhir apa adanya)
- Slice 5: selesai (`/todo`, daftar tugas dalam sesi, tidak disimpan)
- Slice 6: selesai (`--resume`, `--no-save`, `/find`; sesi disimpan di `.plaesy/chats`)
- Slice 7: selesai (`/skill <nama>` memuat SKILL.md ke sesi ini)
- Slice 8: selesai (`--learn`: ringkasan sesi disimpan sebagai keputusan lewat perekam yang sama dengan `/decision`)
- Slice 9: sebagian lanjut. `plaesy memory schedule list` menambah daftar jadwal; `list_schedules` (baca) dan `generate_image` (butuh --allow-writes, lewat `plaesy images create`).
- Slice 10: `/model <nama>` ganti model untuk provider aktif (format `provider` di `open_provider`).
- Slice 12: `web_search` lewat `plaesy reach exa` (butuh `"chat": {"web": true}` dan platform exa dikonfigurasi).
- Slice 9: sebagian. `create_task` dan `schedule_once` (butuh --allow-writes) dan `list_tasks` (baca). Cron tidak punya perintah daftar.
- Slice 10: sebagian. `/model` menampilkan. Ganti model belum: model ada di entri provider, dan ProviderOpener hanya menerima nama.
- Slice 11: selesai. `/subagent <tugas>` (butuh --allow-writes): worktree git terpisah di `.plaesy/subagents`, tanpa merge.
- Slice 12: sebagian. `web_fetch` dengan `--web`. Web search belum.
- Slice 13: ditunda. Butuh dukungan multimodal dan I/O audio.
- Slice 9: sebagian (`list_tasks` baca-saja untuk model; cron belum)
- Slice 10: sebagian (`/model` menampilkan provider dan model; mengganti model belum)
- Slice 11: ditunda (butuh keputusan: worktree git di luar repo utama)
- Slice 12: ditunda (butuh keputusan: jaringan opt-in, dan entri di docs/platform/decisions.md)
- Slice 13: ditunda (butuh dukungan multimodal dan I/O audio)
