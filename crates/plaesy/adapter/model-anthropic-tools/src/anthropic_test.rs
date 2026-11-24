use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

type Seen = Arc<Mutex<Vec<(Value, Vec<(String, String)>)>>>;

/// Server HTTP palsu di loopback. Membalas tiap permintaan dengan `replies[i]` (atau yang terakhir
/// bila habis). Mengembalikan endpoint, body dan header setiap permintaan.
fn fake_anthropic(replies: Vec<Value>) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    thread::spawn(move || {
        for index in 0.. {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut length = 0usize;
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let trimmed = header.trim_end().to_owned();
                if trimmed.is_empty() {
                    break;
                }
                if let Some((name, value)) = trimmed.split_once(':') {
                    let name = name.trim().to_ascii_lowercase();
                    let value = value.trim().to_owned();
                    if name == "content-length" {
                        length = value.parse().unwrap();
                    }
                    headers.push((name, value));
                }
            }
            let mut buf = vec![0u8; length];
            reader.read_exact(&mut buf).unwrap();
            let body: Value = serde_json::from_slice(&buf).unwrap();
            recorded.lock().unwrap().push((body, headers));
            let reply = replies
                .get(index)
                .or_else(|| replies.last())
                .cloned()
                .unwrap();
            let text = reply.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                text.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (endpoint, seen)
}

/// Alat tiruan: mencatat panggilan dan mengembalikan nilai tetap.
#[derive(Default)]
struct FakeTools {
    calls: Vec<String>,
}

impl ToolHost for FakeTools {
    fn read_text_file(&mut self, path: &str) -> Result<String, String> {
        self.calls.push(format!("read {path}"));
        Ok("isi berkas".to_owned())
    }

    fn write_text_file(&mut self, _path: &str, _content: &str) -> Result<(), String> {
        Ok(())
    }

    fn request_permission(
        &mut self,
        _title: &str,
        _options: &[(&str, &str)],
    ) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn run_command(&mut self, _command: &str, _args: &[String]) -> Result<CommandResult, String> {
        Err("tidak dipakai di tes ini".to_owned())
    }
}

/// Variabel lingkungan khusus tes, diisi hanya di dalam tes yang membutuhkannya.
const KEY_ENV: &str = "PLAESY_TEST_ANTHROPIC_KEY";

fn model(endpoint: String, key_env: &str) -> AnthropicToolModel {
    AnthropicToolModel {
        endpoint,
        model: "claude-test".to_owned(),
        api_key_env: key_env.to_owned(),
        timeout: Duration::from_secs(10),
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn anthropic_tool_use_is_executed_and_result_returned_as_tool_result() {
    std::env::set_var(KEY_ENV, "rahasia-uji");
    let (endpoint, seen) = fake_anthropic(vec![
        json!({
            "type": "message", "role": "assistant", "stop_reason": "tool_use",
            "content": [
                { "type": "text", "text": "Saya baca dulu." },
                { "type": "tool_use", "id": "toolu_1", "name": "read_file", "input": { "path": "/proyek/a.txt" } }
            ]
        }),
        json!({
            "type": "message", "role": "assistant", "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": "Isinya: isi berkas" }]
        }),
    ]);
    let mut tools = FakeTools::default();
    let answer = model(endpoint, KEY_ENV)
        .complete("baca a.txt", &mut tools)
        .unwrap();

    assert_eq!(answer, "Isinya: isi berkas");
    assert_eq!(tools.calls, ["read /proyek/a.txt"]);

    let requests = seen.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let (first_body, first_headers) = &requests[0];
    assert_eq!(first_body["max_tokens"], MAX_TOKENS);
    assert!(
        first_body["tools"][0].get("input_schema").is_some(),
        "pakai input_schema"
    );
    assert_eq!(header(first_headers, "x-api-key"), Some("rahasia-uji"));
    assert_eq!(
        header(first_headers, "anthropic-version"),
        Some(ANTHROPIC_VERSION)
    );

    let (second_body, _) = &requests[1];
    let messages = second_body["messages"].as_array().unwrap();
    let last = messages.last().unwrap();
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"][0]["type"], "tool_result");
    assert_eq!(last["content"][0]["tool_use_id"], "toolu_1");
    assert_eq!(last["content"][0]["content"], "isi berkas");
}

#[test]
fn anthropic_missing_key_env_stops_before_any_request() {
    let (endpoint, seen) = fake_anthropic(vec![
        json!({ "type": "message", "stop_reason": "end_turn", "content": [] }),
    ]);
    let mut tools = FakeTools::default();
    let error = model(endpoint, "PLAESY_TEST_TIDAK_ADA_KUNCI")
        .complete("halo", &mut tools)
        .unwrap_err();
    assert!(error.contains("tidak diisi"));
    assert!(error.contains("PLAESY_TEST_TIDAK_ADA_KUNCI"));
    assert!(
        seen.lock().unwrap().is_empty(),
        "tidak boleh ada permintaan"
    );
}

#[test]
fn anthropic_endless_tool_use_stops_at_the_round_limit() {
    std::env::set_var(KEY_ENV, "rahasia-uji");
    let (endpoint, seen) = fake_anthropic(vec![json!({
        "type": "message", "role": "assistant", "stop_reason": "tool_use",
        "content": [{ "type": "tool_use", "id": "toolu_x", "name": "read_file", "input": { "path": "/p/x" } }]
    })]);
    let mut tools = FakeTools::default();
    let error = model(endpoint, KEY_ENV)
        .complete("x", &mut tools)
        .unwrap_err();
    assert!(error.contains("putaran"), "{error}");
    assert_eq!(seen.lock().unwrap().len(), MAX_ROUNDS);
}

#[test]
fn anthropic_unknown_tool_is_reported_not_executed() {
    std::env::set_var(KEY_ENV, "rahasia-uji");
    let (endpoint, _seen) = fake_anthropic(vec![
        json!({
            "type": "message", "role": "assistant", "stop_reason": "tool_use",
            "content": [{ "type": "tool_use", "id": "toolu_y", "name": "hapus_semua", "input": {} }]
        }),
        json!({
            "type": "message", "role": "assistant", "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": "baik" }]
        }),
    ]);
    let mut tools = FakeTools::default();
    assert_eq!(
        model(endpoint, KEY_ENV).complete("x", &mut tools).unwrap(),
        "baik"
    );
    assert!(tools.calls.is_empty());
}

#[test]
fn anthropic_unreachable_endpoint_is_an_error() {
    std::env::set_var(KEY_ENV, "rahasia-uji");
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let mut tools = FakeTools::default();
    let error = model(format!("http://127.0.0.1:{port}"), KEY_ENV)
        .complete("x", &mut tools)
        .unwrap_err();
    assert!(error.contains("tidak terjangkau"), "{error}");
}
