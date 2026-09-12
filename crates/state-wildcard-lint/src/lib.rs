#![forbid(unsafe_code)]

//! Local source lint for wildcard arms on state-like enum matches.
//!
//! The scanner is deliberately conservative. It resolves enum definitions and typed local
//! bindings in the same source file. A wildcard on a resolved state-like enum is a violation.
//! A wildcard on an explicitly primitive/non-state type is allowed. A state-like scrutinee whose
//! type is not locally resolvable is reported as UNKNOWN rather than silently passing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

/// Why a wildcard match was reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingKind {
    WildcardState,
    UnresolvedStateType,
}

impl fmt::Display for FindingKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WildcardState => formatter.write_str("WILDCARD_STATE"),
            Self::UnresolvedStateType => formatter.write_str("UNRESOLVED_STATE_TYPE"),
        }
    }
}

/// One wildcard arm that must be reviewed or rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    pub match_line: usize,
    pub wildcard_line: usize,
    /// The offending arm, verbatim from the source (trimmed, bounded).
    ///
    /// A refusal that names a count and not the offence is not actionable: an
    /// agent that hits it cannot tell its own violation from a neighbour's.
    /// Measured 2026-09-02: the pre-commit gate printed
    /// `state-wildcard-lint: 2 finding(s)` with no path, no line and no arm,
    /// while this crate had already computed all three and discarded them at
    /// the reporting boundary.
    pub wildcard_text: String,
    pub scrutinee: String,
    pub inferred_type: Option<String>,
    pub kind: FindingKind,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = self.inferred_type.as_deref().unwrap_or("unknown");
        write!(
            formatter,
            "{}:{}: wildcard arm `{}` in `match {}` (match opens line {}, type={}, kind={})",
            self.file,
            self.wildcard_line,
            self.wildcard_text,
            self.scrutinee,
            self.match_line,
            ty,
            self.kind
        )
    }
}

/// One DECLARED suppression. Never inferred.
///
/// A row exists only where the lint's predicate is provably wrong about a
/// specific site, and it must say WHY in prose a reader can check. An
/// exclusion the tool derives for itself is indistinguishable from a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowRow {
    /// Repo-relative path, as `lint_workspace` reports it.
    pub file: &'static str,
    /// The match scrutinee, as the scanner normalizes it.
    pub scrutinee: &'static str,
    pub reason: &'static str,
}

/// The complete suppression set. Keyed by file+scrutinee, not by line, so a
/// row does not silently re-target when the file shifts; a row that matches
/// nothing is an ERROR (see `apply_allowlist`), so it cannot rot into a
/// standing carve-out.
///
/// EMPTY BY DESIGN as of 2026-09-02. The two live findings
/// (`crates/cargo-lane-budget/src/main.rs` scrutinee `mode`,
/// `crates/check-publish/src/json.rs` scrutinee `verdict`) were both repaired
/// at the source by annotating the scrutinee's type, so neither needs one.
/// THE SELF-REFERENCE IS NOT SUPPRESSED HERE EITHER: see `DECLARED_SKIP_DIRS`
/// and `mask_line` — a row for it would match nothing and therefore be stale.
pub const DECLARED_ALLOWLIST: &[AllowRow] = &[];

/// One DECLARED directory prune, with the reason it is not production source.
///
/// This replaces a bare `matches!(name, ".git" | "target" | "tests" | "fixtures")`.
/// The set was identical; the difference is that each entry now states WHY and
/// the gate prints them, so the scan's boundary is auditable rather than
/// buried in a pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkipDir {
    pub name: &'static str,
    pub reason: &'static str,
}

