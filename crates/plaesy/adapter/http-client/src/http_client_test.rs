use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

/// Server satu koneksi di loopback. Membalas dengan `status` dan `body`, lalu mengembalikan
/// baris request yang diterimanya (method, path, dan body).
fn serve_once(status: u16, body: &'static str) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut seen = Vec::new();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        seen.push(line.trim_end().to_owned());
        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            let trimmed = header.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some(v) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap();
            }
            seen.push(trimmed.to_owned());
        }
        let mut received = vec![0u8; content_length];
        std::io::Read::read_exact(&mut reader, &mut received).unwrap();
        seen.push(String::from_utf8(received).unwrap());
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        seen
    });
    (url, handle)
}

fn request(method: Method, url: String, body: Option<&str>) -> HttpRequest {
    HttpRequest {
        method,
        url,
        headers: vec![("X-Test".to_owned(), "ya".to_owned())],
        body: body.map(str::to_owned),
        timeout: Duration::from_secs(5),
    }
}

#[test]
fn http_get_returns_status_and_body() {
    let (url, server) = serve_once(200, "halo");
    let response = UreqClient.send(&request(Method::Get, url, None)).unwrap();
    assert_eq!(
        response,
        HttpResponse {
            status: 200,
            body: "halo".to_owned()
        }
    );
    let seen = server.join().unwrap();
    assert_eq!(seen[0], "GET / HTTP/1.1");
    assert!(seen.iter().any(|h| h.eq_ignore_ascii_case("x-test: ya")));
}

#[test]
fn http_post_sends_body() {
    let (url, server) = serve_once(201, "dibuat");
    let response = UreqClient
        .send(&request(Method::Post, url, Some("{\"a\":1}")))
        .unwrap();
    assert_eq!(response.status, 201);
    let seen = server.join().unwrap();
    assert_eq!(seen.last().unwrap(), "{\"a\":1}");
}

#[test]
fn http_error_status_is_a_response_not_an_error() {
    let (url, server) = serve_once(500, "rusak");
    let response = UreqClient.send(&request(Method::Get, url, None)).unwrap();
    assert_eq!(response.status, 500);
    assert_eq!(response.body, "rusak");
    server.join().unwrap();
}

#[test]
fn http_connection_refused_is_a_transport_error() {
    // Port yang baru dilepas: tidak ada yang mendengarkan.
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let error = UreqClient
        .send(&request(
            Method::Get,
            format!("http://127.0.0.1:{port}"),
            None,
        ))
        .unwrap_err();
    assert!(error.to_string().starts_with("http: "));
}
