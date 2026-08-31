//! The no-shell/no-python gate for this repository (bead omp-orchestrator-4ak).
//!
//! WHAT THIS MECHANICALLY ENFORCES — the floor, and no more:
//! every file in the git INDEX (`git ls-files`) whose final path component has
//! extension `sh` or `py` — matched ASCII case-insensitively — is a violation,
//! and the gate refuses it. The exemption list is EMPTY by design: an exception
//! would be a named row with a reason, never silence. control-plane accreted
//! 160 tracked shell scripts and 60,467 lines because nothing said no; this is
//! the thing that says no, and it lands before any crate is copied.
//!
//! WHAT STILL PASSES — do not read this gate as more than it is:
//! * RUNTIME SHELL-OUTS. A crate invoking `std::process::Command` with `bash`
//!   or `python` is untouched here. AGENTS.md names that a separate, unbuilt
//!   check; this gate covers FILE EXTENSIONS of tracked files, nothing else.
//! * OTHER EXTENSIONS. `.bash`, `.zsh`, `.pl`, `.rb`, or extensionless files
//!   pass. The refusal is `.sh`/`.py` because that is the accretion this
//!   fleet actually measured.
//! * UNTRACKED FILES. This scans the index, not the working tree. A `.sh`
//!   sitting on disk untracked is invisible until staged (C88: the tracked
//!   set and the on-disk set diverge in both directions — this gate answers
//!   the index question).
//!
//! The gate itself runs `git` via `std::process::Command`; that is the
//! mechanism of the gate, not a violation of the rule it enforces.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::Path;
use std::process::Command;

/// File extensions this repo refuses, matched against the FINAL path
/// component only (`scripts/deploy.sh` → `sh`; `notes.sh.txt` → `txt`, passes).
/// The exemption list is empty by design — there is deliberately no
/// `check.sh` carve-out of any kind.
pub const FORBIDDEN_EXTENSIONS: &[&str] = &["sh", "py"];

/// One tracked path the gate refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The tracked path as `git ls-files` reported it (repo-root-relative).
    pub path: String,
    /// The lowercased final-component extension that matched.
    pub extension: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.path, self.extension)
    }
}

/// Every way the gate can refuse to render a verdict. Fail closed: an error is
/// never a pass.
#[derive(Debug, Clone)]
pub enum GateError {
    /// `git ls-files` could not be spawned or exited nonzero (missing repo,
    /// broken git, unreadable index). The gate reports the failure instead of
    /// guessing clean.
    GitFailed(String),
    /// ANTI-VACUITY: the scan set was empty. A gate that scanned nothing
    /// reports identically to one that passed, so it is an error, never a pass.
    EmptyScanSet,
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateError::GitFailed(detail) => {
                write!(f, "git ls-files failed: {detail}")
            }
            GateError::EmptyScanSet => write!(
                f,
                "scan set is empty (git ls-files returned nothing): a gate that \
                 scanned nothing reports identically to one that passed, so it \
                 is an error, never a pass"
            ),
        }
    }
}

impl std::error::Error for GateError {}

/// The outcome of checking one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The scan set was non-empty and contained no `.sh`/`.py`.
    Clean,
    /// The scan set contained at least one refusal.
    Violations(Vec<Violation>),
}

/// Classify a single tracked path. Pure: no I/O, no scan-set policy.
///
/// A file named exactly `.sh` counts as extension `sh` (the leading dot is a
/// stem only by `Path` convention — a file whose whole name is `.sh` is
/// treated as what it almost certainly is).
pub fn violation_for(path: &str) -> Option<Violation> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some((_, ext)) = name.rsplit_once('.') else {
        return None;
    };
    let ext = ext.to_ascii_lowercase();
    if FORBIDDEN_EXTENSIONS
        .iter()
        .any(|forbidden| *forbidden == ext)
    {
        Some(Violation {
            path: path.to_owned(),
            extension: ext,
        })
    } else {
        None
    }
}

/// Classify a set of tracked paths. An EMPTY set is an error, never a pass —
/// anti-vacuity is enforced at this choke point so every caller inherits it.
pub fn scan(paths: &[String]) -> Result<Vec<Violation>, GateError> {
    if paths.is_empty() {
        return Err(GateError::EmptyScanSet);
    }
    Ok(paths.iter().filter_map(|p| violation_for(p)).collect())
}

/// Enumerate the tracked paths (the git index) at `repo_root`, NUL-separated
/// so paths containing spaces or newlines survive.
pub fn tracked_files(repo_root: &Path) -> Result<Vec<String>, GateError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| GateError::GitFailed(format!("spawn git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(GateError::GitFailed(format!(
            "exit {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Check a whole repository: enumerate the index, then classify it.
pub fn check_repo(repo_root: &Path) -> Result<Verdict, GateError> {
    let violations = scan(&tracked_files(repo_root)?)?;
    Ok(if violations.is_empty() {
        Verdict::Clean
    } else {
        Verdict::Violations(violations)
    })
}
