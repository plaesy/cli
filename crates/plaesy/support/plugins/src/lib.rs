//! Model plugin: manifest, pemeriksaan ABI, capability, dan batas sumber daya (ADR 0003: plugin).
//!
//! Crate ini tidak melakukan I/O dan tidak memuat runtime WebAssembly. Manifest diberikan sebagai
//! string, dan runtime (Wasmtime, belum ditambahkan) dihubungkan lewat `runtime::PluginRuntime`.

pub mod abi;
pub mod capability;
pub mod error;
pub mod limits;
pub mod manifest;
pub mod runtime;

pub use error::PluginError;
