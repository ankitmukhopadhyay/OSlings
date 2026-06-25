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
}

#[derive(Debug, Clone, Deserialize)]
pub struct Info {
    pub meta: Meta,
    pub exercises: Vec<Exercise>,
}

/// Everything the commands need: where the project lives and its config.
#[derive(Clone)]
pub struct Project {
    pub root: PathBuf,
    pub info: Info,
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
        Ok(Project { root, info })
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
    for file in &ex.files {
        let from = src_dir.join(file);
        let to = dst_dir.join(file);
        fs::copy(&from, &to)
            .with_context(|| format!("copying {} -> {}", from.display(), to.display()))?;
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
