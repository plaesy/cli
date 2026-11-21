# ADR 0002: Visual flow IR yang netral bahasa (flow-ir/1)

- Status: diterima. Keputusan di "Keputusan yang diterima" disetujui owner di chat (2026-11-18). Format dan aturan validasi di "Format" diterima lewat delegasi owner (2026-11-21, dapat dicabut; lihat `docs/platform/decisions.md`).
- Tanggal: 2026-11-18 (persetujuan lewat chat)
- Pengambil keputusan: owner (persetujuan di chat, tanpa nama orang tercatat)
- Berlaku untuk: `flow-ir/1`
- Menggantikan: -
- Digantikan oleh: -
- Terkait: ADR 0009 (konvensi IR), ADR 0010 (design IR). Flow dipisah dari desain karena flow adalah logika, bukan tata letak.
- Spec: R16. Task: `high_chore-decide-visual-flow-ir-and-codegen-adr`, `high_feature-implement-visual-flow-editor-and-codegen`.

## Ringkasan untuk pembaca cepat

- Flow disimpan sekali sebagai IR netral. Dari IR itu dibuat kode untuk bahasa target lewat adapter.
- Sudah diterima: katalog node, ekspresi CEL, rumus retry, batas loop, kanal persetujuan, dan Rust sebagai adapter codegen pertama.
- Diterima lewat delegasi (2026-11-21, dapat dicabut): bentuk JSON dokumen flow dan aturan validasi di bagian "Format".

## Konteks

Pengguna membuat flow di editor visual. Flow disimpan sebagai IR, lalu dibuat menjadi kode untuk bahasa yang dipilih (Rust, Go, JavaScript, atau lainnya). IR tidak boleh memuat field yang menyebut bahasa target. Plaesy ditulis dalam Rust (R14). Editor memakai Dioxus (pilihan owner).

## Dorongan keputusan

- Netral bahasa: IR tidak menyebut bahasa, runtime, atau library.
- Perilaku eksplisit: setiap langkah punya input dan output bertipe, dan hasil gagal yang jelas.
- Identitas stabil: ID node tetap setelah diedit, diurutkan ulang, atau diimpor ulang.
- Kompatibilitas maju: field yang tidak dikenal disimpan, tidak dibuang (aturan final di ADR 0009).
- Kehilangan eksplisit: node yang tidak bisa diekspresikan target dicatat, tidak dilewati diam-diam.

## Opsi yang dipertimbangkan

1. **Flow berbasis kode** (pengguna menulis kode, editor menampilkan graf). Ditolak untuk iris pertama: model terikat ke satu bahasa.
2. **Format flow vendor** (menyalin skema produk lain). Ditolak: menyalin produk, bukan model milik sendiri.
3. **IR milik Plaesy dengan katalog node tetap.** Dipilih.

## Keputusan yang diterima

### Katalog node (iris pertama)

