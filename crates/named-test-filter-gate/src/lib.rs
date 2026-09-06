#![forbid(unsafe_code)]

//! A missing `cargo test` filter name exits 0 with `0 passed; N filtered out`.
//! That is a vacuous pass. Grade the tally, not the exit code.
//!
//! `--exact` does not help: cargo still exits 0 when the filter matches nothing.
//! This crate parses `N passed` and requires N >= 1.
//! Unparseable output is a named error, never a pass.

use input_manifest::{CargoInputBound, CargoTestResult, InputManifest, ManifestError};
use serde_json::Value;
use std::fmt;
use std::path::Path;

const RESULT_MARKER: &str = concat!("test ", "result:");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub passed: u64,
    pub filtered: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradeError {
    Unparseable,
    Vacuous { passed: u64, filtered: u64 },
    CargoFailed { exit: i32, passed: u64 },
    ManifestRejected { state: String, detail: String },
}

impl fmt::Display for GradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unparseable => write!(
                f,
                "NAMED_TEST_UNPARSEABLE: no result line — not a pass"
            ),
            Self::Vacuous { passed, filtered } => write!(
                f,
                "NAMED_TEST_VACUOUS: {passed} passed, {filtered} filtered out — a missing name exits 0"
            ),
            Self::CargoFailed { exit, passed } => {
                write!(f, "NAMED_TEST_CARGO_FAILED: exit={exit} passed={passed}")
            }
            Self::ManifestRejected { state, detail } => {
                write!(f, "NAMED_TEST_MANIFEST_REJECTED: state={state} detail={detail}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grade {
    Admit {
        passed: u64,
        manifest: InputManifest,
    },
    Refuse {
        error: GradeError,
        manifest: InputManifest,
    },
}

impl Grade {
    pub fn is_admit(&self) -> bool {
        matches!(self, Self::Admit { .. })
    }

    pub fn manifest(&self) -> &InputManifest {
        match self {
            Self::Admit { manifest, .. } | Self::Refuse { manifest, .. } => manifest,
        }
    }

    pub fn acceptance_evidence(&self) -> Result<u64, GradeError> {
        match self {
            Self::Admit { passed, manifest } => manifest
                .require_full()
                .map(|()| *passed)
                .map_err(manifest_rejection),
            Self::Refuse { error, .. } => Err(error.clone()),
        }
    }
}

pub fn parse_tally(output: &str) -> Result<Tally, GradeError> {
    let mut found = false;
    let mut passed = 0u64;
    let mut filtered = 0u64;
    for line in output.lines() {
        if let Some(tally) = parse_result_line(line) {
            found = true;
            passed = passed.saturating_add(tally.passed);
            filtered = filtered.saturating_add(tally.filtered);
        }
    }
    if !found {
        return Err(GradeError::Unparseable);
    }
    Ok(Tally { passed, filtered })
}

fn parse_result_line(line: &str) -> Option<Tally> {
    let rest = line.trim().strip_prefix(RESULT_MARKER)?.trim();
    let passed = number_before(rest, " passed")?;
    let filtered = number_before(rest, " filtered").unwrap_or(0);
    Some(Tally { passed, filtered })
}

fn number_before(haystack: &str, needle: &str) -> Option<u64> {
    let end = haystack.find(needle)?;
    let prefix = &haystack[..end];
    let digits = prefix.rsplit(|c: char| !c.is_ascii_digit()).next()?;
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Grade cargo test output. Exit 0 with 0 passed is refuse. The default
/// path consumes every target line and carries FULL.
pub fn grade(output: &str, exit: i32) -> Grade {
    grade_with_bound(output, exit, CargoInputBound::All)
}

/// Grade a deliberately bounded extraction. Dropped target lines are declared
/// PARTIAL and cannot become acceptance evidence.
pub fn grade_with_bound(output: &str, exit: i32, bound: CargoInputBound) -> Grade {
    let result = match CargoTestResult::extract(output, bound, "cargo test") {
        Ok(result) => result,
        Err(error) => {
            let manifest =
                InputManifest::refused(error.to_string()).expect("extraction refusal has a reason");
            return Grade::Refuse {
                error: GradeError::Unparseable,
                manifest,
            };
        }
    };
    let passed = result.passed();
    let manifest = result.manifest;
    if passed == 0 {
        Grade::Refuse {
            error: GradeError::Vacuous {
                passed,
                filtered: result.targets.iter().map(|target| target.filtered).sum(),
            },
            manifest,
        }
    } else if exit != 0 {
        Grade::Refuse {
            error: GradeError::CargoFailed { exit, passed },
            manifest,
        }
    } else {
        Grade::Admit { passed, manifest }
    }
}

fn manifest_rejection(error: ManifestError) -> GradeError {
    match error {
        ManifestError::NonCitable { state, detail } => {
            GradeError::ManifestRejected { state, detail }
        }
        other => GradeError::ManifestRejected {
            state: "REFUSED".to_owned(),
            detail: other.to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTestRef {
    pub bead_id: String,
    pub test_fn: String,
    pub resolved: bool,
}

/// Extract `cargo test --test <target> <fn>` and `foo.rs::<fn>` names from bead text.
pub fn named_tests_in(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for token in text.split_whitespace() {
        if let Some((_, fn_name)) = token.split_once("rs::") {
            push_ident(&mut names, fn_name);
        }
    }
    let bytes = text.as_bytes();
    let needle = b"--test";
    let mut i = 0;
    while i + needle.len() < bytes.len() {
        if bytes[i..].starts_with(needle) {
            let after = &text[i + needle.len()..];
            let mut parts = after.split_whitespace();
            let _target = parts.next();
            if let Some(fn_name) = parts.next() {
                if !fn_name.starts_with('-') {
                    push_ident(&mut names, fn_name);
                }
            }
        }
        i += 1;
    }
    names.sort();
    names.dedup();
    names
}

fn push_ident(names: &mut Vec<String>, raw: &str) {
    let ident: String = raw
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let starts_lower = ident
        .chars()
        .next()
        .map(|c| c.is_ascii_lowercase())
        .unwrap_or(false);
    if starts_lower && ident.contains('_') {
        names.push(ident);
    }
}

pub fn census_beads(jsonl: &str, implemented: &[String]) -> Result<Vec<NamedTestRef>, GradeError> {
    if jsonl.trim().is_empty() {
        return Err(GradeError::Unparseable);
    }
    let implemented: std::collections::BTreeSet<&str> =
        implemented.iter().map(String::as_str).collect();
    let mut rows = Vec::new();
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|_| GradeError::Unparseable)?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if id.is_empty() {
            continue;
        }
        let mut blob = String::new();
        for key in ["acceptance_criteria", "description", "title"] {
            if let Some(text) = value.get(key).and_then(Value::as_str) {
                blob.push_str(text);
                blob.push('\n');
            }
        }
        for test_fn in named_tests_in(&blob) {
            let resolved = implemented.contains(test_fn.as_str());
            rows.push(NamedTestRef {
                bead_id: id.clone(),
                test_fn,
                resolved,
            });
        }
    }
    if rows.is_empty() {
        return Err(GradeError::Unparseable);
    }
    Ok(rows)
}

/// Collect `fn <name>` identifiers under each crate's `tests/` tree. Does not invent tests.
pub fn implemented_test_fns(crates_root: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    let entries = match std::fs::read_dir(crates_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(err) => return Err(err),
    };
    for entry in entries {
        walk_rs(&entry?.path().join("tests"), &mut names)?;
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn walk_rs(dir: &Path, names: &mut Vec<String>) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            walk_rs(&path, names)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !ident.is_empty() {
                    names.push(ident);
                }
            }
        }
    }
    Ok(())
}
