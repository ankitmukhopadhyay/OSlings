//! Project model: locating the workspace, parsing `info.toml`, and the
//! persisted learner state in `.oslings/state.toml`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// How an exercise decides pass/fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Passes when the kernel compiles for the bare-metal target.
    Build,
    /// Passes when the booted kernel prints the pass marker in QEMU.
    Qemu,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Exercise {
    pub name: String,
    pub path: String,
    pub mode: Mode,
    #[serde(default)]
    pub files: Vec<String>,
    /// Which curriculum part this belongs to (1 = build the kernel, 2 = boot &
    /// shell, 3 = persistence). Defaults to 1 for the original exercises.
    #[serde(default = "default_part")]
    pub part: usize,
    /// Extra cargo features to build the kernel with for this exercise (e.g.
    /// `["harness"]` so the test self-checks and prints OSLINGS:PASS).
    #[serde(default)]
    pub features: Vec<String>,
}

fn default_part() -> usize {
    1
}

/// How much guidance an exercise offers. Set course-wide in `info.toml`
/// (`[meta] difficulty`), overridable locally in `.oslings/config.toml` or via
/// the `OSLINGS_DIFFICULTY` environment variable. Defaults to `guided`, so a
/// fresh install behaves exactly as before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    /// Full step-by-step `// IMPLEMENT` comments + all three hints (the original
    /// experience).
    #[default]
    Guided,
    /// Task lines kept, detailed steps stripped; hints capped at 2.
    Standard,
    /// Minimal `// TODO` markers only; a single conceptual hint.
    Challenge,
}

impl Difficulty {
    pub fn as_str(self) -> &'static str {
        match self {
            Difficulty::Guided => "guided",
            Difficulty::Standard => "standard",
            Difficulty::Challenge => "challenge",
        }
    }

    pub fn parse(s: &str) -> Option<Difficulty> {
        match s.trim().to_lowercase().as_str() {
            "guided" => Some(Difficulty::Guided),
            "standard" => Some(Difficulty::Standard),
            "challenge" => Some(Difficulty::Challenge),
            _ => None,
        }
    }

    /// The highest hint level (1-based) a learner may reveal at this difficulty,
    /// given how many hints the exercise has.
    pub fn hint_cap(self, total: usize) -> usize {
        let cap = match self {
            Difficulty::Guided => total,
            Difficulty::Standard => 2,
            Difficulty::Challenge => 1,
        };
        cap.min(total)
    }
}

