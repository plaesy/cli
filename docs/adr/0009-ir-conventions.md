# ADR 0009: Konvensi bersama untuk semua IR Plaesy

- Status: proposed. Belum disetujui owner. Aturan yang berasal dari ADR yang sudah diterima (0001 dan 0002) dicatat di bagian "Asal aturan".
- Tanggal: 2026-11-21
- Approver: owner (belum ada persetujuan)
- Berlaku untuk: `flow-ir/1` (ADR 0002), `data-ir/1` (ADR 0004), `api-ir/1` (ADR 0005), `app-ir/1` (ADR 0006), `design-ir/1` (ADR 0010), dan impor kode (ADR 0008)
- Related: ADR 0003 (struktur crate), ADR 0011 (struktur klien)

## Context

Lima IR dan satu jalur impor ditulis di ADR terpisah. Aturan yang sama (versi, field tidak dikenal, ID, laporan kehilangan, batas input, pemisahan validasi bentuk dan semantik) ditulis berulang dengan detail yang berbeda. Akibatnya beberapa ADR mengklaim perilaku yang tidak sama, dan belum ada satu skema laporan kehilangan. ADR ini menjadi satu sumber untuk aturan lintas-IR. ADR per-IR hanya menyebut hal yang khusus untuk IR itu.

## Decision drivers

- Satu aturan untuk satu hal. Setiap IR merujuk ADR ini, tidak menyalin aturannya.
- Field tidak dikenal tidak boleh hilang diam-diam dan tidak boleh diterima diam-diam.
- Input dari luar (file desain, skema database, kode sumber) tidak tepercaya: ada batas ukuran dan kedalaman, dan error berupa error bertipe, bukan panik.
- Setiap klaim verifikasi harus bisa dijalankan dari repo. Klaim yang belum bisa dijalankan ditandai sebagai belum.

## Decision

### 1. Versi

- Nama IR berbentuk `<nama>-ir/<MAJOR>`. Dokumen menyatakan `ir` dengan nilai itu.
- MAJOR berubah bila makna atau nama field berubah, atau field wajib ditambah. Perubahan ini butuh ADR baru dan jalur migrasi.
- Penambahan field opsional atau kind baru adalah perubahan MINOR. MINOR tidak mengubah MAJOR.
- Dokumen tanpa `ir` atau dengan MAJOR yang tidak dikenal ditolak dengan error bertipe.

### 2. Field tidak dikenal

- Skema bersifat tertutup (`additionalProperties: false`) di setiap objek, termasuk akar dokumen. Field yang tidak dikenal menjadi error validasi bentuk.
- Penulis yang ingin membawa data tambahan memakai objek `extensions` pada objek yang memilikinya. Isi `extensions` tidak diinterpretasi oleh validator, tetapi disimpan dan ditulis ulang apa adanya.
- Pembaca dari versi lama yang menerima dokumen versi lebih baru memakai mode lenient: field tidak dikenal dipindahkan ke `extensions` dan dicatat sebagai peringatan. Mode ini belum diimplementasi (lihat rencana, fase 0).
- Dengan aturan ini, typo tetap tertangkap, dan data baru tidak hilang.

### 3. ID

- ID dokumen dan ID node/module ditetapkan sekali dan tidak berubah saat diedit, diurutkan ulang, atau diimpor ulang.
- ID yang sudah pensiun (dihapus) tidak dipakai lagi. Pembuat ID wajib memeriksa daftar ID pensiun.
- Prefiks tetap:

| Prefiks | Dipakai untuk |
|---|---|
| `flw_` | dokumen flow |
| `rel_` | relasi data |
| `qry_` | query data |
| `dsg_` | dokumen desain |
| `cp_` | komponen desain |
| `nd_` | node desain |
| `scr_` | layar aplikasi |
| `cmp_` | komponen aplikasi |

- Module flow memakai `m<angka>` atau ID lain yang unik dalam flow. Aturannya tetap: unik, dan tidak berubah.

### 4. Laporan kehilangan (satu bentuk untuk semua IR)

Setiap konstruksi yang tidak bisa dipetakan (impor, ekspor, atau codegen) menghasilkan satu entri laporan dengan bentuk:

```json
{ "ir": "design-ir/1", "location": "pages/0/frames/2", "construct": "video-node", "reason": "unsupported_node_kind", "preserved_as": "opaque" }
```

