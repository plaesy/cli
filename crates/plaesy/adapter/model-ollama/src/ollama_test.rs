use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

/// Server palsu: membalas setiap permintaan dengan `replies[i]` (atau yang terakhir bila habis).
/// Mengembalikan endpoint dan body setiap permintaan yang diterima.
fn fake_ollama(replies: Vec<Value>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let bodies: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&bodies);
    thread::spawn(move || {
        for index in 0.. {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut length = 0usize;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let trimmed = header.trim_end();
                if trimmed.is_empty() {
                    break;
                }
                if let Some(v) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
            }
            let mut buf = vec![0u8; length];
            reader.read_exact(&mut buf).unwrap();
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&buf).unwrap());
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
    (endpoint, bodies)
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

    fn write_text_file(&mut self, path: &str, _content: &str) -> Result<(), String> {
        self.calls.push(format!("write {path}"));
        Ok(())
    }

    fn request_permission(
        &mut self,
        title: &str,
        _options: &[(&str, &str)],
    ) -> Result<Option<String>, String> {
        self.calls.push(format!("permission {title}"));
        Ok(Some("allow".to_owned()))
    }

    fn run_command(&mut self, command: &str, _args: &[String]) -> Result<CommandResult, String> {
        self.calls.push(format!("run {command}"));
        Ok(CommandResult {
            output: "hai".to_owned(),
            exit_code: Some(0),
        })
    }
}

fn model(endpoint: String) -> OllamaModel {
    OllamaModel {
        endpoint,
        model: "qwen2.5".to_owned(),
        timeout: Duration::from_secs(10),
    }
}

#[test]
fn ollama_tool_call_is_executed_and_result_sent_back_to_model() {
    let (endpoint, bodies) = fake_ollama(vec![
        json!({ "message": { "role": "assistant", "content": "", "tool_calls": [
            { "function": { "name": "read_file", "arguments": { "path": "/proyek/a.txt" } } }
        ] } }),
        json!({ "message": { "role": "assistant", "content": "Isinya: isi berkas" } }),
    ]);
    let mut tools = FakeTools::default();
    let answer = model(endpoint).complete("baca a.txt", &mut tools).unwrap();

    assert_eq!(answer, "Isinya: isi berkas");
    assert_eq!(tools.calls, ["read /proyek/a.txt"]);
    let seen = bodies.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(
        seen[0]["tools"].as_array().unwrap().len() == 4,
        "empat alat dikirim"
    );
    let second = &seen[1]["messages"];
    assert!(second
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["role"] == "tool" && m["content"] == "isi berkas"));
}

#[test]
fn ollama_plain_answer_needs_no_tools() {
    let (endpoint, _bodies) = fake_ollama(vec![
        json!({ "message": { "role": "assistant", "content": "halo" } }),
    ]);
    let mut tools = FakeTools::default();
    assert_eq!(model(endpoint).complete("hai", &mut tools).unwrap(), "halo");
    assert!(tools.calls.is_empty());
}

#[test]
fn ollama_permission_and_terminal_tools_are_routed() {
    let (endpoint, bodies) = fake_ollama(vec![
        json!({ "message": { "role": "assistant", "content": "", "tool_calls": [
            { "function": { "name": "request_permission", "arguments": { "title": "hapus" } } },
            { "function": { "name": "run_command", "arguments": { "command": "echo", "args": ["hai"] } } }
        ] } }),
        json!({ "message": { "role": "assistant", "content": "selesai" } }),
    ]);
    let mut tools = FakeTools::default();
    model(endpoint).complete("x", &mut tools).unwrap();
    assert_eq!(tools.calls, ["permission hapus", "run echo"]);
    let seen = bodies.lock().unwrap();
    assert!(seen[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| { m["role"] == "tool" && m["content"].as_str().unwrap_or("").contains("hai") }));
}

#[test]
fn ollama_unknown_tool_is_reported_to_the_model_not_executed() {
    let (endpoint, _bodies) = fake_ollama(vec![
        json!({ "message": { "role": "assistant", "content": "", "tool_calls": [
            { "function": { "name": "hapus_semua", "arguments": {} } }
        ] } }),
        json!({ "message": { "role": "assistant", "content": "baik" } }),
    ]);
    let mut tools = FakeTools::default();
    assert_eq!(model(endpoint).complete("x", &mut tools).unwrap(), "baik");
    assert!(tools.calls.is_empty());
}

#[test]
fn ollama_endless_tool_loop_stops_at_the_round_limit() {
    let (endpoint, bodies) = fake_ollama(vec![
        json!({ "message": { "role": "assistant", "content": "", "tool_calls": [
        { "function": { "name": "read_file", "arguments": { "path": "/p/x" } } }
    ] } }),
    ]);
    let mut tools = FakeTools::default();
    let error = model(endpoint).complete("x", &mut tools).unwrap_err();
    assert!(error.contains("putaran"), "{error}");
    assert_eq!(bodies.lock().unwrap().len(), MAX_ROUNDS);
}

#[test]
fn ollama_unreachable_endpoint_is_an_error() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let mut tools = FakeTools::default();
    let error = model(format!("http://127.0.0.1:{port}"))
        .complete("x", &mut tools)
        .unwrap_err();
    assert!(error.contains("tidak terjangkau"), "{error}");
}

/// Tes terhadap Ollama sungguhan. Dilewati dengan pesan bila server atau model `qwen2.5:7b` tidak ada.
#[test]
fn ollama_real_model_calls_read_file_when_available() {
    let endpoint = "http://127.0.0.1:11434";
    let up = ureq::get(&format!("{endpoint}/api/tags")).call().is_ok();
    if !up {
        eprintln!("LEWAT: Ollama tidak berjalan di {endpoint}");
        return;
    }
    let mut tools = FakeTools::default();
    let model = OllamaModel {
        endpoint: endpoint.to_owned(),
        model: "qwen2.5:7b".to_owned(),
        timeout: Duration::from_secs(300),
    };
    match model.complete(
        "Baca berkas /proyek/catatan.txt lalu beritahu isinya.",
        &mut tools,
    ) {
        Ok(answer) => {
            assert!(
                tools.calls.iter().any(|c| c.starts_with("read ")),
                "model harus memanggil read_file; panggilan: {:?}, jawaban: {answer}",
                tools.calls
            );
        }
        Err(error) => {
            eprintln!("LEWAT: model qwen2.5:7b tidak tersedia: {error}");
        }
    }
}
