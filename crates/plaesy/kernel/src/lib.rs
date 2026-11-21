//! Primitif bersama Plaesy. Tanpa I/O, tanpa runtime async, tanpa crate transport.
//!
//! Isi: pembantu akses JSON (`json`) untuk validator IR. Tipe baru masuk di sini hanya
//! jika dipakai oleh lebih dari satu konteks domain (ADR 0003).

pub mod json;