/// THE DECLARED SCAN BOUNDARY.
///
/// This crate embeds the forbidden pattern as specimen data, which makes it the
/// seventh checker this session whose own input contains text about the thing it
/// checks. It is handled by two DECLARED mechanisms and by NO carve-out:
///
///   1. `mask_line` blanks string-literal contents before scanning, so a
///      pattern quoted as test data is not code. `src/lib.rs` IS scanned, and
///      `self_scan_of_lib_is_clean_without_an_exclusion` is the standing proof.
///   2. `tests/` and `fixtures/` are pruned by the rows below, so specimen
///      files are never production source.
pub const DECLARED_SKIP_DIRS: &[SkipDir] = &[
    SkipDir {
        name: ".git",
        reason: "object store, not source; blobs are not Rust even when they end in .rs",
    },
    SkipDir {
        name: "target",
        reason: "build output; generated code is not authored code and cannot be repaired here",
    },
    SkipDir {
        name: "tests",
        reason: "specimen data: a known-bad fixture MUST contain the forbidden pattern, so \
                 scanning it would make every fires-on-known-bad test a violation",
    },
    SkipDir {
        name: "fixtures",
        reason: "specimen data, same reason as tests/",
    },
];

/// Which files a lint run covered. Named, because a verdict whose scope is
/// implied is a verdict a reader cannot check.
///
/// MEASURED 2026-09-02 (omp-orchestrator-oej2): this lint ran REPO-WIDE from a
/// pre-commit hook that keys on the staged set, exactly as `path-literal-guard`
/// did. While repairing that gate, the rebuilt hook refused a commit staging
/// only `AGENTS.md` because of a wildcard arm in an UNTRACKED file the author
/// had never staged -- the same fleet-blocking shape, through a different gate.
/// Scoping one and leaving the other would have moved the block, not removed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// Every `.rs` file under `crates/`, pruning [`DECLARED_SKIP_DIRS`] -- CI and audits.
    RepoWide,
    /// Only the paths handed in, filtered by [`is_in_scan_scope`] -- the pre-commit hook.
    StagedPaths,
}

impl fmt::Display for ScanMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepoWide => formatter.write_str("repo-wide"),
            Self::StagedPaths => formatter.write_str("staged"),
        }
    }
}

/// The lint's verdict. THREE outcomes plus an error: a run that covered nothing
/// is not a run that found nothing (omp-orchestrator-calr).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Clean,
    Violation,
    /// Staged mode with zero eligible paths: this lint has no opinion here.
    NothingToCheck,
    /// Repo-wide empty scan set, a stale DECLARED row, or an unreadable tree.
    VacuousError,
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => formatter.write_str("CLEAN"),
            Self::Violation => formatter.write_str("VIOLATION"),
            Self::NothingToCheck => formatter.write_str("NOTHING_TO_CHECK"),
            Self::VacuousError => formatter.write_str("VACUOUS_ERROR"),
        }
    }
}

/// THE ONE ELIGIBILITY PREDICATE, shared by both modes.
///
/// True for a repo-relative `.rs` path under `crates/` whose components include
/// none of [`DECLARED_SKIP_DIRS`]. Both modes route through the same floor, so a
/// scoped run and a sweep can never disagree about WHAT is in scope -- only
/// about which subset was read.
pub fn is_in_scan_scope(relative: &Path) -> bool {
    if !relative.extension().is_some_and(|extension| extension == "rs") {
        return false;
    }
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    if parts.first() != Some(&"crates") || parts.len() < 3 {
        return false;
    }
    !parts
        .iter()
        .any(|part| DECLARED_SKIP_DIRS.iter().any(|row| row.name == *part))
}

/// One line stating the scan boundary, for callers that refuse. A gate whose
/// scope is invisible cannot be argued with, and a scoped green that reads like
/// a repo-wide one is the next overclaim.
pub fn declared_scope_line(mode: ScanMode) -> String {
    let pruned: Vec<&str> = DECLARED_SKIP_DIRS.iter().map(|row| row.name).collect();
    let common = format!(
        "string-literal contents are masked before scanning, so a pattern quoted as \
         specimen data is not code. DECLARED allowlist rows: {} (a row matching nothing \
         in repo-wide mode is an ERROR, not a pass)",
        DECLARED_ALLOWLIST.len()
    );
    match mode {
        ScanMode::RepoWide => format!(
            "DECLARED SCOPE repo-wide: crates/**/*.rs, pruning {}, untracked included; {common}.",
            pruned.join(", ")
        ),
        ScanMode::StagedPaths => format!(
            "DECLARED SCOPE staged set only: crates/**/*.rs among the staged paths, pruning \
             {}. A wildcard arm in an UNSTAGED or UNTRACKED file is not this commit's problem \
             and cannot refuse it; only `state-wildcard-lint <root>` (repo-wide mode) claims \
             the workspace is clean. {common}.",
            pruned.join(", ")
        ),
    }
}

