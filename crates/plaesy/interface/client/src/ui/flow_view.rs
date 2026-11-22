//! Tampilan HTML read-only untuk satu flow (default web, delegasi owner: dapat dicabut).
//!
//! Renderer ini pure Rust: tanpa framework dan tanpa I/O. Semua teks dari dokumen di-escape,
//! sehingga nilai berbahaya tidak bisa menyisipkan markup.

use crate::features::flow::model::{edges, validate, LocalIssue, ModuleView};

/// Mengubah teks menjadi aman untuk konten dan atribut HTML.
#[must_use]
pub fn escape(text: &str) -> String {
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

/// Halaman HTML lengkap untuk daftar modul flow, edge, dan masalah validasi lokal.
#[must_use]
pub fn render_flow(name: &str, modules: &[ModuleView]) -> String {
    let mut body = String::new();
    body.push_str(&format!("<h1>{}</h1>\n", escape(name)));

    body.push_str("<section><h2>Modul</h2>\n<ul>\n");
    for module in modules {
        body.push_str(&format!(
            "<li><strong>{}</strong> <code>{}</code>",
            escape(&module.id),
            escape(&module.kind)
        ));
        if !module.inputs.is_empty() {
            body.push_str("<ul>");
            for (key, source) in &module.inputs {
                body.push_str(&format!(
                    "<li>{} = <code>{}</code></li>",
                    escape(key),
                    escape(source)
                ));
            }
            body.push_str("</ul>");
        }
        body.push_str("</li>\n");
    }
    body.push_str("</ul></section>\n");

    body.push_str("<section><h2>Alur data</h2>\n<ul>\n");
    for edge in edges(modules) {
        body.push_str(&format!(
            "<li>{} &rarr; {}</li>\n",
            escape(&edge.from),
            escape(&edge.to)
        ));
    }
    body.push_str("</ul></section>\n");

    let issues = validate(modules);
    body.push_str("<section><h2>Masalah</h2>\n");
    if issues.is_empty() {
        body.push_str("<p>Tidak ada masalah lokal.</p>\n");
    } else {
        body.push_str("<ul>\n");
        for issue in &issues {
            body.push_str(&format!("<li>{}</li>\n", escape(&describe(issue))));
        }
        body.push_str("</ul>\n");
    }
    body.push_str("</section>\n");

    format!(
        "<!doctype html>\n<html lang=\"id\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n</head>\n<body>\n{}</body>\n</html>\n",
        escape(name),
        body
    )
}

fn describe(issue: &LocalIssue) -> String {
    match issue {
        LocalIssue::EmptyId => "modul tanpa id".to_owned(),
        LocalIssue::DuplicateId(id) => format!("id modul ganda: {id}"),
        LocalIssue::UnknownSource { module, source } => {
            format!("modul {module} merujuk ke sumber tidak dikenal: {source}")
        }
    }
}

#[cfg(test)]
#[path = "flow_view_test.rs"]
mod flow_view_test;
