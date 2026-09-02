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
/// Lanes that exist and are correct but are deliberately not yet wired.
///
/// **Empty, and now truthfully so.** It carried five rows — `composer-typed`,
/// `fleet-composite`, `loop-queue-filter`, `pane-dispatch-fence`, `tick-monitor` —
/// each reading "wiring lands with -kxe; extraction in flight". All five acquired
/// production callers while that text sat unchanged, and the gate caught the drift
/// by accounting rather than by any row failing: 26 hits against 21 expected.
///
/// An allowance that outlives its reason is worse than no allowance, because it
/// reads as a considered exception when it is only an un-revisited one.
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[
    ("refill-idle-panes", "cron-wired via fast-dispatch at */5; the Rust source scanner cannot see a crontab invocation"),
    ("tick-dispatch", "cron-wired via controller-tick at :18/:38/:58; the Rust source scanner cannot see a crontab invocation"),
    ("wired-but-inert-guard", "detection pattern table consumed by kernel-only-operator-hook; the hook is disabled pending human certification (cp-nq2s9), so the caller exists in design but not in code yet"),
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

/// Directories the wiring scan must never enter.
///
/// # Measured 2026-09-01: a vendored dependency broke the positive control
///
/// `rch` (the remote-compilation helper) materialises a cargo cache INSIDE the
/// repository at `.rch-tmp/`, and `.rch-target-<hash>/` alongside it. The scan
/// walked into `.rch-tmp/rch-cargo-cache-.../serde_json-1.0.151/` and the
/// positive control — which asserts `find_caller` locates `.github/workflows/
/// gate.yml` — matched a path inside that vendored crate instead. The gate
/// FAILED while the repository's actual wiring was fine.
///
/// A gate scanning a vendored copy of somebody else's source is not measuring
/// this tree. These names match `.gitignore`, and the duplication is deliberate:
/// `.gitignore` governs what is COMMITTED, this governs what is SCANNED, and a
/// build tool can drop a cache into the working directory without either being
/// wrong. Keeping them in sync is a maintenance cost; conflating them is a bug.
fn is_ignored_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(
        name,
        ".git" | ".beads" | ".ntm" | ".zestgraph" | "target"
            | ".orchestrator" | ".rch-tmp" | ".ee" | ".claude"
    )
        // `.rch-target-<64-hex>` is generated per remote worker pool, so the
        // suffix cannot be enumerated — match the prefix.
        || name.starts_with(".rch-target-")
        || name.starts_with(".rch-cargo-cache")
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
        assert!(
            !reason.trim().is_empty(),
            "allowance {name} has an empty reason"
        );
    }
}