/// A finding that a DECLARED row suppressed, carried so the suppression is
/// visible instead of subtracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedFinding {
    pub finding: Finding,
    pub reason: &'static str,
}

/// Partition findings against a suppression set.
///
/// Returns `(kept, allowed, stale)`. `stale` names rows that matched nothing:
/// a carve-out that suppresses nothing is a silent carve-out, and callers MUST
/// treat it as an error rather than a pass.
pub fn apply_allowlist(
    findings: Vec<Finding>,
    allowlist: &[AllowRow],
) -> (Vec<Finding>, Vec<AllowedFinding>, Vec<AllowRow>) {
    let mut used = vec![false; allowlist.len()];
    let mut kept = Vec::new();
    let mut allowed = Vec::new();
    for finding in findings {
        let matched = allowlist
            .iter()
            .position(|row| row.file == finding.file && row.scrutinee == finding.scrutinee);
        match matched {
            Some(index) => {
                used[index] = true;
                allowed.push(AllowedFinding {
                    finding,
                    reason: allowlist[index].reason,
                });
            }
            None => kept.push(finding),
        }
    }
    let stale = allowlist
        .iter()
        .zip(used.iter())
        .filter(|(_, hit)| !**hit)
        .map(|(row, _)| *row)
        .collect();
    (kept, allowed, stale)
}

/// Result of one lint run, in one named mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintReport {
    /// Which files this run covered.
    pub mode: ScanMode,
    pub scanned: Vec<String>,
    pub findings: Vec<Finding>,
    /// Findings a DECLARED row suppressed, with the reason.
    pub allowed: Vec<AllowedFinding>,
    pub error: Option<String>,
}

impl LintReport {
    /// The run's verdict. Staged mode with nothing eligible is NOTHING-TO-CHECK,
    /// never CLEAN: reporting an uncovered change as clean is the vacuous-green
    /// inversion.
    pub fn verdict(&self) -> Verdict {
        if self.error.is_some() {
            return Verdict::VacuousError;
        }
        if !self.findings.is_empty() {
            return Verdict::Violation;
        }
        match (self.mode, self.scanned.is_empty()) {
            (ScanMode::RepoWide, true) => Verdict::VacuousError,
            (ScanMode::StagedPaths, true) => Verdict::NothingToCheck,
            (_, false) => Verdict::Clean,
        }
    }

    /// Green only when files were actually covered AND zero findings survived.
    pub fn is_pass(&self) -> bool {
        self.verdict() == Verdict::Clean
    }

    /// One line stating exactly what this verdict covers.
    pub fn declared_scope_line(&self) -> String {
        declared_scope_line(self.mode)
    }
}

fn mask_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
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

