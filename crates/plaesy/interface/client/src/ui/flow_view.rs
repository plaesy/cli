//! HTML markup for the flow editor's module and flow lists.
//!
//! Framework-free string rendering. It is a stand-in until the Dioxus editor lands, so it does not
//! define that editor's components. The accessibility contract is kept here: each list has a role
//! and an `aria-label`, and each status is shown as text (`Berhasil`, `Gagal`, `Draf`), not only as
//! a color class.

/// Status of a module or flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Failed,
    Draft,
}

impl Status {
    /// Visible, screen-reader-readable label in Indonesian.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "Berhasil",
            Self::Failed => "Gagal",
            Self::Draft => "Draf",
        }
    }

    /// CSS class for the color cue. It supplements the label and never replaces it.
    fn css_class(self) -> &'static str {
        match self {
            Self::Ok => "status-ok",
            Self::Failed => "status-failed",
            Self::Draft => "status-draft",
        }
    }
}

/// One named module or flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub status: Status,
}

/// The data shown by the flow view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowView {
    pub modules: Vec<Entry>,
    pub flows: Vec<Entry>,
}

/// Renders the module and flow lists as HTML.
#[must_use]
pub fn render(view: &FlowView) -> String {
    let mut html = String::from("<div class=\"flow-view\">\n");
    push_list(&mut html, "Daftar modul", "modul", &view.modules);
    push_list(&mut html, "Daftar alur", "alur", &view.flows);
    html.push_str("</div>\n");
    html
}

/// Escapes the five HTML-special characters for use in text and quoted attribute values.
#[must_use]
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn push_list(html: &mut String, label: &str, noun: &str, items: &[Entry]) {
    let label = escape_html(label);
    if items.is_empty() {
        html.push_str(&format!("<p class=\"flow-empty\">Tidak ada {noun}.</p>\n"));
        return;
    }
    html.push_str(&format!("<ul role=\"list\" aria-label=\"{label}\" class=\"flow-list\">\n"));
    for item in items {
        html.push_str(&format!(
            "  <li role=\"listitem\" class=\"flow-item {}\"><span class=\"flow-name\">{}</span> <span class=\"flow-status\">Status: {}</span></li>\n",
            item.status.css_class(),
            escape_html(&item.name),
            item.status.label(),
        ));
    }
    html.push_str("</ul>\n");
}

#[cfg(test)]
#[path = "flow_view_test.rs"]
mod flow_view_test;
