//! The interactive OSlings terminal app — the default experience.
//!
//! Inspired by modern Rustlings: launch it and a welcome **Menu** greets you;
//! from there single keypresses drive everything, with a progress gauge pinned
//! to the bottom. The model is page-based:
//!
//!   Menu  ──Continue──▶  Lesson  ──n──▶  Watch (auto-runs on every file save)
//!     ▲                    ▲   ◀──p────────┘
//!     │ m                  │
//!     ├── List ────────────┼── l ──▶  List   (jump to a reached exercise)
//!     ├── About            └── h ──▶  Hints  (reveal one per press)
//!     └── Quit
//!
//! `m` returns to the menu from anywhere. Editing `rv6/src/...` while on the
//! Watch page re-runs the test automatically; on success, `n` advances.

use crate::model::{self, Project, State};
use crate::render;
use crate::runner::{self, Outcome};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::io::{stdout, Stdout, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};

const SPINNER: [&str; 4] = ["|", "/", "-", "\\"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Menu,
    About,
    Lesson,
    Watch,
    List,
    Hint,
}

/// The menu items on the home screen, in display order.
const MENU_LEN: usize = 4;
const MENU_CONTINUE: usize = 0;
const MENU_LIST: usize = 1;
const MENU_ABOUT: usize = 2;
const MENU_QUIT: usize = 3;

/// Restores the terminal to a sane state on drop, even if we panic.
struct TermGuard;

impl TermGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        Ok(TermGuard)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

struct App<'a> {
    project: &'a Project,
    state: State,
    ex_index: usize,
    view: View,
    /// Where to return to from an overlay (Hint / List).
    prev_view: View,
    list_sel: usize,
    menu_sel: usize,
    /// The scrollable markdown panel for Lesson/Watch/Hint (None for List).
    content: Option<termimad::MadView>,
    running: bool,
    rerun: bool,
    outcome: Option<Outcome>,
    finished: bool,
    spinner: usize,
    anim: usize,
    quit: bool,
    result_tx: Sender<Outcome>,
    result_rx: Receiver<Outcome>,
}

pub fn run(project: &Project) -> Result<()> {
    let state = State::load(project)?;
    let ex_index = state
        .current
        .as_ref()
        .and_then(|n| project.index_of(n))
        .unwrap_or(0);
    let finished = state.current.is_none() && !project.info.exercises.is_empty();

    let (result_tx, result_rx) = channel();
    let mut app = App {
        project,
        state,
        ex_index,
        view: View::Menu,
        prev_view: View::Menu,
        list_sel: ex_index,
        menu_sel: 0,
        content: None,
        running: false,
        rerun: false,
        outcome: None,
        finished,
        spinner: 0,
        anim: 0,
        quit: false,
        result_tx,
        result_rx,
    };

    // Watch the kernel source so saves re-run the test while on the Watch page.
    let (file_tx, file_rx) = channel();
    let mut debouncer = new_debouncer(
        Duration::from_millis(350),
        move |res: DebounceEventResult| {
            if res.is_ok() {
                let _ = file_tx.send(());
            }
        },
    )?;
    debouncer
        .watcher()
        .watch(&project.rv6_src(), RecursiveMode::Recursive)?;

    let _guard = TermGuard::enter()?;
    app.rebuild_content();

    let mut out = stdout();
    let mut dirty = true;
    while !app.quit {
        if dirty {
            app.render(&mut out)?;
            dirty = false;
        }

        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    app.on_key(k);
                    dirty = true;
                }
                Event::Resize(_, _) => {
                    app.rebuild_content();
                    dirty = true;
                }
                _ => {}
            }
        }

        // File saves → re-run when on the Watch page.
        let mut saw_save = false;
        while file_rx.try_recv().is_ok() {
            saw_save = true;
        }
        if saw_save && app.view == View::Watch {
            app.trigger_run();
            dirty = true;
        }

        // Background test result arrived.
        if let Ok(o) = app.result_rx.try_recv() {
            app.on_result(o);
            dirty = true;
        }

        if app.running {
            app.spinner = app.spinner.wrapping_add(1);
            dirty = true; // animate the spinner
        }
        if app.view == View::Menu {
            app.anim = app.anim.wrapping_add(1);
            dirty = true; // animate the crab + machinery on the home screen
        }
    }

    app.state.save(project)?;
    Ok(())
}

