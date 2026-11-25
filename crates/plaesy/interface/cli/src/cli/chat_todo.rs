//! `/todo`: a short checklist for the chat session. It lives in memory and ends with the session,
//! like `/goal`.
//!
//! `/todo` lists the items; `/todo add <text>` appends one; `/todo done <n>` and `/todo undone <n>`
//! toggle item `n` (1-based); `/todo rm <n>` removes it; `/todo clear` empties the list.

/// One checklist item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub text: String,
    pub done: bool,
}

/// The session checklist.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Todos {
    items: Vec<Item>,
}

impl Todos {
    /// Applies one `/todo` argument string and returns the text to show.
    pub fn apply(&mut self, arg: &str) -> String {
        let arg = arg.trim();
        let (verb, rest) = arg
            .split_once(' ')
            .map_or((arg, ""), |(v, r)| (v, r.trim()));
        match verb {
            "" => self.render(),
            "add" if !rest.is_empty() => {
                self.items.push(Item {
                    text: rest.to_owned(),
                    done: false,
                });
                self.render()
            }
            "done" => self.set(rest, true),
            "undone" => self.set(rest, false),
            "rm" => match self.index(rest) {
                Some(i) => {
                    let item = self.items.remove(i);
                    format!("dihapus: {}\n{}", item.text, self.render())
                }
                None => format!("nomor tidak dikenal: {rest}"),
            },
            "clear" => {
                self.items.clear();
                "daftar dikosongkan".to_owned()
            }
            _ => "pakai: /todo | /todo add <teks> | /todo done <n> | /todo undone <n> | /todo rm <n> | /todo clear"
                .to_owned(),
        }
    }

    /// Items as numbered lines, with `[x]` for done ones. Empty when there are none.
    #[must_use]
    pub fn render(&self) -> String {
        if self.items.is_empty() {
            return "daftar kosong".to_owned();
        }
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let mark = if item.done { "x" } else { " " };
                format!("{}. [{mark}] {}", i + 1, item.text)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Number of items, for the status line and tests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when the checklist has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn set(&mut self, rest: &str, done: bool) -> String {
        match self.index(rest) {
            Some(i) => {
                self.items[i].done = done;
                self.render()
            }
            None => format!("nomor tidak dikenal: {rest}"),
        }
    }

    fn index(&self, rest: &str) -> Option<usize> {
        let n: usize = rest.trim().parse().ok()?;
        (1..=self.items.len()).contains(&n).then(|| n - 1)
    }
}

#[cfg(test)]
#[path = "chat_todo_test.rs"]
mod chat_todo_test;