fn identifier_after_keyword(line: &str, keyword: &str) -> Option<String> {
    let position = line.find(keyword)? + keyword.len();
    let name: String = line[position..]
        .chars()
        .skip_while(|character| character.is_ascii_whitespace())
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn state_like(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["state", "status", "phase", "stage", "mode", "lifecycle", "verdict", "outcome"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

fn parse_enum_names(code: &[String]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut all = BTreeSet::new();
    let mut state = BTreeSet::new();
    for line in code {
        if let Some(name) = identifier_after_keyword(line, "enum") {
            if state_like(&name) {
                state.insert(name.clone());
            }
            all.insert(name);
        }
    }
    (all, state)
}

fn parse_typed_bindings(code: &[String]) -> BTreeMap<String, String> {
    let mut bindings = BTreeMap::new();
    for line in code {
        let Some(let_position) = line.find("let ") else { continue };
        let after_let = line[let_position + 4..].trim_start();
        let after_mut = after_let.strip_prefix("mut ").unwrap_or(after_let);
        let Some(colon) = after_mut.find(':') else { continue };
        let name = after_mut[..colon].trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            continue;
        }
        let type_text = after_mut[colon + 1..]
            .split(['=', ';', ','])
            .next()
            .unwrap_or("")
            .trim();
        if !type_text.is_empty() {
            bindings.insert(name.to_owned(), type_text.to_owned());
        }
    }
    bindings
}

fn brace_delta(line: &str) -> i32 {
    line.bytes().fold(0, |delta, byte| match byte {
        b'{' => delta + 1,
        b'}' => delta - 1,
        _ => delta,
    })
}

fn match_end(code: &[String], start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut opened = false;
    for (index, line) in code.iter().enumerate().skip(start) {
        depth += brace_delta(line);
        opened |= line.contains('{');
        if opened && depth <= 0 {
            return Some(index);
        }
    }
    None
}

fn normalized_scrutinee(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('&')
        .trim_start_matches('*')
        .trim_matches(['(', ')', ' '])
        .to_owned()
}

fn binding_name(scrutinee: &str) -> Option<&str> {
    let candidate = scrutinee.rsplit('.').next()?.trim();
    let candidate = candidate.trim_start_matches('&').trim_start_matches('*');
    (!candidate.is_empty()
        && candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_'))
    .then_some(candidate)
}

fn known_non_state_type(type_name: &str) -> bool {
    let compact: String = type_name
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let lower = compact.to_ascii_lowercase();
    lower == "bool"
        || lower == "char"
        || lower == "string"
        || lower == "&str"
        || lower == "str"
        || (lower.starts_with('u') && lower[1..].chars().all(|character| character.is_ascii_digit()))
        || (lower.starts_with('i') && lower[1..].chars().all(|character| character.is_ascii_digit()))
        || (lower.starts_with('f') && lower[1..].chars().all(|character| character.is_ascii_digit()))
        || lower.starts_with("option<")
        || lower.starts_with("result<")
        || lower.starts_with("vec<")
        || lower.starts_with("hashmap<")
        || lower.starts_with("btreemap<")
}

/// Byte offset of a wildcard arm's `_` on this line, if any.
///
/// Token-aware on purpose. `Some(_) =>` is a partial pattern, not a wildcard,
/// and `x_ =>` is an identifier; the old substring test (`line.contains("_ =>")`)
/// would have been happy to call a `_` anywhere on the line an arm.
fn wildcard_offset(line: &str) -> Option<usize> {
    let mut previous: Option<char> = None;
    for (index, character) in line.char_indices() {
        if character == '_' && !previous.is_some_and(|p| p.is_alphanumeric() || p == '_') {
            let rest = line[index + 1..].trim_start();
            let guarded = rest.starts_with("if ") && line[index..].contains("=>");
            if rest.starts_with("=>") || guarded {
                return Some(index);
            }
        }
        previous = Some(character);
    }
    None
}

/// The wildcard arm of THIS match, at this match's own arm depth.
///
/// Measured 2026-09-02: `crates/cargo-lane-budget/src/main.rs` was reported as
/// `match line 29, wildcard arm line 60`. Line 60 is the `_ =>` of the INNER
/// `match args[index].as_str()` at line 43; the outer `match mode` has its own
/// wildcard at line 94. The old scan took the first wildcard anywhere inside
/// the outer match's brace span, so the file:line it names can belong to a
/// different match — a refusal pointing at the wrong arm is worse than a count,
/// because it looks actionable and is not.
///
/// Depth-scoping loses no coverage: every `match ` line gets its own iteration,
/// so a nested match's wildcard is still reported against the nested match.
fn wildcard_arm_line(code: &[String], start: usize, end: usize) -> Option<usize> {
    if let Some(offset) = wildcard_offset(&code[start]) {
        // Single-line match: `match x { A => (), _ => () }`. The arm sits on
        // the header line, after the brace that opens the match body.
        if code[start][..offset].contains('{') {
            return Some(start);
        }
    }
    let mut depth = 0i32;
    for index in start..=end {
        let line = &code[index];
        if index > start {
            if let Some(offset) = wildcard_offset(line) {
                // Depth AT the arm, not before the line: `} _ => {` closes the
                // previous arm on the same line it opens this one.
                if depth + brace_delta(&line[..offset]) == 1 {
                    return Some(index);
                }
            }
        }
        depth += brace_delta(line);
    }
    None
}

/// Split a parameter list on top-level commas, respecting `<>`, `()` and `[]`.
fn split_top_level(list: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut previous = ' ';
    for character in list.chars() {
        match character {
            '<' | '(' | '[' => {
                depth += 1;
                current.push(character);
            }
            '>' | ')' | ']' => {
                // `->` and `=>` inside a parameter type are arrows, not closers.
                if !(character == '>' && (previous == '-' || previous == '=')) {
                    depth = (depth - 1).max(0);
                }
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_owned());
                current.clear();
            }
            other => current.push(other),
        }
        previous = character;
    }
    let tail = current.trim();
    if !tail.is_empty() {
        parts.push(tail.to_owned());
    }
    parts.retain(|part| !part.is_empty());
    parts
}

/// Strip references, `mut`, `dyn` and lifetimes so a parameter type can be
/// compared against the enum set and the primitive set.
fn normalize_param_type(text: &str) -> String {
    let mut current = text.trim();
    loop {
        let mut next = current.trim_start_matches('&').trim_start();
        if let Some(rest) = next.strip_prefix("mut ") {
            next = rest.trim_start();
        }
        if let Some(rest) = next.strip_prefix("dyn ") {
            next = rest.trim_start();
        }
        if next.starts_with('\'') {
            next = next
                .find(char::is_whitespace)
                .map_or("", |space| next[space..].trim_start());
        }
        if next == current {
            return current.to_owned();
        }
        current = next;
    }
}

/// Offset of a `fn` keyword used as a token on this line.
fn fn_keyword_offset(line: &str) -> Option<usize> {
    let mut search = 0;
    while let Some(relative) = line[search..].find("fn ") {
        let position = search + relative;
        let preceding = line[..position].chars().next_back();
        if !preceding.is_some_and(|p| p.is_alphanumeric() || p == '_') {
            return Some(position);
        }
        search = position + 3;
    }
    None
}

/// Typed bindings from function signatures.
///
/// `parse_typed_bindings` only sees `let name: Type`. A parameter is just as
/// authoritative and far more common as a match scrutinee. Measured 2026-09-02:
/// `pub fn count_key(verdict: &str)` in `crates/check-publish/src/json.rs` was
/// reported as UNRESOLVED_STATE_TYPE — a `match` on `&str` cannot be exhaustive,
/// so its `_` arm is compiler-required. The type was declared three characters
/// from the match; only the resolver could not see it.
fn parse_fn_param_bindings(code: &[String]) -> BTreeMap<String, String> {
    let mut bindings = BTreeMap::new();
    for (index, line) in code.iter().enumerate() {
        let Some(fn_position) = fn_keyword_offset(line) else { continue };
        let Some(open) = line[fn_position..].find('(') else { continue };
        let mut list = String::new();
        let mut depth = 0i32;
        let mut closed = false;
        let mut cursor = fn_position + open;
        let mut scan = index;
        // A signature spanning more than a dozen lines is not a signature.
        while scan < code.len() && scan <= index + 12 {
            for character in code[scan][cursor..].chars() {
                if character == '(' {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                } else if character == ')' {
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                        break;
                    }
                }
                if depth >= 1 {
                    list.push(character);
                }
            }
            if closed {
                break;
            }
            list.push(' ');
            scan += 1;
            cursor = 0;
        }
        if !closed {
            continue;
        }
        for part in split_top_level(&list) {
            let Some(colon) = part.find(':') else { continue };
            let name = part[..colon].trim();
            if name.is_empty()
                || name == "self"
                || !name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                continue;
            }
            let type_text = normalize_param_type(&part[colon + 1..]);
            if !type_text.is_empty() {
                bindings.insert(name.to_owned(), type_text);
            }
        }
    }
    bindings
}

