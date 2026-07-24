//! OSlings — a Rustlings-style CLI that teaches operating-system concepts by
//! having you build the `rv6` kernel exercise by exercise.

mod cheatsheet;
mod model;
mod render;
mod runner;
mod tui;
mod watch;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use model::{Difficulty, Exercise, Project, State};
use std::fs;

#[derive(Parser)]
#[command(
    name = "oslings",
    version,
    about = "Learn OS internals by building the rv6 kernel, one exercise at a time."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
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
    Progress {
        /// Emit a gradebook-ready CSV report instead of the pretty view.
        #[arg(long)]
        export: bool,
    },
    /// List all exercises in order.
    List,
    /// Display an exercise's lesson (its README) in the terminal.
    Lesson { exercise: Option<String> },
    /// Show the cheatsheet: bit layouts, magic numbers, and concepts.
    Cheatsheet,
    /// Restore an exercise's starter code, discarding your edits.
    Reset { exercise: Option<String> },
    /// Show an exercise's reference solution (unlocks once you've passed it).
    Solution { exercise: Option<String> },
    /// Jump the "current" pointer to a specific exercise (or the next one).
    Goto { exercise: Option<String> },
    /// Show or set the guidance level: guided | standard | challenge.
    Difficulty {
        /// New level to set locally; omit to just show the current one.
        level: Option<String>,
    },
    /// Re-grade committed submissions by re-running the harness (for CI).
    Grade {
        /// One exercise (exits non-zero on failure, for a CI check), or all.
        exercise: Option<String>,
    },
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

    // No subcommand → launch the interactive TUI (the default experience).
    let Some(command) = cli.command else {
        return tui::run(&project);
    };

    match command {
        Command::Watch => watch::watch(&project),
        Command::Run { exercise } => cmd_run(&project, exercise),
        Command::Hint { exercise, all, reset } => cmd_hint(&project, exercise, all, reset),
        Command::Progress { export } => {
            let state = State::load(&project)?;
            if export {
                cmd_progress_export(&project, &state)
            } else {
                render::progress(&project, &state);
                Ok(())
            }
        }
        Command::List => cmd_list(&project),
        Command::Lesson { exercise } => cmd_lesson(&project, exercise),
        Command::Cheatsheet => {
            render::markdown(cheatsheet::markdown());
            Ok(())
        }
        Command::Reset { exercise } => cmd_reset(&project, exercise),
        Command::Solution { exercise } => cmd_solution(&project, exercise),
        Command::Goto { exercise } => cmd_goto(&project, exercise),
        Command::Difficulty { level } => cmd_difficulty(&project, level),
        Command::Grade { exercise } => cmd_grade(&project, exercise),
    }
}

/// Re-verify committed submissions by re-running the real harness against them.
/// This is what CI calls; the grade cannot be faked by editing state, because it
/// rebuilds and reboots the student's snapshotted code from scratch.
///
/// With an exercise argument it grades just that one and exits non-zero on
/// failure (a single CI check); with no argument it grades every submission and
/// prints an aggregate report.
fn cmd_grade(project: &Project, arg: Option<String>) -> Result<()> {
    let single = arg.is_some();
    let targets: Vec<Exercise> = match &arg {
        Some(q) => vec![project
            .find(q)
            .ok_or_else(|| anyhow!("no exercise matching `{q}`"))?
            .clone()],
        None => project.info.exercises.clone(),
    };

    let mut graded = 0usize;
    let mut passed = 0usize;
    for ex in &targets {
        let subdir = project.submissions_dir().join(&ex.name);
        if !subdir.exists() {
            if single {
                return Err(anyhow!(
                    "no submission for {}: submissions/{}/ is missing",
                    ex.name,
                    ex.name
                ));
            }
            println!("  [ -- ] {:<26} no submission", ex.name);
            continue;
        }
        graded += 1;
        model::stage_from_dir(project, ex, &subdir)?;
        let outcome = runner::run(project, ex)?;
        if outcome.passed {
            passed += 1;
            println!("  [ OK ] {}", ex.name);
        } else {
            println!("  [FAIL] {:<26} {}", ex.name, outcome.summary);
        }
    }

    println!();
    render::info(&format!("Graded {graded} submission(s): {passed} passed."));
    // For a single-exercise CI check, signal failure through the exit code.
    if single && passed != graded {
        std::process::exit(1);
    }
    Ok(())
}

