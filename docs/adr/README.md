# Architecture Decision Records (ADR) Plaesy

Indeks keputusan arsitektur untuk IR, struktur crate, dan konversi desain-kode. Keputusan yang sudah diterima tidak diubah diam-diam. Perubahan dilakukan dengan ADR baru yang menggantikan (supersede) yang lama, dan status ADR lama diubah menjadi `superseded` dengan tautan ke penggantinya.

## Daftar

| ADR | Judul | Status | Catatan |
|---|---|---|---|
| [0001](0001-design-code-ir.md) | Canonical IR desain-kode | superseded | Digantikan 0010. Keputusan yang masih berlaku dipindah ke 0009 dan 0010. |
| [0002](0002-visual-flow-ir.md) | Visual flow IR (flow-ir/1) | accepted (keputusan), format usulan | Bagian "IR format" belum disetujui owner. |
| [0003](0003-crate-structure.md) | Struktur folder dan crate | proposed | Bagian klien dipindah ke 0011. |
| [0004](0004-data-ir.md) | Data IR (data-ir/1) | proposed | |
| [0005](0005-api-ir.md) | API IR (api-ir/1) | proposed | |
| [0006](0006-app-ir.md) | App IR (app-ir/1) | proposed | |
| [0007](0007-design-ir.md) | Design IR (design-ir/1) | superseded | Digantikan 0010. |
| [0008](0008-code-to-visual-import.md) | Impor kode ke visual | proposed | |
| [0009](0009-ir-conventions.md) | Konvensi bersama untuk semua IR | proposed | Versi, `extensions`, ID, laporan kehilangan, batas input, validasi. |
| [0010](0010-design-ir.md) | Design IR dan konversi desain-kode | proposed | Menggantikan 0001 dan 0007. |
| [0011](0011-client-structure.md) | Struktur klien (plaesy-interface-client) | proposed | Dipisah dari 0003. |

Rencana implementasi: [implementation-plan.md](implementation-plan.md).

## Pemeriksaan

Dijalankan dari `docs/adr/tools/`:

```sh
npm ci
npm run check          # bentuk: contoh harus VALID, fixture negatif harus INVALID
node validate-ir.js ../examples/*/*.json   # semantik (prototipe)
```

Belum ada langkah CI yang menjalankan keduanya. Ditambahkan di Fase 0 rencana implementasi.

## Struktur folder

- `0000`-`0011`: ADR. Nomor baru berikutnya adalah `0012`.
- `schemas/`: JSON Schema draft 2020-12, satu per IR.
- `examples/`: contoh positif. Harus VALID.
- `examples-negative/`: fixture negatif. Harus INVALID.
- `tools/`: pemeriksaan (`check-ir.js`) dan prototipe validator semantik (`validate-ir.js`).

## Menulis ADR baru

1. Ambil nomor berikutnya. Jangan memakai ulang nomor.
2. Isi header: `Status`, `Tanggal`, `Approver`, `Related`. Status awal: `proposed`.
3. Bagian wajib: Context, Decision drivers, Decision, Considered options, Consequences, Confirmation, Open items.
4. Klaim verifikasi hanya boleh ditulis jika bisa dijalankan dari repo. Jika belum, tulis "belum diperiksa".
5. Aturan lintas-IR merujuk ADR 0009, tidak disalin.
6. Untuk mengganti ADR lama: ubah status lama menjadi `superseded` dan tunjuk penggantinya; cantumkan ID lama di `Supersedes` pada ADR baru.
