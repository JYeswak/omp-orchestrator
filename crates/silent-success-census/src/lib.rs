#![forbid(unsafe_code)]

//! Deterministic source census for silent-success-shaped control flow.
//!
//! This is an inventory, not a defect detector. A syntactic hit is retained with a typed
//! classification and a machine-readable reason so downstream gates can distinguish an intentional
//! no-op from a fallback or a site that needs human review.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use subprocess_contract::{bounded_output, BoundedOutcome};

const SCHEMA: &str = "silent-success-census";
const VERSION: u32 = 1;
const MAX_EXCERPT_BYTES: usize = 240;

/// A source predicate recognized by the census.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    OkZero,
    ExitCodeSuccess,
    ExitZero,
    UnwrapOrDefault,
    EmptyCollectionReturn,
}

impl Predicate {
    fn all() -> [Self; 5] {
        [
            Self::OkZero,
            Self::ExitCodeSuccess,
            Self::ExitZero,
            Self::UnwrapOrDefault,
            Self::EmptyCollectionReturn,
        ]
    }

    fn reason(self) -> &'static str {
        match self {
            Self::OkZero => "typed numeric zero wrapped in Ok; zero may be an intentional no-work result",
            Self::ExitCodeSuccess => "explicit successful process exit; verify that success is not standing in for work completion",
            Self::ExitZero => "explicit zero process exit; verify the surrounding operation reports failures independently",
            Self::UnwrapOrDefault => "default fallback can erase an upstream error or missing value; inspect the resolved type and caller contract",
            Self::EmptyCollectionReturn => "empty collection is returned; this may be a valid empty result or a swallowed failure",
        }
    }

    fn classification(self) -> Classification {
        match self {
            Self::OkZero => Classification::TypedNonzero,
            Self::ExitCodeSuccess | Self::ExitZero => Classification::HealthyNoop,
            Self::UnwrapOrDefault | Self::EmptyCollectionReturn => Classification::Unresolved,
        }
    }
}
const POSITIVE_CONTROL_SPECS: [(&str, usize, Predicate); 3] = [
    (
        "crates/admission-reason/src/main.rs",
        40,
        Predicate::ExitCodeSuccess,
    ),
    (
        "crates/loop-tick/src/lib.rs",
        77,
        Predicate::UnwrapOrDefault,
    ),
    (
        "crates/state-wildcard-lint/src/main.rs",
        80,
        Predicate::ExitCodeSuccess,
    ),
];

/// Static classification of a candidate. It deliberately does not call every hit a defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    HealthyNoop,
    TypedNonzero,
    Error,
    Unresolved,
}

/// One source row emitted by the census.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub file: String,
    pub line: usize,
    pub predicate: Predicate,
    pub classification: Classification,
    pub reason: String,
    pub source_excerpt: String,
}
/// A fixed source row used to prove the census still finds known controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PositiveControl {
    pub file: String,
    pub line: usize,
    pub predicate: Predicate,
    pub found: bool,
}

/// Complete machine-readable census artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CensusReport {
    pub schema: String,
    pub version: u32,
    pub status: String,
    pub scan_root: String,
    pub git_head: String,
    pub file_count: usize,
    pub predicate_counts: BTreeMap<Predicate, usize>,
    pub positive_controls: Vec<PositiveControl>,
    pub candidates: Vec<Candidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Scan `<root>/crates/*/src/**/*.rs`, returning a report even when the scan is invalid.
///
/// An empty or unreadable scan is represented as `status = "error"`; callers must not treat it as
/// a clean result. Source paths in candidates are root-relative, stable, and sorted.
pub fn scan_workspace(root: &Path) -> CensusReport {
    let mut counts = empty_counts();
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let crates_dir = root.join("crates");

    match fs::read_dir(&crates_dir) {
        Ok(entries) => {
            let mut crate_dirs = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            crate_dirs.sort();
            for crate_dir in crate_dirs {
                let src = crate_dir.join("src");
                if src.is_dir() {
                    collect_rs_files(&src, &mut files, &mut errors);
                }
            }
        }
        Err(error) => errors.push(format!("cannot read {}: {error}", crates_dir.display())),
    }
    files.sort();

    let mut candidates = Vec::new();
    for file in &files {
        let display_file = relative_path(root, file);
        match fs::read_to_string(file) {
            Ok(source) => scan_source(&display_file, &source, &mut counts, &mut candidates),
            Err(error) => errors.push(format!("cannot read {display_file}: {error}")),
        }
    }

    candidates.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.predicate.cmp(&right.predicate))
            .then(left.source_excerpt.cmp(&right.source_excerpt))
    });
    let positive_controls = evaluate_positive_controls(&candidates);

    let empty = files.is_empty();
    if empty {
        errors.push(format!("empty scan set under {}", crates_dir.display()));
    }
    let error = (!errors.is_empty()).then(|| errors.join("; "));
    CensusReport {
        schema: SCHEMA.to_owned(),
        version: VERSION,
        status: if error.is_some() { "error" } else { "ok" }.to_owned(),
        scan_root: root.to_string_lossy().into_owned(),
        git_head: git_head(root),
        file_count: files.len(),
        predicate_counts: counts,
        positive_controls,
        candidates,
        error,
    }
}

