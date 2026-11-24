//! `plaesy remote serve`: a read-only HTTP view of this project's task board.
//!
//! Endpoints: `GET /health`, `GET /tasks` (JSON summary), and `GET /events` (Server-Sent Events).
//! The server binds to loopback by default. A non-loopback bind requires a bearer token, and a
//! configured token is enforced on every route, loopback included. HTTP/1.1 is handled directly on
//! `std::net`, so this command adds no dependency. Nothing here writes to `.plaesy/`.
//!
//! Routing, authentication, request parsing, and response bodies are pure functions so they can be
//! tested without a socket. Only [`serve`] and its helpers touch the network.

use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::CliResult;
use crate::tasks::{self, TaskRef, STATUSES};

/// Environment variable read when `--auth-token` is absent, so the token stays out of argv.
pub const TOKEN_ENV: &str = "PLAESY_REMOTE_TOKEN";
/// Shortest accepted bearer token, the same floor as the memory MCP server.
const MIN_TOKEN_BYTES: usize = 32;
/// Largest request head accepted before the server answers 400.
const MAX_HEAD_BYTES: usize = 8 * 1024;
/// Connections served at once; further connections get 503.
const MAX_CONNECTIONS: usize = 32;
/// How often `/events` rechecks the task folders.
const EVENT_POLL: Duration = Duration::from_secs(2);
/// Keepalive comment interval, so dead streams are noticed and proxies keep the stream open.
const KEEPALIVE: Duration = Duration::from_secs(15);
/// Read and write timeout for each connection.
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Serves a read-only view of the project's tasks.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: RemoteCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum RemoteCommand {
    /// Serve /health, /tasks, and /events over HTTP (read-only)
    Serve(ServeArgs),
}

#[derive(Debug, clap::Args)]
pub struct ServeArgs {
    /// TCP port to listen on
    #[arg(long, default_value_t = 7878)]
    pub port: u16,
    /// Address to bind; a non-loopback address requires a token
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: IpAddr,
    /// Bearer token, at least 32 bytes (default: the PLAESY_REMOTE_TOKEN environment variable)
    #[arg(long)]
    pub auth_token: Option<String>,
    /// Project root that holds .plaesy/ (default: the nearest project above the current directory)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

/// Failures of the remote server. Messages are printed as `Error: <message>`.
#[derive(Debug, Error)]
pub enum RemoteError {
    #[error(
        "bind address {0} is not loopback; set --auth-token or PLAESY_REMOTE_TOKEN to serve it"
    )]
    TokenRequired(IpAddr),
    #[error("auth token must be at least 32 bytes and have no surrounding whitespace")]
    WeakToken,
    #[error("bad request: {0}")]
    BadRequest(&'static str),
    #[error(transparent)]
    Project(#[from] tasks::TasksError),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        RemoteCommand::Serve(serve_args) => serve_command(serve_args),
    }
}

/// Checks the bind address against the token rule. Loopback may run without a token; any other
/// address needs one, and a supplied token must meet the length and whitespace rules.
///
/// # Errors
///
/// [`RemoteError::TokenRequired`] for a non-loopback bind without a token, and
/// [`RemoteError::WeakToken`] for a token that is too short or padded with whitespace.
pub fn validate_config(bind: IpAddr, token: Option<&str>) -> Result<(), RemoteError> {
    match token {
        Some(token) => {
            if token.len() < MIN_TOKEN_BYTES || token.trim() != token {
                return Err(RemoteError::WeakToken);
            }
            Ok(())
        }
        None if bind.is_loopback() => Ok(()),
        None => Err(RemoteError::TokenRequired(bind)),
    }
}

/// A parsed request head. Only the method, path, and `Authorization` header matter to routing.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// Target without its query string.
    pub path: String,
    pub authorization: Option<String>,
}

/// Parses an HTTP/1.x request head (request line plus headers, ending in a blank line). Any body
/// is ignored, since every route is a read.
///
/// # Errors
///
/// [`RemoteError::BadRequest`] for an oversized, non-UTF-8, or malformed head, or an HTTP version
/// other than 1.x.
pub fn parse_request(head: &[u8]) -> Result<Request, RemoteError> {
    if head.len() > MAX_HEAD_BYTES {
        return Err(RemoteError::BadRequest("request head too large"));
    }
    let text =
        std::str::from_utf8(head).map_err(|_| RemoteError::BadRequest("request is not UTF-8"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts
        .next()
        .filter(|method| !method.is_empty())
        .ok_or(RemoteError::BadRequest("missing method"))?;
    let target = parts
        .next()
        .filter(|target| !target.is_empty())
        .ok_or(RemoteError::BadRequest("missing request target"))?;
    let version = parts
        .next()
        .ok_or(RemoteError::BadRequest("missing HTTP version"))?;
    if !version.starts_with("HTTP/1.") {
        return Err(RemoteError::BadRequest("unsupported HTTP version"));
    }

    let mut authorization = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("authorization") && authorization.is_none() {
                authorization = Some(value.trim().to_owned());
            }
        }
    }

    let path = target.split('?').next().unwrap_or(target);
    Ok(Request {
        method: method.to_owned(),
        path: path.to_owned(),
        authorization,
    })
}

