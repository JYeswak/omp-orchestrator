//! Live gate-bead acceptance path census.
//!
//! A gate acceptance that names a missing repository input is not a refusal; it is an
//! unexecutable instruction. This test keeps that distinction loud for every authored
//! gate acceptance. Gate beads whose acceptance field is still empty are a separate
//! dispatchability defect and are not silently treated as path evidence.

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("gate-runner repository root resolves")
}

fn strip_line_suffix(token: &str) -> &str {
    let Some(index) = token.find(':') else {
        return token;
    };
    let suffix = &token[index + 1..];
    if !suffix.is_empty()
        && suffix
            .chars()
            .all(|character| character.is_ascii_digit() || character == '-')
    {
        &token[..index]
    } else {
        token
    }
}

fn repository_path_token(raw: &str) -> Option<String> {
    let mut token = raw.trim_matches(|character: char| "`'\"()[]{}<>,;".contains(character));
    while token
        .chars()
        .last()
        .is_some_and(|character| "`.".contains(character))
    {
        token = &token[..token.len() - 1];
    }
    token = strip_line_suffix(token);
    if token.contains('<') || token.contains('>') || token.contains("...") {
        return None;
    }
    let is_repository_path = token.starts_with("docs/")
        || token.starts_with("crates/")
        || token.starts_with(".github/")
        || matches!(token, "AGENTS.md" | "CLAUDE.md" | "Cargo.toml" | "SCHEMAS.toml");
    is_repository_path.then(|| token.to_owned())
}


fn repository_line_range_token(raw: &str) -> Option<String> {
    let mut token = raw.trim_matches(|character: char| "'\"()[]{}<>,;".contains(character));
    while token
        .chars()
        .last()
        .is_some_and(|character| character == char::from(96) || character == '.')
    {
        token = &token[..token.len() - 1];
    }
    let index = token.find(':')?;
    let suffix = &token[index + 1..];
    if suffix.is_empty()
        || !suffix
            .chars()
            .all(|character| character.is_ascii_digit() || character == '-')
    {
        return None;
    }
    repository_path_token(&token[..index]).map(|path| format!("{path}:{suffix}"))
}

fn validate_no_line_ranges(input: &str) -> Result<usize, String> {
    let rows = gate_rows(input)?;
    let mut scanned = 0usize;
    for (bead, acceptance) in rows {
        for raw in acceptance.split_whitespace() {
            if let Some(path) = repository_line_range_token(raw) {
                return Err(format!(
                    "GATE_ACCEPTANCE_LINE_RANGE_FORBIDDEN bead={bead} path={path}"
                ));
            }
            if repository_path_token(raw).is_some() {
                scanned += 1;
            }
        }
    }
    if scanned == 0 {
        return Err("GATE_ACCEPTANCE_CENSUS_EMPTY reason=no_repository_paths".to_owned());
    }
    Ok(scanned)
}

fn gate_rows(input: &str) -> Result<Vec<(String, String)>, String> {
    if input.trim().is_empty() {
        return Err("GATE_ACCEPTANCE_CENSUS_EMPTY reason=tracker_input_empty".to_owned());
    }
    let mut rows = Vec::new();
    for (line_index, raw) in input.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(raw).map_err(|error| {
            format!("GATE_ACCEPTANCE_CENSUS_UNREADABLE line={} detail={error}", line_index + 1)
        })?;
        let Some(id) = value.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !id.starts_with("omp-orchestrator-gate-") {
            continue;
        }
        let acceptance = value
            .get("acceptance_criteria")
            .and_then(Value::as_str)
            .unwrap_or("");
        if acceptance.trim().is_empty() {
            continue;
        }
        rows.push((id.to_owned(), acceptance.to_owned()));
    }
    if rows.is_empty() {
        return Err("GATE_ACCEPTANCE_CENSUS_EMPTY reason=no_authored_gate_acceptances".to_owned());
    }
    Ok(rows)
}

fn validate_gate_acceptance_paths(input: &str, root: &Path) -> Result<usize, String> {
    let rows = gate_rows(input)?;
    let mut references = BTreeSet::new();
    let mut missing = Vec::new();
    for (bead, acceptance) in rows {
        for raw in acceptance.split_whitespace() {
            let Some(path) = repository_path_token(raw) else {
                continue;
            };
            if !references.insert((bead.clone(), path.clone())) {
                continue;
            }
            if !root.join(&path).exists() {
                missing.push(format!(
                    "GATE_ACCEPTANCE_PATH_MISSING bead={bead} path={path}"
                ));
            }
        }
    }
    if missing.is_empty() {
        Ok(references.len())
    } else {
        Err(missing.join("\n"))
    }
}

#[test]
fn live_gate_acceptance_paths_all_exist() {
    let root = repo_root();
    let input = fs::read_to_string(root.join(".beads/issues.jsonl"))
        .expect("the bead mirror must be readable");
    let references = validate_gate_acceptance_paths(&input, &root)
        .unwrap_or_else(|error| panic!("gate acceptance path census failed:\n{error}"));
    assert!(references > 0, "positive control: no repository paths were scanned");
}



#[test]
fn live_gate_acceptance_citations_have_no_line_ranges() {
    let root = repo_root();
    let input = fs::read_to_string(root.join(".beads/issues.jsonl"))
        .expect("the bead mirror must be readable");
    let scanned = validate_no_line_ranges(&input)
        .unwrap_or_else(|error| panic!("gate acceptance line-range census failed: {error}"));
    assert!(scanned > 0, "positive control: no repository paths were scanned");
}

#[test]
fn line_range_citation_is_a_typed_refusal() {
    let input = r#"{"id":"omp-orchestrator-gate-fixture","acceptance_criteria":"read docs/plan/flow/CONTRACT.md:12-15"}"#;
    let error = validate_no_line_ranges(input).expect_err("line-range citations must be refused");
    assert_eq!(
        error,
        "GATE_ACCEPTANCE_LINE_RANGE_FORBIDDEN bead=omp-orchestrator-gate-fixture path=docs/plan/flow/CONTRACT.md:12-15"
    );
}

#[test]
fn missing_box_is_a_typed_refusal() {
    let input = r#"{"id":"omp-orchestrator-gate-fixture","acceptance_criteria":"read docs/plan/flow/boxes/guaranteed-absent.json"}"#;
    let error = validate_gate_acceptance_paths(input, Path::new("."))
        .expect_err("a missing box must be refused");
    assert_eq!(
        error,
        "GATE_ACCEPTANCE_PATH_MISSING bead=omp-orchestrator-gate-fixture path=docs/plan/flow/boxes/guaranteed-absent.json"
    );
}

#[test]
fn empty_gate_input_is_a_typed_error() {
    let error = validate_gate_acceptance_paths("", Path::new("."))
        .expect_err("an empty gate corpus must be refused");
    assert_eq!(
        error,
        "GATE_ACCEPTANCE_CENSUS_EMPTY reason=tracker_input_empty"
    );
}
