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
    /// Line of the stdout Stdio::piped() call.
    pub piped_line: usize,
    /// Line of the stderr Stdio::piped() call.
    pub stderr_piped_line: usize,
    /// Line of the try_wait() call that polls without draining.
    pub try_wait_line: usize,
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: stdout piped at line {}, stderr piped at line {}, try_wait poll at line {} — a child filling either 64 KiB pipe buffer blocks forever",
            self.file, self.piped_line, self.stderr_piped_line, self.try_wait_line
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

/// Strip a line comment while respecting escaped quotes.
pub fn strip_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            return &line[..index];
        }
    }
    line
}

/// Mask comments and ordinary quoted strings without changing line count.
fn code_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut in_string = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            output.push(' ');
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            output.push(' ');
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            output.extend(std::iter::repeat(' ').take(bytes.len() - index));
            break;
        }
        output.push(byte as char);
        index += 1;
    }
    output
}

#[derive(Debug, Clone)]
struct FunctionRegion {
    name: String,
    start: usize,
    end: usize,
}

fn function_name(line: &str) -> Option<String> {
    let position = line.find("fn ")? + 3;
    let name: String = line[position..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn brace_delta(line: &str) -> i32 {
    line.bytes().fold(0, |delta, byte| match byte {
        b'{' => delta + 1,
        b'}' => delta - 1,
        _ => delta,
    })
}

fn function_regions(code: &[String]) -> Vec<FunctionRegion> {
    let mut regions = Vec::new();
    for (start, line) in code.iter().enumerate() {
        let Some(name) = function_name(line) else { continue };
        let mut depth = 0;
        let mut opened = false;
        let mut end = start;
        for (index, body) in code.iter().enumerate().skip(start) {
            depth += brace_delta(body);
            opened |= body.contains('{');
            if opened && depth <= 0 {
                end = index;
                break;
            }
        }
        if opened {
            regions.push(FunctionRegion { name, start, end });
        }
    }
    regions
}

fn piped_pair(code: &[String], region: &FunctionRegion) -> Option<(usize, usize)> {
    let mut stdout = None;
    let mut stderr = None;
    for index in region.start..=region.end {
        let line = &code[index];
        if line.contains("stdout") && line.contains("Stdio::piped()") {
            stdout = Some(index);
        }
        if line.contains("stderr") && line.contains("Stdio::piped()") {
            stderr = Some(index);
        }
    }
    match (stdout, stderr) {
        (Some(out), Some(err)) if out.abs_diff(err) <= 20 => Some((out, err)),
        _ => None,
    }
}

fn try_wait_line(code: &[String], region: &FunctionRegion) -> Option<usize> {
    (region.start..=region.end).find(|index| code[*index].contains("try_wait("))
}

fn drains_before_poll(code: &[String], start: usize, poll: usize) -> bool {
    let mut stdout_taken = false;
    let mut stderr_taken = false;
    let mut read_to_end = 0;
    let mut spawned_reader = false;
    for line in code.iter().take(poll + 1).skip(start) {
        stdout_taken |= line.contains("stdout.take");
        stderr_taken |= line.contains("stderr.take");
        read_to_end += usize::from(line.contains("read_to_end"));
        spawned_reader |= line.contains("thread::spawn");
        if line.contains("wait_with_output(") || line.contains("output_async(") {
            return true;
        }
    }
    stdout_taken && stderr_taken && read_to_end >= 2 && spawned_reader
}

/// Detailed violations: (stdout-piped line, stderr-piped line, try_wait line).
pub fn find_detailed_violations_in_source(source: &str) -> Vec<(usize, usize, usize)> {
    let code: Vec<String> = source.lines().map(code_line).collect();
    let regions = function_regions(&code);
    let mut violations = Vec::new();
    for region in &regions {
        let Some((stdout_line, stderr_line)) = piped_pair(&code, region) else { continue };
        if let Some(poll) = try_wait_line(&code, region) {
            if !drains_before_poll(&code, stdout_line.min(stderr_line), poll) {
                violations.push((stdout_line + 1, stderr_line + 1, poll + 1));
            }
            continue;
        }
        let body = code[region.start..=region.end].join("\n");
        for callee in &regions {
            if callee.name == region.name || !body.contains(&format!("{}(", callee.name)) {
                continue;
            }
            if let Some(poll) = try_wait_line(&code, callee) {
                if !drains_before_poll(&code, callee.start, poll) {
                    violations.push((stdout_line + 1, stderr_line + 1, poll + 1));
                }
            }
        }
    }
    violations.sort_unstable();
    violations.dedup();
    violations
}

/// Find violations while preserving the original two-field test helper API.
pub fn find_violations_in_source(source: &str) -> Vec<(usize, usize)> {
    find_detailed_violations_in_source(source)
        .into_iter()
        .map(|(stdout, _, poll)| (stdout, poll))
        .collect()
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
                for (piped_line, stderr_piped_line, try_wait_line) in
                    find_detailed_violations_in_source(&source)
                {
                    violations.push(Violation {
                        file: rel.clone(),
                        piped_line,
                        stderr_piped_line,
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
            // ANTI-VACUITY FIX (bead -w4j clause 6): the old code pushed a
            // sentinel INTO the scan set and a zero-line Violation — the CLI
            // then printed a phantom VIOLATION at exit 1 instead of the typed
            // empty-scan error at exit 3. The fix: return an EMPTY scan set so
            // the caller's empty-scan-set check fires with the typed exit code.
            eprintln!(
                "UNRAINED-PIPE-LINT ERROR: cannot read {}: {error}",
                crates_dir.display()
            );
            return LintReport {
                scanned: vec![],
                violations: vec![],
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
                for (piped_line, stderr_piped_line, try_wait_line) in
                    find_detailed_violations_in_source(&source)
                {
                    violations.push(Violation {
                        file: path.display().to_string(),
                        piped_line,
                        stderr_piped_line,
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
