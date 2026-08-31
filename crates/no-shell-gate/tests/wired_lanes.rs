#![forbid(unsafe_code)]

//! Conformance coverage for the workspace's declared production lanes.
//!
//! The current extraction baseline declares one lane, `no-shell-gate`. The declaration list grows
//! with each extracted lane; it must never be silently replaced by an empty scan. This suite proves
//! reachability only: a caller can invoke a lane while the invoked mode may still be weaker than the
//! lane's live guarantee.

use std::fs;
use std::path::{Path, PathBuf};

/// Lanes that are correct but deliberately not yet wired. Every exception must name a
/// lane AND a reason; a row naming an undeclared lane is an error, not a pass. Silence
/// is forbidden — but so is an exception without its story (omp-orchestrator-0hk).
///
/// MAINTENANCE CONTRACT: rows are checked against the DERIVED lane set at every run.
/// Stale rows are refused ("allowance names undeclared lane ...") — which fired live
/// when extraction removed two members mid-grade. Rows come out as wiring lands (-kxe).
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[
    (
        "composer-typed",
        "wiring lands with -kxe (conductor lifecycle binary); extraction in flight",
    ),
    (
        "fleet-composite",
        "wiring lands with -kxe (conductor lifecycle binary); extraction in flight",
    ),
    (
        "loop-queue-filter",
        "wiring lands with -kxe (conductor lifecycle binary); extraction in flight",
    ),
    (
        "pane-dispatch-fence",
        "wiring lands with -kxe (conductor lifecycle binary); extraction in flight",
    ),
    (
        "tick-monitor",
        "wiring lands with -kxe (conductor lifecycle binary); extraction in flight",
    ),
];

/// A workspace lane: one member crate, derived — NEVER hand-listed. A hand-listed
/// expectation set is the same defect control-plane carries (check.sh EXPECTED_GATES
/// hand-lists gates while the verdict claims completeness): the list drifts and the
/// suite reports vacuously green while most lanes are unexamined (bead -0hk, found
/// by the -a3p grade). The needle pair covers both real caller forms: the hyphen
/// name as CI/-p/subprocess references spell it, and the underscore name as other
/// crates import it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Lane {
    name: String,
    needle_hyphen: String,
    needle_underscore: String,
}

/// Derive the lane set from the workspace member crates on disk.
///
/// An empty or unreadable derivation is an ERROR, never a pass: a deliverable that
/// was never checked must never report like one that passed.
fn derive_lanes(root: &Path) -> Result<Vec<Lane>, String> {
    let crates_dir = root.join("crates");
    let entries = fs::read_dir(&crates_dir)
        .map_err(|error| format!("ERROR: read {}: {error}", crates_dir.display()))?;
    let mut lanes = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("ERROR: read directory entry: {error}"))?;
        let path = entry.path();
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let name = name.to_owned();
        if name.trim().is_empty() {
            return Err("ERROR: workspace member crate with an empty name".to_owned());
        }
        lanes.push(Lane {
            needle_hyphen: name.clone(),
            needle_underscore: name.replace('-', "_"),
            name,
        });
    }
    if lanes.is_empty() {
        return Err(format!(
            "ERROR: derived lane set is empty — no member crates found under {}",
            crates_dir.display()
        ));
    }
    lanes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(lanes)
}

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

