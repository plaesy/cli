//! Envelope transport over HTTP (spec R6, best-practice default: HTTPS at the edge).
//!
//! One route: `POST /v1/messages` with an envelope as the body. The server verifies
//! the envelope, applies the replay guard, and stores the message in the shared inbox.
//! The listener binds to loopback by default. A non-loopback bind is refused unless
//! the caller states that TLS is terminated by a proxy in front of it.
//!
//! Status codes: 202 accepted, 400 malformed, 401 unknown key, bad tag, or expired,
//! 409 replayed, 404 other path, 405 other method, 413 body too large.

use super::auth::{open, peek_key_id, AuthError, ReplayGuard};
use super::keys::KeySource;
use super::shared::{RecipientState, SharedInbox};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// Only route accepted by the server.
pub const PATH: &str = "/v1/messages";

/// Largest accepted envelope, in bytes.
pub const MAX_BODY: usize = 64 * 1024;

/// Shared state behind the route.
pub struct Service<K: KeySource> {
    keys: K,
    inbox: Mutex<SharedInbox>,
    guard: Mutex<ReplayGuard>,
}

impl<K: KeySource> Service<K> {
    /// Creates a service with a key source and a replay window in seconds.
    #[must_use]
    pub fn new(keys: K, window_secs: u64) -> Self {
        Self {
            keys,
            inbox: Mutex::new(SharedInbox::new()),
            guard: Mutex::new(ReplayGuard::new(window_secs)),
        }
    }

    /// Number of stored messages with a real effect.
    #[must_use]
    pub fn effects(&self) -> u32 {
        self.inbox.lock().map(|inbox| inbox.effects()).unwrap_or(0)
    }
}

/// Handles one envelope and returns the status code and JSON body.
pub fn handle<K: KeySource>(service: &Service<K>, envelope: &str, now: u64) -> (u16, String) {
    let Some(key_id) = peek_key_id(envelope) else {
        return (400, error_json("malformed"));
    };
    let Some(key) = service.keys.key(key_id) else {
        return (401, error_json("unknown_key"));
    };
    let opened = match open(&key, envelope) {
        Ok(opened) => opened,
        Err(AuthError::Malformed) => return (400, error_json("malformed")),
        Err(_) => return (401, error_json("bad_tag")),
    };
    let accepted = match service.guard.lock() {
        Ok(mut guard) => guard.accept(&opened, now),
        Err(_) => return (500, error_json("internal")),
    };
    match accepted {
        Ok(()) => {}
        Err(AuthError::Replayed) => return (409, error_json("replayed")),
        Err(_) => return (401, error_json("expired")),
    }
    let Ok(mut inbox) = service.inbox.lock() else {
        return (500, error_json("internal"));
    };
    // Presence is not observed in this slice, so the message is queued, never reported as read.
    let state = inbox.send(
        &opened.project,
        &opened.recipient,
        RecipientState::Offline,
        &opened.nonce,
        &opened.body,
    );
    (202, format!("{{\"state\":\"{state:?}\"}}"))
}

fn error_json(code: &str) -> String {
    format!("{{\"error\":\"{code}\"}}")
}

/// Binds a listener. Refuses non-loopback addresses unless TLS is terminated upstream.
///
/// # Errors
///
/// Returns `InvalidInput` for a public bind without upstream TLS, or the bind error.
pub fn bind(addr: SocketAddr, tls_terminated_upstream: bool) -> io::Result<TcpListener> {
    if !addr.ip().is_loopback() && !tls_terminated_upstream {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing non-loopback plain HTTP without upstream TLS",
        ));
    }
    TcpListener::bind(addr)
}

fn respond(stream: &mut TcpStream, code: u16, body: &str) -> io::Result<()> {
    let reason = match code {
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

fn read_request(stream: &TcpStream) -> io::Result<(String, String, Option<Vec<u8>>)> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts.next().unwrap_or("").to_owned();

    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().ok();
            }
        }
    }
    let Some(length) = content_length else {
        return Ok((method, path, None));
    };
    if length > MAX_BODY {
        return Ok((method, path, Some(Vec::new())));
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok((method, path, Some(body)))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn serve_one<K: KeySource>(mut stream: TcpStream, service: &Service<K>) -> io::Result<()> {
    let (method, path, body) = read_request(&stream)?;
    if path != PATH {
        return respond(&mut stream, 404, &error_json("not_found"));
    }
    if method != "POST" {
        return respond(&mut stream, 405, &error_json("method_not_allowed"));
    }
    let Some(body) = body else {
        return respond(&mut stream, 400, &error_json("malformed"));
    };
    if body.len() > MAX_BODY {
        return respond(&mut stream, 413, &error_json("too_large"));
    }
    let Ok(envelope) = String::from_utf8(body) else {
        return respond(&mut stream, 400, &error_json("malformed"));
    };
    let (code, json) = handle(service, &envelope, now_secs());
    respond(&mut stream, code, &json)
}

/// Serves requests on `listener`. Stops after `max_requests` when given.
///
/// # Errors
///
/// Returns the accept error from the listener.
pub fn serve<K: KeySource>(
    listener: &TcpListener,
    service: &Arc<Service<K>>,
    max_requests: Option<usize>,
) -> io::Result<()> {
    for (served, stream) in listener.incoming().enumerate() {
        let stream = stream?;
        // A broken client connection must not stop the server.
        let _ = serve_one(stream, service);
        if max_requests.is_some_and(|max| served + 1 >= max) {
            break;
        }
    }
    Ok(())
}

/// Posts one envelope to a server and returns the status code and body.
///
/// # Errors
///
/// Returns the connection or protocol error.
pub fn post(addr: SocketAddr, envelope: &str) -> io::Result<(u16, String)> {
    let mut stream = TcpStream::connect(addr)?;
    let head = format!(
        "POST {PATH} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        envelope.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(envelope.as_bytes())?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let status_line = response.lines().next().unwrap_or("");
    let code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    Ok((code, body))
}

#[cfg(test)]
#[path = "transport_test.rs"]
mod transport_test;