fn type_for_match(
    scrutinee: &str,
    body: &str,
    bindings: &BTreeMap<String, String>,
    state_enums: &BTreeSet<String>,
) -> (Option<String>, bool) {
    // An enum named in the match BODY may be the match's OUTPUT rather than its
    // scrutinee, and conflating the two is how this lint produced an 8-of-9 false
    // positive rate that blocked every commit in the repo (measured 2026-09-01).
    //
    //     match value {                        // scrutinee: Option<Vec<_>>
    //         Some(items) if !items.is_empty() => ProbeState::Known,
    //         _ => ProbeState::Unknown,        // <- ProbeState is the RESULT
    //     }
    //
    // The old rule saw `ProbeState::` anywhere in the body, concluded the
    // scrutinee was ProbeState, and demanded exhaustive arms for an `Option`
    // match whose `_` the COMPILER REQUIRES (a guarded `Some` arm does not cover
    // `Some`). An over-strict gate gets routed around, which is a slower death
    // than no gate — so resolution now requires PATTERN position.
    for enum_name in state_enums {
        let needle = format!("{enum_name}::");
        let in_pattern = body.lines().any(|l| {
            match l.find("=>") {
                // left of the fat arrow is pattern position
                Some(arrow) => l[..arrow].contains(&needle),
                // no arrow on this line: the `match X {` header itself counts
                None => l.contains("match ") && l.contains(&needle),
            }
        });
        if in_pattern {
            return (Some(enum_name.clone()), true);
        }
    }
    if let Some(name) = binding_name(scrutinee) {
        if let Some(type_name) = bindings.get(name) {
            if state_enums.contains(type_name) || state_like(type_name) {
                return (Some(type_name.clone()), true);
            }
            if known_non_state_type(type_name) {
                return (Some(type_name.clone()), false);
            }
            return (Some(type_name.clone()), false);
        }
    }
    let stateish = state_like(scrutinee)
        || scrutinee
            .split('.')
            .next_back()
            .is_some_and(state_like);
    (None, stateish)
}

