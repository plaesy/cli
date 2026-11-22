//! Titik masuk web untuk `plaesy-interface-client` (fitur `web`).
//!
//! Build: `cargo build -p plaesy-interface-client --example web --features web --target wasm32-unknown-unknown`

// Dipanggil dari JavaScript setelah `init()` memberi objek exports.
#[no_mangle]
pub extern "C" fn plaesy_start() {
    plaesy_interface_client::platform::web::run();
}

// `fn main` wajib untuk contoh. Titik masuk wasm adalah `plaesy_start`.
fn main() {}
