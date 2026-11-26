//! `plaesy agent serve`: the socket layer for the agent protocol routes in `agent_http`.
//!
//! The server binds to loopback only, so nothing is reachable from the network. When
//! `PLAESY_AGENT_TOKEN` is set, every request must send `Authorization: Bearer <token>`. Each
//! connection is served on its own thread, and the relay store is shared behind a mutex.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use plaesy_adapter_relay::relay::MessageStore;

use super::agent_http::{self, MAX_BODY_BYTES};
use super::CliResult;

/// Environment variable with the bearer token. Empty or unset means no token is required.
pub const TOKEN_ENV: &str = "PLAESY_AGENT_TOKEN";
/// Largest request head accepted, in bytes.
const MAX_HEAD_BYTES: usize = 8 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Loopback port to listen on.
    #[arg(long, default_value_t = 8765)]
    pub port: u16,
    /// Repository whose relay database is served.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
}

/// `plaesy agent`: the agent-to-agent command group.
#[derive(Debug, clap::Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum AgentAction {
    /// Serve the agent protocol routes on loopback.
    Serve(Args),
}

pub fn run_agent(args: &AgentArgs) -> CliResult {
    match &args.command {
        AgentAction::Serve(serve_args) => run(serve_args),
    }
}

pub fn run(args: &Args) -> CliResult {
    let db = args.path.join(".plaesy").join("relay.sqlite");
    let store = MessageStore::open(&db)?;
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, args.port)))?;
    let token = std::env::var(TOKEN_ENV).ok().filter(|value| !value.is_empty());
    eprintln!(
        "agent: listening on 127.0.0.1:{} (token {})",
        args.port,
        if token.is_some() { "required" } else { "not set" }
    );
    serve(listener, Arc::new(Mutex::new(store)), token);
    Ok(())
}

/// Accepts connections forever, one thread each.
pub fn serve(listener: TcpListener, store: Arc<Mutex<MessageStore>>, token: Option<String>) {
    let token = token.map(Arc::new);
    for stream in listener.incoming().flatten() {
        let store = Arc::clone(&store);
        let token = token.clone();
        thread::spawn(move || {
            let _ = handle(stream, &store, token.as_deref().map(String::as_str));
        });
    }
}

fn handle(
    mut stream: TcpStream,
    store: &Mutex<MessageStore>,
    token: Option<&str>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let (head, rest) = match read_head(&mut stream)? {
        Some(parts) => parts,
        None => return write_reply(&mut stream, 400, r#"{"error":"request head is too large"}"#),
    };
    let request = parse_head(&head);
    let Some(request) = request else {
        return write_reply(&mut stream, 400, r#"{"error":"malformed request line"}"#);
    };
    if let Some(expected) = token {
        if request.authorization.as_deref() != Some(&format!("Bearer {expected}")) {
            return write_reply(&mut stream, 401, r#"{"error":"missing or wrong token"}"#);
        }
    }
    if request.content_length > MAX_BODY_BYTES {
        return write_reply(&mut stream, 413, r#"{"error":"request body is too large"}"#);
    }
    let mut body = rest;
    while body.len() < request.content_length {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(request.content_length);
    let (status, reply) = match store.lock() {
        Ok(mut guard) => agent_http::route(&mut guard, &request.method, &request.path, &body),
        Err(_) => (500, r#"{"error":"store unavailable"}"#.to_owned()),
    };
    write_reply(&mut stream, status, &reply)
}

struct Head {
    method: String,
    path: String,
    content_length: usize,
    authorization: Option<String>,
}

/// Reads up to the blank line that ends the head. Returns the head and any body bytes already read.
fn read_head(stream: &mut TcpStream) -> io::Result<Option<(Vec<u8>, Vec<u8>)>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            let rest = buffer.split_off(end + 4);
            return Ok(Some((buffer, rest)));
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return Ok(None);
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

fn parse_head(head: &[u8]) -> Option<Head> {
    let text = std::str::from_utf8(head).ok()?;
    let mut lines = text.split("\r\n");
    let mut request_line = lines.next()?.split(' ');
    let method = request_line.next()?.to_owned();
    let path = request_line.next()?.to_owned();
    let mut content_length = 0_usize;
    let mut authorization = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse().ok()?;
        } else if name.eq_ignore_ascii_case("authorization") {
            authorization = Some(value.to_owned());
        }
    }
    Some(Head {
        method,
        path,
        content_length,
        authorization,
    })
}

fn write_reply(stream: &mut impl Write, status: u16, body: &str) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

#[cfg(test)]
#[path = "agent_serve_test.rs"]
mod agent_serve_test;