/// Human-readable banner for a curriculum part (used as a list divider).
pub fn part_label(part: usize) -> &'static str {
    match part {
        1 => "Part 1 · Build the kernel  (→ an OS is built)",
        2 => "Part 2 · Boot it & build a shell  (→ bootable & runnable)",
        3 => "Part 3 · Persistence",
        _ => "More exercises",
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub pass_marker: String,
    pub fail_marker: String,
    /// Course-wide guidance level (default `guided`).
    #[serde(default)]
    pub difficulty: Difficulty,
    /// When true, a passing solution is archived into `submissions/<exercise>/`
    /// (committed, so CI can re-grade it). Default false, so a solo repo stays
    /// uncluttered; the student course profile sets it true.
    #[serde(default)]
    pub snapshots: bool,
    /// When true, `oslings` prompts for the learner's name/email on first use
    /// (so exported progress is attributable). Default false.
    #[serde(default)]
    #[allow(dead_code)] // consumed by the progress-export increment
    pub require_identity: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Info {
    pub meta: Meta,
    pub exercises: Vec<Exercise>,
}

/// Machine-local settings in `.oslings/config.toml` (never committed to a
/// student's course repo). Overrides the course defaults in `info.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Local difficulty override (falls back to `info.toml` when unset).
    #[serde(default)]
    pub difficulty: Option<Difficulty>,
    /// Local snapshot override (falls back to `info.toml` when unset). Handy
    /// for testing; the course profile normally sets it in `info.toml`.
    #[serde(default)]
    pub snapshots: Option<bool>,
    /// The learner's identity, for attributing exported progress.
    #[serde(default)]
    pub student: Option<Student>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Student {
    pub name: String,
    pub email: String,
}

impl Config {
    pub fn load(root: &Path) -> Config {
        let path = root.join(".oslings").join("config.toml");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let dir = root.join(".oslings");
        fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let raw = toml::to_string_pretty(self).context("serializing config")?;
        fs::write(&path, raw).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

/// Everything the commands need: where the project lives and its config.
#[derive(Clone)]
pub struct Project {
    pub root: PathBuf,
    pub info: Info,
    pub config: Config,
}

impl Project {
    /// Find the project root by walking up from the current directory looking
    /// for `info.toml`, then parse it.
    pub fn discover() -> Result<Self> {
        let start = env::current_dir().context("cannot read current directory")?;
        let root = find_root(&start).ok_or_else(|| {
            anyhow!(
                "could not find `info.toml` in {} or any parent directory.\n\
                 Run oslings from inside the OSlings project.",
                start.display()
            )
        })?;
        let info_path = root.join("info.toml");
        let raw = fs::read_to_string(&info_path)
            .with_context(|| format!("reading {}", info_path.display()))?;
        let info: Info = toml::from_str(&raw)
            .with_context(|| format!("parsing {}", info_path.display()))?;
        let config = Config::load(&root);
        Ok(Project { root, info, config })
    }

    #[allow(dead_code)] // used by the progress-export increment
    pub fn config_path(&self) -> PathBuf {
        self.root.join(".oslings").join("config.toml")
    }

    /// The guidance level in effect: `OSLINGS_DIFFICULTY` env var, else the
    /// local `.oslings/config.toml`, else the course default in `info.toml`.
    pub fn effective_difficulty(&self) -> Difficulty {
        if let Ok(v) = env::var("OSLINGS_DIFFICULTY") {
            if let Some(d) = Difficulty::parse(&v) {
                return d;
            }
        }
        self.config
            .difficulty
            .unwrap_or(self.info.meta.difficulty)
    }

    /// Whether passing an exercise archives a graded snapshot: local config
    /// override, else the course setting in `info.toml`.
    pub fn snapshots_enabled(&self) -> bool {
        self.config.snapshots.unwrap_or(self.info.meta.snapshots)
    }

    /// Where graded snapshots live (committed, so CI can re-run them).
    pub fn submissions_dir(&self) -> PathBuf {
        self.root.join("submissions")
    }

    pub fn rv6_dir(&self) -> PathBuf {
        self.root.join("rv6")
    }

    pub fn rv6_src(&self) -> PathBuf {
        self.rv6_dir().join("src")
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join(".oslings").join("state.toml")
    }

    /// Look up an exercise by exact name or by numeric prefix (e.g. "01").
    pub fn find(&self, query: &str) -> Option<&Exercise> {
        self.info
            .exercises
            .iter()
            .find(|e| e.name == query)
            .or_else(|| {
                self.info
                    .exercises
                    .iter()
                    .find(|e| e.name.starts_with(query))
            })
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.info.exercises.iter().position(|e| e.name == name)
    }
}

/// Split an exercise's `hints.md` into its `## Hint N` sections.
pub fn parse_hints(project: &Project, ex: &Exercise) -> Vec<String> {
    let path = project.root.join(&ex.path).join("hints.md");
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return vec![],
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
    hints
}

/// Copy an exercise's files from `<kind>/` (skeleton|solution) into rv6/src.
///
/// This is how the *cumulative* kernel advances: each exercise's skeleton
/// already contains the concepts taught by earlier exercises (with their
/// `IMPLEMENT` markers resolved), plus the new files for the current step.
pub fn stage_files(project: &Project, ex: &Exercise, kind: &str) -> Result<()> {
    let src_dir = project.root.join(&ex.path).join(kind);
    let dst_dir = project.rv6_src();
    fs::create_dir_all(&dst_dir)?;
    let difficulty = project.effective_difficulty();
    for file in &ex.files {
        let from = src_dir.join(file);
        let to = dst_dir.join(file);
        // Skeletons get their step-by-step `IMPLEMENT` guidance trimmed at
        // harder difficulties; solutions and `guided` are copied verbatim.
        if kind == "skeleton" && difficulty != Difficulty::Guided {
            let raw = fs::read_to_string(&from)
                .with_context(|| format!("reading {}", from.display()))?;
            fs::write(&to, strip_guidance(&raw, difficulty))
                .with_context(|| format!("writing {}", to.display()))?;
        } else {
            fs::copy(&from, &to)
                .with_context(|| format!("copying {} -> {}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

/// Trim the step-by-step `IMPLEMENT` guidance out of skeleton source to match a
/// difficulty. Only *comment* lines are ever changed, so the staged file still
/// compiles and still fails its test exactly as at `guided` — we remove hints,
/// never code.
///
/// A guidance block is an `// IMPLEMENT` / `/// IMPLEMENT` marker line plus the
/// run of comment lines immediately below it (the detailed steps and code
/// snippets). At `standard` the marker line (the one-line task) stays and the
/// detail is dropped; at `challenge` the whole block becomes a bare `TODO`.
/// Any description *above* the marker (e.g. a function's doc comment) is kept.
fn strip_guidance(content: &str, difficulty: Difficulty) -> String {
    if difficulty == Difficulty::Guided {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len());
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        if is_implement_marker(line) {
            match difficulty {
                Difficulty::Standard => {
                    // Keep the task line; drop the detail below it.
                    out.push_str(line);
                    out.push('\n');
                }
                Difficulty::Challenge => {
                    let (indent, slashes) = comment_prefix(line);
                    out.push_str(&format!(
                        "{indent}{slashes} TODO: implement this — read the lesson (`oslings lesson`).\n"
                    ));
                }
                Difficulty::Guided => unreachable!(),
            }
            // Swallow the contiguous comment lines that make up the detail.
            while lines.peek().is_some_and(|n| is_comment_line(n)) {
                lines.next();
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// A comment line whose text (after the slashes) begins with `IMPLEMENT`.
fn is_implement_marker(line: &str) -> bool {
    let t = line.trim_start();
    (t.starts_with("//")) && t.trim_start_matches('/').trim_start().starts_with("IMPLEMENT")
}

fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// The leading whitespace and the slash run (`//` or `///`) of a comment line.
fn comment_prefix(line: &str) -> (String, &'static str) {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let slashes = if line.trim_start().starts_with("///") {
        "///"
    } else {
        "//"
    };
    (indent, slashes)
}

/// Metadata written next to a graded snapshot. Informational only — the
/// authoritative grade comes from CI re-running the harness on the snapshot, so
/// a student editing this file cannot change their score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionMeta {
    pub exercise: String,
    pub passed_at: String,   // human-readable UTC
    pub passed_at_unix: u64, // sortable epoch seconds
    pub difficulty: String,
    pub hints_used: usize,
}

/// Read the metadata recorded when `name` was snapshotted, if it exists.
pub fn read_submission_meta(project: &Project, name: &str) -> Option<SubmissionMeta> {
    let path = project
        .submissions_dir()
        .join(name)
        .join("oslings-meta.toml");
    let raw = fs::read_to_string(path).ok()?;
    toml::from_str(&raw).ok()
}

/// Record that `ex` just passed: mark it complete and, when the course enables
/// snapshots, archive the current rv6/src solution into `submissions/<ex>/` for
/// grading. Call this instead of `State::mark_completed` at every pass site, so
/// the snapshot is taken BEFORE the next skeleton is staged over rv6/src.
pub fn record_pass(project: &Project, state: &mut State, ex: &Exercise) -> Result<()> {
    state.mark_completed(&ex.name);
    if project.snapshots_enabled() {
        snapshot_submission(project, state, ex)?;
    }
    Ok(())
}

fn snapshot_submission(project: &Project, state: &State, ex: &Exercise) -> Result<()> {
    let dst = project.submissions_dir().join(&ex.name);
    fs::create_dir_all(&dst)?;
    let src = project.rv6_src();
    for file in &ex.files {
        let from = src.join(file);
        let to = dst.join(file);
        fs::copy(&from, &to)
            .with_context(|| format!("snapshotting {} -> {}", from.display(), to.display()))?;
    }
    let secs = now_unix();
    let meta = SubmissionMeta {
        exercise: ex.name.clone(),
        passed_at: unix_to_utc(secs),
        passed_at_unix: secs,
        difficulty: project.effective_difficulty().as_str().to_string(),
        hints_used: state.hints.get(&ex.name).copied().unwrap_or(0),
    };
    let raw = toml::to_string_pretty(&meta).context("serializing submission metadata")?;
    fs::write(dst.join("oslings-meta.toml"), raw)?;
    Ok(())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Format epoch seconds as an RFC 3339 UTC string, without a date crate
/// (Howard Hinnant's `civil_from_days`).
fn unix_to_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u64, u64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Copy an exercise's files from an arbitrary directory into rv6/src verbatim.
/// Used by `oslings grade` to re-stage a committed submission before re-running
/// the harness against it.
pub fn stage_from_dir(project: &Project, ex: &Exercise, src_dir: &Path) -> Result<()> {
    let dst_dir = project.rv6_src();
    fs::create_dir_all(&dst_dir)?;
    for file in &ex.files {
        let from = src_dir.join(file);
        let to = dst_dir.join(file);
        fs::copy(&from, &to).with_context(|| {
            format!("staging submission {} -> {}", from.display(), to.display())
        })?;
    }
    Ok(())
}

fn find_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join("info.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Persisted learner progress.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// Name of the exercise the learner is currently on.
    #[serde(default)]
    pub current: Option<String>,
    /// Exercise names that have been passed.
    #[serde(default)]
    pub completed: Vec<String>,
    /// How many hints have been revealed per exercise.
    #[serde(default)]
    pub hints: BTreeMap<String, usize>,
}

impl State {
    pub fn load(project: &Project) -> Result<State> {
        let path = project.state_path();
        if !path.exists() {
            // First run: start at the first exercise.
            let mut s = State::default();
            s.current = project.info.exercises.first().map(|e| e.name.clone());
            return Ok(s);
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let state: State =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        Ok(state)
    }

    pub fn save(&self, project: &Project) -> Result<()> {
        let path = project.state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self).context("serializing state")?;
        fs::write(&path, raw).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    pub fn is_completed(&self, name: &str) -> bool {
        self.completed.iter().any(|c| c == name)
    }

    pub fn mark_completed(&mut self, name: &str) {
        if !self.is_completed(name) {
            self.completed.push(name.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const SAMPLE: &str = "\
/// Install `file` and return the fd, or -1 if full.
///
/// IMPLEMENT: scan the table for a free slot.
///     for fd in 0..NOFILE { ... }
///     -1
fn fdalloc(file: File) -> isize {
    // IMPLEMENT: find a free slot.
    //   1. scan ofile
    //   2. store and return the index
    let _ = file; // keep this stub line
    -1
}";

    #[test]
    fn guided_is_untouched() {
        assert_eq!(strip_guidance(SAMPLE, Difficulty::Guided), SAMPLE);
    }

    #[test]
    fn standard_keeps_task_line_drops_detail_and_code_survives() {
        let out = strip_guidance(SAMPLE, Difficulty::Standard);
        // description above the marker is kept
        assert!(out.contains("Install `file` and return the fd"));
        // the marker (task) line stays, both the doc and inline ones
        assert!(out.contains("/// IMPLEMENT: scan the table for a free slot."));
        assert!(out.contains("// IMPLEMENT: find a free slot."));
        // the step-by-step detail is gone
        assert!(!out.contains("for fd in 0..NOFILE"));
        assert!(!out.contains("1. scan ofile"));
        assert!(!out.contains("store and return the index"));
        // code (including the stub) is never touched
        assert!(out.contains("fn fdalloc(file: File) -> isize {"));
        assert!(out.contains("let _ = file; // keep this stub line"));
        assert!(out.contains("    -1\n}"));
    }

    #[test]
    fn challenge_collapses_to_todo() {
        let out = strip_guidance(SAMPLE, Difficulty::Challenge);
        assert!(out.contains("/// TODO: implement this"));
        assert!(out.contains("// TODO: implement this"));
        assert!(!out.contains("scan the table"));
        assert!(!out.contains("find a free slot"));
        // code still intact
        assert!(out.contains("let _ = file; // keep this stub line"));
        assert!(out.contains("fn fdalloc"));
    }

    #[test]
    fn understand_comments_are_not_stripped() {
        let src = "// UNDERSTAND: this is given context\nlet x = 1;\n";
        assert_eq!(strip_guidance(src, Difficulty::Challenge).trim_end(), src.trim_end());
    }

    #[test]
    fn timestamp_formats_utc() {
        assert_eq!(super::unix_to_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(super::unix_to_utc(1_600_000_000), "2020-09-13T12:26:40Z");
    }

    #[test]
    fn hint_cap_by_level() {
        assert_eq!(Difficulty::Guided.hint_cap(3), 3);
        assert_eq!(Difficulty::Standard.hint_cap(3), 2);
        assert_eq!(Difficulty::Challenge.hint_cap(3), 1);
        // never exceeds the number of hints that exist
        assert_eq!(Difficulty::Guided.hint_cap(1), 1);
        assert_eq!(Difficulty::Standard.hint_cap(1), 1);
        assert_eq!(Difficulty::Standard.hint_cap(0), 0);
    }

    #[test]
    fn difficulty_parse_and_roundtrip() {
        assert_eq!(Difficulty::parse("guided"), Some(Difficulty::Guided));
        assert_eq!(Difficulty::parse("STANDARD"), Some(Difficulty::Standard));
        assert_eq!(Difficulty::parse(" challenge "), Some(Difficulty::Challenge));
        assert_eq!(Difficulty::parse("bogus"), None);
        assert_eq!(Difficulty::Challenge.as_str(), "challenge");
    }

    #[test]
    fn strip_handles_multiple_blocks_and_keeps_code_between() {
        let src = "// IMPLEMENT: first.\n//   detail A\nlet a = 1;\n// IMPLEMENT: second.\n//   detail B\nlet b = 2;\n";
        let out = strip_guidance(src, Difficulty::Standard);
        assert!(out.contains("// IMPLEMENT: first."));
        assert!(out.contains("// IMPLEMENT: second."));
        assert!(!out.contains("detail A"));
        assert!(!out.contains("detail B"));
        assert!(out.contains("let a = 1;"));
        assert!(out.contains("let b = 2;"));
    }

    // ---- integration-style tests over a temp fixture (no QEMU needed) ----

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new() -> TempRoot {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("oslings-test-{}-{}", std::process::id(), n));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TempRoot(p)
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_file(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn make_project(root: PathBuf, snapshots: bool, difficulty: Difficulty) -> Project {
        Project {
            root,
            info: Info {
                meta: Meta {
                    pass_marker: "PASS".into(),
                    fail_marker: "FAIL".into(),
                    difficulty,
                    snapshots,
                    require_identity: false,
                },
                exercises: vec![Exercise {
                    name: "01_test".into(),
                    path: "exercises/01_test".into(),
                    mode: Mode::Build,
                    files: vec!["foo.rs".into()],
                    part: 1,
                    features: vec![],
                }],
            },
            config: Config::default(),
        }
    }

    const FIXTURE_SKEL: &str =
        "fn f() {\n    // IMPLEMENT: do the thing.\n    //   step one\n    //   step two\n    let _ = 0; // stub stays\n}\n";

    #[test]
    fn stage_files_guided_is_byte_identical() {
        let tr = TempRoot::new();
        write_file(&tr.0.join("exercises/01_test/skeleton/foo.rs"), FIXTURE_SKEL);
        let proj = make_project(tr.0.clone(), false, Difficulty::Guided);
        let ex = proj.info.exercises[0].clone();
        stage_files(&proj, &ex, "skeleton").unwrap();
        let staged = fs::read_to_string(tr.0.join("rv6/src/foo.rs")).unwrap();
        assert_eq!(staged, FIXTURE_SKEL);
    }

    #[test]
    fn stage_files_standard_and_challenge_strip_but_keep_code() {
        let tr = TempRoot::new();
        write_file(&tr.0.join("exercises/01_test/skeleton/foo.rs"), FIXTURE_SKEL);
        let ex = make_project(tr.0.clone(), false, Difficulty::Guided).info.exercises[0].clone();

        let std_proj = make_project(tr.0.clone(), false, Difficulty::Standard);
        stage_files(&std_proj, &ex, "skeleton").unwrap();
        let s = fs::read_to_string(tr.0.join("rv6/src/foo.rs")).unwrap();
        assert!(s.contains("// IMPLEMENT: do the thing."));
        assert!(!s.contains("step one"));
        assert!(s.contains("let _ = 0; // stub stays"));

        let chal = make_project(tr.0.clone(), false, Difficulty::Challenge);
        stage_files(&chal, &ex, "skeleton").unwrap();
        let c = fs::read_to_string(tr.0.join("rv6/src/foo.rs")).unwrap();
        assert!(c.contains("// TODO: implement this"));
        assert!(!c.contains("do the thing"));
        assert!(c.contains("let _ = 0; // stub stays"));
    }

    #[test]
    fn record_pass_snapshots_when_enabled() {
        let tr = TempRoot::new();
        write_file(&tr.0.join("rv6/src/foo.rs"), "solution code\n");
        let proj = make_project(tr.0.clone(), true, Difficulty::Guided);
        let ex = proj.info.exercises[0].clone();
        let mut state = State::default();
        record_pass(&proj, &mut state, &ex).unwrap();
        assert!(state.is_completed("01_test"));
        let snap = tr.0.join("submissions/01_test/foo.rs");
        assert!(snap.exists());
        assert_eq!(fs::read_to_string(&snap).unwrap(), "solution code\n");
        assert!(tr.0.join("submissions/01_test/oslings-meta.toml").exists());
    }

    #[test]
    fn record_pass_no_snapshot_when_disabled() {
        let tr = TempRoot::new();
        write_file(&tr.0.join("rv6/src/foo.rs"), "x\n");
        let proj = make_project(tr.0.clone(), false, Difficulty::Guided);
        let ex = proj.info.exercises[0].clone();
        let mut state = State::default();
        record_pass(&proj, &mut state, &ex).unwrap();
        assert!(state.is_completed("01_test"));
        assert!(!tr.0.join("submissions").exists());
    }
}
