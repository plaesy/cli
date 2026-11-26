use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use plaesy_adapter_relay::relay::MessageStore;

use super::serve;

fn start(token: Option<&str>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let store = Arc::new(Mutex::new(MessageStore::in_memory().expect("store")));
    let token = token.map(str::to_owned);
    thread::spawn(move || serve(listener, store, token));
    addr
}

fn request(addr: std::net::SocketAddr, raw: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.write_all(raw.as_bytes()).expect("write");
    let mut reply = String::new();
    stream.read_to_string(&mut reply).expect("read");
    reply
}

fn status_of(reply: &str) -> u16 {
    reply
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status")
}

fn post(path: &str, body: &str, auth: Option<&str>) -> String {
    let auth = auth.map_or(String::new(), |token| format!("Authorization: Bearer {token}\r\n"));
    format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Content-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn send_then_read_over_a_real_socket() {
    let addr = start(None);
    let body = r#"{"id":"s-1","from":"alpha","to":"beta","text":"halo"}"#;
    let sent = request(addr, &post("/message:send", body, None));
    assert_eq!(status_of(&sent), 201);
    let read = request(addr, "GET /tasks/alpha/s-1 HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert_eq!(status_of(&read), 200);
    assert!(read.contains(r#""state":"submitted""#));
}

#[test]
fn a_token_is_required_when_configured() {
    let addr = start(Some("secret-token"));
    let body = r#"{"id":"s-2","from":"alpha","to":"beta","text":"halo"}"#;
    assert_eq!(status_of(&request(addr, &post("/message:send", body, None))), 401);
    assert_eq!(
        status_of(&request(addr, &post("/message:send", body, Some("wrong")))),
        401
    );
    assert_eq!(
        status_of(&request(addr, &post("/message:send", body, Some("secret-token")))),
        201
    );
}

#[test]
fn malformed_request_line_is_rejected() {
    let addr = start(None);
    let reply = request(addr, "\r\n\r\n");
    assert_eq!(status_of(&reply), 400);
}
