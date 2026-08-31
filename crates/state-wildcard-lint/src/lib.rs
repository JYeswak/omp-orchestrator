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
use std::path::{Path, PathBuf};

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
    pub scrutinee: String,
    pub inferred_type: Option<String>,
    pub kind: FindingKind,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = self.inferred_type.as_deref().unwrap_or("unknown");
        write!(
            formatter,
            "{}: match line {}, wildcard arm line {}, scrutinee={:?}, type={}, kind={}",
            self.file,
            self.match_line,
            self.wildcard_line,
            self.scrutinee,
            ty,
            self.kind
        )
    }
}

/// Result of scanning a repository source root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintReport {
    pub scanned: Vec<String>,
    pub findings: Vec<Finding>,
    pub error: Option<String>,
}

impl LintReport {
    pub fn is_pass(&self) -> bool {
        self.error.is_none() && !self.scanned.is_empty() && self.findings.is_empty()
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
    let compact: String = type_name.chars().filter(|character| !character.is_whitespace()).collect();
    let lower = compact.to_ascii_lowercase();
    lower == "bool"
        || lower == "char"
        || lower == "string"
        || lower == "&str"
        || lower == "str"
        || lower.starts_with('u') && lower[1..].chars().all(|character| character.is_ascii_digit())
        || lower.starts_with('i') && lower[1..].chars().all(|character| character.is_ascii_digit())
        || lower.starts_with('f') && lower[1..].chars().all(|character| character.is_ascii_digit())
        || lower.starts_with("option<")
        || lower.starts_with("result<")
        || lower.starts_with("vec<")
        || lower.starts_with("hashmap<")
        || lower.starts_with("btreemap<")
}

fn wildcard_arm_line(code: &[String], start: usize, end: usize) -> Option<usize> {
    (start + 1..=end).find(|index| {
        let trimmed = code[*index].trim_start();
        trimmed.starts_with("_ =>") || trimmed.starts_with("_=>") || trimmed.starts_with("_ if")
    })
}

fn type_for_match(
    scrutinee: &str,
    body: &str,
    bindings: &BTreeMap<String, String>,
    state_enums: &BTreeSet<String>,
) -> (Option<String>, bool) {
    for enum_name in state_enums {
        if body.contains(&format!("{enum_name}::")) {
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
pub fn find_findings_in_source(source: &str) -> Vec<(usize, usize, String, Option<String>, FindingKind)> {
    let code: Vec<String> = source.lines().map(mask_line).collect();
    let (all_enums, state_enums) = parse_enum_names(&code);
    let bindings = parse_typed_bindings(&code);
    let mut findings = Vec::new();
    for (start, line) in code.iter().enumerate() {
        let Some(match_position) = line.find("match ") else { continue };
        let after_match = &line[match_position + 6..];
        let Some(end) = match_end(&code, start) else { continue };
        let scrutinee = normalized_scrutinee(after_match.split('{').next().unwrap_or(after_match));
        let Some(wildcard) = wildcard_arm_line(&code, start, end) else { continue };
        let body = code[start..=end].join("\n");
        let (inferred_type, state_candidate) = type_for_match(&scrutinee, &body, &bindings, &state_enums);
        if let Some(type_name) = inferred_type.as_deref() {
            if state_enums.contains(type_name) {
                findings.push((start + 1, wildcard + 1, scrutinee, inferred_type, FindingKind::WildcardState));
            } else if !all_enums.contains(type_name) && state_candidate {
                findings.push((start + 1, wildcard + 1, scrutinee, inferred_type, FindingKind::UnresolvedStateType));
            }
        } else if state_candidate {
            findings.push((start + 1, wildcard + 1, scrutinee, inferred_type, FindingKind::UnresolvedStateType));
        }
    }
    findings
}

fn scan_source(file: &str, source: &str) -> Vec<Finding> {
    find_findings_in_source(source)
        .into_iter()
        .map(|(match_line, wildcard_line, scrutinee, inferred_type, kind)| Finding {
            file: file.to_owned(),
            match_line,
            wildcard_line,
            scrutinee,
            inferred_type,
            kind,
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
            if !matches!(name, ".git" | "target" | "tests" | "fixtures") {
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

/// Scan production Rust sources below a repository's crates directory.
pub fn lint_workspace(root: &Path) -> LintReport {
    let source_root = root.join("crates");
    let mut scanned = Vec::new();
    let mut findings = Vec::new();
    if let Err(error) = visit_rs(root, &source_root, &mut scanned, &mut findings) {
        return LintReport {
            scanned: Vec::new(),
            findings: Vec::new(),
            error: Some(error),
        };
    }
    scanned.sort();
    findings.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.wildcard_line.cmp(&right.wildcard_line))
    });
    LintReport {
        scanned,
        findings,
        error: None,
    }
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
        assert_eq!(findings[0].4, FindingKind::WildcardState);
    }
}