/// Find a production caller for one lane, skipping the lane's own crate: a lane
/// naming ITSELF is not wiring. Both real caller forms are searched — the hyphen
/// name (CI jobs, `-p` flags, subprocess invocations) and the underscore name
/// (`use` statements from other crates).
fn find_caller(
    lane: &Lane,
    sources: &[CallerSource],
    strip_tests: bool,
) -> Result<Option<CallerHit>, String> {
    // An EMPTY needle matches every line — the inverse vacuity of a never-matching
    // needle — so both needles must be non-empty before the scan runs.
    if lane.needle_hyphen.trim().is_empty() || lane.needle_underscore.trim().is_empty() {
        return Err(format!(
            "ERROR: lane {} has an empty caller needle",
            lane.name
        ));
    }
    if sources.is_empty() {
        return Err("ERROR: production caller scan set is empty".to_owned());
    }
    let own_prefix = format!("crates/{}/", lane.name);
    for source in sources {
        if source.path.starts_with(&own_prefix) {
            continue;
        }
        let cleaned = cleaned_source(source, strip_tests);
        if let Some((line, _)) = cleaned.lines().enumerate().find(|(_, line)| {
            line.contains(&lane.needle_hyphen) || line.contains(&lane.needle_underscore)
        }) {
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
        match find_caller(lane, sources, strip_tests)? {
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

    // The lane set is DERIVED from the workspace (bead -0hk): a hand-listed
    // DECLARED_LANES is the defect this crate exists to prevent. An empty or
    // unreadable derivation is an error, never a pass.
    let lanes = derive_lanes(&repo_root()).expect("lane derivation must be readable and non-empty");
    validate_allowance(&lanes, UNWIRED_LANE_ALLOWANCE).expect("allowance must be valid");

    let positive = find_caller(&positive_control(), &sources, STRIP_TEST_CODE)
        .expect("positive-control search must run")
        .expect("known wired checkout action must be found");
    assert_eq!(positive.path, PathBuf::from(".github/workflows/gate.yml"));
    assert_eq!(positive.line, 20);

    let hits = check_wiring(&lanes, &sources, UNWIRED_LANE_ALLOWANCE, STRIP_TEST_CODE)
        .expect("every workspace lane must be wired or carry a named allowance reason");
    let allowlisted = UNWIRED_LANE_ALLOWANCE.len();
    assert_eq!(
        hits.len(),
        lanes.len() - allowlisted,
        "wired hits must account for every lane minus the named allowances"
    );
}

/// A lane the suite knows is wired, used as the caller-search positive control
/// (criterion 6 of bead -a3p, carried here): a zero from a pattern that can never
/// match is not evidence of absence.
fn positive_control() -> Lane {
    Lane {
        name: "positive-control".to_owned(),
        needle_hyphen: "actions/checkout".to_owned(),
        needle_underscore: "actions/checkout".to_owned(),
    }
}

#[test]
fn planted_unwired_lane_is_red_then_green_in_one_run() {
    let lane = Lane {
        name: "planted-lane".to_owned(),
        needle_hyphen: "planted-lane".to_owned(),
        needle_underscore: "planted_lane".to_owned(),
    };
    let test_only = rust_source(
        "src/planted.rs",
        "#[cfg(test)]\nmod tests {\n    fn fake() { let _ = \"planted-lane\"; }\n}\n",
    );
    let no_caller = check_wiring(&[lane.clone()], &[test_only.clone()], &[], STRIP_TEST_CODE);
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
    let lane = Lane {
        name: "comment-only-lane".to_owned(),
        needle_hyphen: "comment-only-lane".to_owned(),
        needle_underscore: "comment_only_lane".to_owned(),
    };
    let hit = find_caller(&lane, &[source], STRIP_TEST_CODE).expect("caller search must run");
    assert!(
        hit.is_none(),
        "comments and cfg(test) code must not prove wiring"
    );
}
#[test]
fn empty_scan_sets_are_errors_not_passes() {
    let lanes = derive_lanes(&repo_root()).expect("derivation must work in this repo");
    assert_eq!(
        check_wiring(&lanes, &[], UNWIRED_LANE_ALLOWANCE, STRIP_TEST_CODE),
        Err("ERROR: production caller scan set is empty".to_owned())
    );
    assert_eq!(
        check_wiring(&[], &[], UNWIRED_LANE_ALLOWANCE, STRIP_TEST_CODE),
        Err("ERROR: declared lane scan set is empty".to_owned())
    );
}

#[test]
fn every_allowance_row_names_a_lane_and_carries_a_reason() {
    let lanes = derive_lanes(&repo_root()).expect("derivation must work in this repo");
    // Silence is forbidden: with derivation, the allowance legitimately carries named
    // rows for lanes whose wiring lands later — but every row must name a DERIVED
    // lane and carry a reason. A row for an undeclared lane, or a row with an empty
    // reason, is an error, not a pass.
    validate_allowance(&lanes, UNWIRED_LANE_ALLOWANCE).expect("allowance must validate");
    assert!(
        validate_allowance(&lanes, &[("not-a-workspace-crate", "a reason")]).is_err(),
        "an allowance row naming an undeclared lane must be rejected"
    );
    assert!(
        validate_allowance(&lanes, &[("no-shell-gate", "")]).is_err(),
        "an allowance row without a reason must be rejected"
    );
    // No invented rows (validated above), and no silent gaps (check_wiring enforces
    // them): the allowance is the only sanctioned unwired state.
    for (name, reason) in UNWIRED_LANE_ALLOWANCE {
        assert!(!reason.trim().is_empty(), "allowance {name} has an empty reason");
    }
}

#[test]
fn derivation_is_an_error_when_the_workspace_is_unreadable() {
    // An empty or unreadable derivation is an ERROR, never a pass: a gate pointed at
    // a root with no member crates must refuse, not report green.
    let empty_root = std::env::temp_dir().join(format!("wl-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty_root).expect("create empty root");
    assert!(derive_lanes(&empty_root).is_err(), "an empty derivation must be an error");
    let _ = std::fs::remove_dir_all(&empty_root);
}

#[test]
fn a_lane_naming_itself_is_not_wired() {
    // Self-exclusion: a crate's own source mentioning its own name proves nothing.
    let lane = Lane {
        name: "selfy".to_owned(),
        needle_hyphen: "selfy".to_owned(),
        needle_underscore: "selfy".to_owned(),
    };
    let self_source = rust_source(
        "crates/selfy/src/lib.rs",
        "pub fn init() { register(\"selfy\"); }\n",
    );
    let other_source = rust_source("crates/other/src/lib.rs", "pub fn unrelated() {}\n");
    let verdict = check_wiring(&[lane], &[self_source, other_source], &[], STRIP_TEST_CODE);
    assert_eq!(verdict, Err("UNWIRED LANE: selfy".to_owned()));
}

#[test]
fn workflow_comments_are_removed_without_breaking_quoted_values() {
    let quoted = Lane {
        name: "quoted-lane".to_owned(),
        needle_hyphen: "quoted-lane # value".to_owned(),
        needle_underscore: "quoted_lane".to_owned(),
    };
    let commented = Lane {
        name: "comment-only-lane".to_owned(),
        needle_hyphen: "comment-only-lane".to_owned(),
        needle_underscore: "comment_only_lane".to_owned(),
    };
    let source = workflow_source(
        ".github/workflows/fixture.yml",
        "# quoted-lane\nrun: \"quoted-lane # value\" # comment-only-lane\n",
    );
    let hit = find_caller(&quoted, &[source.clone()], STRIP_TEST_CODE)
        .expect("caller search must run")
        .expect("quoted workflow value must remain searchable");
    assert_eq!(hit.line, 2);
    // The comment part of that same line must not itself count as a caller when
    // searched as its own lane: the scan runs (non-empty set) and finds nothing.
    assert!(
        find_caller(&commented, &[source], STRIP_TEST_CODE)
            .expect("caller search must run")
            .is_none(),
        "a workflow comment must not prove wiring even trailing a quoted value"
    );
}