impl App<'_> {
    fn ex_name(&self) -> &str {
        &self.project.info.exercises[self.ex_index].name
    }

    fn total(&self) -> usize {
        self.project.info.exercises.len()
    }

    fn done_count(&self) -> usize {
        self.project
            .info
            .exercises
            .iter()
            .filter(|e| self.state.is_completed(&e.name))
            .count()
    }

    /// Furthest exercise reached — the last one that's navigable in the list.
    fn furthest(&self) -> usize {
        self.state
            .current
            .as_ref()
            .and_then(|n| self.project.index_of(n))
            .unwrap_or_else(|| self.total().saturating_sub(1))
    }

    fn passed(&self) -> bool {
        !self.running && self.outcome.as_ref().map(|o| o.passed).unwrap_or(false)
    }

    // ---- content (markdown panel) ---------------------------------------

    fn rebuild_content(&mut self) {
        // Menu and List draw their own custom screens (no scrollable panel).
        if self.view == View::Menu || self.view == View::List {
            self.content = None;
            return;
        }
        let md = match self.view {
            View::Lesson => self.lesson_md(),
            View::Hint => self.hint_md(),
            View::Watch => self.watch_md(),
            View::About => about_md(),
            View::Menu | View::List => unreachable!(),
        };
        let (w, h) = term_size();
        let top = 2u16;
        let reserved = 3u16; // separator + progress + keys
        let height = h.saturating_sub(top + reserved).max(1);
        let area = termimad::Area::new(0, top, w, height);
        self.content = Some(termimad::MadView::from(md, area, render::skin()));
    }

    fn lesson_md(&self) -> String {
        if self.finished {
            return finished_md();
        }
        let ex = &self.project.info.exercises[self.ex_index];
        let path = self.project.root.join(&ex.path).join("README.md");
        std::fs::read_to_string(path)
            .unwrap_or_else(|_| format!("# {}\n\n_(README.md missing)_", ex.name))
    }

    fn hint_md(&self) -> String {
        let ex = &self.project.info.exercises[self.ex_index];
        let hints = model::parse_hints(self.project, ex);
        if hints.is_empty() {
            return "# Hints\n\n_No hints for this exercise._".into();
        }
        let revealed = self
            .state
            .hints
            .get(&ex.name)
            .copied()
            .unwrap_or(0)
            .min(hints.len());
        let mut s = format!("# Hints — {}\n\n", ex.name);
        for (i, h) in hints.iter().enumerate().take(revealed) {
            s.push_str(&format!("## Hint {}\n\n{}\n\n", i + 1, h));
        }
        if revealed < hints.len() {
            s.push_str(&format!(
                "_Press **h** to reveal hint {} of {}._",
                revealed + 1,
                hints.len()
            ));
        } else {
            s.push_str("_That's every hint. You've got this._");
        }
        s
    }

    fn watch_md(&self) -> String {
        let ex = &self.project.info.exercises[self.ex_index];
        if self.running {
            return format!(
                "# {} Running tests…\n\nCompiling `rv6`{}.",
                SPINNER[self.spinner % SPINNER.len()],
                match ex.mode {
                    model::Mode::Qemu => " and booting it in QEMU",
                    model::Mode::Build => "",
                }
            );
        }
        match &self.outcome {
            None => "# Ready\n\nPress **n** to run this exercise's test.".into(),
            Some(o) if o.passed => format!(
                "# ✓ Passed\n\n{}\n\nPress **n** for the next exercise, or **p** to revisit the lesson.",
                o.summary
            ),
            Some(o) => {
                let detail = tail(&o.detail, 6000);
                format!(
                    "# ✗ Not yet\n\n{}\n\n```\n{}\n```\n\nEdit `rv6/src/…` and save to re-run automatically. Press **h** for a hint.",
                    o.summary, detail
                )
            }
        }
    }

    // ---- actions --------------------------------------------------------

    fn enter_watch(&mut self) {
        self.view = View::Watch;
        self.trigger_run();
    }

    fn go_lesson(&mut self) {
        self.view = View::Lesson;
        self.rebuild_content();
    }

    fn go_menu(&mut self) {
        self.view = View::Menu;
        self.menu_sel = MENU_CONTINUE; // returning home re-highlights the primary action
        self.content = None;
    }

    fn open_about(&mut self) {
        self.view = View::About;
        self.rebuild_content();
    }

    /// The label for the first menu item, which adapts to the learner's progress.
    fn continue_label(&self) -> String {
        if self.finished {
            "Review exercises".into()
        } else if self.done_count() == 0 {
            format!("Start  ›  {}", self.ex_name())
        } else {
            format!("Continue  ›  {}", self.ex_name())
        }
    }

    /// Act on the currently highlighted menu item.
    fn menu_select(&mut self) {
        match self.menu_sel {
            MENU_CONTINUE => self.go_lesson(), // resumes current exercise (or the finished screen)
            MENU_LIST => self.open_list(),
            MENU_ABOUT => self.open_about(),
            MENU_QUIT => self.quit = true,
            _ => {}
        }
    }

    fn open_list(&mut self) {
        self.prev_view = self.view;
        self.view = View::List;
        self.list_sel = self.ex_index.min(self.total().saturating_sub(1));
        self.content = None;
    }

    fn open_hint(&mut self) {
        self.prev_view = match self.view {
            View::Hint | View::List => self.prev_view,
            other => other,
        };
        // Reveal the first hint automatically on open.
        let name = self.ex_name().to_string();
        let n = self.state.hints.entry(name).or_insert(0);
        if *n == 0 {
            *n = 1;
        }
        let _ = self.state.save(self.project);
        self.view = View::Hint;
        self.rebuild_content();
    }

    fn reveal_next_hint(&mut self) {
        let ex = &self.project.info.exercises[self.ex_index];
        let max = model::parse_hints(self.project, ex).len();
        let name = ex.name.clone();
        let n = self.state.hints.entry(name).or_insert(0);
        if *n < max {
            *n += 1;
        }
        let _ = self.state.save(self.project);
        self.rebuild_content();
    }

    fn back_from_overlay(&mut self) {
        self.view = self.prev_view;
        self.rebuild_content();
    }

    fn reset_exercise(&mut self) {
        let ex = self.project.info.exercises[self.ex_index].clone();
        let _ = model::stage_files(self.project, &ex, "skeleton");
        self.trigger_run();
    }

    fn advance(&mut self) {
        let next = self.ex_index + 1;
        if next >= self.total() {
            // Curriculum complete.
            self.state.current = None;
            let _ = self.state.save(self.project);
            self.finished = true;
            self.view = View::Lesson;
            self.outcome = None;
            self.rebuild_content();
            return;
        }
        // First time reaching this exercise: stage its starter files.
        if next > self.furthest() {
            let ex_next = self.project.info.exercises[next].clone();
            let _ = model::stage_files(self.project, &ex_next, "skeleton");
            self.state.current = Some(ex_next.name.clone());
            let _ = self.state.save(self.project);
        }
        self.ex_index = next;
        self.outcome = None;
        self.view = View::Lesson;
        self.rebuild_content();
    }

    fn open_selected(&mut self) {
        if self.list_sel <= self.furthest() {
            self.ex_index = self.list_sel;
            self.outcome = None;
            self.finished = false;
            self.view = View::Lesson;
            self.rebuild_content();
        }
    }

    fn trigger_run(&mut self) {
        if self.running {
            self.rerun = true;
            return;
        }
        self.running = true;
        self.outcome = None;
        self.rebuild_content();

        let project = self.project.clone();
        let ex = self.project.info.exercises[self.ex_index].clone();
        let tx = self.result_tx.clone();
        thread::spawn(move || {
            let outcome = runner::run(&project, &ex).unwrap_or_else(|e| Outcome {
                passed: false,
                summary: format!("harness error: {e}"),
                detail: String::new(),
            });
            let _ = tx.send(outcome);
        });
    }

    fn on_result(&mut self, o: Outcome) {
        self.running = false;
        if o.passed {
            let name = self.ex_name().to_string();
            self.state.mark_completed(&name);
            let _ = self.state.save(self.project);
        }
        self.outcome = Some(o);
        if self.rerun {
            self.rerun = false;
            self.trigger_run();
        } else {
            self.rebuild_content();
        }
    }

    fn scroll(&mut self, lines: i32) {
        if let Some(v) = self.content.as_mut() {
            v.try_scroll_lines(lines);
        }
    }

    fn scroll_page(&mut self, pages: i32) {
        if let Some(v) = self.content.as_mut() {
            v.try_scroll_pages(pages);
        }
    }

    // ---- input ----------------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        // Global quit.
        if key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            self.quit = true;
            return;
        }
        // Global: return to the menu from anywhere but the menu itself.
        if key.code == KeyCode::Char('m') && self.view != View::Menu {
            self.go_menu();
            return;
        }

        match self.view {
            View::Menu => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.menu_sel = self.menu_sel.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.menu_sel + 1 < MENU_LEN {
                        self.menu_sel += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => self.menu_select(),
                KeyCode::Char('1') => {
                    self.menu_sel = MENU_CONTINUE;
                    self.menu_select();
                }
                KeyCode::Char('2') => {
                    self.menu_sel = MENU_LIST;
                    self.menu_select();
                }
                KeyCode::Char('3') => {
                    self.menu_sel = MENU_ABOUT;
                    self.menu_select();
                }
                KeyCode::Char('4') => {
                    self.menu_sel = MENU_QUIT;
                    self.menu_select();
                }
                _ => {}
            },
            View::About => match key.code {
                KeyCode::Char('p') | KeyCode::Esc => self.go_menu(),
                KeyCode::Char('l') => self.open_list(),
                KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
                KeyCode::PageUp => self.scroll_page(-1),
                KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_page(1),
                _ => {}
            },
            View::Lesson => match key.code {
                KeyCode::Char('n') if !self.finished => self.enter_watch(),
                KeyCode::Char('l') => self.open_list(),
                KeyCode::Char('h') if !self.finished => self.open_hint(),
                KeyCode::Esc => self.go_menu(),
                KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
                KeyCode::PageUp => self.scroll_page(-1),
                KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_page(1),
                _ => {}
            },
            View::Watch => match key.code {
                KeyCode::Char('p') => self.go_lesson(),
                KeyCode::Esc => self.go_menu(),
                KeyCode::Char('n') if self.passed() => self.advance(),
                KeyCode::Char('l') => self.open_list(),
                KeyCode::Char('h') => self.open_hint(),
                KeyCode::Char('r') => self.reset_exercise(),
                KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
                KeyCode::PageUp => self.scroll_page(-1),
                KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_page(1),
                _ => {}
            },
            View::Hint => match key.code {
                KeyCode::Char('h') => self.reveal_next_hint(),
                KeyCode::Char('p') | KeyCode::Esc => self.back_from_overlay(),
                KeyCode::Char('l') => self.open_list(),
                KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
                _ => {}
            },
            View::List => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.list_sel = self.list_sel.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.list_sel + 1 < self.total() {
                        self.list_sel += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.open_selected(),
                KeyCode::Char('p') | KeyCode::Esc | KeyCode::Left => self.back_from_overlay(),
                _ => {}
            },
        }
    }

    // ---- rendering ------------------------------------------------------

    fn render(&self, out: &mut Stdout) -> Result<()> {
        let (w, h) = term_size();
        execute!(out, Clear(ClearType::All))?;

        // Header bar.
        let title = match self.view {
            View::Menu => " OSlings — learn operating systems by building a kernel".to_string(),
            View::About => " OSlings · about".to_string(),
            View::List => " OSlings · exercises".to_string(),
            View::Lesson if self.finished => " OSlings · complete".to_string(),
            View::Lesson => format!(" OSlings · {} · lesson", self.ex_name()),
            View::Watch => format!(" OSlings · {} · tests", self.ex_name()),
            View::Hint => format!(" OSlings · {} · hint", self.ex_name()),
        };
        queue!(
            out,
            MoveTo(0, 0),
            SetAttribute(Attribute::Bold),
            SetForegroundColor(Color::AnsiValue(45)),
            Print(fit(&title, w)),
            ResetColor,
            SetAttribute(Attribute::Reset),
        )?;

        // Body.
        match self.view {
            View::Menu => self.render_menu(out, w, h)?,
            View::List => self.render_list(out, w, h)?,
            _ => {
                if let Some(v) = &self.content {
                    v.write()?;
                }
            }
        }

        // Footer: separator, progress gauge, key hints.
        let sep_row = h.saturating_sub(3);
        let prog_row = h.saturating_sub(2);
        let keys_row = h.saturating_sub(1);

        queue!(
            out,
            MoveTo(0, sep_row),
            SetForegroundColor(Color::AnsiValue(240)),
            Print("─".repeat(w as usize)),
            ResetColor,
        )?;

        queue!(out, MoveTo(0, prog_row))?;
        self.render_progress(out, w)?;

        queue!(
            out,
            MoveTo(0, keys_row),
            SetForegroundColor(Color::AnsiValue(244)),
            Print(fit(&self.footer_keys(), w)),
            ResetColor,
        )?;

        out.flush()?;
        Ok(())
    }

    fn footer_keys(&self) -> String {
        match self.view {
            View::Menu => " ↑↓ move    ⏎ select    1-4 jump    q quit".into(),
            View::About => " ↑↓ scroll    m menu    q quit".into(),
            View::Lesson if self.finished => " l list    m menu    q quit".into(),
            View::Lesson => " n run ▸    l list    h hint    m menu    ↑↓ scroll    q quit".into(),
            View::Watch if self.passed() => {
                " n next ▸    p lesson    l list    r reset    m menu    q quit".into()
            }
            View::Watch => {
                " p lesson    h hint    l list    r reset    m menu    ↑↓ scroll    q quit".into()
            }
            View::Hint => " h more    p back    l list    m menu    ↑↓ scroll    q quit".into(),
            View::List => " ↑↓ move    ⏎ open    p back    m menu    q quit".into(),
        }
    }

    fn render_progress(&self, out: &mut Stdout, w: u16) -> Result<()> {
        let done = self.done_count();
        let total = self.total();
        let pct = if total > 0 { done * 100 / total } else { 0 };
        let label = format!(" {done}/{total}  {pct}% ");
        let barw = (w as usize)
            .saturating_sub(label.len() + 12)
            .clamp(10, 50);
        let filled = if total > 0 { done * barw / total } else { 0 };
        let bar = "█".repeat(filled) + &"░".repeat(barw.saturating_sub(filled));
        queue!(
            out,
            SetForegroundColor(Color::AnsiValue(244)),
            Print(" Progress ["),
            SetForegroundColor(Color::Green),
            Print(bar),
            SetForegroundColor(Color::AnsiValue(244)),
            Print("]"),
            Print(label),
            ResetColor,
        )?;
        Ok(())
    }

    fn render_list(&self, out: &mut Stdout, w: u16, h: u16) -> Result<()> {
        let top = 2u16;
        let max_row = h.saturating_sub(4);
        let furthest = self.furthest();
        for (i, ex) in self.project.info.exercises.iter().enumerate() {
            let row = top + i as u16;
            if row > max_row {
                break;
            }
            let locked = i > furthest;
            let mark = if self.state.is_completed(&ex.name) {
                "✓"
            } else if locked {
                "🔒"
            } else if i == self.ex_index {
                "➤"
            } else {
                "▢"
            };
            let line = format!("  {mark}  {:<30}", ex.name);
            queue!(out, MoveTo(0, row))?;
            if i == self.list_sel {
                queue!(out, SetAttribute(Attribute::Reverse))?;
            }
            let color = if locked {
                Color::AnsiValue(240)
            } else if self.state.is_completed(&ex.name) {
                Color::Green
            } else {
                Color::White
            };
            queue!(
                out,
                SetForegroundColor(color),
                Print(fit(&line, w)),
                ResetColor,
                SetAttribute(Attribute::Reset),
            )?;
        }
        Ok(())
    }

    fn render_menu(&self, out: &mut Stdout, w: u16, h: u16) -> Result<()> {
        let max_row = h.saturating_sub(4); // leave room for the footer (3 rows) + margin
        let mut row = 1u16;

        let banner = oslings_banner();
        let bw = banner[0].chars().count(); // banner width in cells (= 41)
        let pad = (((w as usize).saturating_sub(bw)) / 2).max(2);
        let pad_s = " ".repeat(pad);

        // Full animated banner needs ~13 rows; fall back to a compact title on
        // short terminals so the menu items always remain visible.
        let compact = (max_row as usize) < (row as usize) + 13 + MENU_LEN + 1;

        if compact {
            menu_line(out, &mut row, max_row, w, &pad_s, "OSlings 🦀⚙️", Color::AnsiValue(45))?;
            row += 1;
        } else {
            // ---- the machinery + crab, animated on top of the OSLINGS art ----
            let gears = ['◴', '◷', '◶', '◵'];
            let gear_cols = [4usize, bw / 2, bw.saturating_sub(5)];

            // the crab: a taller, 3-row critter — claws up top, a face, and
            // legs that step as it walks.
            let crab_w = 11usize;
            let crab_art: [&str; 3] = if (self.anim / 6) % 2 == 0 {
                ["(\\/)   (\\/)", "   (°ᴥ°)   ", "  / | | \\  "]
            } else {
                ["(\\/)   (\\/)", "   (°ᴥ°)   ", "  \\ | | /  "]
            };
            let span = bw.saturating_sub(crab_w).max(1);
            // pace back and forth across the top of the text (triangle wave)
            let t = (self.anim / 2) % (2 * span);
            let crab_x = if t < span { t } else { 2 * span - t };
            let crab_center = crab_x + crab_w / 2;

            // machine: gears on a moving belt.
            let mut machine = vec![' '; bw];
            for c in 2..bw.saturating_sub(2) {
                machine[c] = if (c + self.anim / 2) % 4 == 0 { '═' } else { '─' };
            }
            for (i, &gc) in gear_cols.iter().enumerate() {
                if gc >= 1 && gc + 1 < bw {
                    machine[gc - 1] = '[';
                    machine[gc] = gears[(self.anim / 3 + i) % 4];
                    machine[gc + 1] = ']';
                }
            }
            // connector: the rod the crab raises into the belt as it walks.
            let mut connector = vec![' '; bw];
            if crab_center < bw {
                connector[crab_center] = '│';
            }

            let machine_s: String = machine.into_iter().collect();
            let connector_s: String = connector.into_iter().collect();
            menu_line(out, &mut row, max_row, w, &pad_s, &machine_s, Color::AnsiValue(220))?;
            menu_line(out, &mut row, max_row, w, &pad_s, &connector_s, Color::AnsiValue(244))?;
            // draw the crab's three rows.
            for art in crab_art.iter() {
                let mut line = vec![' '; bw];
                for (i, ch) in art.chars().enumerate() {
                    if crab_x + i < bw {
                        line[crab_x + i] = ch;
                    }
                }
                let s: String = line.into_iter().collect();
                menu_line(out, &mut row, max_row, w, &pad_s, &s, Color::AnsiValue(209))?;
            }
            for bl in &banner {
                menu_line(out, &mut row, max_row, w, &pad_s, bl, Color::AnsiValue(45))?;
            }
            menu_line(
                out,
                &mut row,
                max_row,
                w,
                &pad_s,
                "learn operating systems by building a kernel",
                Color::AnsiValue(250),
            )?;
            row += 1; // blank spacer before the menu items
        }

        // ---- selectable menu items (numbers match the 1–4 shortcuts) ----
        let cont = self.continue_label();
        let items = [cont.as_str(), "Exercise list", "How OSlings works", "Quit"];
        for (i, item) in items.iter().enumerate() {
            if row > max_row {
                break;
            }
            let selected = i == self.menu_sel;
            let marker = if selected { "▸" } else { " " };
            let line = format!("{pad_s} {marker}  {}.  {}", i + 1, item);
            queue!(out, MoveTo(0, row))?;
            if selected {
                queue!(out, SetAttribute(Attribute::Reverse))?;
            }
            let color = if selected {
                Color::AnsiValue(45)
            } else {
                Color::White
            };
            queue!(
                out,
                SetForegroundColor(color),
                Print(fit(&line, w)),
                ResetColor,
                SetAttribute(Attribute::Reset),
            )?;
            row += 1;
        }
        Ok(())
    }
}

