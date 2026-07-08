//! The OSlings cheatsheet: a single-page reference of every bit layout, magic
//! number, and concept the course covers. The content lives in `cheatsheet.md`
//! (GitHub-flavored markdown) and is baked into the binary at compile time, so
//! both the `cheatsheet` subcommand and the TUI's Cheatsheet page render the
//! same source.

/// The cheatsheet as markdown.
pub fn markdown() -> &'static str {
    include_str!("cheatsheet.md")
}
