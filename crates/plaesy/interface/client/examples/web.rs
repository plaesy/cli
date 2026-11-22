//! Titik masuk web untuk `plaesy-interface-client` (fitur `web`).
//!
//! Build: `cargo build -p plaesy-interface-client --example web --features web --target wasm32-unknown-unknown`

// `dx serve` dan `dx build` menjalankan `main` contoh ini sebagai titik masuk web.
fn main() {
    plaesy_interface_client::platform::web::run();
}
