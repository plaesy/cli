//! Message transport server for a deployment behind a TLS-terminating proxy.
//!
//! Environment:
//! - `CREDENTIALS_DIRECTORY`: directory holding one file per key id (systemd `LoadCredential=`).
//! - `PLAESY_MESSAGES_ADDR`: loopback address to listen on. Default `127.0.0.1:8787`.
//!
//! Binds with `tls_terminated_upstream = false`, so a public address is refused.

use plaesy::relay::keys::FileKeySource;
use plaesy::relay::transport::{bind, serve, Service};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

fn main() -> io::Result<()> {
    let credentials = std::env::var("CREDENTIALS_DIRECTORY")
        .map_err(|_| io::Error::other("CREDENTIALS_DIRECTORY is not set"))?;
    let addr_text =
        std::env::var("PLAESY_MESSAGES_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_owned());
    let addr: SocketAddr = addr_text.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bad address {addr_text}: {error}"),
        )
    })?;
    let listener = bind(addr, false)?;
    let service = Arc::new(Service::new(FileKeySource::new(credentials), 300));
    serve(&listener, &service, None)
}
