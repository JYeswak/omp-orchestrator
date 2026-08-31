#![forbid(unsafe_code)]

//! Conformance coverage for the workspace's declared production lanes.
//!
//! The current extraction baseline declares one lane, `no-shell-gate`. The declaration list grows
//! with each extracted lane; it must never be silently replaced by an empty scan. This suite proves
//! reachability only: a caller can invoke a lane while the invoked mode may still be weaker than the
//! lane's live guarantee.

use std::fs;
use std::path::{Path, PathBuf};

/// Lanes that are correct but deliberately not yet wired. Every exception must name a reason.
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[];

/// Keep this list synchronized with the lanes declared by this workspace.
///
/// Extraction adds rows here. A lane is not considered wired merely because it has a unit test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lane {
    name: &'static str,
    needle: &'static str,
}

const DECLARED_LANES: &[Lane] = &[Lane {
    name: "no-shell-gate",
    needle: "no-shell-gate",
}];

/// The test-code stripping switch is deliberately named so its mutation is attributable.
const STRIP_TEST_CODE: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Rust,
    Workflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallerSource {
    path: PathBuf,
    kind: SourceKind,
    contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallerHit {
    path: PathBuf,
    line: usize,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live beneath the workspace root")
        .to_path_buf()
}

fn is_ignored_directory(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".beads" | ".ntm" | ".zestgraph" | "target")
    )
}

fn is_production_path(path: &Path) -> bool {
    !path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if name == "tests" || name == "test" || name == "fixtures"
        )
    }) && !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.rs"))
}

fn source_kind(path: &Path) -> Option<SourceKind> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => Some(SourceKind::Rust),
        Some("yml" | "yaml") => Some(SourceKind::Workflow),
        _ => None,
    }
}

fn collect_sources(root: &Path) -> Result<Vec<CallerSource>, String> {
    fn visit(root: &Path, directory: &Path, sources: &mut Vec<CallerSource>) -> Result<(), String> {
        let entries = fs::read_dir(directory)
            .map_err(|error| format!("ERROR: read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("ERROR: read directory entry: {error}"))?;
            let path = entry.path();
            if path.is_dir() {
                if !is_ignored_directory(&path) {
                    visit(root, &path, sources)?;
                }
                continue;
            }
            if !is_production_path(&path) {
                continue;
            }
            let Some(kind) = source_kind(&path) else {
                continue;
            };
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("ERROR: read {}: {error}", path.display()))?;
            sources.push(CallerSource {
                path: path
                    .strip_prefix(root)
                    .map_err(|error| format!("ERROR: relativize {}: {error}", path.display()))?
                    .to_path_buf(),
                kind,
                contents,
            });
        }
        Ok(())
    }

    let mut sources = Vec::new();
    visit(root, root, &mut sources)?;
    if sources.is_empty() {
        return Err("ERROR: production caller scan set is empty".to_owned());
    }
    Ok(sources)
}

fn strip_comments(contents: &str, kind: SourceKind) -> String {
    let mut output = String::with_capacity(contents.len());
    let mut in_block_comment = false;
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut escaped = false;
    let bytes = contents.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_block_comment {
            if kind == SourceKind::Rust && byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                in_block_comment = false;
                index += 2;
                continue;
            }
            if byte == b'\n' {
                output.push('\n');
            }
            index += 1;
            continue;
        }

        if !in_double_quote && !in_single_quote {
            if kind == SourceKind::Rust && byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
                in_block_comment = true;
                index += 2;
                continue;
            }
            if kind == SourceKind::Rust && byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            if kind == SourceKind::Workflow && byte == b'#' {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
        }

        output.push(byte as char);
        if escaped {
            escaped = false;
        } else if (in_double_quote || in_single_quote) && byte == b'\\' {
            escaped = true;
        } else if byte == b'"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if byte == b'\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        }
        index += 1;
    }
    output
}

fn brace_delta(line: &str) -> i32 {
    line.bytes().fold(0, |delta, byte| match byte {
        b'{' => delta + 1,
        b'}' => delta - 1,
        _ => delta,
    })
}

