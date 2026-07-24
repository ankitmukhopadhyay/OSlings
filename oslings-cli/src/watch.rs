//! `oslings watch` — re-run the current exercise whenever kernel source is
//! saved, advancing through the curriculum as exercises pass.

use crate::model::{Project, State};
use crate::{render, runner};
use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::sync::mpsc::channel;
use std::time::Duration;

pub fn watch(project: &Project) -> Result<()> {
    let mut state = State::load(project)?;

    render::info("OSlings watch — edit rv6/src and save to re-run. Ctrl-C to quit.");
    run_current(project, &mut state)?;

    let (tx, rx) = channel();
    let mut debouncer = new_debouncer(
        Duration::from_millis(400),
        move |res: DebounceEventResult| {
            let _ = tx.send(res);
        },
    )?;
    debouncer
        .watcher()
        .watch(&project.rv6_src(), RecursiveMode::Recursive)?;

    for res in rx {
        match res {
            Ok(_events) => {
                println!();
                render::note("change detected — re-running...");
                // Reload state so manual `next`/`reset` between saves is honored.
                state = State::load(project)?;
                run_current(project, &mut state)?;
            }
            Err(e) => render::note(&format!("watch error: {e:?}")),
        }
    }
    Ok(())
}

/// Run whatever exercise the learner is currently on, and on success advance
/// the pointer to the next one.
fn run_current(project: &Project, state: &mut State) -> Result<()> {
    let Some(current) = state.current.clone() else {
        render::info("🎉 All exercises complete — nothing left to watch!");
        return Ok(());
    };
    let Some(ex) = project.find(&current).cloned() else {
        render::note(&format!("unknown current exercise `{current}`"));
        return Ok(());
    };

    let outcome = runner::run(project, &ex)?;
    if outcome.passed {
        render::pass_banner(&ex.name);
        crate::model::record_pass(project, state, &ex)?;
        // Advance to the next not-yet-completed exercise.
        let next = project
            .index_of(&ex.name)
            .and_then(|i| project.info.exercises.get(i + 1))
            .map(|e| e.name.clone());
        match next {
            Some(n) => {
                if let Some(next_ex) = project.find(&n) {
                    crate::model::stage_files(project, next_ex, "skeleton")?;
                }
                render::info(&format!("Next up: {n}  (run `oslings lesson` to read it)"));
                state.current = Some(n);
            }
            None => {
                render::info("🎉 That was the last exercise. You built rv6!");
                state.current = None;
            }
        }
    } else {
        render::fail_banner(&ex.name, &outcome.summary);
        render::detail_block(&outcome.detail);
        render::note("Need a nudge?  oslings hint");
    }
    state.save(project)?;
    Ok(())
}