/// Serialize a report with stable field and map ordering.
pub fn render_json(report: &CensusReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|error| {
        format!("{{\"schema\":\"{SCHEMA}\",\"version\":{VERSION},\"status\":\"error\",\"error\":{error:?}}}")
    })
}

fn empty_counts() -> BTreeMap<Predicate, usize> {
    Predicate::all()
        .into_iter()
        .map(|predicate| (predicate, 0))
        .collect()
}
fn evaluate_positive_controls(candidates: &[Candidate]) -> Vec<PositiveControl> {
    POSITIVE_CONTROL_SPECS
        .into_iter()
        .map(|(file, line, predicate)| PositiveControl {
            file: file.to_owned(),
            line,
            predicate,
            found: candidates.iter().any(|candidate| {
                candidate.file == file && candidate.line == line && candidate.predicate == predicate
            }),
        })
        .collect()
}

fn collect_rs_files(directory: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<String>) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!("cannot read {}: {error}", directory.display()));
            return;
        }
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_rs_files(&path, files, errors);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn git_head(root: &Path) -> String {
    let root_text = root.to_string_lossy();
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root_text.as_ref())
        .args(["rev-parse", "HEAD"]);
    match bounded_output(&mut command, std::time::Duration::from_secs(5)) {
        BoundedOutcome::Completed(output) if output.status.success() => String::from_utf8(output.stdout)
            .ok()
            .map(|head| head.trim().to_owned())
            .filter(|head| !head.is_empty())
            .unwrap_or_else(|| "unknown".to_owned()),
        BoundedOutcome::Completed(_) => "unknown".to_owned(),
        BoundedOutcome::TimedOut => {
            eprintln!("silent-success-census: git head timed out before its deadline");
            "unknown".to_owned()
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("silent-success-census: git head could not spawn: {error}");
            "unknown".to_owned()
        }
    }
}

fn scan_source(
    file: &str,
    source: &str,
    counts: &mut BTreeMap<Predicate, usize>,
    candidates: &mut Vec<Candidate>,
) {
    let masked = mask_non_code(source);
    let mut matches = Vec::new();
    for (offset, predicate) in find_predicates(&masked) {
        matches.push((offset, predicate));
    }
    for (offset, predicate) in matches {
        *counts.entry(predicate).or_insert(0) += 1;
        candidates.push(Candidate {
            file: file.to_owned(),
            line: line_at(source, offset),
            predicate,
            classification: predicate.classification(),
            reason: predicate.reason().to_owned(),
            source_excerpt: excerpt_at(source, offset),
        });
    }
}

fn find_predicates(code: &str) -> Vec<(usize, Predicate)> {
    let bytes = code.as_bytes();
    let mut matches = Vec::new();
    for offset in 0..bytes.len() {
        if starts_token(bytes, offset, b"Ok") && match_ok_zero(bytes, offset) {
            matches.push((offset, Predicate::OkZero));
        }
        if starts_token(bytes, offset, b"ExitCode::SUCCESS") {
            matches.push((offset, Predicate::ExitCodeSuccess));
        }
        if starts_token(bytes, offset, b"exit") && match_exit_zero(bytes, offset) {
            matches.push((offset, Predicate::ExitZero));
        }
        if starts_token(bytes, offset, b"unwrap_or_default") {
            matches.push((offset, Predicate::UnwrapOrDefault));
        }
        if starts_token(bytes, offset, b"Vec::new")
            && is_empty_constructor(bytes, offset, 8)
            && is_return_context(bytes, offset)
        {
            matches.push((offset, Predicate::EmptyCollectionReturn));
        }
        if starts_token(bytes, offset, b"HashMap::new")
            && is_empty_constructor(bytes, offset, 11)
            && is_return_context(bytes, offset)
        {
            matches.push((offset, Predicate::EmptyCollectionReturn));
        }
        if starts_token(bytes, offset, b"HashSet::new")
            && is_empty_constructor(bytes, offset, 10)
            && is_return_context(bytes, offset)
        {
            matches.push((offset, Predicate::EmptyCollectionReturn));
        }
        if starts_token(bytes, offset, b"BTreeMap::new")
            && is_empty_constructor(bytes, offset, 12)
            && is_return_context(bytes, offset)
        {
            matches.push((offset, Predicate::EmptyCollectionReturn));
        }
        if starts_token(bytes, offset, b"BTreeSet::new")
            && is_empty_constructor(bytes, offset, 11)
            && is_return_context(bytes, offset)
        {
            matches.push((offset, Predicate::EmptyCollectionReturn));
        }
        if starts_token(bytes, offset, b"vec")
            && match_empty_vec_macro(bytes, offset)
            && is_return_context(bytes, offset)
        {
            matches.push((offset, Predicate::EmptyCollectionReturn));
        }
    }
    matches
}