fn strip_test_code(contents: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;
    let mut saw_body = false;
    let mut depth = 0;

    for line in contents.lines() {
        if !skipping && line.trim_start().starts_with("#[cfg(test)]") {
            skipping = true;
            saw_body = false;
            depth = 0;
            continue;
        }
        if skipping {
            let delta = brace_delta(line);
            if line.contains('{') {
                saw_body = true;
            }
            depth += delta;
            if saw_body && depth <= 0 {
                skipping = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn cleaned_source(source: &CallerSource, strip_tests: bool) -> String {
    let without_comments = strip_comments(&source.contents, source.kind);
    if source.kind == SourceKind::Rust && strip_tests {
        strip_test_code(&without_comments)
    } else {
        without_comments
    }
}

fn find_caller(
    needle: &str,
    sources: &[CallerSource],
    strip_tests: bool,
) -> Result<Option<CallerHit>, String> {
    if sources.is_empty() {
        return Err("ERROR: production caller scan set is empty".to_owned());
    }
    for source in sources {
        let cleaned = cleaned_source(source, strip_tests);
        if let Some((line, _)) = cleaned
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(needle))
        {
            return Ok(Some(CallerHit {
                path: source.path.clone(),
                line: line + 1,
            }));
        }
    }
    Ok(None)
}

fn validate_allowance(lanes: &[Lane], allowance: &[(&str, &str)]) -> Result<(), String> {
    for (lane, reason) in allowance {
        if lane.trim().is_empty() || reason.trim().is_empty() {
            return Err(
                "ERROR: every unwired-lane allowance entry needs a lane and reason".to_owned(),
            );
        }
        if !lanes.iter().any(|declared| declared.name == *lane) {
            return Err(format!("ERROR: allowance names undeclared lane {lane}"));
        }
    }
    Ok(())
}

fn check_wiring(
    lanes: &[Lane],
    sources: &[CallerSource],
    allowance: &[(&str, &str)],
    strip_tests: bool,
) -> Result<Vec<CallerHit>, String> {
    if lanes.is_empty() {
        return Err("ERROR: declared lane scan set is empty".to_owned());
    }
    if sources.is_empty() {
        return Err("ERROR: production caller scan set is empty".to_owned());
    }
    validate_allowance(lanes, allowance)?;

    let mut hits = Vec::with_capacity(lanes.len());
    for lane in lanes {
        match find_caller(lane.needle, sources, strip_tests)? {
            Some(hit) => hits.push(hit),
            None if allowance.iter().any(|(name, _)| *name == lane.name) => continue,
            None => return Err(format!("UNWIRED LANE: {}", lane.name)),
        }
    }
    Ok(hits)
}

fn rust_source(path: &str, contents: &str) -> CallerSource {
    CallerSource {
        path: PathBuf::from(path),
        kind: SourceKind::Rust,
        contents: contents.to_owned(),
    }
}

fn workflow_source(path: &str, contents: &str) -> CallerSource {
    CallerSource {
        path: PathBuf::from(path),
        kind: SourceKind::Workflow,
        contents: contents.to_owned(),
    }
}

#[test]
fn every_declared_lane_has_a_production_caller() {
    let sources = collect_sources(&repo_root()).expect("production sources must be readable");
    assert!(
        !DECLARED_LANES.is_empty(),
        "empty lane declarations are an error"
    );
    validate_allowance(DECLARED_LANES, UNWIRED_LANE_ALLOWANCE).expect("allowance must be valid");

    let positive = find_caller("actions/checkout", &sources, STRIP_TEST_CODE)
        .expect("positive-control search must run")
        .expect("known wired checkout action must be found");
    assert_eq!(positive.path, PathBuf::from(".github/workflows/gate.yml"));
    assert_eq!(positive.line, 20);

    let hits = check_wiring(
        DECLARED_LANES,
        &sources,
        UNWIRED_LANE_ALLOWANCE,
        STRIP_TEST_CODE,
    )
    .expect("every declared lane must have a production caller");
    assert_eq!(hits.len(), DECLARED_LANES.len());
}

#[test]
fn planted_unwired_lane_is_red_then_green_in_one_run() {
    let lane = Lane {
        name: "planted-lane",
        needle: "planted-lane",
    };
    let test_only = rust_source(
        "src/planted.rs",
        "#[cfg(test)]\nmod tests {\n    fn fake() { let _ = \"planted-lane\"; }\n}\n",
    );
    let no_caller = check_wiring(&[lane], &[test_only.clone()], &[], STRIP_TEST_CODE);
    assert_eq!(no_caller, Err("UNWIRED LANE: planted-lane".to_owned()));

    let wired = rust_source(
        "src/production.rs",
        "fn run() { invoke(\"planted-lane\"); }\n",
    );
    let green = check_wiring(&[lane], &[test_only, wired], &[], STRIP_TEST_CODE);
    assert!(
        green.is_ok(),
        "a real production caller must turn the planted lane green"
    );
}

#[test]
fn comments_and_test_only_code_do_not_prove_wiring() {
    let source = rust_source(
        "src/commented.rs",
        "// comment-only-lane\n#[cfg(test)]\nmod tests {\n    fn fake() { let _ = \"comment-only-lane\"; }\n}\n",
    );
    let hit = find_caller("comment-only-lane", &[source], STRIP_TEST_CODE)
        .expect("caller search must run");
    assert!(
        hit.is_none(),
        "comments and cfg(test) code must not prove wiring"
    );
}

#[test]
fn empty_scan_sets_are_errors_not_passes() {
    assert_eq!(
        check_wiring(DECLARED_LANES, &[], UNWIRED_LANE_ALLOWANCE, STRIP_TEST_CODE),
        Err("ERROR: production caller scan set is empty".to_owned())
    );
    assert_eq!(
        check_wiring(&[], &[], UNWIRED_LANE_ALLOWANCE, STRIP_TEST_CODE),
        Err("ERROR: declared lane scan set is empty".to_owned())
    );
}

#[test]
fn allowance_is_empty_or_requires_a_reason() {
    assert!(UNWIRED_LANE_ALLOWANCE.is_empty());
    assert!(validate_allowance(DECLARED_LANES, UNWIRED_LANE_ALLOWANCE).is_ok());
    assert!(validate_allowance(DECLARED_LANES, &[("no-shell-gate", "")]).is_err());
}

#[test]
fn workflow_comments_are_removed_without_breaking_quoted_values() {
    let source = workflow_source(
        ".github/workflows/fixture.yml",
        "# quoted-lane\nrun: \"quoted-lane # value\" # comment-only-lane\n",
    );
    let hit = find_caller("quoted-lane # value", &[source], STRIP_TEST_CODE)
        .expect("caller search must run")
        .expect("quoted workflow value must remain searchable");
    assert_eq!(hit.line, 2);
    assert!(find_caller("comment-only-lane", &[], STRIP_TEST_CODE).is_err());
}
