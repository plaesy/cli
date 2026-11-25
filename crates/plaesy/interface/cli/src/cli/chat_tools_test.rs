//! Unit tests for `chat_tools.rs`, included via `#[path]`.

use std::collections::VecDeque;
use std::fs;
use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::delegate::{ProviderReply, Request};

/// Provider that answers from a script and records each prompt it was sent.
struct Scripted {
    replies: Mutex<VecDeque<String>>,
    prompts: Mutex<Vec<String>>,
}

impl Scripted {
    fn new(replies: &[&str]) -> Self {
        Self {
            replies: Mutex::new(replies.iter().map(|r| (*r).to_owned()).collect()),
            prompts: Mutex::new(Vec::new()),
        }
    }
}

impl Provider for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        self.prompts.lock().expect("lock").push(req.prompt.clone());
        let text = self
            .replies
            .lock()
            .expect("lock")
            .pop_front()
            .ok_or_else(|| DelegateError::terminal("skrip habis"))?;
        Ok(ProviderReply {
            text,
            input_tokens: None,
            output_tokens: None,
        })
    }
}

fn request() -> Request {
    Request {
        prompt: String::new(),
        max_output_tokens: 0,
        timeout: std::time::Duration::from_secs(5),
        dir: String::new(),
    }
}

fn tools(root: &std::path::Path, allow_writes: bool) -> Tools {
    Tools {
        root: root.to_path_buf(),
        allow_writes,
    }
}

fn repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("src")).expect("mkdir");
    fs::write(temp.path().join("src/main.rs"), "fn main() {}\n").expect("write");
    fs::write(temp.path().join("README.md"), "halo").expect("write");
    temp
}

#[test]
fn a_tool_block_is_found_and_its_json_parsed() {
    let text = "Saya cek dulu.\n```tool\n{\"tool\":\"read_file\",\"path\":\"README.md\"}\n```\n";
    let call = find_call(text).expect("call");
    assert_eq!(call["tool"], "read_file");
    assert_eq!(call["path"], "README.md");
}

#[test]
fn text_without_a_block_or_with_bad_json_is_not_a_call() {
    assert!(find_call("tidak ada alat").is_none());
    assert!(find_call("```tool\nbukan json\n```").is_none());
    assert!(find_call("```tool\n[1,2]\n```").is_none());
}

#[test]
fn read_file_returns_the_content_under_the_root() {
    let temp = repo();
    let out = execute(
        &tools(temp.path(), false),
        &json!({"tool":"read_file","path":"src/main.rs"}),
    );
    assert_eq!(out, "fn main() {}\n");
}

#[test]
fn list_dir_marks_folders_and_sorts_names() {
    let temp = repo();
    let out = execute(
        &tools(temp.path(), false),
        &json!({"tool":"list_dir","path":"."}),
    );
    assert_eq!(out, "README.md\nsrc/");
}

