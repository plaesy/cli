//! Server-Sent Events framing (pure logic, no I/O).
//!
//! Turns one event into the wire format `event:` / `data:` / blank line. Multi-line data
//! is split into one `data:` line per line, and CR/LF inside a field name or value is
//! rejected so a caller cannot inject a second event into the stream.

/// One event ready to be framed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Event name, sent as `event:`. Must not be empty.
    pub name: String,
    /// Payload, sent as one `data:` line per line.
    pub data: String,
}

/// Framing errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SseError {
    /// The event name is empty.
    #[error("sse event name must not be empty")]
    EmptyName,
    /// The event name contains a line break, which would inject a second field.
    #[error("sse event name must not contain a line break")]
    NameLineBreak,
}

/// Frames one event as the SSE wire format, ending with the blank line that terminates it.
///
/// # Errors
///
/// Returns [`SseError::EmptyName`] or [`SseError::NameLineBreak`] for an invalid name.
pub fn frame(event: &Event) -> Result<String, SseError> {
    if event.name.is_empty() {
        return Err(SseError::EmptyName);
    }
    if event.name.contains(['\r', '\n']) {
        return Err(SseError::NameLineBreak);
    }
    let mut out = format!("event: {}\n", event.name);
    for line in event.data.split('\n') {
        out.push_str("data: ");
        out.push_str(line.trim_end_matches('\r'));
        out.push('\n');
    }
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
#[path = "sse_test.rs"]
mod sse_test;
