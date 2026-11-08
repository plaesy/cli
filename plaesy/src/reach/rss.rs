//! RSS 2.0 and Atom parsing, and the built-in feed fetch. The Python feedparser backend is a
//! command line built here.

use std::time::Duration;

use chrono::Utc;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::exec::{argv, Deadline};
use super::types::{BackendResult, Platform};

const USER_AGENT: &str = "Mozilla/5.0 (compatible; plaesy-reach/1.0)";
const MAX_ITEMS: usize = 20;
const MAX_SUMMARY_CHARS: usize = 200;

const FEEDPARSER_SCRIPT: &str = r#"
import feedparser, json, sys
feed = feedparser.parse(sys.argv[1])
result = {
    "title": feed.feed.get("title", ""),
    "description": feed.feed.get("description", ""),
    "entries": []
}
for entry in feed.entries[:20]:
    result["entries"].append({
        "title": entry.get("title", ""),
        "link": entry.get("link", ""),
        "published": entry.get("published", ""),
        "summary": entry.get("summary", ""),
    })
print(json.dumps(result))
"#;

/// A feed reduced to what the report shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedFeed {
    pub title: String,
    pub description: String,
    pub items: Vec<ParsedItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub published: String,
}

/// The `python3` command that runs feedparser on `url`.
#[must_use]
pub fn feedparser_command(url: &str) -> Vec<String> {
    let mut args = argv(&["python3", "-c", FEEDPARSER_SCRIPT]);
    args.push(url.to_owned());
    args
}

/// Fetches `url` and renders it as text: the title, then up to twenty items.
#[must_use]
pub fn fetch_builtin(url: &str, deadline: Deadline) -> BackendResult {
    let mut result = BackendResult::new("builtin", Platform::Rss);
    let body = match http_get(url, deadline) {
        Ok(body) => body,
        Err(e) => {
            result.error = e;
            return result;
        }
    };
    let feed = match parse_rss2(&body) {
        Ok(feed) => feed,
        Err(_) => match parse_atom(&body) {
            Ok(feed) => feed,
            Err(e) => {
                result.error = format!("failed to parse feed: {e}");
                return result;
            }
        },
    };
    result.success = true;
    result.data = render(&feed);
    result.timestamp = Utc::now();
    result
}

fn render(feed: &ParsedFeed) -> String {
    let mut out = format!(
        "Title: {}\nDescription: {}\n\n",
        feed.title, feed.description
    );
    for (i, item) in feed.items.iter().take(MAX_ITEMS).enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, item.title));
        out.push_str(&format!("   Link: {}\n", item.link));
        if !item.published.is_empty() {
            out.push_str(&format!("   Published: {}\n", item.published));
        }
        if !item.description.is_empty() {
            out.push_str(&format!(
                "   Summary: {}\n",
                truncate_chars(&item.description, MAX_SUMMARY_CHARS)
            ));
        }
        out.push('\n');
    }
    out
}

/// The first `max` characters, with `...` added when cut. Counts characters, not bytes, so a
/// multi-byte character is never split.
fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}...", &text[..cut]),
        None => text.to_owned(),
    }
}

fn http_get(url: &str, deadline: Deadline) -> Result<Vec<u8>, String> {
    let timeout = Duration::from_secs(30).min(deadline.remaining().ok_or("deadline exceeded")?);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    response.body_mut().read_to_vec().map_err(|e| e.to_string())
}

/// A parsed XML element. Only what the feed readers need is kept.
#[derive(Debug, Default, Clone)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn child_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The text of the last child called `name`. A repeated element overwrites, as in Go's decoder.
    fn last_text(&self, name: &str) -> String {
        self.child_named(name)
            .last()
            .map(|n| n.text.trim().to_owned())
            .unwrap_or_default()
    }

    fn attr(&self, name: &str) -> String {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
}

fn node_from(e: &BytesStart<'_>) -> Result<Node, String> {
    let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map_err(|e| e.to_string())?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Node {
        name,
        attrs,
        ..Node::default()
    })
}