/// Returns true when the request may proceed. With no configured token every request passes.
/// Otherwise the header must be `Bearer <token>` and the token must match. The comparison runs over
/// SHA-256 digests in constant time, so the response time does not reveal the token's prefix.
#[must_use]
pub fn check_auth(expected: Option<&str>, authorization: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    let Some(presented) = authorization.and_then(bearer_token) else {
        return false;
    };
    let left = Sha256::digest(expected.as_bytes());
    let right = Sha256::digest(presented.as_bytes());
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn bearer_token(header: &str) -> Option<&str> {
    let (scheme, token) = header.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// A route the server knows about, after the method check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    Health,
    Tasks,
    Events,
    MethodNotAllowed,
    NotFound,
}

/// Maps a method and path to a route. Every known route accepts only GET.
#[must_use]
pub fn route(method: &str, path: &str) -> Routed {
    match path {
        "/health" | "/tasks" | "/events" if method != "GET" => Routed::MethodNotAllowed,
        "/health" => Routed::Health,
        "/tasks" => Routed::Tasks,
        "/events" => Routed::Events,
        _ => Routed::NotFound,
    }
}

/// What the connection handler should send for a request.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    /// A complete JSON response.
    Json { status: u16, body: String },
    /// Start an SSE stream; the handler then calls [`stream_events`].
    Events,
}

/// Decides the response for a request. Authentication runs first, so an unauthenticated client
/// learns nothing about which routes exist. `tasks` is called only for an authenticated `/tasks`
/// request, so tests can supply canned data without touching the filesystem.
pub fn reply(
    request: &Request,
    token: Option<&str>,
    version: &str,
    tasks: impl FnOnce() -> Result<Vec<TaskRef>, String>,
) -> Reply {
    if !check_auth(token, request.authorization.as_deref()) {
        return Reply::Json {
            status: 401,
            body: error_body("unauthorized"),
        };
    }
    match route(&request.method, &request.path) {
        Routed::Health => Reply::Json {
            status: 200,
            body: health_body(version),
        },
        Routed::Tasks => match tasks() {
            Ok(refs) => Reply::Json {
                status: 200,
                body: tasks_body(&refs),
            },
            Err(message) => Reply::Json {
                status: 500,
                body: error_body(&message),
            },
        },
        Routed::Events => Reply::Events,
        Routed::MethodNotAllowed => Reply::Json {
            status: 405,
            body: error_body("method not allowed; this server is read-only"),
        },
        Routed::NotFound => Reply::Json {
            status: 404,
            body: error_body("not found"),
        },
    }
}

#[must_use]
pub fn health_body(version: &str) -> String {
    json!({ "status": "ok", "version": version }).to_string()
}

/// JSON summary of the task board: a total, a count per status, and one item per task.
#[must_use]
pub fn tasks_body(refs: &[TaskRef]) -> String {
    let mut by_status = Map::new();
    for status in STATUSES {
        let count = refs.iter().filter(|task| task.status == status).count();
        by_status.insert(status.as_str().to_owned(), json!(count));
    }
    let items: Vec<Value> = refs
        .iter()
        .map(|task| {
            json!({
                "spec": task.spec,
                "status": task.status.as_str(),
                "name": task.name,
                "title": task.title,
                "phase": task.phase,
            })
        })
        .collect();
    json!({
        "count": refs.len(),
        "by_status": by_status,
        "tasks": items,
    })
    .to_string()
}

#[must_use]
pub fn error_body(message: &str) -> String {
    json!({ "error": message }).to_string()
}

/// One Server-Sent Events frame: an `event:` line, one `data:` line per line of `data`, and a
/// blank line. Control characters are removed from the event name, so it cannot end the frame.
#[must_use]
pub fn sse_event(event: &str, data: &str) -> String {
    let name: String = event.chars().filter(|c| !c.is_control()).collect();
    let mut frame = format!("event: {name}\n");
    for line in data.split('\n') {
        frame.push_str("data: ");
        frame.push_str(line.trim_end_matches('\r'));
        frame.push('\n');
    }
    frame.push('\n');
    frame
}

/// An SSE comment frame, used as a keepalive. Line breaks in `text` are replaced, so the comment
/// stays on one line.
#[must_use]
pub fn sse_comment(text: &str) -> String {
    let flat = text.replace(['\r', '\n'], " ");
    format!(": {flat}\n\n")
}

/// Per-server state shared by connection threads.
struct Shared {
    token: Option<String>,
    tasks_dir: PathBuf,
    active: AtomicUsize,
}

