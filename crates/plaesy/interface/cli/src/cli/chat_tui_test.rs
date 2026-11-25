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