Katalog diverifikasi terhadap skema OpenFlow publik milik Windmill (versi schema 1.835.0, [openflow.openapi.yaml](https://github.com/windmill-labs/windmill/blob/main/openflow.openapi.yaml)). Plaesy mengganti nama kind dan tidak menyalin JSON Windmill.

Setiap module punya `id`, `kind`, `inputs`, `outputs` (bertipe), dan kebijakan error.

| Kind Plaesy | Padanan Windmill | Fungsi |
|---|---|---|
| `action` | `script` (PathScript), `rawscript` | Memanggil operasi bernama dengan kontrak bertipe. Kode inline tidak disimpan di IR. |
| `subflow` | `flow` (PathFlow) | Memanggil flow lain dengan ID dan versi. |
| `pass` | `identity` | Meneruskan nilai, untuk pemetaan dan debug. |
| `branch_one` | `branchone` | Cabang pertama yang cocok dijalankan. Cabang default wajib. |
| `branch_all` | `branchall` | Semua cabang dijalankan, berurutan atau paralel, dengan kebijakan gagal masing-masing. |
| `for_each` | `forloopflow` | Iterasi daftar dengan `iterator`, `parallel`, `parallelism`, `skip_failures`. |
| `while` | `whileloopflow` | Loop dengan `max_iterations` dan `max_duration_secs`, keduanya wajib. |
| `approval` | script dengan `suspend` | Menunggu `required_events` persetujuan, dengan `timeout` dan hasil saat timeout. |

Pengaturan di tingkat langkah:

- `retry`: `constant` atau `exponential`, dengan `retry_if`.
- `timeout`: batas waktu dalam detik.
- `stop_after_if`: berhenti awal setelah langkah.
- `skip_if`: lewati bersyarat.
- `continue_on_error`: flow lanjut walau langkah gagal.
- `scope`: izin per langkah, misalnya `domain:action[:resource]`. Langkah hanya boleh mempersempit scope flow, tidak boleh memperluasnya. Terkait R5 dan R9.
- `early_return`: mengakhiri flow dengan output eksplisit.
- `failure_handler`: satu per flow. Dijalankan setelah retry habis.
- `preprocessor`: dijalankan sebelum langkah pertama pada pemicu eksternal. Hanya untuk `script` atau `rawscript`.

Tidak masuk iris pertama:

- `aiagent`, `aidecision`, memori, dan transformasi AI: butuh ADR sendiri.
- `identity` sebagai langkah AI: tidak diperlukan.
- `debounce`: ditandai EE di skema Windmill.
- `mock`: konsep fixture uji, bukan node runtime.
- `sleep` dan `cache_ttl`: ditunda sampai runtime ada.

### Kontrak codegen

- Setiap target adalah adapter yang mengimplementasikan satu trait: menghasilkan kode per kind, atau melaporkan kehilangan.
- Adapter harus mendukung setiap kind yang diklaimnya. Kind yang tidak bisa diekspresikan masuk laporan kehilangan.
- Kode hasil codegen lulus fixture flow yang sama dengan IR untuk adapter itu.

### Ekspresi: CEL

- CEL (Common Expression Language, crate `cel`, lisensi MIT) dipilih. Ekspresi selalu berhenti karena CEL tidak Turing-complete. Evaluasi bisa dihentikan lewat `cel::Interrupt`, sehingga ada batas waktu ([cel-rust](https://github.com/cel-rust/cel-rust)).
- Rhai ditolak. Ia bahasa skrip penuh. Keamanannya bergantung pada `set_max_operations`, dan fungsi Rust eksternal dihitung sebagai satu operasi walau biayanya besar ([Rhai max operations](https://rhai.rs/book/safety/max-operations.html)).

### Rumus retry

- Jeda sebelum percobaan ke-k (k ≥ 2) = min(base × multiplier^(k−2), cap). Default: base 1 detik, multiplier 2, cap 60 detik, tanpa jitter (deterministik untuk uji fixture).
- Simulasi: 3 percobaan → jeda 1 dan 2 detik (total 3 detik). 5 percobaan → 1, 2, 4, 8 detik (total 15 detik). 8 percobaan → total 123 detik, cap aktif pada percobaan ke-8.
- `retry_if` adalah ekspresi CEL atas objek error.

### Batas loop

- `for_each`: waktu total = ceil(N / P) × d. Default usulan: maksimum 10.000 item dan paralelisme maksimum 50. Pada d = 0,2 detik: 10.000 item dengan P = 10 butuh 200 detik; dengan P = 50 butuh 40 detik. 100.000 item dengan P = 50 butuh 400 detik.
- `while`: batas efektif = min(max_iterations, ceil(max_duration / d)). Default usulan: `max_iterations` 1.000, `max_duration_secs` 3.600. Pada d = 1 detik batas 1.000 iterasi; pada d = 10 detik batas durasi aktif di 360 iterasi.
- Kedua field wajib, sehingga tidak ada loop tanpa batas.

### Kanal persetujuan

- Utama: API HTTP yang mengembalikan URL lanjut sekali pakai dengan masa berlaku, plus halaman web di editor Dioxus. Windmill juga memakai URL rahasia untuk approver ([dokumentasi approval Windmill](https://www.windmill.dev/docs/flows/flow_approval)).
- Setiap approval wajib punya `required_events`, `timeout` eksplisit, dan hasil untuk timeout dan penolakan.

### Adapter codegen pertama

- Rust. Keputusan 2026-11-18, untuk menguji dengan aplikasi Plaesy sendiri. Go dan JavaScript menyusul sebagai adapter terpisah.

## Format (diterima lewat delegasi, 2026-11-21)

Bagian ini diterima lewat delegasi owner pada 2026-11-21 dan bisa dicabut. Versi tetap `flow-ir/1`. Perubahan format butuh ADR baru atau pencabutan delegasi.

### Serialisasi

- Kanonik: JSON (UTF-8). Disimpan di database dan dikirim lewat API.
- Validasi bentuk: JSON Schema draft 2020-12 (`schemas/flow-ir-1.schema.json`).
- YAML boleh untuk dibaca dan diedit manusia, tetapi dikonversi ke JSON sebelum diproses. Tidak ada dua bentuk yang diproses berbeda.
- Tidak ada field yang menyebut bahasa target, runtime, atau library.

### Contoh dokumen

```json
{
  "ir": "flow-ir/1",
  "id": "flw_01J9Z3K8",
  "version": 3,
  "name": "kirim-faktur",
  "inputs": { "invoice_id": { "type": "string" } },
  "outputs": { "status": { "type": "string" } },
  "scope": ["billing:read", "billing:write"],
  "modules": [
    {
      "id": "m1",
      "kind": "action",
      "operation": "billing.get_invoice",
      "inputs": { "id": "flow.input.invoice_id" },
      "outputs": { "invoice": { "type": "object" } },
      "retry": { "kind": "exponential", "max_attempts": 3, "base_secs": 1, "multiplier": 2, "cap_secs": 60, "retry_if": "error.code != 'not_found'" },
      "timeout_secs": 30,
      "on_error": "fail"
    },
    {
      "id": "m2",
      "kind": "branch_one",
      "branches": [
        { "when": "m1.invoice.status == 'draft'", "modules": ["m3"] }
      ],
      "default": ["m4"]
    },
    { "id": "m3", "kind": "action", "operation": "billing.send_invoice", "inputs": { "id": "m1.invoice.id" }, "outputs": {} },
    { "id": "m4", "kind": "pass", "inputs": { "status": "'skipped'" }, "outputs": {} }
  ],
  "failure_handler": "m5",
  "early_return": null
}
```

### Aturan bentuk

- `id` dokumen dan `id` module tidak berubah saat diedit, diurutkan ulang, atau diimpor ulang.
- Module punya `id`, `kind`, `inputs`, `outputs`, dan `on_error` (`fail`, `continue`, atau id module penanganan).
- Referensi data: `flow.input.<nama>` untuk input flow, dan `<id_module>.<output>` untuk module yang sudah jalan. Referensi ke module yang belum jalan adalah error validasi.
- Ekspresi (`when`, `retry_if`, `stop_after_if`, `skip_if`) adalah string CEL.
- Tipe memakai subset JSON Schema: `string`, `number`, `boolean`, `object`, `array`, `null`. Tipe lain didefinisikan dengan `$ref`.
- Field yang tidak dikenal masuk `extensions` (ADR 0009).
- Konstruksi yang tidak dipetakan adapter masuk laporan kehilangan.
- Inline code tidak disimpan di IR. `action` hanya merujuk operasi bernama.

### Field per kind

| `kind` | Field khusus |
|---|---|
| `action` | `operation`, `inputs`, `outputs` |
| `subflow` | `flow_id`, `flow_version`, `inputs`, `outputs` |
| `pass` | `inputs`, `outputs` |
| `branch_one` | `branches` (daftar `when` dan `modules`), `default` (wajib) |
| `branch_all` | `branches`, `parallel` (bool), `on_error` per cabang |
| `for_each` | `iterator`, `modules`, `parallel`, `parallelism`, `skip_failures` |
| `while` | `condition`, `modules`, `max_iterations` (wajib), `max_duration_secs` (wajib) |
| `approval` | `required_events`, `timeout_secs` (wajib), `on_timeout`, `on_reject` |

Pemicu flow (event UI dari `domain/app` atau endpoint dari `domain/api`) disimpan di konteks itu, bukan di dokumen flow.

### Aturan semantik (domain/flow)

Skema hanya memeriksa bentuk. Aturan berikut dicek di `domain/flow::validate`:

- Id module unik dalam satu flow.
- Referensi data hanya ke input flow atau module yang sudah jalan.
- Tidak ada siklus, kecuali lewat `while` dan `for_each` yang punya batas.
- Ekspresi CEL bisa di-parse.
- Scope langkah hanya menyempitkan scope flow.
- `failure_handler` merujuk id module yang ada.
- Batas loop (`max_iterations`, `max_duration_secs`, `parallelism`) tidak boleh dilampaui walau skema membolehkannya.
- Subflow merujuk flow dan versi yang ada, dan tidak rekursif.
- Kehilangan saat codegen dicatat per module.

## Bukti dan catatan

- Scope per langkah ada di skema Windmill. Plaesy mengadopsinya karena mendukung R5 dan R9.
- Kind module, pengaturan langkah, retry, approval, timeout, early return, dan failure module diambil dari skema OpenFlow publik Windmill, bukan dari halaman pemasaran. Verifikasi ulang terhadap versi terbaru sebelum mengklaim paritas.
- Semua tautan di bagian ini dapat diakses (diperiksa 2026-11-21). Isi klaim tidak diverifikasi ulang satu per satu; verifikasi ulang diperlukan sebelum klaim paritas dengan Windmill.
- Catatan komunitas: deskripsi retry eksponensial Windmill tidak sama dengan rumusnya ([issue 11545](https://github.com/windmill-labs/windmill/issues/11545)). Plaesy mendefinisikan rumus secara eksplisit.
- Dokumentasi approval Windmill berbeda soal default timeout (30 menit atau tanpa batas). Plaesy mewajibkan timeout eksplisit.

## Konfirmasi

Yang sudah dijalankan (2026-11-21):

- `examples/flow-ir-1/kirim-faktur.json` dan `retry-parallel-branches.json` VALID terhadap skema (`docs/adr/tools/check-ir.js`).
- Fixture negatif `examples-negative/flow-ir-1/unknown-module-kind.json` dan `integer-port-type.json` INVALID (`check-ir.js`).
- `tools/validate-ir.js` melaporkan semantik OK untuk kedua contoh.

Belum ada: validasi semantik di Rust, interpreter, dan codegen.

## Kriteria penerimaan (implementasi flow-ir/1)

1. `domain/flow::validate` mengimplementasikan semua aturan semantik dan lulus uji positif dan negatif untuk setiap aturan.
2. `domain/flow::eval` menjalankan kedua contoh dan menghasilkan output yang sama dengan fixture.
3. Codegen Rust menghasilkan kode yang dikompilasi dan lulus fixture yang sama dengan interpreter.
4. Versi tetap `flow-ir/1`. Perubahan semantik butuh ADR baru dan versi baru.

## Konsekuensi

- Positif: satu model untuk semua bahasa target. Loop, approval, dan jalur error eksplisit.
- Negatif: katalog lebih besar, sehingga editor awal juga lebih besar.
- Netral: ADR ini tidak memilih bahasa target selain Rust sebagai adapter pertama.

## Open items

| # | Pertanyaan | Pemilik | Status |
|---|---|---|---|
| 1 | Pencabutan atau koreksi format JSON dan aturan validasi | owner | tidak ada; diterima lewat delegasi |
| 2 | Nama approver tercatat | owner | opsional; persetujuan di chat sudah berlaku |