fn serve_command(args: &ServeArgs) -> CliResult {
    let token = args
        .auth_token
        .clone()
        .or_else(|| std::env::var(TOKEN_ENV).ok());
    validate_config(args.bind, token.as_deref())?;

    let start = match &args.root {
        Some(root) => root.clone(),
        None => std::env::current_dir()?,
    };
    let project = tasks::find_project_root(&start)?;
    let tasks_dir = tasks::tasks_dir(&project);

    let addr = SocketAddr::new(args.bind, args.port);
    let listener = TcpListener::bind(addr).map_err(io_context(format!("binding {addr}")))?;
    let auth = if token.is_some() {
        "bearer token"
    } else {
        "none, loopback only"
    };
    // Stdout stays free for protocol output, so the banner goes to stderr.
    eprintln!(
        "plaesy remote: serving {} read-only on http://{addr} (auth: {auth})",
        tasks_dir.display()
    );
    serve(
        &listener,
        &Arc::new(Shared {
            token,
            tasks_dir,
            active: AtomicUsize::new(0),
        }),
    );
    Ok(())
}

fn serve(listener: &TcpListener, state: &Arc<Shared>) {
    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("plaesy remote: accept failed: {error}");
                continue;
            }
        };
        if state.active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            state.active.fetch_sub(1, Ordering::SeqCst);
            let _ = write_json(&mut stream, 503, &error_body("too many connections"));
            continue;
        }
        let worker = Arc::clone(state);
        thread::spawn(move || {
            if let Err(error) = handle(stream, &worker) {
                if !is_disconnect(&error) {
                    eprintln!("plaesy remote: {error}");
                }
            }
            worker.active.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

fn handle(mut stream: TcpStream, state: &Shared) -> Result<(), RemoteError> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(io_context("configuring socket"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(io_context("configuring socket"))?;

    let request = match read_head(&mut stream).and_then(|head| parse_request(&head)) {
        Ok(request) => request,
        Err(error) => return write_json(&mut stream, 400, &error_body(&error.to_string())),
    };

    let outcome = reply(&request, state.token.as_deref(), crate::version(), || {
        tasks::list_all_tasks(&state.tasks_dir).map_err(|error| error.to_string())
    });
    match outcome {
        Reply::Json { status, body } => write_json(&mut stream, status, &body),
        Reply::Events => stream_events(&mut stream, &state.tasks_dir),
    }
}

/// Reads until the blank line that ends the request head. The head is capped at
/// [`MAX_HEAD_BYTES`], so a client cannot grow the buffer without limit.
fn read_head(stream: &mut TcpStream) -> Result<Vec<u8>, RemoteError> {
    let mut head = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    loop {
        if let Some(end) = head_end(&head) {
            head.truncate(end);
            return Ok(head);
        }
        if head.len() >= MAX_HEAD_BYTES {
            return Err(RemoteError::BadRequest("request head too large"));
        }
        let read = stream
            .read(&mut chunk)
            .map_err(io_context("reading request"))?;
        if read == 0 {
            return Err(RemoteError::BadRequest(
                "connection closed before end of request head",
            ));
        }
        head.extend_from_slice(&chunk[..read]);
    }
}

fn head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

/// Sends the SSE headers, then one `tasks` event whenever the board changes and a keepalive
/// comment when it has been quiet. Returns with an error when the client disconnects, which the
/// next write detects.
fn stream_events(stream: &mut TcpStream, tasks_dir: &Path) -> Result<(), RemoteError> {
    send(
        stream,
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
    )?;
    let mut last: Option<String> = None;
    let mut last_write = Instant::now();
    loop {
        let (name, body) = match tasks::list_all_tasks(tasks_dir) {
            Ok(refs) => ("tasks", tasks_body(&refs)),
            Err(error) => ("error", error_body(&error.to_string())),
        };
        let key = format!("{name}\n{body}");
        if last.as_deref() != Some(key.as_str()) {
            send(stream, sse_event(name, &body).as_bytes())?;
            last = Some(key);
            last_write = Instant::now();
        } else if last_write.elapsed() >= KEEPALIVE {
            send(stream, sse_comment("keepalive").as_bytes())?;
            last_write = Instant::now();
        }
        thread::sleep(EVENT_POLL);
    }
}

fn write_json(stream: &mut impl Write, status: u16, body: &str) -> Result<(), RemoteError> {
    let challenge = if status == 401 {
        "WWW-Authenticate: Bearer\r\n"
    } else {
        ""
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\nCache-Control: no-store\r\n{challenge}Connection: close\r\n\r\n",
        reason = reason_phrase(status),
        length = body.len(),
    );
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(body.as_bytes());
    send(stream, &bytes)
}

fn send(stream: &mut impl Write, bytes: &[u8]) -> Result<(), RemoteError> {
    stream
        .write_all(bytes)
        .map_err(io_context("writing response"))?;
    stream.flush().map_err(io_context("writing response"))
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

fn io_context(context: impl Into<String>) -> impl FnOnce(io::Error) -> RemoteError {
    let context = context.into();
    move |source| RemoteError::Io { context, source }
}

/// A client that went away mid-response is normal for SSE; it is not logged as an error.
fn is_disconnect(error: &RemoteError) -> bool {
    matches!(
        error,
        RemoteError::Io { source, .. }
            if matches!(
                source.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::WouldBlock
            )
    )
}

#[cfg(test)]
#[path = "remote_test.rs"]
mod remote_test;