fn starts_token(bytes: &[u8], offset: usize, token: &[u8]) -> bool {
    bytes.get(offset..offset + token.len()) == Some(token)
        && (offset == 0 || !is_ident(bytes[offset - 1]))
        && (offset + token.len() == bytes.len() || !is_ident(bytes[offset + token.len()]))
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn match_ok_zero(bytes: &[u8], offset: usize) -> bool {
    let mut index = skip_ws(bytes, offset + 2);
    if bytes.get(index) != Some(&b'(') {
        return false;
    }
    index = skip_ws(bytes, index + 1);
    if bytes.get(index) != Some(&b'0') {
        return false;
    }
    index = skip_ws(bytes, index + 1);
    bytes.get(index) == Some(&b')')
}

fn match_exit_zero(bytes: &[u8], offset: usize) -> bool {
    let mut index = skip_ws(bytes, offset + 4);
    if bytes.get(index) == Some(&b'(') {
        index = skip_ws(bytes, index + 1);
        if bytes.get(index) != Some(&b'0') {
            return false;
        }
        index = skip_ws(bytes, index + 1);
        bytes.get(index) == Some(&b')')
    } else {
        bytes.get(index) == Some(&b'0')
    }
}

fn is_empty_constructor(bytes: &[u8], offset: usize, name_len: usize) -> bool {
    let mut index = skip_ws(bytes, offset + name_len);
    if bytes.get(index) != Some(&b'(') {
        return false;
    }
    index = skip_ws(bytes, index + 1);
    bytes.get(index) == Some(&b')')
}

fn match_empty_vec_macro(bytes: &[u8], offset: usize) -> bool {
    let mut index = skip_ws(bytes, offset + 3);
    if bytes.get(index) != Some(&b'!') {
        return false;
    }
    index = skip_ws(bytes, index + 1);
    if bytes.get(index) != Some(&b'[') {
        return false;
    }
    index = skip_ws(bytes, index + 1);
    bytes.get(index) == Some(&b']')
}

fn is_return_context(bytes: &[u8], offset: usize) -> bool {
    let start = offset.saturating_sub(160);
    let context = std::str::from_utf8(&bytes[start..offset]).unwrap_or_default();
    context
        .rsplit_once('\n')
        .map(|(_, line)| line.contains("return") || line.contains("=>"))
        .unwrap_or_else(|| context.contains("return") || context.contains("=>"))
}

/// Replace comments and string/character literal contents while preserving byte offsets and lines.
fn mask_non_code(source: &str) -> String {
    let mut bytes = source.as_bytes().to_vec();
    let mut index = 0;
    let mut block_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    while index < bytes.len() {
        if block_depth > 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                block_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                block_depth -= 1;
                index += 2;
            } else {
                if bytes[index] != b'\n' {
                    bytes[index] = b' ';
                }
                index += 1;
            }
            continue;
        }
        if let Some(end_quote) = quote {
            if bytes[index] == b'\n' {
                quote = None;
                escaped = false;
                index += 1;
            } else {
                let closes = bytes[index] == end_quote && !escaped;
                if bytes[index] != b'\n' {
                    bytes[index] = b' ';
                }
                escaped = bytes[index] == b'\\' && !escaped;
                index += 1;
                if closes {
                    quote = None;
                    escaped = false;
                }
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                bytes[index] = b' ';
                index += 1;
            }
        } else if bytes.get(index..index + 2) == Some(b"/*") {
            bytes[index] = b' ';
            bytes[index + 1] = b' ';
            block_depth = 1;
            index += 2;
        } else if bytes[index] == b'"' || bytes[index] == b'\'' {
            quote = Some(bytes[index]);
            bytes[index] = b' ';
            escaped = false;
            index += 1;
        } else {
            index += 1;
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_owned())
}

fn line_at(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset.min(source.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn excerpt_at(source: &str, offset: usize) -> String {
    let line = source
        .lines()
        .nth(line_at(source, offset).saturating_sub(1))
        .unwrap_or_default()
        .trim();
    if line.len() <= MAX_EXCERPT_BYTES {
        line.to_owned()
    } else {
        let mut excerpt = line[..MAX_EXCERPT_BYTES].to_owned();
        excerpt.push_str("…");
        excerpt
    }
}
