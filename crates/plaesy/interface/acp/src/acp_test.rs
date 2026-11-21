use super::*;
use plaesy_adapter_sqlite::SqliteStore;
use std::collections::VecDeque;

/// Transport skrip: menyediakan pesan dari klien berurutan, dan mencatat semua yang dikirim agen.
#[derive(Default)]
struct Scripted {
    incoming: VecDeque<Incoming>,
    sent: Vec<Value>,
}

impl Scripted {
    fn with(incoming: Vec<Value>) -> Self {
        Self {
            incoming: incoming.into_iter().map(Incoming::Message).collect(),
            sent: Vec::new(),
        }
    }
}

impl Transport for Scripted {
    fn send(&mut self, message: &Value) -> io::Result<()> {
        self.sent.push(message.clone());
        Ok(())
    }

    fn recv(&mut self) -> io::Result<Option<Incoming>> {
        Ok(self.incoming.pop_front())
    }
}

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("docs.sqlite")).unwrap();
    (dir, store)
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// Balasan agen untuk permintaan `id` (bukan permintaan ke klien).
fn reply_for(sent: &[Value], id: u64) -> &Value {
    sent.iter()
        .find(|m| m.get("id") == Some(&json!(id)) && m.get("method").is_none())
        .expect("balasan untuk permintaan ini harus ada")
}

fn updates(sent: &[Value]) -> Vec<&Value> {
    sent.iter()
        .filter(|m| m.get("method") == Some(&json!("session/update")))
        .collect()
}

fn cwd_string() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    (dir, cwd)
}

fn client_caps(fs_read: bool, fs_write: bool, terminal: bool) -> Value {
    json!({
        "fs": { "readTextFile": fs_read, "writeTextFile": fs_write },
        "terminal": terminal
    })
}

/// Membuat sesi dan mengembalikan id-nya.
fn new_session(agent: &Agent<'_>, io: &mut Scripted, cwd: &str) -> String {
    agent
        .handle(
            &request(900, "session/new", json!({ "cwd": cwd, "mcpServers": [] })),
            io,
        )
        .unwrap();
    reply_for(&io.sent, 900)["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn prompt(agent: &Agent<'_>, io: &mut Scripted, session: &str, text: &str) {
    agent
        .handle(
            &request(
                901,
                "session/prompt",
                json!({ "sessionId": session, "prompt": [{ "type": "text", "text": text }] }),
            ),
            io,
        )
        .unwrap();
}

struct Echo;

impl ModelBackend for Echo {
    fn complete(&self, prompt: &str, _tools: &mut dyn ToolHost) -> Result<String, String> {
        Ok(format!("gema: {prompt}"))
    }
}

struct Broken;

impl ModelBackend for Broken {
    fn complete(&self, _prompt: &str, _tools: &mut dyn ToolHost) -> Result<String, String> {
        Err("kunci API tidak ada".to_owned())
    }
}

/// Model yang membaca satu berkas lewat klien, lalu mengembalikan isinya.
struct ReadModel(String);

impl ModelBackend for ReadModel {
    fn complete(&self, _prompt: &str, tools: &mut dyn ToolHost) -> Result<String, String> {
        tools.read_text_file(&self.0)
    }
}

/// Model yang meminta izin, lalu melaporkan opsi yang dipilih.
struct PermissionModel;

impl ModelBackend for PermissionModel {
    fn complete(&self, _prompt: &str, tools: &mut dyn ToolHost) -> Result<String, String> {
        let chosen = tools.request_permission("Hapus berkas", &[("allow", "Izinkan")])?;
        Ok(format!("{chosen:?}"))
    }
}

/// Model yang menjalankan satu perintah lewat terminal klien.
struct TerminalModel;

impl ModelBackend for TerminalModel {
    fn complete(&self, _prompt: &str, tools: &mut dyn ToolHost) -> Result<String, String> {
        let result = tools.run_command("echo", &["hai".to_owned()])?;
        Ok(format!("{}|{:?}", result.output, result.exit_code))
    }
}

#[test]
fn acp_initialize_advertises_v1_and_load_session() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(
            &request(1, "initialize", json!({ "protocolVersion": 1 })),
            &mut io,
        )
        .unwrap();
    let result = &reply_for(&io.sent, 1)["result"];
    assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(result["agentCapabilities"]["loadSession"], true);
}

