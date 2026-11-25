use super::*;
use std::io::Read;
use std::sync::mpsc;

#[test]
fn transcript_splits_lines_and_keeps_a_partial_line_until_its_end() {
    let mut transcript = Transcript::default();
    transcript.push_text("satu\r\ndua");
    assert_eq!(transcript.visible(10, 0), vec!["satu", "dua"]);
    transcript.push_text(" lanjut\n");
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript.visible(10, 0), vec!["satu", "dua lanjut"]);
}

#[test]
fn visible_shows_the_newest_lines_and_scrolls_back() {
    let mut transcript = Transcript::default();
    for n in 1..=5 {
        transcript.push_text(&format!("{n}\n"));
    }
    assert_eq!(transcript.visible(2, 0), vec!["4", "5"]);
    assert_eq!(transcript.visible(2, 2), vec!["2", "3"]);
    assert_eq!(transcript.visible(10, 99), Vec::<String>::new());
}

#[test]
fn transcript_drops_the_oldest_lines_past_the_cap() {
    let mut transcript = Transcript::default();
    for n in 0..(MAX_LINES + 5) {
        transcript.push_text(&format!("{n}\n"));
    }
    assert_eq!(transcript.len(), MAX_LINES);
    assert_eq!(transcript.visible(1, 0), vec![(MAX_LINES + 4).to_string()]);
}

#[test]
fn input_edits_at_the_cursor_and_take_clears_it() {
    let mut input = Input::default();
    for c in "halo".chars() {
        input.insert(c);
    }
    input.left();
    input.left();
    input.insert('X');
    assert_eq!(input.text(), "haXlo");
    input.backspace();
    assert_eq!(input.text(), "halo");
    input.home();
    input.backspace();
    assert_eq!(input.text(), "halo", "backspace at the start does nothing");
    input.end();
    assert_eq!(input.take(), "halo");
    assert_eq!(input.text(), "");
}