- `location`: jalur di dokumen sumber atau di IR.
- `construct`: nama konstruksi sumber.
- `reason`: kode tetap dari daftar di ADR per-IR. Kode baru ditambah lewat MINOR.
- `preserved_as`: `opaque` bila konstruksi disimpan sebagai node buram, atau `dropped` bila tidak disimpan. `dropped` tidak boleh dipakai untuk impor yang didukung.

Istilah "opaque" dipakai untuk node buram. Istilah "blok buram" di ADR 0008 adalah sinonimnya.

### 5. Batas input

- Ukuran file input maksimum 5 MB, dan kedalaman nesting maksimum 64. Batas ini berlaku untuk semua IR dan semua adapter impor.
- Input yang melebihi batas, atau tidak valid, menghasilkan error bertipe. Tidak ada panik dan tidak ada output parsial.

### 6. Pemisahan validasi

- **Bentuk**: JSON Schema draft 2020-12 di `schemas/`.
- **Semantik**: aturan per-IR di `domain/*` (Rust). Sampai `domain/*` ada, prototipe `tools/validate-ir.js` dipakai.
- **Fixture**: contoh positif di `examples/` harus VALID. Fixture negatif di `examples-negative/` harus INVALID. Setiap aturan yang diklaim di ADR per-IR wajib punya minimal satu fixture negatif.
- Pemeriksaan bentuk dijalankan oleh `docs/adr/tools/check-ir.js` (`npm run check`, dengan `ajv` versi tetap di `package.json`).

Keadaan saat ADR ini ditulis: pemeriksaan bentuk lulus untuk 10 contoh positif dan 7 fixture negatif. Pemeriksaan semantik lulus untuk 10 contoh. Belum ada langkah CI yang menjalankan keduanya.

### 7. Migrasi

- Perubahan MAJOR wajib menyertakan alat migrasi dokumen dari versi lama ke baru, dan fixture sebelum/sesudah.
- Migrasi skema database (`data-ir/1`) terhadap database yang sudah berjalan adalah topik ADR sendiri. ADR ini tidak mendefinisikannya.

### 8. Batas aturan

- Aturan di ADR ini tidak memilih bahasa, framework, atau database.
- Format ekspor tiap adapter dicatat di ADR adapter masing-masing.

## Asal aturan

| Aturan | Asal | Status |
|---|---|---|
| Field tidak dikenal disimpan, tidak dibuang | ADR 0001 dan 0002 | Diubah: sekarang lewat `extensions` dan mode lenient (bagian 2). Ini mengganti kalimat "disimpan apa adanya" yang berarti pembaca menerima field tak dikenal. |
| Batas 5 MB dan kedalaman 64 | ADR 0001 (diterima lewat delegasi) | Diperluas ke semua IR |
| Loss tidak boleh dilewati diam-diam | ADR 0002 dan 0008 | Dibakukan ke satu bentuk (bagian 4) |
| ID stabil dan tidak dipakai ulang | ADR 0001 dan 0002 | Dibakukan dengan prefiks (bagian 3) |

## Consequences

- Positif: satu tempat untuk aturan lintas-IR. ADR per-IR menjadi lebih pendek dan tidak saling bertentangan.
- Negatif: skema yang sebelumnya terbuka di akar (`additionalProperties: true`) menjadi tertutup. Dokumen yang memakai field baru harus menunggu MINOR atau memakai `extensions`.
- Netral: mode lenient belum ada, sehingga pembaca versi lama belum bisa membaca dokumen versi baru secara otomatis.

## Acceptance criteria

1. `docs/adr/tools/check-ir.js` lulus untuk semua contoh dan fixture negatif (sudah lulus saat ini).
2. Langkah CI menjalankan `npm run check` dan `tools/validate-ir.js` pada setiap perubahan `docs/adr/`. Belum ada.
3. Mode lenient diimplementasi di `domain/*`. Belum ada.
4. Setiap ADR per-IR tidak lagi mendefinisikan ulang aturan di bagian ini, kecuali dengan merujuk ADR ini.

## Open items

- Owner menyetujui aturan `extensions` dan penutupan skema akar (bagian 2).
- Daftar kode `reason` awal untuk setiap IR.
- Tempat mode lenient: library Rust atau CLI.
