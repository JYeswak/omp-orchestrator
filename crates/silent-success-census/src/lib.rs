#![forbid(unsafe_code)]

//! Deterministic source census for silent-success-shaped control flow.
//!
//! This is an inventory, not a defect detector. A syntactic hit is retained with a typed
//! classification and a machine-readable reason so downstream gates can distinguish an intentional
//! no-op from a fallback or a site that needs human review.

use serde::Serialize;
use text_structure::code_and_literals;
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

    /// The classification this predicate always carries.
    ///
    /// Public because the positive-control leg asserts the CLASSIFICATION and not merely the
    /// predicate (`poumg.5` leg 4): a detector that finds the row and mislabels it is a
    /// regression the predicate check alone cannot see.
    pub fn classification(self) -> Classification {
        match self {
            Self::OkZero => Classification::TypedNonzero,
            Self::ExitCodeSuccess | Self::ExitZero => Classification::HealthyNoop,
            Self::UnwrapOrDefault | Self::EmptyCollectionReturn => Classification::Unresolved,
        }
    }
}
/// The positive controls: rows the scanner MUST re-find, anchored by CONTENT, never by line.
///
/// # Why there are no line numbers here any more — `omp-orchestrator-poumg.5`
///
/// This table used to be `(file, LINE, predicate)` over files in OTHER crates, and it was
/// unkeepable by construction: **any edit anywhere above a pinned line reds this crate.** It had
/// already been re-pinned once and said so in its own comment — *"Re-recorded 2026-09-05:
/// ExitCode::SUCCESS moved 80 -> 78 … Dies when that match arm is reordered again."* It then died
/// again, exactly as predicted. Measured at the filing sha `cb9d3941`:
///
/// ```text
/// crates/state-wildcard-lint/src/main.rs:78  at HEAD      "    report(&linted);"       NO MATCH
/// crates/state-wildcard-lint/src/main.rs:78  in worktree  "Verdict::Clean => ExitCode::SUCCESS," MATCH
/// ```
///
/// The pin was correct only because a PEER's uncommitted edit happened to shift line 78 back onto
/// an `ExitCode::SUCCESS`. This crate was green on a dirty worktree and red in CI for that reason
/// alone, and its own code was never implicated.
///
/// # Two changes, and the second matters more than the first
///
/// 1. **Content, not coordinate.** A control matches on `(file, predicate, token in excerpt)`, so
///    it survives every edit that moves a line and fails only when the construct itself goes.
/// 2. **OWNED, not foreign.** Both rows now live in THIS crate's own `src/`. A content anchor on
///    a foreign file would still be hostage to another crate's author deleting the construct —
///    weaker, but still someone else's decision. These cannot be moved by an unrelated edit in
///    another crate because no other crate can edit them at all.
///
/// Cardinality dropped 3 -> 2 deliberately. The control's job is to prove the scanner can find
/// ANYTHING; three fragile coordinates over foreign files were never worth more than two robust
/// anchors this crate owns, and each row now asserts strictly more (predicate AND classification
/// AND a token present in the excerpt) than the coordinate form ever did.
const POSITIVE_CONTROL_SPECS: [(&str, Predicate, &str); 2] = [
    (
        "crates/silent-success-census/src/main.rs",
        Predicate::ExitCodeSuccess,
        "ExitCode::SUCCESS",
    ),
    (
        "crates/silent-success-census/src/lib.rs",
        Predicate::UnwrapOrDefault,
        "unwrap_or_default",
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
/// A source row the census must re-find, and the evidence that it did.
///
/// `line` is an OBSERVATION, not a pin — `poumg.5`. It reports where the row was found and is
/// never compared against an expected value; `0` means not found. The JSON shape is unchanged,
/// so no consumer breaks, but the SEMANTICS inverted: this field used to be an input the scanner
/// was graded against, and is now an output the scanner produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PositiveControl {
    pub file: String,
    /// Where the row was found, or `0` when `found` is false. Never an expectation.
    pub line: usize,
    pub predicate: Predicate,
    /// The token that had to appear in the candidate's `source_excerpt` for this to match.
    pub excerpt_token: String,
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
        .map(|(file, predicate, token)| {
            // FIRST match wins only for REPORTING the line; `found` does not depend on which
            // occurrence matched. A file may legitimately carry the same predicate more than
            // once, and picking one of them must not become a new coordinate to drift against.
            let hit = candidates.iter().find(|candidate| {
                candidate.file == file
                    && candidate.predicate == predicate
                    && candidate.source_excerpt.contains(token)
            });
            PositiveControl {
                file: file.to_owned(),
                line: hit.map_or(0, |candidate| candidate.line),
                predicate,
                excerpt_token: token.to_owned(),
                found: hit.is_some(),
            }
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
    code_and_literals(source)
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
