use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

type Captured<T> = Arc<Mutex<Vec<T>>>;

/// Server HTTP palsu di loopback. Membalas tiap permintaan dengan `replies[i]` (atau yang terakhir
/// bila habis). Mengembalikan endpoint, body setiap permintaan, dan header Authorization yang diterima.
fn fake_openai(replies: Vec<Value>) -> (String, Captured<Value>, Captured<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let bodies: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let auths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_bodies = Arc::clone(&bodies);
    let seen_auths = Arc::clone(&auths);
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
                let lower = trimmed.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                if let Some(v) = lower.strip_prefix("authorization:") {
                    seen_auths.lock().unwrap().push(v.trim().to_owned());
                }
            }
            let mut buf = vec![0u8; length];
            reader.read_exact(&mut buf).unwrap();
            seen_bodies
                .lock()
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
    (endpoint, bodies, auths)
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

fn model(endpoint: String, api_key_env: &str) -> OpenAiToolModel {
    OpenAiToolModel {
        endpoint,
        model: "model-uji".to_owned(),
        api_key_env: api_key_env.to_owned(),
        timeout: Duration::from_secs(10),
    }
}

fn tool_call_reply(id: &str, name: &str, arguments: &str) -> Value {
    json!({ "choices": [{ "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": id,
            "type": "function",
            "function": { "name": name, "arguments": arguments }
        }]
    } }] })
}

fn text_reply(content: &str) -> Value {
    json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] })
}

#[test]
fn openai_tool_call_arguments_are_parsed_and_result_sent_with_tool_call_id() {
    let (endpoint, bodies, _auths) = fake_openai(vec![
        tool_call_reply("call_1", "read_file", r#"{"path":"/proyek/a.txt"}"#),
        text_reply("Isinya: isi berkas"),
    ]);
    let mut tools = FakeTools::default();
    let answer = model(endpoint, "")
        .complete("baca a.txt", &mut tools)
        .unwrap();

    assert_eq!(answer, "Isinya: isi berkas");
    assert_eq!(tools.calls, ["read /proyek/a.txt"]);
    let seen = bodies.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0]["tools"].as_array().unwrap().len(), 4);
    let second = seen[1]["messages"].as_array().unwrap();
    assert!(second.iter().any(|m| {
        m["role"] == "tool" && m["tool_call_id"] == "call_1" && m["content"] == "isi berkas"
    }));
}

#[test]
fn openai_plain_answer_needs_no_tools() {
    let (endpoint, _bodies, _auths) = fake_openai(vec![text_reply("halo")]);
    let mut tools = FakeTools::default();
    assert_eq!(
        model(endpoint, "").complete("hai", &mut tools).unwrap(),
        "halo"
    );
    assert!(tools.calls.is_empty());
}

#[test]
fn openai_unparseable_arguments_become_an_empty_object_not_a_panic() {
    let (endpoint, _bodies, _auths) = fake_openai(vec![
        tool_call_reply("call_x", "read_file", "bukan json"),
        text_reply("baik"),
    ]);
    let mut tools = FakeTools::default();
    assert_eq!(
        model(endpoint, "").complete("x", &mut tools).unwrap(),
        "baik"
    );
    // Tanpa argumen `path`, alat dipanggil dengan path kosong dan tidak menyebabkan galat fatal.
    assert_eq!(tools.calls, ["read "]);
}

#[test]
fn openai_api_key_is_sent_as_bearer_from_env_var() {
    // Variabel unik untuk tes ini; nilainya tidak pernah dicetak.
    std::env::set_var("PLAESY_TEST_OPENAI_KEY_UJI", "rahasia-uji-123");
    let (endpoint, _bodies, auths) = fake_openai(vec![text_reply("ok")]);
    let mut tools = FakeTools::default();
    model(endpoint, "PLAESY_TEST_OPENAI_KEY_UJI")
        .complete("x", &mut tools)
        .unwrap();
    let seen = auths.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0], "bearer rahasia-uji-123");
}

#[test]
fn openai_missing_api_key_env_is_an_error_without_leaking_anything() {
    let (endpoint, bodies, _auths) = fake_openai(vec![text_reply("ok")]);
    let mut tools = FakeTools::default();
    let error = model(endpoint, "PLAESY_TEST_VAR_TIDAK_ADA_XYZ")
        .complete("x", &mut tools)
        .unwrap_err();
    assert!(error.contains("PLAESY_TEST_VAR_TIDAK_ADA_XYZ"));
    assert!(
        bodies.lock().unwrap().is_empty(),
        "tidak boleh mengirim permintaan tanpa kunci"
    );
}

#[test]
fn openai_endless_tool_loop_stops_at_the_round_limit() {
    let (endpoint, bodies, _auths) = fake_openai(vec![tool_call_reply(
        "call_loop",
        "read_file",
        r#"{"path":"/p/x"}"#,
    )]);
    let mut tools = FakeTools::default();
    let error = model(endpoint, "").complete("x", &mut tools).unwrap_err();
    assert!(error.contains("putaran"), "{error}");
    assert_eq!(bodies.lock().unwrap().len(), MAX_ROUNDS);
}

#[test]
fn openai_unreachable_endpoint_is_an_error() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let mut tools = FakeTools::default();
    let error = model(format!("http://127.0.0.1:{port}"), "")
        .complete("x", &mut tools)
        .unwrap_err();
    assert!(error.contains("tidak terjangkau"), "{error}");
}