/// Emit a gradebook-ready CSV of the learner's progress: one row per exercise,
/// with completion, difficulty, hints used, and pass time (from the committed
/// snapshot metadata when present). Prints to stdout; redirect to a file to
/// submit (`oslings progress --export > progress.csv`).
fn cmd_progress_export(project: &Project, state: &State) -> Result<()> {
    let (name, email) = project
        .config
        .student
        .as_ref()
        .map(|s| (s.name.clone(), s.email.clone()))
        .unwrap_or_default();

    let mut out = String::new();
    out.push_str("student_name,student_email,exercise,part,completed,difficulty,hints_used,passed_at\n");
    for ex in &project.info.exercises {
        let completed = state.is_completed(&ex.name);
        let hints = state.hints.get(&ex.name).copied().unwrap_or(0);
        let meta = model::read_submission_meta(project, &ex.name);
        let difficulty = meta
            .as_ref()
            .map(|m| m.difficulty.clone())
            .unwrap_or_else(|| project.effective_difficulty().as_str().to_string());
        let passed_at = meta.map(|m| m.passed_at).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            csv(&name),
            csv(&email),
            csv(&ex.name),
            ex.part,
            if completed { "yes" } else { "no" },
            csv(&difficulty),
            hints,
            csv(&passed_at),
        ));
    }
    print!("{out}");
    Ok(())
}

/// Quote a CSV field if it contains a comma, quote, or newline (RFC 4180).
fn csv(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn cmd_difficulty(project: &Project, level: Option<String>) -> Result<()> {
    match level {
        None => {
            let d = project.effective_difficulty();
            render::info(&format!("Difficulty: {}", d.as_str()));
            render::note(
                "Levels: guided (full guidance), standard (task lines + 2 hints), \
                 challenge (minimal + 1 hint).\nSet with `oslings difficulty <level>`.",
            );
            Ok(())
        }
        Some(s) => {
            let d = Difficulty::parse(&s).ok_or_else(|| {
                anyhow!("unknown difficulty `{s}` (expected: guided, standard, or challenge)")
            })?;
            let mut cfg = project.config.clone();
            cfg.difficulty = Some(d);
            cfg.save(&project.root)?;
            render::info(&format!(
                "Difficulty set to {} (saved in .oslings/config.toml).",
                d.as_str()
            ));
            if d != Difficulty::Guided {
                render::note(
                    "Applies to each exercise as it is staged. To re-stage the CURRENT \
                     exercise at this level, run `oslings reset` (this discards your edits).",
                );
            }
            Ok(())
        }
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
        model::record_pass(project, &mut state, &ex)?;
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

    let hints = model::parse_hints(project, &ex);
    if hints.is_empty() {
        render::note("No hints for this exercise.");
        return Ok(());
    }

    // Difficulty caps how many hints may be revealed.
    let difficulty = project.effective_difficulty();
    let cap = difficulty.hint_cap(hints.len());

    if reset {
        state.hints.insert(ex.name.clone(), 0);
        state.save(project)?;
        render::note(&format!("Hints reset for {}.", ex.name));
        return Ok(());
    }

    if cap == 0 {
        render::note(&format!(
            "Hints are turned off at `{}` difficulty — re-read the lesson: `oslings lesson`.",
            difficulty.as_str()
        ));
        return Ok(());
    }

    if all {
        for (i, h) in hints.iter().take(cap).enumerate() {
            render::info(&format!("── Hint {} ──", i + 1));
            render::markdown(h);
        }
        if cap < hints.len() {
            render::note(&format!(
                "Showing {} of {} hints — the rest are held back at `{}` difficulty.",
                cap,
                hints.len(),
                difficulty.as_str()
            ));
        }
        return Ok(());
    }

    let revealed = state.hints.get(&ex.name).copied().unwrap_or(0);
    if revealed >= cap {
        if cap < hints.len() {
            render::note(&format!(
                "That's all the hints available at `{}` difficulty ({} of {}).",
                difficulty.as_str(),
                cap,
                hints.len()
            ));
        } else {
            render::note(&format!(
                "All {} hints already shown. Use `oslings hint --all` to re-read, \
                 or `--reset` to start over.",
                hints.len()
            ));
        }
        return Ok(());
    }

    let level = revealed; // show the next unseen hint
    render::info(&format!("── Hint {} of {} ──", level + 1, cap));
    render::markdown(&hints[level]);

    state.hints.insert(ex.name.clone(), level + 1);
    state.save(project)?;

    let left = cap - (level + 1);
    if left > 0 {
        render::note(&format!("{left} more hint(s) available — run `oslings hint` again."));
    }
    Ok(())
}

fn cmd_list(project: &Project) -> Result<()> {
    let state = State::load(project)?;
    println!();
    let mut last_part = 0usize;
    for (i, ex) in project.info.exercises.iter().enumerate() {
        if ex.part != last_part {
            last_part = ex.part;
            println!("\n  ── {} ──", model::part_label(ex.part));
        }
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


#[cfg(test)]
mod tests {
    use super::csv;

    #[test]
    fn csv_quotes_only_when_needed() {
        assert_eq!(csv("plain"), "plain");
        assert_eq!(csv("a,b"), "\"a,b\"");
        assert_eq!(csv("she said \"hi\""), "\"she said \"\"hi\"\"\"");
        assert_eq!(csv("line\nbreak"), "\"line\nbreak\"");
    }
}
