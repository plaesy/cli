//! Full-screen terminal interface for `plaesy chat --tui`. The chat itself is unchanged: the same
//! `converse` loop runs on a worker thread and reads lines from a channel, and its output goes to a
//! channel the screen draws from. So every command, hook, and provider works the same in both modes.
//!
//! Layout: the conversation on top, the input line below, and a status line at the bottom. Enter
//! sends, PageUp and PageDown scroll the conversation, and Esc or Ctrl-C ends the session after the
//! current reply. The session also ends when the chat itself exits (`/exit`).

use std::io::{self, BufReader, Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::chat::TURN_MARK;
use super::chat_text::Lang;

/// Most lines kept in the conversation view. Older lines are dropped.
pub const MAX_LINES: usize = 2000;
/// How far one PageUp or PageDown moves.
const PAGE: usize = 10;
/// Most commands listed at once under the input.
const MAX_SHOWN: usize = 8;

/// Lines typed by the person, read by the chat as if they came from stdin. Reading blocks until a
/// line arrives, and reports end of input once the sender is gone.
pub struct LineSource {
    rx: Receiver<String>,
    pending: Vec<u8>,
    pos: usize,
}

impl LineSource {
    pub fn new(rx: Receiver<String>) -> Self {
        Self {
            rx,
            pending: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for LineSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.pending.len() {
            match self.rx.recv() {
                Ok(line) => {
                    self.pending = format!("{line}\n").into_bytes();
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len() - self.pos);
        buf[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Output of the chat, sent to the screen as text chunks.
pub struct OutputSink {
    tx: Sender<String>,
}

impl Write for OutputSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // A closed screen channel only means nobody is drawing any more, so output is dropped.
        let _ = self.tx.send(String::from_utf8_lossy(buf).into_owned());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The conversation as lines. Text without a newline waits in `partial` until its line ends.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    lines: Vec<String>,
    partial: String,
}

impl Transcript {
    pub fn push_text(&mut self, text: &str) {
        self.partial.push_str(text);
        while let Some(index) = self.partial.find('\n') {
            let line = self.partial[..index].trim_end_matches('\r').to_owned();
            self.lines.push(line);
            self.partial.drain(..=index);
        }
        if self.lines.len() > MAX_LINES {
            let extra = self.lines.len() - MAX_LINES;
            self.lines.drain(..extra);
        }
    }

    pub fn push_user(&mut self, text: &str) {
        self.push_text(&format!("\n› {text}\n"));
    }

    /// The lines that fit in `height`, ending `scroll` lines above the newest.
    pub fn visible(&self, height: usize, scroll: usize) -> Vec<String> {
        let mut all: Vec<String> = self.lines.clone();
        if !self.partial.is_empty() {
            all.push(self.partial.clone());
        }
        let end = all.len().saturating_sub(scroll);
        let start = end.saturating_sub(height);
        all[start..end].to_vec()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.partial.is_empty()
    }
}

/// The line being typed, with a cursor counted in characters.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Input {
    chars: Vec<char>,
    cursor: usize,
}

impl Input {
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.chars.len();
    }

    /// Replaces the whole line and puts the cursor at its end.
    pub fn set(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
    }

    /// Returns the line and clears the input.
    pub fn take(&mut self) -> String {
        self.cursor = 0;
        self.chars.drain(..).collect()
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Insert(char),
    Backspace,
    Left,
    Right,
    Home,
    End,
    Submit,
    ScrollUp,
    ScrollDown,
    /// Up or Down: moves the highlight when the command list is open.
    Up,
    Down,
    /// Tab: completes the highlighted command without sending.
    Complete,
    /// Esc: closes the command list if it is open, otherwise ends the session.
    Cancel,
    Quit,
    Ignore,
}

/// Maps a key press to what the screen does with it.
pub fn action(key: &KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if ctrl => Action::Quit,
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Submit,
        KeyCode::Tab => Action::Complete,
        KeyCode::Up => Action::Up,
        KeyCode::Down => Action::Down,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Left => Action::Left,
        KeyCode::Right => Action::Right,
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::PageUp => Action::ScrollUp,
        KeyCode::PageDown => Action::ScrollDown,
        KeyCode::Char(c) if !ctrl => Action::Insert(c),
        _ => Action::Ignore,
    }
}

/// The chat commands offered when the person types `/`: name, English note, Indonesian note.
pub const COMMANDS: &[(&str, &str, &str)] = &[
    ("/exit", "end the chat", "akhiri chat"),
    ("/clear", "reset the history", "hapus riwayat"),
    ("/provider", "switch provider", "ganti provider"),
    ("/providers", "list providers", "daftar provider"),
    ("/role", "switch role", "ganti role"),
    ("/decision", "save a decision", "simpan keputusan"),
    ("/decisions", "list decisions", "daftar keputusan"),
    ("/ask", "ask a question", "ajukan pertanyaan"),
    ("/pending", "open questions", "pertanyaan terbuka"),
    ("/pick", "answer a question", "jawab pertanyaan"),
    ("/tasks", "task menu", "menu tugas"),
    ("/mcp", "MCP servers", "server MCP"),
    ("/plugins", "list plugins", "daftar plugin"),
    ("/plugin", "run a plugin", "jalankan plugin"),
    ("/agent", "messages to agents", "pesan ke agen"),
    ("/goal", "set the session goal", "atur goal sesi"),
    ("/compact", "summarize the history", "ringkas riwayat"),
    ("/advisor", "ask the advisor", "minta review advisor"),
    ("/council", "ask every provider", "tanya semua provider"),
    (
        "/lsp",
        "definition, references, hover",
        "definisi, referensi, hover",
    ),
    ("/debug", "session details", "rincian sesi"),
];

/// Commands that start with the typed text. Only a single word typed after `/` opens the list.
pub fn suggestions(input: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    if !input.starts_with('/') || input.contains(char::is_whitespace) {
        return Vec::new();
    }
    COMMANDS
        .iter()
        .filter(|(name, _, _)| name.starts_with(input))
        .copied()
        .collect()
}

/// The text Enter or Tab puts in the input when a command is highlighted: the command and a space,
/// so the person can add arguments. `None` when the input already is that command, so Enter sends.
pub fn completion(input: &str, selected: usize) -> Option<String> {
    let matches = suggestions(input);
    let (name, _, _) = matches.get(selected)?;
    if *name == input {
        None
    } else {
        Some(format!("{name} "))
    }
}

/// Everything the screen shows and remembers between key presses.
struct Screen {
    lang: Lang,
    repo: String,
    transcript: Transcript,
    input: Input,
    scroll: usize,
    quitting: bool,
    finished: bool,
    /// Which suggestion is highlighted.
    selected: usize,
    /// Set by Esc; cleared when the input changes, so the list comes back on the next key.
    popup_hidden: bool,
    /// Lines sent before, oldest first, recalled with Up and Down.
    history: Vec<String>,
    /// Position in `history` while recalling; `None` while typing a new line.
    history_pos: Option<usize>,
    /// The line being typed when recall began; Down past the newest entry restores it.
    draft: String,
    /// The provider in use, shown in the status line.
    provider: String,
    /// Lines sent whose turn-end mark has not arrived yet; above zero, a reply is still coming.
    pending: usize,
    /// When the oldest pending line was sent; shown as elapsed time while a reply is coming.
    sent_at: Option<Instant>,
    /// Advances each frame, to turn the busy indicator.
    tick: usize,
}

/// Elapsed time as `5s`, or `1m05s` from a minute on.
fn clock(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

/// Frames of the busy indicator, in ASCII so every terminal shows them.
const SPINNER: [&str; 4] = ["|", "/", "-", "\\"];

impl Screen {
    /// Adds chat output to the view. Each turn-end mark closes one pending line and is not shown.
    fn receive(&mut self, text: &str) {
        for (index, part) in text.split(char::from(TURN_MARK)).enumerate() {
            if index > 0 {
                self.pending = self.pending.saturating_sub(1);
                if self.pending == 0 {
                    self.sent_at = None;
                }
            }
            if !part.is_empty() {
                self.transcript.push_text(part);
            }
        }
    }

    /// Up: the previous line sent. The first Up keeps the line being typed as the draft.
    fn recall_older(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let index = match self.history_pos {
            None => {
                self.draft = self.input.text();
                self.history.len() - 1
            }
            Some(0) => 0,
            Some(index) => index - 1,
        };
        self.history_pos = Some(index);
        self.input.set(&self.history[index]);
    }

    /// Down: the next line sent, or the draft once past the newest one.
    fn recall_newer(&mut self) {
        match self.history_pos {
            None => {}
            Some(index) if index + 1 < self.history.len() => {
                self.history_pos = Some(index + 1);
                self.input.set(&self.history[index + 1]);
            }
            Some(_) => {
                self.history_pos = None;
                let draft = std::mem::take(&mut self.draft);
                self.input.set(&draft);
            }
        }
    }

    /// The commands to show now, or none when the list is hidden.
    fn open_suggestions(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        if self.popup_hidden {
            return Vec::new();
        }
        suggestions(&self.input.text())
    }

    fn draw(&self, frame: &mut Frame<'_>) {
        let [log, entry, status] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        self.draw_log(frame, log);
        self.draw_input(frame, entry);
        self.draw_suggestions(frame, entry);
        frame.render_widget(Paragraph::new(self.status_text()), status);
    }

    /// The command list, just above the input line and over the conversation.
    fn draw_suggestions(&self, frame: &mut Frame<'_>, entry: Rect) {
        let matches = self.open_suggestions();
        if matches.is_empty() {
            return;
        }
        let shown = matches.len().min(MAX_SHOWN);
        let height = shown as u16 + 2;
        let width = entry.width.min(64);
        let top = entry.y.saturating_sub(height);
        let area = Rect::new(entry.x, top, width, height);
        let lines: Vec<Line<'_>> = matches
            .iter()
            .take(shown)
            .enumerate()
            .map(|(index, (name, en, id))| {
                let note = match self.lang {
                    Lang::En => en,
                    Lang::Id => id,
                };
                let text = format!("{name:<12} {note}");
                if index == self.selected.min(shown - 1) {
                    Line::from(text).style(Style::new().reversed())
                } else {
                    Line::from(text)
                }
            })
            .collect();
        let title = match self.lang {
            Lang::En => " commands ",
            Lang::Id => " perintah ",
        };
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(Text::from(lines)).block(Block::bordered().title(title)),
            area,
        );
    }

    fn draw_log(&self, frame: &mut Frame<'_>, area: Rect) {
        let height = area.height.saturating_sub(2) as usize;
        let lines: Vec<Line<'_>> = self
            .transcript
            .visible(height, self.scroll)
            .into_iter()
            .map(Line::from)
            .collect();
        let title = format!(" plaesy chat · {} ", self.repo);
        let log = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(title));
        frame.render_widget(log, area);
    }

    fn draw_input(&self, frame: &mut Frame<'_>, area: Rect) {
        let prompt = match self.lang {
            Lang::En => " message ",
            Lang::Id => " pesan ",
        };
        let entry = Paragraph::new(self.input.text()).block(Block::bordered().title(prompt));
        frame.render_widget(entry, area);
        // Place the cursor inside the box, after the typed characters.
        let max_x = area.x + area.width.saturating_sub(2);
        let x = (area.x + 1 + self.input.cursor as u16).min(max_x);
        frame.set_cursor_position((x, area.y + 1));
    }

    fn status_text(&self) -> String {
        if self.pending > 0 {
            let frame = SPINNER[self.tick % SPINNER.len()];
            let label = match self.lang {
                Lang::En => "working…",
                Lang::Id => "sedang menjawab…",
            };
            let timing = self
                .sent_at
                .map(|at| format!(" {} ·", clock(at.elapsed())))
                .unwrap_or_else(|| " ·".to_owned());
            return format!(
                "[{}] {frame} {label}{timing} {}",
                self.provider,
                self.hints()
            );
        }
        format!("[{}] {}", self.provider, self.hints())
    }

    fn hints(&self) -> String {
        match (self.lang, self.finished, self.quitting) {
            (Lang::En, true, _) => "session ended".to_owned(),
            (Lang::Id, true, _) => "sesi selesai".to_owned(),
            (Lang::En, false, true) => "closing after the current reply…".to_owned(),
            (Lang::Id, false, true) => "menutup setelah balasan selesai…".to_owned(),
            (Lang::En, false, false) => {
                "Enter send · PgUp/PgDn scroll · Esc quit · /exit ends the chat".to_owned()
            }
            (Lang::Id, false, false) => {
                "Enter kirim · PgUp/PgDn gulir · Esc keluar · /exit mengakhiri chat".to_owned()
            }
        }
    }
}

/// Runs `session` on a worker thread and shows it in the terminal until it ends. `session` reads
/// the typed lines and writes the chat's output, exactly as `converse` does with stdin and stdout.
pub fn run<F>(lang: Lang, repo: &str, provider: &str, session: F) -> io::Result<()>
where
    F: FnOnce(BufReader<LineSource>, &mut OutputSink) + Send,
{
    let (line_tx, line_rx) = mpsc::channel::<String>();
    let (out_tx, out_rx) = mpsc::channel::<String>();
    let mut terminal = ratatui::init();
    let mut screen = Screen {
        lang,
        repo: repo.to_owned(),
        transcript: Transcript::default(),
        input: Input::default(),
        scroll: 0,
        quitting: false,
        finished: false,
        selected: 0,
        popup_hidden: false,
        history: Vec::new(),
        history_pos: None,
        draft: String::new(),
        provider: provider.to_owned(),
        pending: 0,
        sent_at: None,
        tick: 0,
    };
    let mut line_tx = Some(line_tx);
    let outcome = thread::scope(|scope| {
        scope.spawn(move || {
            let mut sink = OutputSink { tx: out_tx };
            session(BufReader::new(LineSource::new(line_rx)), &mut sink);
        });
        let outcome = loop {
            screen.tick = screen.tick.wrapping_add(1);
            // Drain everything the chat printed since the last frame.
            loop {
                match out_rx.try_recv() {
                    Ok(text) => screen.receive(&text),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        screen.finished = true;
                        break;
                    }
                }
            }
            if screen.finished {
                let _ = terminal.draw(|frame| screen.draw(frame));
                break Ok(());
            }
            if let Err(err) = terminal.draw(|frame| screen.draw(frame)) {
                break Err(err);
            }
            if let Err(err) = poll_key(&mut screen, &mut line_tx) {
                break Err(err);
            }
        };
        // Closing the line channel lets a waiting session see end of input, so the scope can end.
        line_tx = None;
        outcome
    });
    ratatui::restore();
    outcome
}

/// Waits briefly for one key press and applies it.
fn poll_key(screen: &mut Screen, line_tx: &mut Option<Sender<String>>) -> io::Result<()> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(());
    }
    let Event::Key(key) = event::read()? else {
        return Ok(());
    };
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }
    let open = screen.open_suggestions();
    match action(&key) {
        Action::Insert(c) => {
            screen.input.insert(c);
            screen.selected = 0;
            screen.popup_hidden = false;
            screen.history_pos = None;
        }
        Action::Backspace => {
            screen.input.backspace();
            screen.selected = 0;
            screen.popup_hidden = false;
            screen.history_pos = None;
        }
        Action::Left => screen.input.left(),
        Action::Right => screen.input.right(),
        Action::Home => screen.input.home(),
        Action::End => screen.input.end(),
        Action::ScrollUp => screen.scroll = screen.scroll.saturating_add(PAGE),
        Action::ScrollDown => screen.scroll = screen.scroll.saturating_sub(PAGE),
        Action::Up if !open.is_empty() => {
            screen.selected = (screen.selected + open.len() - 1) % open.len();
        }
        Action::Down if !open.is_empty() => {
            screen.selected = (screen.selected + 1) % open.len();
        }
        Action::Up => screen.recall_older(),
        Action::Down => screen.recall_newer(),
        Action::Complete => {
            if !open.is_empty() {
                if let Some(text) = completion(&screen.input.text(), screen.selected) {
                    screen.input.set(&text);
                }
            }
        }
        Action::Submit => {
            // With the list open, Enter first completes the highlighted command; a second Enter sends.
            if !open.is_empty() {
                if let Some(text) = completion(&screen.input.text(), screen.selected) {
                    screen.input.set(&text);
                    return Ok(());
                }
            }
            let line = screen.input.take();
            screen.scroll = 0;
            screen.selected = 0;
            screen.history_pos = None;
            screen.draft.clear();
            if !line.trim().is_empty() {
                screen.history.push(line.clone());
            }
            if let Some(tx) = line_tx.as_ref() {
                screen.transcript.push_user(&line);
                // A send fails only when the session has already ended; the frame loop notices that.
                let _ = tx.send(line);
                screen.pending += 1;
                screen.sent_at.get_or_insert_with(Instant::now);
            }
        }
        Action::Cancel => {
            if open.is_empty() {
                screen.quitting = true;
                *line_tx = None;
            } else {
                screen.popup_hidden = true;
            }
        }
        Action::Quit => {
            screen.quitting = true;
            *line_tx = None;
        }
        Action::Ignore => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "chat_tui_test.rs"]
mod chat_tui_test;