/// Find wildcard arms on state-like matches in one Rust source file.
///
/// The returned `Finding::file` is empty: this entry point has no filename.
/// `lint_workspace` fills it in. The line numbers and the arm text are final.
pub fn find_findings_in_source(source: &str) -> Vec<Finding> {
    let raw: Vec<&str> = source.lines().collect();
    let code: Vec<String> = raw.iter().map(|line| mask_line(line)).collect();
    let (all_enums, state_enums) = parse_enum_names(&code);
    // Parameters first, `let` bindings second: the nearer declaration wins.
    let mut bindings = parse_fn_param_bindings(&code);
    bindings.extend(parse_typed_bindings(&code));
    let mut findings = Vec::new();
    for (start, line) in code.iter().enumerate() {
        let Some(match_position) = line.find("match ") else { continue };
        let after_match = &line[match_position + 6..];
        let Some(end) = match_end(&code, start) else { continue };
        let scrutinee = normalized_scrutinee(after_match.split('{').next().unwrap_or(after_match));
        let Some(wildcard) = wildcard_arm_line(&code, start, end) else { continue };
        let body = code[start..=end].join("\n");
        let (inferred_type, state_candidate) = type_for_match(&scrutinee, &body, &bindings, &state_enums);
        let kind = if let Some(type_name) = inferred_type.as_deref() {
            if state_enums.contains(type_name) {
                FindingKind::WildcardState
            } else if !all_enums.contains(type_name) && state_candidate {
                FindingKind::UnresolvedStateType
            } else {
                continue;
            }
        } else if state_candidate {
            FindingKind::UnresolvedStateType
        } else {
            continue;
        };
        findings.push(Finding {
            file: String::new(),
            match_line: start + 1,
            wildcard_line: wildcard + 1,
            wildcard_text: arm_text(raw.get(wildcard).copied().unwrap_or("")),
            scrutinee,
            inferred_type,
            kind,
        });
    }
    findings
}

