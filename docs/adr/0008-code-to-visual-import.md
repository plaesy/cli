# ADR 0008: Impor kode ke visual (import-code)

- Status: diusulkan (belum disetujui).
- Tanggal: 2026-11-21
- Pengambil keputusan: owner (belum ada persetujuan)
- Berlaku untuk: adapter `import-*` untuk membaca kode dan skema yang sudah ada
- Menggantikan: -
- Digantikan oleh: -
- Terkait: ADR 0002, 0004, 0005, 0006, 0009, 0010

## Ringkasan

- Kode dan skema yang sudah ada diimpor lewat satu adapter per sumber. Setiap adapter menghasilkan IR yang sama dengan laporan pemetaan dan kehilangan.
- Bahasa pertama dan parser belum dipilih.
- Status: diusulkan.

## Konteks

Plaesy harus bisa membaca kode yang sudah ada dan menampilkannya sebagai visual (kode ke visual), selain menghasilkan kode dari visual (visual ke kode). Tanpa arah ini, pengguna dengan basis kode yang sudah ada tidak bisa masuk ke Plaesy tanpa menulis ulang.

## Dorongan keputusan

- Impor menghasilkan IR yang sudah didefinisikan (flow, data, api, app, design), bukan format baru.
- Setiap konstruksi yang tidak bisa dipetakan dicatat sebagai blok buram dengan lokasi sumber. Tidak ada yang dibuang diam-diam.
- Impor dan ekspor harus berbalik: impor lalu ekspor ulang menghasilkan IR yang sama. Ini diuji.
- Parser per bahasa adalah adapter terpisah, sehingga tidak ada bahasa yang masuk ke domain.

## Keputusan

### Arsitektur

- Setiap sumber diimpor oleh adapter `import-*` yang membaca berkas dan mengembalikan IR beserta laporan.
- Adapter yang sudah direncanakan:
  - `import-sql`: skema database yang ada menjadi `data-ir/1`. Mencakup introspeksi tabel, kolom, kunci, dan indeks.
  - `import-code-<bahasa>`: kode sumber menjadi IR lain. Dimulai dari satu bahasa, ditambah bertahap.
- Impor dari desain (misalnya berkas desain dari alat lain) juga berbentuk adapter, dan menghasilkan `design-ir/1`.

### Laporan impor

Setiap impor menghasilkan:

- **IR** yang dapat divalidasi dengan skema masing-masing IR.
- **Daftar pemetaan**: konstruksi sumber yang dipetakan ke node IR, beserta lokasinya.
- **Daftar kehilangan**: konstruksi yang tidak dipetakan. Masing-masing berisi lokasi sumber, jenis konstruksi, dan alasan.
- **Blok buram**: konstruksi kehilangan yang tetap disimpan di IR sebagai teks sumber dan lokasi, sehingga ekspor ulang bisa mengembalikannya.

### Uji konformansi impor-ekspor

- Untuk setiap contoh sumber yang didukung, impor lalu ekspor dan impor ulang harus menghasilkan IR yang sama.
- Setiap konstruksi kehilangan harus muncul di laporan. Tidak boleh ada perbedaan yang tidak tercatat.

## Batas yang disepakati

- Impor tidak menjamin semua kode di bahasa tertentu bisa dipetakan. Ia hanya menjamin bahwa yang tidak bisa dipetakan tercatat.
- Kode yang dihasilkan dari visual dan kode yang diimpor tidak harus identik secara teks. Yang harus sama adalah IR-nya.

## Validasi

- Belum ada implementasi impor. Belum ada contoh sumber yang diuji.

## Opsi yang dipertimbangkan

| Opsi | Keuntungan | Kerugian | Keputusan |
|---|---|---|---|
| A. Satu parser untuk semua bahasa | Sekali dibuat | Tidak akurat untuk sintaks tiap bahasa | Ditolak |
| B. Parser per bahasa, keluaran IR yang sama (dipilih) | Akurat per bahasa; batas kehilangan jelas | Biaya per bahasa | Dipilih |
| C. Impor hanya dari spesifikasi yang kita tulis | Tanpa parser bahasa lain | Tidak membantu pengguna dengan basis kode yang ada | Ditolak sebagai satu-satunya jalur |

Riset: tree-sitter dirancang untuk editor. Ia membangun syntax tree konkret dan memperbaruinya secara inkremental, dengan prioritas pada kecepatan dan posisi token ([Tomassetti: incremental parsing](https://tomassetti.me/incremental-parsing-using-tree-sitter/)). Untuk tujuan impor, parser resmi atau tulisan tangan biasanya lebih presisi dalam melaporkan error pada lokasi tertentu ([diskusi tree-sitter #831](https://www.github.com/tree-sitter/tree-sitter/discussions/831)). Untuk Rust, tree-sitter-rust bisa berjalan jauh lebih lambat daripada parser rustc ([DEV: benchmark parser](https://dev.to/herrington_darkholme/benchmark-typescript-parsers-demystify-rust-tooling-performance-2go8)). Tree-sitter sendiri juga punya batasan yang dibahas di [Is tree-sitter good enough?](https://blog.jez.io/tree-sitter-limitations).

Keputusan: opsi B (parser per bahasa). Usulan untuk Rust: parser resmi atau `syn`. Pilihan bahasa pertama dan parser belum disetujui owner (lihat Open items). Untuk bahasa lain, tree-sitter boleh dipakai sebagai kandidat bila tidak ada parser resmi, dan pilihan itu dicatat per bahasa di ADR baru. Tree-sitter tidak dipakai untuk membuat IR langsung tanpa validasi semantik.

## Konfirmasi

- Setiap impor menghasilkan laporan pemetaan dan kehilangan. Diuji dengan contoh kode yang sengaja berisi konstruksi tidak didukung; konstruksi itu harus muncul di laporan.
- Impor lalu ekspor ulang menghasilkan IR yang sama. Diuji pada contoh yang didukung.

## Kriteria penerimaan (implementasi impor kode)

1. Untuk bahasa pertama (Rust), impor menghasilkan IR dan laporan pemetaan/kehilangan. Setiap konstruksi yang tidak didukung muncul di laporan dengan lokasi sumbernya.
2. Impor lalu ekspor ulang pada contoh yang didukung menghasilkan IR yang sama (uji idempoten).
3. Konstruksi yang tidak didukung disimpan sebagai blok buram. Uji memastikan tidak ada konstruksi yang hilang tanpa tercatat.
4. Bahasa kedua dan seterusnya tidak dimulai sebelum bahasa pertama lolos kriteria 1 sampai 3.

## Konsekuensi

- Positif: basis kode yang sudah ada bisa dibawa masuk secara bertahap, dengan laporan jelas tentang apa yang belum tercakup.
- Negatif: setiap bahasa butuh parser sendiri, dan biayanya besar.
- Netral: bahasa pertama untuk impor belum dipilih.

## Open items

- Owner menyetujui arah ini dan batas impor.
- Bahasa pertama untuk `import-code-*`.
- Parser yang dipakai (misalnya tree-sitter atau parser bawaan tiap bahasa).
- Apakah impor database dimulai sebelum impor kode.