#[test]
fn acp_session_new_requires_absolute_cwd() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(
            &request(
                1,
                "session/new",
                json!({ "cwd": "relatif", "mcpServers": [] }),
            ),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 1)["error"]["code"], INVALID_PARAMS);

    let (_cwd_dir, cwd) = cwd_string();
    let session = new_session(&agent, &mut io, &cwd);
    assert!(session.starts_with("sess-"));
}

#[test]
fn acp_prompt_without_model_says_so() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let (_cwd_dir, cwd) = cwd_string();
    let mut io = Scripted::default();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "halo");
    assert_eq!(
        updates(&io.sent)[0]["params"]["update"]["content"]["text"],
        NO_MODEL_TEXT
    );
    assert_eq!(reply_for(&io.sent, 901)["result"]["stopReason"], "end_turn");
}

#[test]
fn acp_prompt_text_goes_to_the_configured_model() {
    let (_store_dir, store) = store();
    let agent = Agent::with_model(&store, Box::new(Echo));
    let (_cwd_dir, cwd) = cwd_string();
    let mut io = Scripted::default();
    let session = new_session(&agent, &mut io, &cwd);
    agent
        .handle(
            &request(
                902,
                "session/prompt",
                json!({
                    "sessionId": session,
                    "prompt": [
                        { "type": "text", "text": "satu" },
                        { "type": "image", "data": "abc" },
                        { "type": "text", "text": "dua" }
                    ]
                }),
            ),
            &mut io,
        )
        .unwrap();
    assert_eq!(
        updates(&io.sent)[0]["params"]["update"]["content"]["text"],
        "gema: satu\ndua"
    );
}

#[test]
fn acp_model_failure_is_a_server_error_and_leaves_history_unchanged() {
    let (_store_dir, store) = store();
    let agent = Agent::with_model(&store, Box::new(Broken));
    let (_cwd_dir, cwd) = cwd_string();
    let mut io = Scripted::default();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "halo");
    let error = &reply_for(&io.sent, 901)["error"];
    assert_eq!(error["code"], SERVER_ERROR);
    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("kunci API tidak ada"));
    assert!(updates(&io.sent).is_empty(), "tidak ada jawaban palsu");
    let stored = store.get(SESSION_KIND, &session).unwrap().unwrap();
    assert_eq!(stored["messages"], json!([]));
}

#[test]
fn acp_session_load_replays_history_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("docs.sqlite");
    let (_cwd_dir, cwd) = cwd_string();

    // Proses pertama: buat sesi dan dua giliran.
    let session = {
        let store = SqliteStore::open(&db).unwrap();
        let agent = Agent::with_model(&store, Box::new(Echo));
        let mut io = Scripted::default();
        let session = new_session(&agent, &mut io, &cwd);
        prompt(&agent, &mut io, &session, "satu");
        prompt(&agent, &mut io, &session, "dua");
        session
    };

    // Proses kedua: sesi dimuat ulang dari penyimpanan dan riwayatnya dikirim ulang.
    let store = SqliteStore::open(&db).unwrap();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(
            &request(
                4,
                "session/load",
                json!({ "sessionId": session, "cwd": cwd, "mcpServers": [] }),
            ),
            &mut io,
        )
        .unwrap();
    let kinds: Vec<&str> = updates(&io.sent)
        .iter()
        .map(|u| u["params"]["update"]["sessionUpdate"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "user_message_chunk",
            "agent_message_chunk",
            "user_message_chunk",
            "agent_message_chunk"
        ]
    );
    assert_eq!(reply_for(&io.sent, 4)["result"], Value::Null);
}

#[test]
fn acp_session_load_rejects_a_different_cwd() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let (_cwd_dir, cwd) = cwd_string();
    let mut io = Scripted::default();
    let session = new_session(&agent, &mut io, &cwd);
    let (_other, other_cwd) = cwd_string();
    agent
        .handle(
            &request(
                2,
                "session/load",
                json!({ "sessionId": session, "cwd": other_cwd, "mcpServers": [] }),
            ),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 2)["error"]["code"], INVALID_PARAMS);
}

