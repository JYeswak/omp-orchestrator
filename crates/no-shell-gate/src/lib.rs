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

/// Bound for hook-context git index reads (ls-files/diff). AmberGate's
/// contract: a wedged git fails the gate CLOSED (typed, exit 3 class),
/// never hangs the hook, never reads as a clean scan.
const GIT_READ_DEADLINE_SECS: u64 = 10;

/// Bound for the workspace-load probe. Reads EVERY member manifest on a
/// cold target dir; generous on purpose, bounded on principle.
const CARGO_METADATA_DEADLINE_SECS: u64 = 300;

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
    // Bounded: this gate runs inside the commit hook; a wedged git must
    // fail CLOSED as a typed gate error (exit 3 class), never hang the
    // hook and never read as a clean scan.
    let mut git_command = Command::new("git");
    git_command.arg("-C").arg(repo_root).args(["ls-files", "-z"]);
    let output = match subprocess_contract::bounded_output(
        &mut git_command,
        std::time::Duration::from_secs(GIT_READ_DEADLINE_SECS),
    ) {
        subprocess_contract::BoundedOutcome::Completed(output) => output,
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(GateError::GitFailed(
                "git ls-files exceeded deadline; group killed".to_owned(),
            ));
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(GateError::GitFailed(format!("spawn git ls-files: {error}")));
        }
    };
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

// ---------------------------------------------------------------------------
// Workspace load gate (bead omp-orchestrator-workspace-load-gate-of3).
//
// THE DEFECT THIS CLOSES: a workspace member whose manifest cannot load makes
// `cargo test` exit nonzero at PARSE time with no test result line — the
// no-shell gate never runs, and a nonzero exit is indistinguishable from the
// gate refusing a violation. The anti-vacuity rule applied upstream of the
// scan: the scan set is not merely possibly empty, it can be UNREACHABLE, and
// an unreachable scan must report a TYPED, NAMED outcome — never a pass, and
// never a bare exit code.
// ---------------------------------------------------------------------------

/// What one workspace-load probe found, with a named detector per outcome so a
/// harness can assert WHICH case fired instead of a bare nonzero exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceLoad {
    /// `cargo metadata` parsed the workspace; `members` lists every crate
    /// directory under `crates/` that carries a manifest (the scan set).
    Loaded { members: Vec<String> },
    /// No root `Cargo.toml` at the given root.
    ManifestMissing { path: String },
    /// `cargo metadata` refused the workspace; `manifest` names the offending
    /// member manifest extracted from cargo's own error output, `detail` is
    /// that error text.
    MemberUnreadable { manifest: String, detail: String },
    /// The workspace loaded but enumerates ZERO member manifests — a scan set
    /// that can never contain a violation. Anti-vacuity: an error, not a pass.
    MembersEmpty,
}

impl WorkspaceLoad {
    /// The specific detector name for this outcome. A harness asserts THIS, not
    /// a bare exit code, so "the guard fired" is distinguishable from "the
    /// check could not start".
    pub fn detector(&self) -> &'static str {
        match self {
            WorkspaceLoad::Loaded { .. } => "WORKSPACE_LOADED",
            WorkspaceLoad::ManifestMissing { .. } => "WORKSPACE_MANIFEST_MISSING",
            WorkspaceLoad::MemberUnreadable { .. } => "WORKSPACE_MEMBER_UNREADABLE",
            WorkspaceLoad::MembersEmpty => "WORKSPACE_MEMBERS_EMPTY",
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, WorkspaceLoad::Loaded { .. })
    }
}

/// Run `cargo metadata --no-deps` against `manifest`, bounded, both pipes
/// drained. Returns (exit code, stderr).
fn cargo_metadata(manifest: &Path) -> (Option<i32>, String) {
    let mut cargo_command = Command::new("cargo");
    cargo_command.args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--manifest-path",
    ]);
    cargo_command.arg(manifest);
    match subprocess_contract::bounded_output(
        &mut cargo_command,
        std::time::Duration::from_secs(CARGO_METADATA_DEADLINE_SECS),
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) => (
            out.status.code(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stderr),
                String::from_utf8_lossy(&out.stdout)
            ),
        ),
        subprocess_contract::BoundedOutcome::TimedOut => (
            None,
            "cargo metadata exceeded deadline; group killed".to_owned(),
        ),
        subprocess_contract::BoundedOutcome::Unspawned(err) => (
            None,
            format!("cargo metadata could not be spawned: {err}"),
        ),
    }
}

/// Probe whether the workspace at `repo_root` can LOAD at all — upstream of
/// every gate scan, because a gate that cannot run reports like a gate that
/// passed unless the load is checked first. `cargo metadata --no-deps` reads
/// the root manifest and EVERY member manifest, which is exactly the surface
/// extraction mutates.
pub fn check_workspace_load(repo_root: &Path) -> WorkspaceLoad {
    let manifest = repo_root.join("Cargo.toml");
    if !manifest.exists() {
        return WorkspaceLoad::ManifestMissing {
            path: manifest.display().to_string(),
        };
    }
    let (code, output) = cargo_metadata(&manifest);
    if code != Some(0) {
        let detail: String = output
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(6)
            .collect::<Vec<_>>()
            .join("\n");
        return WorkspaceLoad::MemberUnreadable {
            manifest: offending_manifest(&output, repo_root),
            detail,
        };
    }
    let mut members = Vec::new();
    if let Ok(entries) = std::fs::read_dir(repo_root.join("crates")) {
        for entry in entries.flatten() {
            if entry.path().join("Cargo.toml").is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    members.push(name.to_string());
                }
            }
        }
    }
    members.sort();
    if members.is_empty() {
        return WorkspaceLoad::MembersEmpty;
    }
    WorkspaceLoad::Loaded { members }
}

/// The last `Cargo.toml` path mentioned in cargo's error output — cargo names
/// the offending manifest in its error text and location pointer, and the
/// DEEPEST mention (the final caused-by line) is the actual broken file. The
/// path may be relative to the manifest directory (`../…/member/Cargo.toml`)
/// and may carry a `:line:col` suffix; both are handled here. Fallback when
/// nothing parseable remains: the root manifest itself.
fn offending_manifest(detail: &str, root: &Path) -> String {
    let mut found: Option<String> = None;
    for line in detail.lines() {
        for token in line.split_whitespace() {
            let bare = token.split(':').next().unwrap_or(token);
            if bare.ends_with("Cargo.toml") {
                found = Some(bare.to_string());
            }
        }
    }
    found.unwrap_or_else(|| root.join("Cargo.toml").display().to_string())
}
