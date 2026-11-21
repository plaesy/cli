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
#[path = "banner_test.rs"]
mod banner_test;