#[test]
fn acp_read_file_inside_cwd_uses_client_when_capable() {
    let (_store_dir, store) = store();
    let (cwd_dir, cwd) = cwd_string();
    let path = cwd_dir
        .path()
        .join("catatan.txt")
        .to_string_lossy()
        .to_string();
    let agent = Agent::with_model(&store, Box::new(ReadModel(path.clone())));
    let mut io = Scripted::with(vec![json!({
        "jsonrpc": "2.0",
        "id": "agent-1",
        "result": { "content": "isi dari klien" }
    })]);
    agent
        .handle(
            &request(1, "initialize", json!({ "protocolVersion": 1, "clientCapabilities": client_caps(true, false, false) })),
            &mut io,
        )
        .unwrap();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "baca");

    assert_eq!(reply_for(&io.sent, 901)["result"]["stopReason"], "end_turn");
    assert_eq!(
        updates(&io.sent)[0]["params"]["update"]["content"]["text"],
        "isi dari klien"
    );
    let sent_request = io
        .sent
        .iter()
        .find(|m| m.get("method") == Some(&json!("fs/read_text_file")))
        .expect("agen harus meminta fs/read_text_file");
    assert_eq!(sent_request["params"]["path"], json!(path));
}

#[test]
fn acp_read_file_outside_cwd_is_refused_without_calling_client() {
    let (_store_dir, store) = store();
    let (_cwd_dir, cwd) = cwd_string();
    let agent = Agent::with_model(&store, Box::new(ReadModel("/etc/rahasia".to_owned())));
    let mut io = Scripted::default();
    agent
        .handle(
            &request(1, "initialize", json!({ "protocolVersion": 1, "clientCapabilities": client_caps(true, true, true) })),
            &mut io,
        )
        .unwrap();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "x");
    assert_eq!(reply_for(&io.sent, 901)["error"]["code"], SERVER_ERROR);
    assert!(io
        .sent
        .iter()
        .all(|m| m.get("method") != Some(&json!("fs/read_text_file"))));
}

#[test]
fn acp_read_file_without_capability_is_refused() {
    let (_store_dir, store) = store();
    let (cwd_dir, cwd) = cwd_string();
    let path = cwd_dir.path().join("x.txt").to_string_lossy().to_string();
    let agent = Agent::with_model(&store, Box::new(ReadModel(path)));
    let mut io = Scripted::default();
    agent
        .handle(
            &request(1, "initialize", json!({ "protocolVersion": 1, "clientCapabilities": client_caps(false, false, false) })),
            &mut io,
        )
        .unwrap();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "x");
    assert_eq!(reply_for(&io.sent, 901)["error"]["code"], SERVER_ERROR);
    assert!(io
        .sent
        .iter()
        .all(|m| m.get("method") != Some(&json!("fs/read_text_file"))));
}

#[test]
fn acp_permission_returns_the_selected_option() {
    let (_store_dir, store) = store();
    let (_cwd_dir, cwd) = cwd_string();
    let agent = Agent::with_model(&store, Box::new(PermissionModel));
    let mut io = Scripted::with(vec![json!({
        "jsonrpc": "2.0",
        "id": "agent-1",
        "result": { "outcome": { "outcome": "selected", "optionId": "allow" } }
    })]);
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "x");
    assert_eq!(
        updates(&io.sent)[0]["params"]["update"]["content"]["text"],
        "Some(\"allow\")"
    );
    assert!(io
        .sent
        .iter()
        .any(|m| m.get("method") == Some(&json!("session/request_permission"))));
}

#[test]
fn acp_permission_cancelled_gives_none() {
    let (_store_dir, store) = store();
    let (_cwd_dir, cwd) = cwd_string();
    let agent = Agent::with_model(&store, Box::new(PermissionModel));
    let mut io = Scripted::with(vec![json!({
        "jsonrpc": "2.0",
        "id": "agent-1",
        "result": { "outcome": { "outcome": "cancelled" } }
    })]);
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "x");
    assert_eq!(
        updates(&io.sent)[0]["params"]["update"]["content"]["text"],
        "None"
    );
}