/// Builds the element tree. The result is the document's root element.
fn parse_dom(data: &[u8]) -> Result<Node, String> {
    let mut reader = Reader::from_reader(data);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut buf = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            Event::Start(e) => stack.push(node_from(&e)?),
            Event::Empty(e) => {
                let node = node_from(&e)?;
                if let Some(top) = stack.last_mut() {
                    top.children.push(node);
                }
            }
            Event::End(_) => {
                if stack.len() > 1 {
                    let node = stack.pop().unwrap_or_default();
                    if let Some(top) = stack.last_mut() {
                        top.children.push(node);
                    }
                }
            }
            Event::Text(t) => {
                let text = t.unescape().map_err(|e| e.to_string())?;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&text);
                }
            }
            Event::CData(c) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&c));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    let mut root = stack.pop().unwrap_or_default();
    if root.children.is_empty() {
        return Err("no root element".to_owned());
    }
    Ok(root.children.swap_remove(0))
}

/// RSS 2.0: `channel` holds `title`, `description`, and `item` elements.
///
/// # Errors
///
/// The document is not XML, or it is not an RSS feed.
pub fn parse_rss2(data: &[u8]) -> Result<ParsedFeed, String> {
    let root = parse_dom(data)?;
    let channel = root
        .child_named("channel")
        .next()
        .cloned()
        .unwrap_or_default();
    let feed = ParsedFeed {
        title: channel.last_text("title"),
        description: channel.last_text("description"),
        items: channel
            .child_named("item")
            .map(|item| ParsedItem {
                title: item.last_text("title"),
                link: item.last_text("link"),
                description: item.last_text("description"),
                published: item.last_text("pubDate"),
            })
            .collect(),
    };
    if feed.title.is_empty() && feed.items.is_empty() {
        return Err("not a valid RSS 2.0 feed".to_owned());
    }
    Ok(feed)
}

/// Atom: a `feed` root with `title` and `entry` elements.
///
/// # Errors
///
/// The document is not XML, the root is not `feed`, or it holds no title or entries.
pub fn parse_atom(data: &[u8]) -> Result<ParsedFeed, String> {
    let root = parse_dom(data)?;
    if root.name != "feed" {
        return Err(format!(
            "expected element type <feed> but have <{}>",
            root.name
        ));
    }
    let feed = ParsedFeed {
        title: root.last_text("title"),
        description: String::new(),
        items: root
            .child_named("entry")
            .map(|entry| {
                let link = entry
                    .child_named("link")
                    .last()
                    .map(|l| l.attr("href"))
                    .unwrap_or_default();
                let summary = entry.last_text("summary");
                ParsedItem {
                    title: entry.last_text("title"),
                    link,
                    description: if summary.is_empty() {
                        entry.last_text("content")
                    } else {
                        summary
                    },
                    published: entry.last_text("updated"),
                }
            })
            .collect(),
    };
    if feed.title.is_empty() && feed.items.is_empty() {
        return Err("not a valid Atom feed".to_owned());
    }
    Ok(feed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel>
  <title>Example</title><description>An example feed</description>
  <item><title>First</title><link>https://example.com/1</link><pubDate>Mon</pubDate></item>
</channel></rss>"#;

    const ATOM: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Example</title>
  <entry><title>Entry</title><link href="https://example.com/a" rel="alternate"/><updated>2026-01-01</updated></entry>
</feed>"#;

    #[test]
    fn rss_items_are_read() {
        let feed = parse_rss2(RSS.as_bytes()).expect("rss");
        assert_eq!(feed.title, "Example");
        assert_eq!(feed.items.len(), 1);
        assert_eq!(feed.items[0].link, "https://example.com/1");
        assert_eq!(feed.items[0].published, "Mon");
    }

    #[test]
    fn atom_entries_take_their_link_from_href() {
        let feed = parse_atom(ATOM.as_bytes()).expect("atom");
        assert_eq!(feed.items[0].link, "https://example.com/a");
        assert_eq!(feed.items[0].published, "2026-01-01");
    }

    #[test]
    fn an_rss_parser_rejects_atom() {
        assert!(parse_rss2(ATOM.as_bytes()).is_err());
    }

    #[test]
    fn long_summaries_are_cut_on_a_character_boundary() {
        let text = "é".repeat(250);
        let cut = truncate_chars(&text, MAX_SUMMARY_CHARS);
        assert!(cut.ends_with("..."));
        assert_eq!(cut.chars().count(), MAX_SUMMARY_CHARS + 3);
    }
}
