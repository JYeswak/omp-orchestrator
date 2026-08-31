#![forbid(unsafe_code)]

//! kernel-bypass-gate — detects raw invocations that duplicate an existing kernel.
//!
//! THE MEASURED INDICTMENT (all pane 1's, all tonight):
//!   observe  — hand-grepped tmux capture-pane for 12h while tick-monitor observe was
//!              installed and returned MORE (state, timer, liveness, session scoping).
//!   dispatch — raw tmux send-keys while five dispatch crates were cron-scheduled.
//!   receipt  — grep -oE on a timer while the receiver-receipt crate existed.
//!   file     — raw br create while crates/finding existed (written 30 min earlier).
//!   queue    — br ready piped to python while bv --robot-triage reported scores.
//!
//! THE MECHANISM: when a kernel was broken, the route was around it instead of through
//! it. Every handroll is locally cheaper and removes exactly the pressure that would
//! have fixed the kernel. That is why the kernels stay broken.
//!
//! WHAT THIS GATE DETECTS: a tracked .rs source file invoking a kernel's raw interface
//! from OUTSIDE the kernel crate that owns it. The kernel registry is DECLARED — a const
//! in this file — not inferred, so adding a kernel requires adding its crate to the
//! allowlist and the gate enforces the declaration.
//!
//! THE LIMIT, stated in the gate's own output: this gate scans COMMITTED SOURCE ONLY.
//! It cannot see an operator handrolling in a shell, which is how five kernels were
//! bypassed tonight. The operator half needs a PreToolUse hook and is a separate bead.

use std::fmt;
use std::path::Path;

/// The kernel registry: maps a raw invocation pattern to the kernel that should
/// have been used. Each entry is (pattern, kernel_name, owning_crate).
///
/// The owning_crate is the ALLOWLIST: if the pattern appears in a file under
/// that crate's directory, it is a legitimate kernel-internal call and is NOT
/// a violation. A pattern in any OTHER crate is a bypass.
///
/// DECLARED, NOT INFERRED: adding a kernel requires adding its crate here.
pub const KERNEL_REGISTRY: &[(&str, &str, &str)] = &[
    ("tmux capture-pane", "tick-monitor observe", "tick-monitor"),
    ("tmux send-keys", "dispatch robot-send", "tick-monitor"),
    ("robot-send", "dispatch robot-send", "tick-monitor"),
    ("br ready", "loop-queue-filter queue", "loop-queue-filter"),
    ("br create", "beads-workflow bead filing", "omp-orchestrator"),
    ("Command::new(\"tmux\")", "tick-monitor pane access", "tick-monitor"),
    ("Command::new(\"ntm\")", "dispatch robot-send", "tick-monitor"),
    ("Command::new(\"br\")", "beads-workflow bead filing", "omp-orchestrator"),
];

/// A detected kernel bypass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bypass {
    pub file: String,
    pub line: usize,
    pub pattern: String,
    pub kernel: String,
}

impl fmt::Display for Bypass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{} uses raw \"{}\" — use the {} kernel instead",
            self.file, self.line, self.pattern, self.kernel
        )
    }
}

/// The result of one lint pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReport {
    pub scanned: Vec<String>,
    pub violations: Vec<Bypass>,
}

impl GateReport {
    pub fn is_pass(&self) -> bool {
        !self.scanned.is_empty() && self.violations.is_empty()
    }
}

/// Strip `//` line comments from a single line, respecting string literals.
pub fn strip_line_comment(line: &str) -> &str {
    let mut in_str = false;
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b'\\' if in_str => {}
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
}

/// Determine which crate a file belongs to, from `crates/<name>/src/...`.
pub fn owning_crate(path: &str) -> Option<String> {
    let components: Vec<&str> = path.split('/').collect();
    if components.len() >= 2 && components[0] == "crates" {
        Some(components[1].to_owned())
    } else {
        None
    }
}

/// Check a single file's source text for kernel bypasses.
///
/// Returns violations for lines that match a kernel's raw pattern where the
/// file's owning crate is NOT the kernel's owning crate (the allowlist).
/// Comment lines are stripped before matching.
pub fn lint_source(file: &str, source: &str) -> Vec<Bypass> {
    let crate_name = owning_crate(file);
    let mut violations = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let stripped = strip_line_comment(line);
        for (pattern, kernel, owning) in KERNEL_REGISTRY {
            if stripped.contains(pattern) {
                let is_owning = crate_name.as_deref() == Some(*owning);
                if !is_owning {
                    violations.push(Bypass {
                        file: file.to_owned(),
                        line: index + 1,
                        pattern: pattern.to_string(),
                        kernel: kernel.to_string(),
                    });
                }
            }
        }
    }
    violations
}

/// Scan a directory tree recursively for `.rs` files and lint each one.
pub fn lint_tree(root: &Path, skip_dirs: &[&str]) -> GateReport {
    let mut scanned = Vec::new();
    let mut violations = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
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
            if !path.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            scanned.push(rel.clone());
            let source = std::fs::read_to_string(&path).unwrap_or_default();
            for bypass in lint_source(&rel, &source) {
                violations.push(bypass);
            }
        }
    }

    scanned.sort();
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    GateReport { scanned, violations }
}

/// Scan `<root>/crates/*/src/` — the workspace variant for bead -024's gate.
pub fn lint_workspace(root: &Path) -> GateReport {
    let crates_dir = root.join("crates");
    let mut scanned = Vec::new();
    let mut violations = Vec::new();

    let entries = match std::fs::read_dir(&crates_dir) {
        Ok(entries) => entries,
        Err(_) => {
            return GateReport {
                scanned: vec![format!(
                    "ERROR: cannot read {}: the scan set is empty",
                    crates_dir.display()
                )],
                violations: vec![Bypass {
                    file: String::new(),
                    line: 0,
                    pattern: String::new(),
                    kernel: String::new(),
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
            let entries = match std::fs::read_dir(&dir) {
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
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                for (line_num, pattern, kernel) in lint_source_lines(&source) {
                    violations.push(Bypass {
                        file: path.display().to_string(),
                        line: line_num,
                        pattern,
                        kernel,
                    });
                }
            }
        }
    }

    scanned.sort();
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    GateReport { scanned, violations }
}

fn owning_crate_from_file(path: &Path) -> Option<String> {
    let components: Vec<_> = path.components().collect();
    for (i, comp) in components.iter().enumerate() {
        if comp.as_os_str() == "crates" {
            if let Some(next) = components.get(i + 1) {
                return Some(next.as_os_str().to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn lint_source_lines(source: &str) -> Vec<(usize, String, String)> {
    let stripped_lines: Vec<&str> = source
        .lines()
        .map(|l| strip_line_comment(l))
        .collect();
    let mut out = Vec::new();
    for (index, line) in stripped_lines.iter().enumerate() {
        for (pattern, kernel, _owning) in KERNEL_REGISTRY {
            if line.contains::<&str>(pattern.as_ref()) {
                out.push((index + 1, pattern.to_string(), kernel.to_string()));
            }
        }
    }
    out
}
