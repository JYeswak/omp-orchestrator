//! DEF-* definition uniqueness across `docs/plan` and `docs/planning`.
//!
//! Bead `omp-orchestrator-def-id-namespace-collision-vjfo`. Atlas Arc R2 forbids
//! duplicate IDs for restatements; `DEF-*` is the review-defect kind, so a colliding
//! definition line makes the file — not the id — the actual identifier.
//!
//! WHAT THIS MECHANICALLY ENFORCES: every `id = "DEF-*"` line under those two trees
//! names a unique string. A duplicate is a refusal that prints `DEF-001 defined 7x`.
//!
//! WHAT STILL PASSES: prose mentions, markdown tables, and `id = "NOTE-*"` rows.
//! This parser keys on the TOML definition line only — the same pattern the bead
//! measured.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PLAN_TREE: &str = "docs/plan";
const PLANNING_TREE: &str = "docs/planning";

/// One `id = "DEF-*"` definition site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub path: PathBuf,
    pub line: usize,
    pub id: String,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} {}", self.path.display(), self.line, self.id)
    }
}

/// Fail-closed scan errors. An empty or unreadable set is never a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    TreeMissing {
        tree: &'static str,
    },
    Unreadable {
        path: PathBuf,
        detail: String,
    },
    EmptyScanSet,
    ZeroDefinitions,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::TreeMissing { tree } => {
                write!(f, "DEF_ID_SCAN_EMPTY tree `{tree}` is missing")
            }
            ScanError::Unreadable { path, detail } => {
                write!(
                    f,
                    "DEF_ID_UNREADABLE path={} detail={detail}",
                    path.display()
                )
            }
            ScanError::EmptyScanSet => {
                write!(
                    f,
                    "DEF_ID_SCAN_EMPTY: no files under {PLAN_TREE} and {PLANNING_TREE}"
                )
            }
            ScanError::ZeroDefinitions => {
                write!(
                    f,
                    "DEF_ID_SCAN_EMPTY: files were read but no `id = \"DEF-*\"` definition was found"
                )
            }
        }
    }
}

impl std::error::Error for ScanError {}

/// One scan of both trees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    pub files_scanned: usize,
    pub by_id: BTreeMap<String, Vec<Site>>,
}

impl Scan {
    pub fn definition_count(&self) -> usize {
        self.by_id.values().map(Vec::len).sum()
    }

    /// Ids defined more than once, with their site counts, sorted by id.
    pub fn duplicates(&self) -> Vec<(String, usize)> {
        self.by_id
            .iter()
            .filter(|(_, sites)| sites.len() > 1)
            .map(|(id, sites)| (id.clone(), sites.len()))
            .collect()
    }

    /// Bead falsifier form: `DEF-001 defined 7x`.
    pub fn duplicate_messages(&self) -> Vec<String> {
        self.duplicates()
            .into_iter()
            .map(|(id, n)| format!("{id} defined {n}x"))
            .collect()
    }
}

/// Parse a TOML definition line `id = "DEF-…"`. Comments and other id families
/// return None. Leading whitespace is ignored so indented table rows still match.
pub fn parse_def_id_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let rest = trimmed.strip_prefix("id")?;
    if !rest.starts_with(|c: char| c == '=' || c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let id = &rest[..end];
    if id.starts_with("DEF-") && id.len() > 4 {
        Some(id)
    } else {
        None
    }
}

fn read_dir_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let entries = fs::read_dir(&cur)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Scan `docs/plan` and `docs/planning` under `root` for `id = "DEF-*"` lines.
pub fn scan(root: &Path) -> Result<Scan, ScanError> {
    let plan = root.join(PLAN_TREE);
    let planning = root.join(PLANNING_TREE);
    if !plan.is_dir() {
        return Err(ScanError::TreeMissing { tree: PLAN_TREE });
    }
    if !planning.is_dir() {
        return Err(ScanError::TreeMissing {
            tree: PLANNING_TREE,
        });
    }

    let mut files = Vec::new();
    read_dir_files(&plan, &mut files).map_err(|e| ScanError::Unreadable {
        path: plan.clone(),
        detail: e.to_string(),
    })?;
    read_dir_files(&planning, &mut files).map_err(|e| ScanError::Unreadable {
        path: planning.clone(),
        detail: e.to_string(),
    })?;
    files.sort();
    if files.is_empty() {
        return Err(ScanError::EmptyScanSet);
    }

    let mut by_id: BTreeMap<String, Vec<Site>> = BTreeMap::new();
    for path in &files {
        let text = fs::read_to_string(path).map_err(|e| ScanError::Unreadable {
            path: path.clone(),
            detail: e.to_string(),
        })?;
        let rel = path.strip_prefix(root).unwrap_or(path);
        for (idx, line) in text.lines().enumerate() {
            if let Some(id) = parse_def_id_line(line) {
                by_id.entry(id.to_string()).or_default().push(Site {
                    path: rel.to_path_buf(),
                    line: idx + 1,
                    id: id.to_string(),
                });
            }
        }
    }
    if by_id.is_empty() {
        return Err(ScanError::ZeroDefinitions);
    }
    Ok(Scan {
        files_scanned: files.len(),
        by_id,
    })
}
