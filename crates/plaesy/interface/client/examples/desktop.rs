//! Titik masuk desktop untuk `plaesy-interface-client` (fitur `desktop`).
//!
//! Jalankan dengan: `dx serve --platform desktop --package plaesy-interface-client --example desktop --features desktop`

fn main() {
    plaesy_interface_client::platform::desktop::run();
}
