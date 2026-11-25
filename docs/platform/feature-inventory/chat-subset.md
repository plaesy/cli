# Subset chat: ukuran celah `plaesy chat`

Tanggal: 2026-11-25. Dasar: enam inventaris (`hermes-agent.md`, `deepseek-harness.md`, `orca.md`, `openbot.md`, `nimbalyst.md`, `agent-orchestrator.md`). Dokumen ini menghitung baris yang masuk subset chat dari kolom Kategori. Belum ada audit baris per baris terhadap kode. Claude Code dan Codex belum punya inventaris.

## Batas subset

- Hermes: kategori `slash-command`, `tool`, `skill`, `model`, `agen`, `memori`, `konfigurasi`, `keamanan`, `otomasi`, `CLI`.
- DeepSeek: kategori `Agen & tool`, `Model & kredensial`, `Sesi`, `Keamanan`, `CLI`, `Otomasi`.
- Orca: kategori `agen`, `CLI`, `orkestrasi agen`, `konfigurasi`.
- OpenBot: kategori `Chat`, `Provider`, `Agen`, `Skill`, `Rutin`, `Antrean`, `Memori`, `Lampiran`, `Automasi lokal`, `Alur kerja`, `Computer Use`, `Keamanan`.
- Nimbalyst: kategori `Mode agen`, `Penyedia AI`, `Sesi AI`, `Mode suara`, `Dikte`, `Multi-agen dan tim`, `Super loop`, `Otomasi`, `Manajemen berkas`, `CLI dan eksekusi jarak jauh`.
- Agent Orchestrator: kategori `agen`, `orkestrasi agen`, `chat`, `otomasi`, `konfigurasi`, `worktree`.
- Di luar subset: platform pesan, browser, editor, remote, UI, git, terminal, Web UI, desktop, SDK, mobile, kolaborasi, tracker, distribusi, dan telemetri. Daftar ini masih usulan dan belum disetujui pemilik.

## Jumlah saat ini

| Inventaris | Baris subset | `belum` | `sebagian` | `ada` |
|---|---|---|---|---|
| Hermes | 498 | 473 | 25 | 0 |
| DeepSeek | 103 | 99 | 4 | 0 |
| Orca | 126 | 115 | 11 | 0 |
| OpenBot | 155 | 145 | 10 | 0 |
| Nimbalyst | 158 | 145 | 13 | 0 |
| Agent Orchestrator | 109 | 107 | 2 | 0 |
| **Total** | **1.149** | **1.084** | **65** | **0** |

Angka `sebagian` masih berasal dari penilaian lama. Angka itu belum diperiksa ulang terhadap slice 1–8 dan tool chat yang sudah masuk. Beberapa baris Agent Orchestrator dan Nimbalyst punya status yang ditulis dengan tanda kutip atau catatan panjang, sehingga hitungannya bisa meleset beberapa baris.

## Kemampuan `plaesy chat` yang ada di kode

Daftar ini diambil dari kode, bukan dari ingatan.

- Perintah slash (`chat.rs`, fungsi parse): `/exit`, `/clear`, `/pending`, `/tasks`, `/plugins`, `/compact`, `/context`, `/undo`, `/model`, `/subagent`, `/resume`, `/learn`, `/skill`, `/find`, `/providers`, `/debug`, `/todo`, `/goal`. Plus `/decision`, `/decisions`, `/ask`, `/mcp`, `/role`, `/agent`, `/plugin`, `/pick`.
- Tool model (`chat_tools.rs`, `protocol`): `read_file`, `list_dir`, `list_tasks`, `list_schedules`, `schedule_once`, `create_task`, `web_fetch`, `web_search` (butuh `--web`), `generate_image` dan `write_file` serta `run_command` (butuh `--allow-writes`).
- Penyimpanan sesi: `chat_history.rs` (simpan, muat, daftar, cari, hapus, bersihkan) dengan tes di `chat_history_test.rs`.
- Pembelajaran: `chat_learn.rs` dengan tes di `chat_learn_test.rs`.

Tes yang ada: 16 berkas `chat_*_test.rs`, dan `chat_test.rs` berisi 69 fungsi tes.

## Celah yang belum ada

- Vision: tidak ada tool `view_image` dan `Message` belum membawa gambar.
- TTS: tidak ada tool `speak`.
- Cron: `schedule_once` dan `list_schedules` ada, tetapi tidak ada `schedule_add` atau `schedule_remove`.
- `/model --save`: belum ada.
- `web_search` tervalidasi dengan kredensial Exa: belum diuji hidup, butuh kredensial pemilik.
- Chat kosong: belum direproduksi, butuh `.plaesy/trace.jsonl`.

## Kandidat baris (kata kunci, belum diverifikasi)

Angka ini menghitung baris di semua enam inventaris, tidak hanya subset. Angka ini bukan status.

| Kemampuan | Kata kunci | Baris cocok |
|---|---|---|
| Sesi tersimpan / resume | resume, sesi tersimpan | 22 |
| Pembelajaran | /learn, pelajaran | 5 |
| Ganti model | /model, ganti model | 88 |
| Web search | web_search, pencarian web | 6 |
| Subagen | subagent, sub-agen | 26 |
| Todo | /todo, daftar tugas | 2 |
| Skill | /skill, skill | 126 |
| Undo | /undo, undo | 6 |
| Compact | /compact, ringkas konteks | 3 |
| Goal | /goal | 9 |
| Gambar | gambar, image | 76 |
| Suara | suara, speech, tts | 47 |
| Vision | vision, lihat gambar | 3 |
| Jadwal | cron, jadwal | 43 |
| Keputusan | /decision, keputusan | 12 |

## Titik selesai yang bisa diukur

Celah dianggap tertutup hanya jika setiap baris subset memiliki salah satu dari:

1. `ada`: ada kode dan tes untuk seluruh perilaku, dengan jalur file dan nama tes sebagai bukti.
2. `sebagian`: ada kode dan tes untuk sebagian perilaku, dengan batas yang dicatat.
3. `belum`: tidak ada padanan, dan prioritasnya tercatat.
4. `di luar batas`: dikeluarkan dari subset dengan alasan, dan disetujui pemilik.

Selesai berarti tidak ada baris subset yang tersisa `belum` tanpa prioritas, dan setiap baris `di luar batas` sudah disetujui pemilik. Batas ini belum tercapai.

## Yang belum dikerjakan

- Audit baris per baris 1.149 baris subset terhadap kode dan tes. Belum dimulai.
- Daftar pengecualian `di luar batas` untuk disetujui pemilik. Belum diberikan ke baris mana pun.
- Daftar `belum` yang berurut prioritas. Belum disusun.
- Inventaris Claude Code dan Codex. Dibuat terakhir, dari dokumentasi resmi dengan tanggal yang dicatat.
