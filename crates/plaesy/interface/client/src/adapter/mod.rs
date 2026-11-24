//! Adapter klien (ADR 0011): menerjemahkan operasi menjadi request dan respons menjadi nilai.
//! Tidak ada I/O di sini. Transport platform (web, desktop, mobile) mengimplementasikan trait
//! di `http` dan `ws`.

pub mod http;
pub mod ws;