#[test]
fn paths_outside_the_root_are_refused() {
    let temp = repo();
    for path in [
        "../secret",
        "/etc/passwd",
        "src/../../x",
        "C:\\Windows\\win.ini",
    ] {
        let out = execute(
            &tools(temp.path(), false),
            &json!({"tool":"read_file","path":path}),
        );
        assert!(
            out.starts_with("error:"),
            "seharusnya ditolak: {path} -> {out}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_that_leaves_the_root_is_refused() {
    let temp = repo();
    let outside = tempfile::tempdir().expect("tempdir");
    fs::write(outside.path().join("rahasia.txt"), "x").expect("write");
    std::os::unix::fs::symlink(outside.path(), temp.path().join("link")).expect("symlink");
    let out = execute(
        &tools(temp.path(), false),
        &json!({"tool":"read_file","path":"link/rahasia.txt"}),
    );
    assert!(out.starts_with("error:"), "{out}");
}

#[test]
fn unknown_tools_and_missing_files_come_back_as_text() {
    let temp = repo();
    assert_eq!(
        execute(&tools(temp.path(), false), &json!({"tool":"hapus_semua"})),
        "error: alat tidak dikenal: hapus_semua"
    );
    assert!(
        execute(
            &tools(temp.path(), false),
            &json!({"tool":"read_file","path":"tidak_ada.txt"})
        )
        .starts_with("error:")
    );
}

#[test]
fn long_output_is_cut_at_a_character_boundary() {
    let temp = repo();
    fs::write(temp.path().join("besar.txt"), "é".repeat(MAX_OUTPUT_BYTES)).expect("write");
    let out = execute(
        &tools(temp.path(), false),
        &json!({"tool":"read_file","path":"besar.txt"}),
    );
    assert!(out.ends_with("[dipotong]"));
    assert!(out.len() <= MAX_OUTPUT_BYTES + "\n[dipotong]".len());
}

#[test]
fn a_turn_runs_the_call_and_sends_its_result_back() {
    let temp = repo();
    let provider = Scripted::new(&[
        "Baik, saya baca dulu.\n```tool\n{\"tool\":\"read_file\",\"path\":\"README.md\"}\n```",
        "README berisi sapaan halo.",
    ]);
    let messages = vec![Message::user("apa isi README?")];
    let mut notes = Vec::new();
    let response = run_turn(
        &provider,
        &messages,
        &request(),
        &tools(temp.path(), false),
        &mut Vec::new(),
        &mut |n| {
            notes.push(n.to_owned());
        },
    )
    .expect("turn");
    assert_eq!(response.text, "README berisi sapaan halo.");
    assert_eq!(notes, vec!["[alat] read_file README.md".to_owned()]);
    let prompts = provider.prompts.lock().expect("lock");
    assert_eq!(prompts.len(), 2);
    assert!(prompts[1].contains("Hasil alat:\nhalo"), "{}", prompts[1]);
}

#[test]
fn a_model_that_keeps_asking_is_stopped_at_the_round_limit() {
    let temp = repo();
    let call = "```tool\n{\"tool\":\"list_dir\",\"path\":\".\"}\n```";
    let replies: Vec<&str> = std::iter::repeat_n(call, MAX_ROUNDS).collect();
    let provider = Scripted::new(&replies);
    let err = run_turn(
        &provider,
        &[Message::user("terus")],
        &request(),
        &tools(temp.path(), false),
        &mut Vec::new(),
        &mut |_| {},
    )
    .expect_err("limit");
    assert!(err.to_string().contains("batas"), "{err}");
}

#[test]
fn writes_are_refused_without_allow_writes() {
    let temp = repo();
    let call = json!({"tool":"write_file","path":"novo.txt","content":"x"});
    let out = execute(&tools(temp.path(), false), &call);
    assert!(out.contains("--allow-writes"), "{out}");
    assert!(!temp.path().join("novo.txt").exists());
    let run = json!({"tool":"run_command","command":"cmd","args":[]});
    assert!(execute(&tools(temp.path(), false), &run).contains("--allow-writes"));
}

#[test]
fn write_file_creates_and_replaces_a_file_under_the_root() {
    let temp = repo();
    let allowed = tools(temp.path(), true);
    let out = execute(
        &allowed,
        &json!({"tool":"write_file","path":"src/nuevo.rs","content":"fn x() {}"}),
    );
    assert!(out.starts_with("ditulis"), "{out}");
    assert_eq!(
        fs::read_to_string(temp.path().join("src/nuevo.rs")).expect("read"),
        "fn x() {}"
    );
    let _ = execute(
        &allowed,
        &json!({"tool":"write_file","path":"README.md","content":"baru"}),
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("README.md")).expect("read"),
        "baru"
    );
}

#[test]
fn write_file_refuses_paths_outside_the_root() {
    let temp = repo();
    let allowed = tools(temp.path(), true);
    for path in ["../luar.txt", "/tmp/luar.txt", "src/../../luar.txt"] {
        let out = execute(
            &allowed,
            &json!({"tool":"write_file","path":path,"content":"x"}),
        );
        assert!(
            out.starts_with("error:"),
            "seharusnya ditolak: {path} -> {out}"
        );
    }
}

#[test]
fn run_command_runs_a_program_in_the_root_and_reports_the_exit_code() {
    let temp = repo();
    let allowed = tools(temp.path(), true);
    let program = if cfg!(windows) { "cmd" } else { "sh" };
    let args: Vec<&str> = if cfg!(windows) {
        vec!["/C", "echo", "halo"]
    } else {
        vec!["-c", "echo halo"]
    };
    let out = execute(
        &allowed,
        &json!({"tool":"run_command","command":program,"args":args}),
    );
    assert!(out.starts_with("exit=Some(0)"), "{out}");
    assert!(out.contains("halo"), "{out}");
}

#[test]
fn run_command_refuses_a_path_as_the_program() {
    let temp = repo();
    let allowed = tools(temp.path(), true);
    let out = execute(
        &allowed,
        &json!({"tool":"run_command","command":"./skrip","args":[]}),
    );
    assert!(out.starts_with("error:"), "{out}");
}

#[test]
fn undo_restores_the_previous_bytes_or_removes_a_new_file() {
    let temp = repo();
    let allowed = tools(temp.path(), true);
    let mut undo = Vec::new();
    let _ = execute_with(
        &allowed,
        &json!({"tool":"write_file","path":"README.md","content":"baru"}),
        &mut undo,
    );
    let _ = execute_with(
        &allowed,
        &json!({"tool":"write_file","path":"baru.txt","content":"x"}),
        &mut undo,
    );
    assert_eq!(undo.len(), 2);
    assert!(
        undo_last(&mut undo)
            .expect("undo")
            .starts_with("dibatalkan")
    );
    assert!(!temp.path().join("baru.txt").exists());
    undo_last(&mut undo).expect("undo");
    assert_eq!(
        fs::read_to_string(temp.path().join("README.md")).expect("read"),
        "halo"
    );
    assert!(undo_last(&mut undo).is_err());
}
