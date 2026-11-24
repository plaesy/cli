//! Lapisan service (ADR 0003). Menjalankan use case di atas domain.
//!
//! Aturan dependensi: `service` boleh memanggil `domain/*` dan `kernel`. Tidak boleh
//! memanggil `interface/*` atau `adapter/*`. Adapter mengimplementasikan port yang
//! didefinisikan di `port`.

pub mod documents;
pub mod port;
pub mod validate;

pub use validate::{validate_document, ValidationReport};

#[cfg(test)]
#[path = "documents_test.rs"]
mod documents_test;