/// The arm as an author would recognize it: trimmed, one line, bounded so a
/// generated or minified line cannot flood a refusal.
fn arm_text(line: &str) -> String {
    let trimmed = line.trim();
    let limit = 96;
    if trimmed.chars().count() <= limit {
        return trimmed.to_owned();
    }
    let head: String = trimmed.chars().take(limit).collect();
    format!("{head}...")
}

fn scan_source(file: &str, source: &str) -> Vec<Finding> {
    find_findings_in_source(source)
        .into_iter()
        .map(|finding| Finding {
            file: file.to_owned(),
            ..finding
        })
        .collect()
}

fn visit_rs(root: &Path, directory: &Path, scanned: &mut Vec<String>, findings: &mut Vec<Finding>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("ERROR: cannot read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("ERROR: read directory entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
            if !DECLARED_SKIP_DIRS.iter().any(|row| row.name == name) {
                visit_rs(root, &path, scanned, findings)?;
            }
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("ERROR: cannot read {}: {error}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        scanned.push(relative.clone());
        findings.extend(scan_source(&relative, &source));
    }
    Ok(())
}

fn finish(mode: ScanMode, mut scanned: Vec<String>, findings: Vec<Finding>) -> LintReport {
    scanned.sort();
    let mut findings = findings;
    findings.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.wildcard_line.cmp(&right.wildcard_line))
    });
    let (findings, allowed, stale) = apply_allowlist(findings, DECLARED_ALLOWLIST);
    // A row that matches nothing is only evidence of rot when the sweep was
    // total. In staged mode a row legitimately matches nothing whenever its
    // file is not part of this commit, and treating that as an error would
    // reintroduce the very defect this scoping removes.
    let stale = match mode {
        ScanMode::RepoWide => stale,
        ScanMode::StagedPaths => Vec::new(),
    };
    let error = (!stale.is_empty()).then(|| {
        let rows: Vec<String> = stale
            .iter()
            .map(|row| format!("{} scrutinee={}", row.file, row.scrutinee))
            .collect();
        format!(
            "ERROR: {} DECLARED allowlist row(s) matched nothing -- a carve-out that suppresses \
             nothing is a silent carve-out: {}",
            stale.len(),
            rows.join("; ")
        )
    });
    LintReport {
        mode,
        scanned,
        findings,
        allowed,
        error,
    }
}

/// REPO-WIDE mode: scan production Rust sources below a repository's crates
/// directory. This is the sweep -- CI, full audits, arrival checks.
pub fn lint_workspace(root: &Path) -> LintReport {
    let source_root = root.join("crates");
    let mut scanned = Vec::new();
    let mut findings = Vec::new();
    if let Err(error) = visit_rs(root, &source_root, &mut scanned, &mut findings) {
        return LintReport {
            mode: ScanMode::RepoWide,
            scanned: Vec::new(),
            findings: Vec::new(),
            allowed: Vec::new(),
            error: Some(error),
        };
    }
    // ANTI-VACUITY. A deliverable that was never checked reports exactly like
    // one that passed, so an empty scan set is an ERROR and never a pass.
    if scanned.is_empty() {
        return LintReport {
            mode: ScanMode::RepoWide,
            scanned: Vec::new(),
            findings: Vec::new(),
            allowed: Vec::new(),
            error: Some(format!(
                "ERROR: empty scan set under {} -- nothing was checked, which is not a pass",
                source_root.display()
            )),
        };
    }
    finish(ScanMode::RepoWide, scanned, findings)
}