#[test]
fn acp_terminal_runs_create_wait_output_release() {
    let (_store_dir, store) = store();
    let (_cwd_dir, cwd) = cwd_string();
    let agent = Agent::with_model(&store, Box::new(TerminalModel));
    let mut io = Scripted::with(vec![
        // Izin pengguna dulu: perintah dari model tidak dijalankan tanpa persetujuan.
        json!({ "jsonrpc": "2.0", "id": "agent-1", "result": { "outcome": { "outcome": "selected", "optionId": "allow" } } }),
        json!({ "jsonrpc": "2.0", "id": "agent-2", "result": { "terminalId": "t1" } }),
        json!({ "jsonrpc": "2.0", "id": "agent-3", "result": { "exitCode": 0 } }),
        json!({ "jsonrpc": "2.0", "id": "agent-4", "result": { "output": "hai\n", "truncated": false } }),
        json!({ "jsonrpc": "2.0", "id": "agent-5", "result": null }),
    ]);
    agent
        .handle(
            &request(1, "initialize", json!({ "protocolVersion": 1, "clientCapabilities": client_caps(false, false, true) })),
            &mut io,
        )
        .unwrap();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "x");
    assert_eq!(
        updates(&io.sent)[0]["params"]["update"]["content"]["text"],
        "hai\n|Some(0)"
    );
    let methods: Vec<&str> = io
        .sent
        .iter()
        .filter_map(|m| m.get("method").and_then(Value::as_str))
        .filter(|m| m.starts_with("terminal/"))
        .collect();
    assert_eq!(
        methods,
        [
            "terminal/create",
            "terminal/wait_for_exit",
            "terminal/output",
            "terminal/release"
        ]
    );
}

#[test]
fn acp_terminal_denied_by_user_is_not_created() {
    let (_store_dir, store) = store();
    let (_cwd_dir, cwd) = cwd_string();
    let agent = Agent::with_model(&store, Box::new(TerminalModel));
    let mut io = Scripted::with(vec![
        json!({ "jsonrpc": "2.0", "id": "agent-1", "result": { "outcome": { "outcome": "cancelled" } } }),
    ]);
    agent
        .handle(
            &request(1, "initialize", json!({ "protocolVersion": 1, "clientCapabilities": client_caps(false, false, true) })),
            &mut io,
        )
        .unwrap();
    let session = new_session(&agent, &mut io, &cwd);
    prompt(&agent, &mut io, &session, "x");
    let created = io
        .sent
        .iter()
        .filter_map(|m| m.get("method").and_then(Value::as_str))
        .any(|m| m == "terminal/create");
    assert!(!created, "perintah tidak boleh dibuat setelah izin ditolak");
}

#[test]
fn acp_document_extension_roundtrip() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(
            &request(
                1,
                "_plaesy/document/put",
                json!({ "kind": "flow-ir/1", "id": "a", "document": {"n": 1} }),
            ),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 1)["result"]["ok"], true);
    agent
        .handle(
            &request(
                2,
                "_plaesy/document/get",
                json!({ "kind": "flow-ir/1", "id": "a" }),
            ),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 2)["result"], json!({"n": 1}));
    agent
        .handle(
            &request(3, "_plaesy/document/list", json!({ "kind": "flow-ir/1" })),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 3)["result"]["ids"], json!(["a"]));
    agent
        .handle(
            &request(
                4,
                "_plaesy/document/delete",
                json!({ "kind": "flow-ir/1", "id": "a" }),
            ),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 4)["result"]["deleted"], true);
}

#[test]
fn acp_invalid_key_is_invalid_params_not_server_error() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(
            &request(1, "_plaesy/document/list", json!({ "kind": "" })),
            &mut io,
        )
        .unwrap();
    assert_eq!(reply_for(&io.sent, 1)["error"]["code"], INVALID_PARAMS);
}

#[test]
fn acp_unknown_method_is_method_not_found() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(&request(1, "nope", json!({})), &mut io)
        .unwrap();
    assert_eq!(reply_for(&io.sent, 1)["error"]["code"], METHOD_NOT_FOUND);
}

#[test]
fn acp_cancel_notification_gets_no_reply() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let mut io = Scripted::default();
    agent
        .handle(
            &json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": {} }),
            &mut io,
        )
        .unwrap();
    assert!(io.sent.is_empty());
}

#[test]
fn acp_serve_writes_only_json_lines_and_reports_parse_errors() {
    let (_store_dir, store) = store();
    let agent = Agent::new(&store);
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1}}\n",
        "bukan json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"session/cancel\",\"params\":{}}\n",
    );
    let mut output = Vec::new();
    serve(&agent, input.as_bytes(), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("setiap baris stdout harus JSON"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["error"]["code"], PARSE_ERROR);
}
