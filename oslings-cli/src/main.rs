//! OSlings — a Rustlings-style CLI that teaches operating-system concepts by
//! having you build the `rv6` kernel exercise by exercise.

mod model;
mod render;
mod runner;
mod watch;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use model::{Exercise, Project, State};
use std::fs;

#[derive(Parser)]
#[command(
    name = "oslings",
    version,
    about = "Learn OS internals by building the rv6 kernel, one exercise at a time."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Re-run the current exercise automatically whenever you save.
    Watch,
    /// Run one exercise's test once (defaults to the current exercise).
    Run {
        /// Exercise name or numeric prefix, e.g. `01` or `01_boot`.
        exercise: Option<String>,
    },
    /// Reveal the next progressive hint for an exercise.
    Hint {
        exercise: Option<String>,
        /// Show all three hints at once.
        #[arg(long)]
        all: bool,
        /// Forget revealed hints and start again from hint 1.
        #[arg(long)]
        reset: bool,
    },
    /// Show how many exercises you've completed.
    Progress,
    /// List all exercises in order.
    List,
    /// Display an exercise's lesson (its README) in the terminal.
    Lesson { exercise: Option<String> },
    /// Restore an exercise's starter code, discarding your edits.
    Reset { exercise: Option<String> },
    /// Show an exercise's reference solution (unlocks once you've passed it).
    Solution { exercise: Option<String> },
    /// Jump the "current" pointer to a specific exercise (or the next one).
    Goto { exercise: Option<String> },
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("\x1b[31merror:\x1b[0m {e:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    let project = Project::discover()?;

    match cli.command {
        Command::Watch => watch::watch(&project),
        Command::Run { exercise } => cmd_run(&project, exercise),
        Command::Hint { exercise, all, reset } => cmd_hint(&project, exercise, all, reset),
        Command::Progress => {
            let state = State::load(&project)?;
            render::progress(&project, &state);
            Ok(())
        }
        Command::List => cmd_list(&project),
        Command::Lesson { exercise } => cmd_lesson(&project, exercise),
        Command::Reset { exercise } => cmd_reset(&project, exercise),
        Command::Solution { exercise } => cmd_solution(&project, exercise),
        Command::Goto { exercise } => cmd_goto(&project, exercise),
    }
}

/// Resolve an exercise argument, falling back to the learner's current one.
fn resolve<'a>(
    project: &'a Project,
    state: &State,
    arg: &Option<String>,
) -> Result<&'a Exercise> {
    let name = match arg {
        Some(q) => {
            return project
                .find(q)
                .ok_or_else(|| anyhow!("no exercise matching `{q}`"));
        }
        None => state
            .current
            .clone()
            .ok_or_else(|| anyhow!("no current exercise — all done, or pass `<exercise>`"))?,
    };
    project
        .find(&name)
        .ok_or_else(|| anyhow!("current exercise `{name}` not found in info.toml"))
}

fn cmd_run(project: &Project, arg: Option<String>) -> Result<()> {
    let mut state = State::load(project)?;
    let ex = resolve(project, &state, &arg)?.clone();

    render::info(&format!("Running {} ...", ex.name));
    let outcome = runner::run(project, &ex)?;

    if outcome.passed {
        render::pass_banner(&ex.name);
        render::note(&outcome.summary);
        state.mark_completed(&ex.name);
        // If we just passed the current exercise, advance the pointer.
        if state.current.as_deref() == Some(ex.name.as_str()) {
            let next = project
                .index_of(&ex.name)
                .and_then(|i| project.info.exercises.get(i + 1))
                .map(|e| e.name.clone());
            state.current = next.clone();
            match &next {
                Some(n) => {
                    if let Some(next_ex) = project.find(n) {
                        model::stage_files(project, next_ex, "skeleton")?;
                    }
                    render::info(&format!("Next up: {n}  (run `oslings lesson` to read it)"));
                }
                None => render::info("🎉 That was the last exercise. You built rv6!"),
            }
        }
        state.save(project)?;
    } else {
        render::fail_banner(&ex.name, &outcome.summary);
        render::detail_block(&outcome.detail);
        render::note("Need a nudge?  oslings hint");
    }
    Ok(())
}