// ---- helpers ------------------------------------------------------------

fn term_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

/// Print one left-padded, colored line on the menu at `*row`, advancing it.
/// Does nothing once we've run past `max_row` (keeps the layout from spilling
/// into the footer on short terminals).
fn menu_line(
    out: &mut Stdout,
    row: &mut u16,
    max_row: u16,
    w: u16,
    pad: &str,
    text: &str,
    color: Color,
) -> Result<()> {
    if *row > max_row {
        return Ok(());
    }
    let line = format!("{pad}{text}");
    queue!(
        out,
        MoveTo(0, *row),
        SetForegroundColor(color),
        Print(fit(&line, w)),
        ResetColor,
    )?;
    *row += 1;
    Ok(())
}

/// The OSLINGS logo as five rows of block-letter ASCII art (41 cells wide).
fn oslings_banner() -> Vec<String> {
    let o = ["█████", "█   █", "█   █", "█   █", "█████"];
    let s = ["█████", "█    ", "█████", "    █", "█████"];
    let l = ["█    ", "█    ", "█    ", "█    ", "█████"];
    let i = ["█████", "  █  ", "  █  ", "  █  ", "█████"];
    let n = ["█   █", "██  █", "█ █ █", "█  ██", "█   █"];
    let g = ["█████", "█    ", "█  ██", "█   █", "█████"];
    let letters = [o, s, l, i, n, g, s];
    (0..5)
        .map(|r| {
            letters
                .iter()
                .map(|ltr| ltr[r])
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Truncate/pad a string to exactly `w` columns (ASCII-ish, good enough for
/// our headers/footers which are plain text).
fn fit(s: &str, w: u16) -> String {
    let w = w as usize;
    let len = s.chars().count();
    if len > w {
        s.chars().take(w).collect()
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}

/// Keep only the last `max` bytes of long output (errors, QEMU logs).
fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let start = s.len() - max;
    // Avoid slicing mid-codepoint.
    let start = (start..s.len())
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(s.len());
    format!("…\n{}", &s[start..])
}

fn finished_md() -> String {
    "# 🎉 You built rv6!\n\nEvery exercise is complete. The kernel boots, manages \
     memory, schedules processes — and you wrote it.\n\nPress **l** to revisit any \
     exercise, **m** for the menu, or **q** to quit."
        .into()
}

fn about_md() -> String {
    "# How OSlings works\n\n\
     OSlings teaches operating-system concepts by having you build **rv6**, a \
     small RISC-V kernel, from scratch — one exercise at a time.\n\n\
     ## The loop\n\n\
     Each exercise has three phases:\n\n\
     - **Learn** — read the lesson (the page you reach with *Continue*).\n\
     - **Understand** — open the files it points to in `rv6/src` and read the \
     `// UNDERSTAND` notes.\n\
     - **Implement** — fill in the `// IMPLEMENT` markers, then save.\n\n\
     ## Running tests\n\n\
     From a lesson, press **n** to start. OSlings boots your kernel in QEMU and \
     watches `rv6/src` — every time you save, it re-runs the test automatically. \
     When it passes, press **n** again to advance to the next exercise.\n\n\
     ## Keys\n\n\
     - **n** begin / next  ·  **p** back to the lesson\n\
     - **l** exercise list (jump to any exercise you've reached)\n\
     - **h** reveal a hint (press again for the next one)\n\
     - **r** reset the current exercise's starter code\n\
     - **m** back to the menu  ·  **q** quit\n\n\
     ## Your work lives in `rv6/src`\n\n\
     You edit the kernel in `rv6/src` with your own editor. The kernel is built \
     *cumulatively*: each exercise already contains everything you completed in \
     earlier ones, plus the new files to work on.\n\n\
     _Press **m** to return to the menu._"
        .into()
}
