//! The repo-wide home-path-literal gate for this repository (bead omp-orchestrator-npq).
//!
//! WHAT THIS MECHANICALLY ENFORCES — the floor, and no more: every `.rs` file under
//! `<repo>/crates/*/src/` must contain zero occurrences of the current user's home-path
//! literal. A reintroduced literal — a new hardcoded checkout, a pasted fixture, a
//! `HOME` fallback — turns `cargo test` RED naming the file and line.
//!
//! WHY THIS GATE EXISTS: a hardcoded repository root COMPILES fine after a move and
//! then silently reads the WRONG repository — the failure is a wrong answer, not an
//! error. -7ai killed the literals in the three ported crates; this gate is -7ai
//! acceptance #1 ("zero occurrences ... anywhere in crates/*/src") enforced where it
//! actually matters: on the tree the extraction copies INTO (omp-orchestrator-815's
//! mechanical copy rule refuses a dirty crate, and this gate catches one that slips
//! past).
//!
//! WHAT STILL PASSES — do not read this gate as more than it is:
//! * NON-`.rs` FILES. Cargo.toml, fixtures, cron/registry TOML are invisible here
//!   (npq acceptance #7 names those out of scope and unmeasured).
//! * NON-SRC `.rs`. `tests/`, `benches/`, `build.rs` are outside the scan set by the
//!   bead's own wording (`crates/*/src`).
//! * OTHER MACHINES' HOMES. The needle is this fleet's home literal; a different
//!   user's path elsewhere is not caught until it runs on this fleet.
//! * THE SCAN IS THE WORKING TREE. Untracked files under crates/*/src are scanned;
//!   files elsewhere are not.
//!
//! The needle is built by `concat!` so this gate's own source never contains the
//! contiguous literal it exists to forbid — the gate must not catch its own needle.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The home-path literal this gate forbids.
pub const USER_HOME_LITERAL: &str = concat!("/Users/", "josh");

/// One forbidden-literal occurrence, named by file and line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Path of the offending file, relative to the scan root when possible.
    pub file: PathBuf,
    /// 1-indexed line number of the occurrence.
    pub line: usize,
}

impl fmt::Display for Hit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.file.display(), self.line)
    }
}

/// The result of one repo-wide scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    /// Every `.rs` file under `<root>/crates/*/src/`, in sorted order — printed with
    /// the verdict so a reader can see the scan set and never mistake a vacuous pass
    /// for coverage.
    pub scanned: Vec<PathBuf>,
    /// Every forbidden-literal occurrence found, named by file and line.
    pub hits: Vec<Hit>,
}

impl ScanReport {
    /// The gate's verdict: green only when the scan set was non-empty AND zero hits.
    pub fn is_pass(&self) -> bool {
        !self.scanned.is_empty() && self.hits.is_empty()
    }
}

/// This repository's root, derived from this crate's manifest (`<repo>/crates/<crate>`,
/// two `ancestors` up) — never a constant, so the gate moves with the repo.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the repository root")
        .to_path_buf()
}

/// Scan `<root>/crates/*/src/` recursively for `.rs` files carrying the home literal.
///
/// An unreadable directory is a panic, not a skip: a scan that silently skipped a
/// directory reports identically to one that covered it (anti-vacuity, C88).
pub fn scan(root: &Path) -> ScanReport {
    let crates_dir = root.join("crates");
    let mut scanned = Vec::new();
    let mut hits = Vec::new();
    let mut stack = Vec::new();

    // A MISSING crates dir is the empty scan set (an ERROR at the gate level, never a
    // pass); any other read failure is a panic, because a scan that silently skipped a
    // directory reports identically to one that covered it (anti-vacuity, C88).
    let entries = match fs::read_dir(&crates_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return ScanReport { scanned, hits },
        Err(error) => panic!("cannot read {}: {error}", crates_dir.display()),
    };
    for entry in entries.flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            stack.push(src);
        }
    }

    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.extension().is_some_and(|extension| extension == "rs") {
                continue;
            }
            scanned.push(path.clone());
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            for (index, line) in text.lines().enumerate() {
                if line.contains(USER_HOME_LITERAL) {
                    hits.push(Hit {
                        file: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                        line: index + 1,
                    });
                }
            }
        }
    }

    scanned.sort();
    ScanReport { scanned, hits }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scanner finds a planted literal in a fake `crates/*/src` tree.
    #[test]
    fn scanner_names_file_and_line_of_a_planted_literal() {
        let root = std::env::temp_dir().join(format!("plg-selftest-{}", std::process::id()));
        let src = root.join("crates/example/src");
        fs::create_dir_all(&src).expect("create fixture tree");
        fs::write(src.join("lib.rs"), "fn main() {}\n").expect("write clean file");
        let dirty = src.join("dirty.rs");
        fs::write(&dirty, "const X: &str = \"placeholder\";\n").expect("write dirty file");
        // Plant the literal the way a real reintroduction would: contiguous in the text.
        let planted = format!("const REPO: &str = \"{}\";\n", USER_HOME_LITERAL);
        fs::write(&dirty, planted).expect("write planted file");

        let report = scan(&root);
        assert_eq!(report.scanned.len(), 2, "scan set: {:?}", report.scanned);
        assert_eq!(report.hits.len(), 1, "planted literal must be caught: {:?}", report.hits);
        let hit = &report.hits[0];
        assert!(hit.file.ends_with("dirty.rs"), "wrong file: {}", hit.file.display());
        assert_eq!(hit.line, 1, "wrong line: {hit:?}");

        let _ = fs::remove_dir_all(&root);
    }

    /// The gate is not vacuous: a root with no `crates/*/src` trees cannot pass.
    #[test]
    fn empty_scan_set_is_not_a_pass() {
        let root = std::env::temp_dir().join(format!("plg-empty-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create empty root");
        let report = scan(&root);
        assert!(report.scanned.is_empty(), "expected an empty scan set");
        assert!(!report.is_pass(), "an empty scan set is an ERROR, never a pass");
        let _ = fs::remove_dir_all(&root);
    }
}
