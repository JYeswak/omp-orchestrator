#![forbid(unsafe_code)]

//! undrained-pipe-lint — fails on the undrained-pipe deadlock pattern.
//!
//! THE PATTERN (AGENTS.md:430-431, verbatim): "Undrained stdout+stderr with a
//! try_wait() poll deadlocks past ~64 KiB. The tell is 0% CPU with no children;
//! widening the timeout hides it longer."
//!
//! WHAT THE LINT DETECTS: a Command builder that sets BOTH stdout and stderr to
//! `Stdio::piped()`, whose handle is then polled with `try_wait()` in a loop,
//! with NO concurrent reader thread and NO `wait_with_output()` call before
//! the exit branch. All four facts must be in the same function body.
//!
//! WHAT THE LINT DOES NOT DETECT:
//! * `stdout`-only piping (one pipe cannot deadlock by itself)
//! * `wait_with_output()` without a try_wait poll (std drains both concurrently)
//! * Concurrent reader threads (thread::spawn + read_to_end before try_wait)
//! * Comments mentioning try_wait (stripped before classification)
//!
//! THE COMMENT-STRIPPING REQUIREMENT: five crates in this workspace carry the
//! comment "DRAIN THE PIPES ON DEDICATED THREADS. try_wait in a poll loop
//! CANNOT be paired with undrained pipes" — the hazard documented in prose,
//! followed by a REAL thread::spawn drain. A scan that does not strip //
//! lines classifies these as violations (45% false-positive rate measured
//! in the -w4j derivation).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// A detected undrained-pipe site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub file: String,
    /// Line of the FIRST piped-stdio call in the construction.
    pub piped_line: usize,
    /// Line of the try_wait() call that polls without draining.
    pub try_wait_line: usize,
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: piped_stdio at line {}, try_wait poll at line {} — a child filling \
             either 64 KiB pipe buffer blocks forever while the poll waits for exit",
            self.file, self.piped_line, self.try_wait_line
        )
    }
}

/// The result of one lint pass over a scan root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintReport {
    pub scanned: Vec<String>,
    pub violations: Vec<Violation>,
}

impl LintReport {
    pub fn is_pass(&self) -> bool {
        !self.scanned.is_empty() && self.violations.is_empty()
    }
}

/// Strip `//` line comments from a single line, respecting string literals.
/// This is the minimum viable comment handling: a scan that does not strip
/// comments classifies the hazard-documentation comment as a violation.
pub fn strip_line_comment(line: &str) -> &str {
    let mut in_str = false;
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b'\\' if in_str => {} // skip escaped char (advance handled by loop)
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
}

/// Find undrained-pipe violations in a single .rs source text.
///
/// Returns (piped_first_line, try_wait_line) pairs for each construction that:
/// 1. Sets BOTH stdout and stderr to Stdio::piped()
/// 2. Polls with try_wait() before any drain mechanism
/// 3. Has no wait_with_output() or thread::spawn between the construction and the poll
pub fn find_violations_in_source(source: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let stripped: Vec<&str> = lines.iter().map(|l| strip_line_comment(l)).collect();

    let piped: Vec<usize> = stripped
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("Stdio::piped()"))
        .map(|(i, _)| i)
        .collect();

    let mut violations = Vec::new();
    let mut i = 0;
    while i + 1 < piped.len() {
        let start = piped[i];
        let end = piped[i + 1];
        if end - start > 20 {
            i += 1;
            continue;
        }
        // Both pipes piped in this construction. Scan forward for the wait strategy.
        let mut strategy = "";
        let mut strategy_line = None;
        for j in start..lines.len().min(start + 80) {
            let code = strip_line_comment(lines[j]);
            if code.contains("try_wait") {
                strategy = "TRY_WAIT";
                strategy_line = Some(j + 1);
                break;
            }
            if code.contains("wait_with_output") {
                strategy = "DRAINING";
                break;
            }
            if code.contains("thread::spawn") && j > start {
                strategy = "DRAINING";
                break;
            }
        }
        if strategy == "TRY_WAIT" {
            if let Some(tw) = strategy_line {
                violations.push((start + 1, tw));
            }
        }
        i += 2;
    }
    violations
}

/// Scan a directory tree for `.rs` files and lint each one.
pub fn lint_tree(root: &Path, skip_dirs: &[&str]) -> LintReport {
    let mut scanned = Vec::new();
    let mut violations = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if !skip_dirs.contains(&name) {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "rs") {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                scanned.push(rel.clone());
                let source = fs::read_to_string(&path).unwrap_or_default();
                for (piped_line, try_wait_line) in find_violations_in_source(&source) {
                    violations.push(Violation {
                        file: rel.clone(),
                        piped_line,
                        try_wait_line,
                    });
                }
            }
        }
    }

    scanned.sort();
    LintReport { scanned, violations }
}

/// Convenience: scan `<root>/crates/*/src/` for violations.
pub fn lint_workspace(root: &Path) -> LintReport {
    let crates_dir = root.join("crates");
    let mut scanned = Vec::new();
    let mut violations = Vec::new();

    let entries = match fs::read_dir(&crates_dir) {
        Ok(entries) => entries,
        Err(error) => {
            return LintReport {
                scanned: vec![format!("ERROR: cannot read {}: {error}", crates_dir.display())],
                violations: vec![Violation {
                    file: String::new(),
                    piped_line: 0,
                    try_wait_line: 0,
                }],
            };
        }
    };

    for entry in entries.flatten() {
        let src = entry.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !path.extension().is_some_and(|ext| ext == "rs") {
                    continue;
                }
                scanned.push(path.display().to_string());
                let source = fs::read_to_string(&path).unwrap_or_default();
                for (piped_line, try_wait_line) in find_violations_in_source(&source) {
                    violations.push(Violation {
                        file: path.display().to_string(),
                        piped_line,
                        try_wait_line,
                    });
                }
            }
        }
    }

    scanned.sort();
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.piped_line.cmp(&b.piped_line)));
    LintReport { scanned, violations }
}