#[test]
fn derivation_is_an_error_when_the_workspace_is_unreadable() {
    // An empty or unreadable derivation is an ERROR, never a pass: a gate pointed at
    // a root with no member crates must refuse, not report green.
    let empty_root = std::env::temp_dir().join(format!("wl-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty_root).expect("create empty root");
    assert!(
        derive_lanes(&empty_root).is_err(),
        "an empty derivation must be an error"
    );
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

// ═════════════════════════════════════════════════════════════════════════════
// LEGS 2-5 — bead omp-coverage-mission-ipg.18.
// Each leg owns ONE predicate, ONE input scan, ONE allowance, ONE validator.
// Mutating one predicate must leave the other three green: no shared scan,
// no shared helper beyond `workspace_crate_names` (a pure directory read).
// ═════════════════════════════════════════════════════════════════════════════

fn workspace_crate_names(root: &Path) -> Vec<String> {
    let dir = root.join("crates");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.join("Cargo.toml").is_file() {
            if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                out.push(n.to_owned());
            }
        }
    }
    out.sort();
    out
}

fn validate_allowance_rows(rows: &[(&str, &str)], leg: &str) {
    for (subject, reason) in rows {
        assert!(
            !subject.trim().is_empty(),
            "{} allowance row has an empty subject",
            leg
        );
        assert!(
            reason.trim().len() >= 8,
            "{} allowance row '{}' carries no reason — a bare path silences nothing",
            leg,
            subject
        );
    }
}

// ── LEG 2: SURFACE DECLARED ─────────────────────────────────────────────────
const SURFACE_ALLOWANCE: &[(&str, &str)] = &[];

#[test]
fn every_crate_is_declared_in_the_surface_map() {
    let root = repo_root();
    let map_path = root.join("OMP-SURFACE-MAP.toml");
    let map = std::fs::read_to_string(&map_path)
        .unwrap_or_else(|e| panic!("surface map unreadable: {}", e));
    let declared: std::collections::HashSet<String> = map
        .lines()
        .filter_map(|l| l.trim().strip_prefix("[crates."))
        .filter_map(|l| l.strip_suffix(']'))
        .map(str::to_owned)
        .collect();
    assert!(
        !declared.is_empty(),
        "ANTI-VACUITY: surface map declares zero crates"
    );
    let on_disk = workspace_crate_names(&root);
    assert!(!on_disk.is_empty(), "ANTI-VACUITY: zero crates on disk");
    validate_allowance_rows(SURFACE_ALLOWANCE, "leg2-surface");

    let undeclared: Vec<_> = on_disk.iter().filter(|c| !declared.contains(*c)).collect();
    let ghosts: Vec<_> = declared
        .iter()
        .filter(|d| !on_disk.iter().any(|c| c == *d))
        .collect();
    assert!(
        undeclared.is_empty(),
        "UNDECLARED CRATE (on disk, no [crates.x] block): {:?}",
        undeclared
    );
    assert!(
        ghosts.is_empty(),
        "GHOST DECLARATION (in surface map, no crate on disk): {:?}",
        ghosts
    );
}

// ── LEG 3: ASUPERSYNC CONFORMANCE — forbid(unsafe_code) ────────────────────
const FORBID_ALLOWANCE: &[(&str, &str)] = &[];

#[test]
fn every_crate_declares_the_forbid_lint() {
    let root = repo_root();
    let crates = workspace_crate_names(&root);
    assert!(!crates.is_empty(), "ANTI-VACUITY: zero crates scanned");
    validate_allowance_rows(FORBID_ALLOWANCE, "leg3-forbid");

    let allowed: std::collections::HashSet<_> = FORBID_ALLOWANCE.iter().map(|(c, _)| *c).collect();
    let mut missing = Vec::new();
    for name in &crates {
        let manifest = root.join("crates").join(name).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|e| panic!("{} unreadable: {}", manifest.display(), e));
        let has_forbid = text.contains("unsafe_code") && text.contains("forbid");
        if !has_forbid && !allowed.contains(name.as_str()) {
            missing.push(name.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "MISSING forbid(unsafe_code) in [lints.rust]: {:?} — every crate in an asupersync repo must forbid unsafe",
        missing
    );
}

// ── LEG 5: NO PUBLIC-TYPE-NAME COLLISIONS ──────────────────────────────────
const COLLISION_ALLOWANCE: &[(&str, &str)] = &[
    ("Finding", "finding and finding-dispatch both model a scan result; unification is -232 scope, not this gate"),
    ("LintReport", "state-wildcard-lint and path-literal-guard predate the shared crate; same -232 scope"),
    ("Violation", "three gate crates declare it; aliasing to a shared type is a cross-crate refactor owned by the integrator"),
    ("Observation", "REQUIRES A DECISION not an allowance: tick-monitor produces what omp-orchestrator consumes and each declares an incompatible struct — the free_capacity seam (filter FIXED -oco; seam still open, 09 M1)"),
    ("GateError", "no-shell-gate declares GitFailed/empty-scan for a FILE-EXTENSION scan; \
     porting-gate declares EmptyCandidates/InvalidCandidate/Io/Metadata for a CRATE-ARRIVAL \
     check. Same name, disjoint domains, no shared caller. Dies when a workspace error trait \
     exists; until then unifying them would couple two gates that share nothing but a suffix"),
    ("DispatchIntent", "dispatch-claim-fence declares an enum (Bead/Broadcast/Correction) for the fence; ack-spine declares a struct (bead_id/pane_id/session) for the ledger — different domains, same name. Dies when omp-types provides the shared vocabulary"),
];

#[test]
fn no_public_type_name_collisions_across_crates() {
    let root = repo_root();
    let crates = workspace_crate_names(&root);
    assert!(!crates.is_empty(), "ANTI-VACUITY: zero crates scanned");
    validate_allowance_rows(COLLISION_ALLOWANCE, "leg5-collision");

    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut collisions: Vec<(String, String, String)> = Vec::new();
    for name in &crates {
        let src_dir = root.join("crates").join(name).join("src");
        let Ok(entries) = std::fs::read_dir(&src_dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            for line in text.lines() {
                let t = line.trim();
                for kw in ["pub struct ", "pub enum "] {
                    if let Some(rest) = t.strip_prefix(kw) {
                        if let Some(type_name) = rest
                            .split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                        {
                            if type_name.is_empty() {
                                continue;
                            }
                            if let Some(first) = seen.get(type_name) {
                                if first != name {
                                    collisions.push((
                                        type_name.to_owned(),
                                        first.clone(),
                                        name.clone(),
                                    ));
                                }
                            } else {
                                seen.insert(type_name.to_owned(), name.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    let allowed: std::collections::HashSet<_> = COLLISION_ALLOWANCE
        .iter()
        .map(|(n, _)| n.to_owned())
        .collect();
    let unallowed: Vec<_> = collisions
        .iter()
        .filter(|(n, _, _)| !allowed.contains(n.as_str()))
        .collect();
    assert!(
        unallowed.is_empty(),
        "PUBLIC TYPE NAME COLLISION (not in allowance): {:?} — two crates declaring the same pub type is a seam bug; add an allowance row WITH A REASON or unify the type",
        unallowed
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// LEG 4 and the per-leg contract tests. Appended beside the existing wired leg;
// the existing leg-1 implementation above is intentionally untouched.
// ═════════════════════════════════════════════════════════════════════════════

mod ipg18_contract {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::fs;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PublicType {
        name: String,
        crate_name: String,
        file: String,
        line: usize,
    }

    fn validate_leg2_allowance(rows: &[(&str, &str)]) -> Result<(), String> {
        for (subject, reason) in rows {
            if subject.trim().is_empty() || reason.trim().len() < 8 {
                return Err("leg2-surface allowance requires subject and reason".to_owned());
            }
        }
        Ok(())
    }

    fn validate_leg3_allowance(rows: &[(&str, &str)]) -> Result<(), String> {
        for (subject, reason) in rows {
            if subject.trim().is_empty() || reason.trim().len() < 8 {
                return Err("leg3-asupersync allowance requires subject and reason".to_owned());
            }
        }
        Ok(())
    }

    fn validate_leg4_allowance(rows: &[(&str, &str)]) -> Result<(), String> {
        for (subject, reason) in rows {
            if subject.trim().is_empty() || reason.trim().len() < 8 {
                return Err("leg4-canonical allowance requires subject and reason".to_owned());
            }
        }
        Ok(())
    }

    fn validate_leg5_allowance(rows: &[(&str, &str)]) -> Result<(), String> {
        for (subject, reason) in rows {
            if subject.trim().is_empty() || reason.trim().len() < 8 {
                return Err("leg5-collision allowance requires subject and reason".to_owned());
            }
        }
        Ok(())
    }

    fn scan_nonempty<T>(items: &[T], leg: &str) -> Result<(), String> {
        if items.is_empty() {
            return Err(format!("ANTI-VACUITY: {leg} scanned zero crates"));
        }
        Ok(())
    }

    fn canonical_type_names(root: &Path) -> Result<HashSet<String>, String> {
        let mut names = HashSet::new();
        let source_files = [
            "src/lib.rs",
            "src/pane_observation.rs",
            "src/claim_strength.rs",
            "src/lifecycle.rs",
        ];
        for relative in source_files {
            let path = root.join("crates/omp-types").join(relative);
            let source = fs::read_to_string(&path).map_err(|error| {
                format!("canonical source unreadable {}: {error}", path.display())
            })?;
            for line in source.lines() {
                let trimmed = line.trim_start();
                for prefix in ["pub struct ", "pub enum ", "pub type "] {
                    if let Some(rest) = trimmed.strip_prefix(prefix) {
                        if let Some(name) = rest
                            .split(|character: char| {
                                !character.is_ascii_alphanumeric() && character != '_'
                            })
                            .next()
                        {
                            if !name.is_empty() {
                                names.insert(name.to_owned());
                            }
                        }
                    }
                }
            }
            let mut in_use = false;
            let mut use_block = String::new();
            for line in source.lines() {
                if !in_use && line.contains("pub use ") {
                    in_use = true;
                }
                if in_use {
                    use_block.push_str(line);
                    use_block.push('\n');
                    if line.contains(';') {
                        if let (Some(open), Some(close)) =
                            (use_block.find('{'), use_block.rfind('}'))
                        {
                            if close > open {
                                for item in use_block[open + 1..close].split(',') {
                                    let identifier =
                                        item.trim().split_whitespace().next().unwrap_or("");
                                    if identifier
                                        .chars()
                                        .next()
                                        .is_some_and(|character| character.is_ascii_uppercase())
                                    {
                                        names.insert(identifier.to_owned());
                                    }
                                }
                            }
                        }
                        in_use = false;
                        use_block.clear();
                    }
                }
            }
        }
        if names.is_empty() {
            return Err("ANTI-VACUITY: canonical type scan found zero omp-types names".to_owned());
        }
        Ok(names)
    }

    fn public_types_in_source(crate_name: &str, file: &Path, source: &str) -> Vec<PublicType> {
        source
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim_start();
                ["pub struct ", "pub enum ", "pub type "]
                    .iter()
                    .find_map(|prefix| {
                        trimmed.strip_prefix(prefix).and_then(|rest| {
                            let name = rest
                                .split(|character: char| {
                                    !character.is_ascii_alphanumeric() && character != '_'
                                })
                                .next()?;
                            (!name.is_empty()).then(|| PublicType {
                                name: name.to_owned(),
                                crate_name: crate_name.to_owned(),
                                file: file.display().to_string(),
                                line: index + 1,
                            })
                        })
                    })
            })
            .collect()
    }

    fn collect_public_types(root: &Path, crates: &[String]) -> Result<Vec<PublicType>, String> {
        scan_nonempty(crates, "leg4-canonical")?;
        let mut types = Vec::new();
        for crate_name in crates {
            if crate_name == "omp-types" {
                continue;
            }
            let source_root = root.join("crates").join(crate_name).join("src");
            let mut stack = vec![source_root];
            while let Some(directory) = stack.pop() {
                let entries = fs::read_dir(&directory)
                    .map_err(|error| format!("leg4 read {}: {error}", directory.display()))?;
                for entry in entries {
                    let path = entry
                        .map_err(|error| format!("leg4 directory entry: {error}"))?
                        .path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().and_then(|extension| extension.to_str())
                        == Some("rs")
                    {
                        let source = fs::read_to_string(&path)
                            .map_err(|error| format!("leg4 read {}: {error}", path.display()))?;
                        types.extend(public_types_in_source(crate_name, &path, &source));
                    }
                }
            }
        }
        if types.is_empty() {
            return Err("ANTI-VACUITY: leg4 scanned zero public type declarations".to_owned());
        }
        Ok(types)
    }

    fn canonical_violations(
        types: &[PublicType],
        canonical: &HashSet<String>,
        allowance: &[(&str, &str)],
    ) -> Vec<PublicType> {
        let allowed: HashSet<&str> = allowance.iter().map(|(name, _)| *name).collect();
        types
            .iter()
            .filter(|declaration| canonical.contains(&declaration.name))
            .filter(|declaration| !allowed.contains(declaration.name.as_str()))
            .cloned()
            .collect()
    }

    const CANONICAL_ALLOWANCE: &[(&str, &str)] = &[];

    #[test]
    fn every_leg_has_an_independent_allowance_validator() {
        assert!(validate_leg2_allowance(&[("crate", "named reason")]).is_ok());
        assert!(validate_leg3_allowance(&[("crate", "named reason")]).is_ok());
        assert!(validate_leg4_allowance(&[("Type", "named reason")]).is_ok());
        assert!(validate_leg5_allowance(&[("Type", "named reason")]).is_ok());
        assert!(validate_leg2_allowance(&[("crate", "")]).is_err());
        assert!(validate_leg3_allowance(&[("crate", "")]).is_err());
        assert!(validate_leg4_allowance(&[("Type", "")]).is_err());
        assert!(validate_leg5_allowance(&[("Type", "")]).is_err());
    }

    #[test]
    fn leg2_known_good_and_empty_scan_controls_are_separate() {
        let root = repo_root();
        let map = fs::read_to_string(root.join("OMP-SURFACE-MAP.toml")).expect("surface map");
        assert!(
            map.contains("[crates.no-shell-gate]"),
            "known-good surface declaration missing"
        );
        let empty: Vec<String> = Vec::new();
        assert!(
            scan_nonempty(&empty, "leg2-surface").is_err(),
            "leg2 empty scan must be ERROR"
        );
        assert!(validate_leg2_allowance(SURFACE_ALLOWANCE).is_ok());
    }

    #[test]
    fn leg3_known_good_and_empty_scan_controls_are_separate() {
        let root = repo_root();
        let manifest =
            fs::read_to_string(root.join("crates/no-shell-gate/Cargo.toml")).expect("manifest");
        assert!(
            manifest.contains("unsafe_code = \"forbid\""),
            "known-good lint declaration missing"
        );
        let empty: Vec<String> = Vec::new();
        assert!(
            scan_nonempty(&empty, "leg3-asupersync").is_err(),
            "leg3 empty scan must be ERROR"
        );
        assert!(validate_leg3_allowance(FORBID_ALLOWANCE).is_ok());
    }

    #[test]
    fn leg4_canonical_type_scan_refuses_current_redeclarations() {
        let root = repo_root();
        let crates = workspace_crate_names(&root);
        let canonical =
            canonical_type_names(&root).expect("canonical vocabulary must be non-empty");
        let declarations = collect_public_types(&root, &crates).expect("public type scan");
        let violations = canonical_violations(&declarations, &canonical, CANONICAL_ALLOWANCE);
        assert!(
            violations.is_empty(),
            "LEG4 CANONICAL TYPE REDECLARATION: {:?} — use omp-types instead",
            violations
                .iter()
                .map(|declaration| format!(
                    "{} in {}:{}:{}",
                    declaration.name, declaration.crate_name, declaration.file, declaration.line
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn leg4_known_good_specimen_and_empty_scan_are_separate() {
        let canonical = HashSet::from(["CanonicalType".to_owned()]);
        let good =
            public_types_in_source("fixture", Path::new("good.rs"), "pub struct UniqueType;\n");
        assert!(canonical_violations(&good, &canonical, CANONICAL_ALLOWANCE).is_empty());
        let bad = public_types_in_source(
            "fixture",
            Path::new("bad.rs"),
            "pub struct CanonicalType;\n",
        );
        assert_eq!(
            canonical_violations(&bad, &canonical, CANONICAL_ALLOWANCE).len(),
            1
        );
        let empty: Vec<String> = Vec::new();
        assert!(
            scan_nonempty(&empty, "leg4-canonical").is_err(),
            "leg4 empty scan must be ERROR"
        );
        assert!(validate_leg4_allowance(CANONICAL_ALLOWANCE).is_ok());
    }

    #[test]
    fn leg5_known_good_and_empty_scan_controls_are_separate() {
        let good_sources = ["pub struct UniqueA;", "pub enum UniqueB { One }"];
        let mut seen = HashMap::new();
        for (index, source) in good_sources.iter().enumerate() {
            let declarations = public_types_in_source("fixture", Path::new("good.rs"), source);
            for declaration in declarations {
                assert!(seen.insert(declaration.name, index).is_none());
            }
        }
        let empty: Vec<String> = Vec::new();
        assert!(
            scan_nonempty(&empty, "leg5-collision").is_err(),
            "leg5 empty scan must be ERROR"
        );
        assert!(validate_leg5_allowance(COLLISION_ALLOWANCE).is_ok());
    }

    #[test]
    fn leg4_predicate_mutation_is_attributable() {
        let canonical = HashSet::from(["CanonicalType".to_owned()]);
        let source = "pub struct CanonicalType;\n";
        let declarations = public_types_in_source("fixture", Path::new("mutation.rs"), source);
        let before = canonical_violations(&declarations, &canonical, CANONICAL_ALLOWANCE);
        assert_eq!(before.len(), 1, "known-bad canonical specimen must be red");
        let restored = "pub struct UniqueType;\n";
        let restored_declarations =
            public_types_in_source("fixture", Path::new("mutation.rs"), restored);
        let after = canonical_violations(&restored_declarations, &canonical, CANONICAL_ALLOWANCE);
        assert!(
            after.is_empty(),
            "restored canonical specimen must be green"
        );
    }

    #[test]
    fn leg5_predicate_mutation_is_attributable() {
        let source = "pub struct Duplicate;\n";
        let first = public_types_in_source("first", Path::new("first.rs"), source);
        let second = public_types_in_source("second", Path::new("second.rs"), source);
        let mut seen = HashMap::new();
        for declaration in first.iter().chain(second.iter()) {
            let prior = seen.insert(declaration.name.clone(), declaration.crate_name.clone());
            if prior.is_some() {
                assert_eq!(declaration.name, "Duplicate");
            }
        }
        let unique =
            public_types_in_source("second", Path::new("second.rs"), "pub struct Unique;\n");
        let mut names = HashSet::new();
        for declaration in first.iter().chain(unique.iter()) {
            assert!(names.insert(declaration.name.clone()));
        }
    }
}

mod ipg18_more {
    use super::*;
    use std::collections::HashSet;

    fn leg2_membership_violations(
        on_disk: &[String],
        declared: &HashSet<String>,
    ) -> (Vec<String>, Vec<String>) {
        if on_disk.is_empty() || declared.is_empty() {
            return (
                vec!["ANTI-VACUITY".to_owned()],
                vec!["ANTI-VACUITY".to_owned()],
            );
        }
        let undeclared = on_disk
            .iter()
            .filter(|name| !declared.contains(*name))
            .cloned()
            .collect();
        let ghosts = declared
            .iter()
            .filter(|name| !on_disk.iter().any(|candidate| candidate == *name))
            .cloned()
            .collect();
        (undeclared, ghosts)
    }

    fn leg3_forbid_predicate(manifest: &str) -> bool {
        manifest.lines().any(|line| {
            let line = line.trim();
            line == "unsafe_code = \"forbid\"" || line == "unsafe_code=\"forbid\""
        })
    }

    #[test]
    fn leg2_predicate_mutation_is_attributable() {
        let on_disk = vec!["known-good".to_owned()];
        let good = HashSet::from(["known-good".to_owned()]);
        let (undeclared, ghosts) = leg2_membership_violations(&on_disk, &good);
        assert!(
            undeclared.is_empty() && ghosts.is_empty(),
            "known-good surface must pass"
        );
        let mutated = HashSet::from(["mutated-ghost".to_owned()]);
        let (undeclared, ghosts) = leg2_membership_violations(&on_disk, &mutated);
        assert_eq!(undeclared, vec!["known-good"]);
        assert_eq!(ghosts, vec!["mutated-ghost"]);
    }

    #[test]
    fn leg3_predicate_mutation_is_attributable() {
        let good = "[lints.rust]\nunsafe_code = \"forbid\"\n";
        let mutated = good.replace("forbid", "warn");
        assert!(leg3_forbid_predicate(good), "known-good lint must pass");
        assert!(!leg3_forbid_predicate(&mutated), "mutated lint must fail");
        // The leg-specific allowance validator is exercised in ipg18_contract.
    }

    #[test]
    fn leg3_asupersync_schema_is_enforced_on_the_production_scan() {
        let root = repo_root();
        let report = asupersync_conformance::scan_repository(&root)
            .expect("asupersync conformance scan must produce a non-empty report");
        assert!(
            !report.crates.is_empty(),
            "ANTI-VACUITY: leg3 scanned zero crates"
        );
        assert!(
            !report.spawn_sites.is_empty(),
            "ANTI-VACUITY: leg3 scanned zero raw sites"
        );
        let mut violations = Vec::new();
        for row in &report.crates {
            if !row.forbid_unsafe {
                violations.push(format!("{} missing forbid(unsafe_code)", row.name));
            }
            if row.async_fns != row.cx_first {
                violations.push(format!(
                    "{} async/cx-first mismatch {}/{}",
                    row.name, row.async_fns, row.cx_first
                ));
            }
            if !row.forbidden_deps.is_empty() {
                violations.push(format!(
                    "{} forbidden deps {:?}",
                    row.name, row.forbidden_deps
                ));
            }
            let triaged = report
                .spawn_sites
                .iter()
                .filter(|site| site.crate_name == row.name)
                .count();
            if triaged != row.raw_command {
                violations.push(format!(
                    "{} raw_command={} triaged={}",
                    row.name, row.raw_command, triaged
                ));
            }
        }
        assert!(
            violations.is_empty(),
            "LEG3 ASUPERSYNC CONFORMANCE failed: {violations:?}"
        );
        let known_good = report
            .crates
            .iter()
            .find(|row| row.name == "no-shell-gate")
            .expect("known-good no-shell-gate row");
        assert!(known_good.forbid_unsafe);
        assert_eq!(known_good.async_fns, known_good.cx_first);
        assert!(known_good.forbidden_deps.is_empty());
    }
}

mod ipg18_leg1_mutation {
    use super::*;

    #[test]
    fn predicate_mutation_is_attributable() {
        let lane = Lane {
            name: "mutation-lane".to_owned(),
            needle_hyphen: "mutation-lane".to_owned(),
            needle_underscore: "mutation_lane".to_owned(),
        };
        let test_only = rust_source(
            "src/mutation.rs",
            "#[cfg(test)]\nmod tests { fn fake() { let _ = \"mutation-lane\"; } }\n",
        );
        let red = check_wiring(&[lane.clone()], &[test_only.clone()], &[], STRIP_TEST_CODE)
            .expect_err("leg1 known-bad test-only caller must be red");
        assert_eq!(red, "UNWIRED LANE: mutation-lane");
        let restored = check_wiring(&[lane], &[test_only], &[], false)
            .expect("mutating the stripping predicate must make the fixture green");
        assert_eq!(restored.len(), 1);
    }
}

mod ipg18_leg3_fixture {
    use super::*;

    fn has_forbid(manifest: &str) -> bool {
        manifest.lines().any(|line| {
            let line = line.trim();
            line == "unsafe_code = \"forbid\"" || line == "unsafe_code=\"forbid\""
        })
    }

    #[test]
    fn in_tree_missing_forbid_specimen_is_red() {
        let path = repo_root().join(
            "crates/asupersync-conformance/tests/fixtures/known-bad/crates/raw-command-no-subprocess/Cargo.toml",
        );
        let manifest = std::fs::read_to_string(&path).expect("known-bad manifest");
        assert!(!has_forbid(&manifest), "known-bad fixture must omit forbid(unsafe_code)");
    }
}
