//! Lapisan service (ADR 0003). Menjalankan use case di atas domain.
//!
//! Aturan dependensi: `service` boleh memanggil `domain/*` dan `kernel`. Tidak boleh
//! memanggil `interface/*` atau `adapter/*`. Adapter akan mengimplementasikan port
//! yang didefinisikan di sini, saat port pertama dibutuhkan.

pub mod validate;

pub use validate::{validate_document, ValidationReport};
