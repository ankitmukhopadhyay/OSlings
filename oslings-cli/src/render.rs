//! Terminal presentation: markdown lessons, hints, progress, and result
//! banners. Uses `termimad` for styled markdown.

use crate::model::{Project, State};
use termimad::crossterm::style::Color;
use termimad::MadSkin;

// A few raw ANSI helpers for banners (termimad is for the markdown body).
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[2m";

/// A skin tuned for reading code-heavy lessons in a terminal.
pub fn skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin.set_headers_fg(Color::AnsiValue(45)); // cyan-ish headers
    skin.bold.set_fg(Color::Yellow);
    skin.italic.set_fg(Color::Magenta);
    skin.inline_code.set_fg(Color::Green);
    skin.code_block.set_fg(Color::AnsiValue(252));
    skin
}

pub fn markdown(md: &str) {
    skin().print_text(md);
}

pub fn pass_banner(name: &str) {
    println!(
        "\n{GREEN}{BOLD}  ✓ PASSED{RESET}  {BOLD}{name}{RESET}\n"
    );
}

pub fn fail_banner(name: &str, summary: &str) {
    println!(
        "\n{RED}{BOLD}  ✗ not yet{RESET}  {BOLD}{name}{RESET}\n    {summary}\n"
    );
}

pub fn info(msg: &str) {
    println!("{CYAN}{msg}{RESET}");
}

pub fn note(msg: &str) {
    println!("{DIM}{msg}{RESET}");
}

/// Render compiler / QEMU output in a dimmed, indented block.
pub fn detail_block(detail: &str) {
    let trimmed = detail.trim_end();
    if trimmed.is_empty() {
        return;
    }
    println!("{DIM}── output ──────────────────────────────────────────────{RESET}");
    for line in trimmed.lines() {
        println!("{DIM}│{RESET} {line}");
    }
    println!("{DIM}────────────────────────────────────────────────────────{RESET}");
}

/// Print the progress map of all exercises.
pub fn progress(project: &Project, state: &State) {
    let total = project.info.exercises.len();
    let done = project
        .info
        .exercises
        .iter()
        .filter(|e| state.is_completed(&e.name))
        .count();

    println!("\n{BOLD}OSlings progress{RESET}  {done}/{total} complete\n");
    for ex in &project.info.exercises {
        let is_current = state.current.as_deref() == Some(ex.name.as_str());
        let (mark, color) = if state.is_completed(&ex.name) {
            ("✓", GREEN)
        } else if is_current {
            ("➤", YELLOW)
        } else {
            ("▢", DIM)
        };
        let pointer = if is_current { format!(" {CYAN}← you are here{RESET}") } else { String::new() };
        println!("  {color}{mark}{RESET} {}{pointer}", ex.name);
    }

    // A small progress bar.
    let width = 30usize;
    let filled = if total == 0 { 0 } else { done * width / total };
    let bar: String = "█".repeat(filled) + &"░".repeat(width - filled);
    println!("\n  {GREEN}{bar}{RESET}\n");
}
