//! Mesin non-produk yang dipakai beberapa fitur klien (ADR 0011). Tidak mengenal fitur apa pun.
//! `canvas`: model graf, viewport, seleksi, riwayat undo/redo, dan layout berlapis.
//! `diff`: diff JSON (IR), diff teks baris, dan changeset yang bisa digabung.

pub mod canvas;
pub mod diff;