fn cmd_hint(project: &Project, arg: Option<String>, all: bool, reset: bool) -> Result<()> {
    let mut state = State::load(project)?;
    let ex = resolve(project, &state, &arg)?.clone();

    let hints = parse_hints(project, &ex)?;
    if hints.is_empty() {
        render::note("No hints for this exercise.");
        return Ok(());
    }

    if reset {
        state.hints.insert(ex.name.clone(), 0);
        state.save(project)?;
        render::note(&format!("Hints reset for {}.", ex.name));
        return Ok(());
    }

    if all {
        for (i, h) in hints.iter().enumerate() {
            render::info(&format!("── Hint {} ──", i + 1));
            render::markdown(h);
        }
        return Ok(());
    }

    let revealed = state.hints.get(&ex.name).copied().unwrap_or(0);
    if revealed >= hints.len() {
        render::note(&format!(
            "All {} hints already shown. Use `oslings hint --all` to re-read, \
             or `--reset` to start over.",
            hints.len()
        ));
        return Ok(());
    }

    let level = revealed; // show the next unseen hint
    render::info(&format!("── Hint {} of {} ──", level + 1, hints.len()));
    render::markdown(&hints[level]);

    state.hints.insert(ex.name.clone(), level + 1);
    state.save(project)?;

    let left = hints.len() - (level + 1);
    if left > 0 {
        render::note(&format!("{left} more hint(s) available — run `oslings hint` again."));
    }
    Ok(())
}

fn cmd_list(project: &Project) -> Result<()> {
    let state = State::load(project)?;
    println!();
    for (i, ex) in project.info.exercises.iter().enumerate() {
        let mark = if state.is_completed(&ex.name) { "✓" } else { " " };
        let mode = match ex.mode {
            model::Mode::Build => "build",
            model::Mode::Qemu => "qemu",
        };
        println!("  [{mark}] {i:>2}  {:<28} ({mode})", ex.name);
    }
    println!();
    Ok(())
}

fn cmd_lesson(project: &Project, arg: Option<String>) -> Result<()> {
    let state = State::load(project)?;
    let ex = resolve(project, &state, &arg)?;
    let readme = project.root.join(&ex.path).join("README.md");
    let content = fs::read_to_string(&readme)
        .with_context(|| format!("reading {}", readme.display()))?;
    render::markdown(&content);
    Ok(())
}

fn cmd_reset(project: &Project, arg: Option<String>) -> Result<()> {
    let state = State::load(project)?;
    let ex = resolve(project, &state, &arg)?.clone();
    model::stage_files(project, &ex, "skeleton")?;
    render::info(&format!(
        "Reset {} — starter code restored in rv6/src ({} file(s)).",
        ex.name,
        ex.files.len()
    ));
    Ok(())
}

fn cmd_solution(project: &Project, arg: Option<String>) -> Result<()> {
    let state = State::load(project)?;
    let ex = resolve(project, &state, &arg)?;

    if !state.is_completed(&ex.name) {
        return Err(anyhow!(
            "the solution for {} is locked until you pass it.\n\
             Give it another go — `oslings hint` can help.",
            ex.name
        ));
    }

    let dir = project.root.join(&ex.path).join("solution");
    println!();
    render::info(&format!("Reference solution for {}", ex.name));
    for file in &ex.files {
        let path = dir.join(file);
        let body = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        render::markdown(&format!("### `rv6/src/{file}`\n\n```rust\n{body}\n```"));
    }
    Ok(())
}

fn cmd_goto(project: &Project, arg: Option<String>) -> Result<()> {
    let mut state = State::load(project)?;
    let target = match &arg {
        Some(q) => project
            .find(q)
            .ok_or_else(|| anyhow!("no exercise matching `{q}`"))?
            .clone(),
        None => {
            // Next exercise after the current one.
            let cur = state
                .current
                .clone()
                .ok_or_else(|| anyhow!("nothing current; pass an exercise name"))?;
            project
                .index_of(&cur)
                .and_then(|i| project.info.exercises.get(i + 1))
                .cloned()
                .ok_or_else(|| anyhow!("already at the last exercise"))?
        }
    };

    // Stage its starter code if the kernel doesn't have it yet.
    model::stage_files(project, &target, "skeleton")?;
    state.current = Some(target.name.clone());
    state.save(project)?;
    render::info(&format!("Now on {} — `oslings lesson` to read it.", target.name));
    Ok(())
}

/// Split an exercise's hints.md into its three "## Hint N" sections.
fn parse_hints(project: &Project, ex: &Exercise) -> Result<Vec<String>> {
    let path = project.root.join(&ex.path).join("hints.md");
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(vec![]),
    };

    let mut hints = Vec::new();
    let mut current: Option<String> = None;
    for line in raw.lines() {
        if line.trim_start().to_lowercase().starts_with("## hint") {
            if let Some(prev) = current.take() {
                hints.push(prev.trim().to_string());
            }
            current = Some(String::new());
        } else if let Some(buf) = current.as_mut() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if let Some(prev) = current.take() {
        hints.push(prev.trim().to_string());
    }
    Ok(hints)
}