#[test]
fn keys_map_to_screen_actions() {
    let key = |code, modifiers| KeyEvent::new(code, modifiers);
    assert_eq!(
        action(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Action::Quit
    );
    assert_eq!(
        action(&key(KeyCode::Esc, KeyModifiers::NONE)),
        Action::Cancel
    );
    assert_eq!(action(&key(KeyCode::Up, KeyModifiers::NONE)), Action::Up);
    assert_eq!(
        action(&key(KeyCode::Tab, KeyModifiers::NONE)),
        Action::Complete
    );
    assert_eq!(
        action(&key(KeyCode::Enter, KeyModifiers::NONE)),
        Action::Submit
    );
    assert_eq!(
        action(&key(KeyCode::Char('a'), KeyModifiers::NONE)),
        Action::Insert('a')
    );
    assert_eq!(
        action(&key(KeyCode::PageUp, KeyModifiers::NONE)),
        Action::ScrollUp
    );
    assert_eq!(
        action(&key(KeyCode::F(1), KeyModifiers::NONE)),
        Action::Ignore
    );
}

#[test]
fn lines_sent_to_the_session_come_back_one_per_read() {
    let (tx, rx) = mpsc::channel();
    let mut source = LineSource::new(rx);
    tx.send("pertama".to_owned()).expect("send");
    tx.send("kedua".to_owned()).expect("send");
    drop(tx);

    let mut text = String::new();
    source.read_to_string(&mut text).expect("read");
    assert_eq!(text, "pertama\nkedua\n");
}

#[test]
fn session_output_reaches_the_screen_channel() {
    let (tx, rx) = mpsc::channel();
    let mut sink = OutputSink { tx };
    writeln!(sink, "balasan").expect("write");
    drop(sink);
    let chunks: Vec<String> = rx.iter().collect();
    assert_eq!(chunks.concat(), "balasan\n");
}

#[test]
fn typing_a_slash_lists_matching_commands_and_nothing_else() {
    let names: Vec<&str> = suggestions("/pro")
        .iter()
        .map(|(name, _, _)| *name)
        .collect();
    assert_eq!(names, vec!["/provider", "/providers"]);
    assert_eq!(suggestions("/").len(), COMMANDS.len());
    assert!(
        suggestions("halo").is_empty(),
        "text without a slash shows no list"
    );
    assert!(
        suggestions("/ask a").is_empty(),
        "after the command name, no list"
    );
    assert!(suggestions("/zzz").is_empty());
}

#[test]
fn enter_completes_the_highlighted_command_and_the_next_enter_sends_it() {
    // First Enter on "/pro" with the first match highlighted: complete, adding a space.
    assert_eq!(completion("/pro", 0).as_deref(), Some("/provider "));
    assert_eq!(completion("/pro", 1).as_deref(), Some("/providers "));
    // The command is already typed in full: nothing to complete, so Enter sends.
    assert_eq!(completion("/exit", 0), None);
    assert_eq!(completion("halo", 0), None);
}

#[test]
fn set_replaces_the_line_and_puts_the_cursor_at_the_end() {
    let mut input = Input::default();
    input.insert('x');
    input.set("/provider ");
    assert_eq!(input.text(), "/provider ");
    input.insert('a');
    assert_eq!(input.text(), "/provider a");
}

fn screen_with_history(lines: &[&str]) -> Screen {
    Screen {
        lang: Lang::En,
        repo: ".".to_owned(),
        transcript: Transcript::default(),
        input: Input::default(),
        scroll: 0,
        quitting: false,
        finished: false,
        selected: 0,
        popup_hidden: false,
        history: lines.iter().map(|line| (*line).to_owned()).collect(),
        history_pos: None,
        draft: String::new(),
        provider: "test".to_owned(),
        model: String::new(),
        pending: 0,
        sent_at: None,
        tokens_in: 0,
        tokens_out: 0,
        capture: None,
        tick: 0,
    }
}

#[test]
fn a_mark_split_across_chunks_is_still_read_and_not_shown() {
    let mut screen = screen_with_history(&[]);
    let state = char::from(STATE_MARK);
    let usage = char::from(USAGE_MARK);
    // The chat writes each piece of a mark separately, so the screen gets them in separate chunks.
    screen.receive(&state.to_string());
    screen.receive("ollama\tqwen2.5:7b");
    screen.receive(&state.to_string());
    screen.receive(&format!("balasan\n{usage}"));
    screen.receive("45 18");
    screen.receive(&usage.to_string());
    assert!(screen.status_text().starts_with("[ollama/qwen2.5:7b] "));
    assert!(screen.status_text().contains("tokens 45 in · 18 out"));
    assert_eq!(screen.transcript.visible(10, 0), vec!["balasan"]);
}

#[test]
fn markdown_headings_code_and_bold_are_styled() {
    let heading = styled_line("## Judul");
    assert_eq!(heading.spans[0].content.as_ref(), "Judul");
    assert_eq!(heading.style, Style::new().bold());

    let mixed = styled_line("pakai `kode` dan **tebal**");
    let texts: Vec<&str> = mixed.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(texts, vec!["pakai ", "kode", " dan ", "tebal"]);
    assert_eq!(mixed.spans[1].style, Style::new().fg(Color::Yellow));
    assert_eq!(mixed.spans[3].style, Style::new().bold());

    assert_eq!(styled_line("› halo").style, Style::new().cyan());
    assert_eq!(
        styled_line("tanpa tanda").spans[0].content.as_ref(),
        "tanpa tanda"
    );
}

#[test]
fn a_state_mark_switches_the_provider_and_model_in_the_status_line() {
    let mut screen = screen_with_history(&[]);
    assert!(screen.status_text().starts_with("[test] "));
    let mark = char::from(STATE_MARK);
    screen.receive(&format!("{mark}ollama\tqwen3{mark}"));
    assert!(screen.status_text().starts_with("[ollama/qwen3] "));
    assert!(!screen.transcript.visible(10, 0).join("").contains(mark));
}

#[test]
fn token_counts_add_up_in_the_status_line_and_are_not_shown() {
    let mut screen = screen_with_history(&[]);
    let mark = char::from(USAGE_MARK);
    screen.receive(&format!(
        "balasan\n{mark}12 4{mark}{}",
        char::from(TURN_MARK)
    ));
    assert_eq!(screen.transcript.visible(10, 0), vec!["balasan"]);
    assert!(screen.status_text().contains("tokens 12 in · 4 out"));

    screen.receive(&format!("lagi\n{mark}3 2{mark}"));
    assert!(screen.status_text().contains("tokens 15 in · 6 out"));

    screen.receive(&format!("tanpa hitungan\n{mark}- -{mark}"));
    assert!(screen.status_text().contains("tokens 15 in · 6 out"));
}

#[test]
fn alt_enter_and_ctrl_j_add_a_line_break_while_enter_sends() {
    let alt_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
    assert_eq!(action(&alt_enter), Action::Newline);
    let ctrl_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    assert_eq!(action(&ctrl_j), Action::Newline);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(action(&enter), Action::Submit);
}

#[test]
fn cursor_position_counts_rows_and_columns_in_a_multi_line_input() {
    let mut input = Input::default();
    input.set("satu\ndua");
    assert_eq!(input.cursor_position(), (1, 3));
    input.home();
    assert_eq!(input.cursor_position(), (0, 0));
}

#[test]
fn a_multi_line_entry_reaches_the_chat_as_one_message() {
    assert_eq!(wire_lines("halo"), vec!["halo".to_owned()]);
    assert_eq!(wire_lines("a\nb"), vec!["\"\"\"", "a", "b", "\"\"\""]);
    let mut reader = crate::cli::chat::BlockReader::default();
    let messages: Vec<String> = wire_lines("a\nb")
        .into_iter()
        .filter_map(|line| reader.push(line))
        .collect();
    assert_eq!(messages, vec!["a\nb".to_owned()]);
}

#[test]
fn elapsed_time_is_shown_while_a_reply_is_pending() {
    assert_eq!(clock(Duration::from_secs(5)), "5s");
    assert_eq!(clock(Duration::from_secs(75)), "1m15s");

    let mut screen = screen_with_history(&[]);
    screen.pending = 1;
    screen.sent_at = Some(Instant::now());
    assert!(screen.status_text().contains("0s ·"));

    screen.receive(&char::from(TURN_MARK).to_string());
    assert_eq!(
        screen.sent_at, None,
        "the timer stops when the last reply ends"
    );
}

#[test]
fn turn_marks_close_pending_lines_and_are_not_shown() {
    let mut screen = screen_with_history(&[]);
    screen.pending = 2;
    screen.receive("balasan satu\n");
    screen.receive(&format!("balasan dua\n{}", char::from(TURN_MARK)));
    assert_eq!(screen.pending, 1, "one mark arrived");
    assert!(!screen
        .transcript
        .visible(10, 0)
        .join("")
        .contains(char::from(TURN_MARK)));
    assert_eq!(
        screen.transcript.visible(10, 0),
        vec!["balasan satu", "balasan dua"]
    );
    screen.receive(&char::from(TURN_MARK).to_string());
    assert_eq!(screen.pending, 0);
    screen.receive(&char::from(TURN_MARK).to_string());
    assert_eq!(screen.pending, 0, "never below zero");
}

#[test]
fn status_shows_a_busy_indicator_while_a_reply_is_pending() {
    let mut screen = screen_with_history(&[]);
    assert!(!screen.status_text().contains("working"));
    screen.pending = 1;
    assert!(screen.status_text().contains("working"));
}

#[test]
fn up_and_down_recall_sent_lines_and_restore_the_draft() {
    let mut screen = screen_with_history(&["satu", "dua", "tiga"]);
    screen.input.set("sedang ketik");

    screen.recall_older();
    assert_eq!(screen.input.text(), "tiga");
    screen.recall_older();
    assert_eq!(screen.input.text(), "dua");
    screen.recall_older();
    screen.recall_older();
    assert_eq!(screen.input.text(), "satu", "stops at the oldest line");

    screen.recall_newer();
    assert_eq!(screen.input.text(), "dua");
    screen.recall_newer();
    screen.recall_newer();
    assert_eq!(
        screen.input.text(),
        "sedang ketik",
        "past the newest, the draft comes back"
    );
    assert_eq!(screen.history_pos, None);
}

#[test]
fn recall_does_nothing_without_history() {
    let mut screen = screen_with_history(&[]);
    screen.input.set("halo");
    screen.recall_older();
    assert_eq!(screen.input.text(), "halo");
}

#[test]
fn status_names_the_provider() {
    let screen = screen_with_history(&[]);
    assert!(screen.status_text().starts_with("[test] "));
}