/// STAGED mode: lint only the handed-in paths, filtered by [`is_in_scan_scope`].
///
/// This is what the pre-commit hook calls. A wildcard arm in an unstaged or
/// untracked file is invisible here BY DESIGN: measured 2026-09-02, this lint
/// refused a commit staging only `AGENTS.md` because of an arm in a file the
/// author had never staged. An in-scope path absent from the worktree (a staged
/// deletion) is skipped rather than read; an unreadable in-scope file is an
/// ERROR, because a run that silently skipped a file reports identically to one
/// that covered it.
pub fn lint_paths<P: AsRef<Path>>(root: &Path, paths: &[P]) -> LintReport {
    let mut scanned = Vec::new();
    let mut findings = Vec::new();
    for path in paths {
        let given = path.as_ref();
        let relative = given.strip_prefix(root).unwrap_or(given);
        if !is_in_scan_scope(relative) {
            continue;
        }
        let absolute = if given.is_absolute() {
            given.to_path_buf()
        } else {
            root.join(relative)
        };
        if !absolute.is_file() {
            continue;
        }
        let source = match fs::read_to_string(&absolute) {
            Ok(source) => source,
            Err(error) => {
                return LintReport {
                    mode: ScanMode::StagedPaths,
                    scanned: Vec::new(),
                    findings: Vec::new(),
                    allowed: Vec::new(),
                    error: Some(format!(
                        "ERROR: cannot read staged {}: {error}",
                        absolute.display()
                    )),
                }
            }
        };
        let name = relative.display().to_string();
        scanned.push(name.clone());
        findings.extend(scan_source(&name, &source));
    }
    finish(ScanMode::StagedPaths, scanned, findings)
}

/// Lint STAGED BLOBS: the bytes the commit is made of, supplied by the caller.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-249hz`, sixth instance). [`lint_paths`]
/// selects the STAGED SET and then reads each file from the WORKTREE -- its own error
/// string says *"cannot read staged {path}"* about bytes that are not staged. In a
/// twelve-agent shared checkout the two trees diverge constantly, and both directions are
/// real: a wildcard arm that IS staged but already fixed in the worktree passes the gate
/// and lands (the false green), and one present only in the unstaged worktree refuses a
/// commit that does not contain it (the false red). Measured on the mirror file the same
/// day: worktree 13856429 B / 434 closed rows against index 13796127 B / 428.
///
/// The caller supplies `(repo-relative name, source)` pairs read with `git show :<path>`,
/// so this function touches no filesystem at all and cannot read the wrong tree. Scope is
/// still decided HERE, by [`is_in_scan_scope`], so the staged and repo-wide modes cannot
/// drift apart on which files count -- `staged_mode_over_the_whole_tree_equals_repo_wide`
/// remains the standing proof.
///
/// `lint_paths` stays for the `state-wildcard-lint --staged <paths>` CLI and the sweep,
/// where the worktree IS the subject the operator asked about. On the COMMIT path it is
/// the wrong reader, and the commit path no longer calls it.
pub fn lint_sources<N: AsRef<str>, S: AsRef<str>>(sources: &[(N, S)]) -> LintReport {
    let mut scanned = Vec::new();
    let mut findings = Vec::new();
    for (name, source) in sources {
        let name = name.as_ref();
        if !is_in_scan_scope(Path::new(name)) {
            continue;
        }
        scanned.push(name.to_owned());
        findings.extend(scan_source(name, source.as_ref()));
    }
    finish(ScanMode::StagedPaths, scanned, findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_wildcard_is_not_state_finding() {
        let source = "fn f(value: i32) { match value { 0 => (), _ => () } }";
        assert!(find_findings_in_source(source).is_empty());
    }

    #[test]
    fn state_wildcard_is_finding() {
        let source = "enum PaneState { Working, Idle }\nfn f(state: PaneState) { match state { PaneState::Working => (), _ => () } }";
        let findings = find_findings_in_source(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::WildcardState);
        assert_eq!(findings[0].wildcard_line, 2);
        assert!(
            findings[0].wildcard_text.contains("_ => ()"),
            "the arm must be named: {}",
            findings[0].wildcard_text
        );
    }
}
