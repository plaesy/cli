//! Terminal banner.

use std::io::{self, Write};

const PURPLE: &str = "\x1b[0;35m";
const RESET: &str = "\x1b[0m";

const ART: [&str; 6] = [
    "██████╗ ██╗      █████╗ ███████╗███████╗██╗   ██╗",
    "██╔══██╗██║     ██╔══██╗██╔════╝██╔════╝╝██╗ ██╔╝",
    "██████╔╝██║     ███████║█████╗  ███████╗ ╚████╔╝ ",
    "██╔═══╝ ██║     ██╔══██║██╔══╝  ╚════██║  ╚██╔╝  ",
    "██║     ███████╗██║  ██║███████╗███████║   ██║   ",
    "╚═╝     ╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ",
];

/// Writes the banner with an optional title and subtitle to `out`.
///
/// # Errors
///
/// Returns any I/O error from `out`.
pub fn write_banner(out: &mut impl Write, title: &str, subtitle: &str) -> io::Result<()> {
    writeln!(out, "{PURPLE}")?;
    for line in ART {
        writeln!(out, "{line}")?;
    }
    writeln!(out)?;
    if !title.is_empty() {
        writeln!(out, "🏛️  {title}")?;
    }
    if !subtitle.is_empty() {
        writeln!(out, "   {subtitle}")?;
    }
    write!(out, "{RESET}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_title_and_subtitle_when_given() {
        let mut buf = Vec::new();
        write_banner(&mut buf, "Reload", "refresh .plaesy").unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("Reload"));
        assert!(text.contains("refresh .plaesy"));
        assert!(text.ends_with(RESET));
    }
}
