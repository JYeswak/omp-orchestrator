#![forbid(unsafe_code)]

//! Conformance coverage for the workspace's declared production lanes.
//!
//! The current extraction baseline declares one lane, `no-shell-gate`. The declaration list grows
//! with each extracted lane; it must never be silently replaced by an empty scan. This suite proves
//! reachability only: a caller can invoke a lane while the invoked mode may still be weaker than the
//! lane's live guarantee.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str, &str, &str)] = &[
    (
        "refill-idle-panes",
        "cron-wired via fast-dispatch at */5; the Rust source scanner cannot see a crontab invocation",
        "control-plane fast-dispatch owner",
        "Dies when the Rust source scanner learns to read crontab invocations or the lane gains a source-visible caller",
    ),
    (
        "tick-dispatch",
        "cron-wired via controller-tick at :18/:38/:58; the Rust source scanner cannot see a crontab invocation",
        "control-plane controller-tick owner",
        "Dies when the Rust source scanner learns to read crontab invocations or the lane gains a source-visible caller",
    ),
    (
        "s1-coverage",
        "S1 depth suspended under Atlas Arc R1; crate exists as a coverage artifact with no production caller",
        "S1 coverage owner",
        "Dies when S1 build waves consume the coverage artifact through a production caller",
    ),
    (
        "fleet-idle-monitor",
        "decision kernel for 47g0, landed ahead of its conductor; the caller that will route a tick through it is item 9 of that bead and is not yet re-armed. Wiring it to crates/fleet-monitor today would be a FALSE green: fleet-monitor's only mention in .github/workflows/gate.yml is a comment at :43, which the census strips, so the chain would terminate at a crate with neither a caller nor an executor trigger",
        "47g0 owner",
        "Dies when a tick routes its queue through fleet_idle_monitor::tick, or when control-plane's cron-invoked fleet-idle-monitor is repointed at this crate",
    ),
];
fn unwired_allowance_refs() -> Vec<(&'static str, &'static str)> {
    UNWIRED_LANE_ALLOWANCE
        .iter()
        .map(|(name, reason, _, _)| (*name, *reason))
        .collect()
}
/// explicitly declared as future work. This registry does not certify gate
/// semantics; it prevents a canonical ID from becoming decorative prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateReferentKind {
    Test,
    Ci,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateIdentifierStatus {
    Wired { kind: GateReferentKind },
    DeclaredNotWired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GateIdentifier {
    id: &'static str,
    status: GateIdentifierStatus,
    referent: Option<&'static str>,
    known_bad: Option<&'static str>,
    known_good: Option<&'static str>,
    anti_vacuity: Option<&'static str>,
    owner: Option<&'static str>,
    dies_when: Option<&'static str>,
}

const GATE_IDENTIFIER_REFERENTS: &[GateIdentifier] = &[
    GateIdentifier { id: "GATE-001", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the 01-idea gate has retained source-pinned population evidence and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-002", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the 01-idea gate has retained measured economics evidence and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-003", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the 01-idea gate has a retained distribution-channel evidence row and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-004", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the first-value journey has a retained external-user receipt and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-005", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the paid-commitment gate has retained evidence and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-006", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the unit-economics gate has source-pinned measured inputs and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-007", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the recurrence gate has a retained cohort or retention evidence row and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-008", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the rights, security, and licensing gate has a retained review receipt and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-009", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the defensibility gate has retained compounding-asset evidence and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-010", status: GateIdentifierStatus::DeclaredNotWired, referent: None, known_bad: None, known_good: None, anti_vacuity: None, owner: Some("S3-01-idea-reviewer"), dies_when: Some("Dies when the substitute comparison has a retained evidence row and a bead acceptance naming its verifier") },
    GateIdentifier { id: "GATE-011", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/no-shell-gate/tests/gate.rs::planted_shell_is_red_then_green_after_delete"), known_good: Some("crates/no-shell-gate/tests/gate.rs::clean_list_passes"), anti_vacuity: Some("crates/no-shell-gate/tests/gate.rs::empty_scan_set_is_an_error_not_a_pass"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-012", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/omp-inventory-map/tests/inventory.rs::surface_map_ghost_is_unknown"), known_good: Some("crates/omp-inventory-map/tests/inventory.rs::subprocess_and_no_shell_positive_controls_are_visible"), anti_vacuity: Some("crates/omp-inventory-map/tests/inventory.rs::empty_metadata_is_a_hard_error"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-013", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/undrained-pipe-lint/tests/specimens.rs::known_bad_both_pipes_try_wait_poll_is_flagged"), known_good: Some("crates/undrained-pipe-lint/tests/specimens.rs::known_good_stdout_only_passes"), anti_vacuity: Some("crates/undrained-pipe-lint/tests/specimens.rs::empty_scan_set_is_an_error_not_a_pass"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-014", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/commit-build-fence/tests/hook.rs::real_hook_refuses_active_registration_with_actionable_identity"), known_good: Some("crates/commit-build-fence/tests/hook.rs::real_hook_allows_commit_with_valid_empty_store"), anti_vacuity: Some("crates/commit-build-fence/tests/hook.rs::real_hook_treats_missing_store_as_error"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-015", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/state-wildcard-lint/tests/specimens.rs::known_bad_state_wildcard_is_flagged"), known_good: Some("crates/state-wildcard-lint/tests/specimens.rs::wildcard_on_integer_and_string_passes"), anti_vacuity: Some("crates/state-wildcard-lint/tests/specimens.rs::empty_or_unreadable_workspace_is_an_error"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-016", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::known_bad_raw_send_keys_outside_kernel_is_flagged"), known_good: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::kernel_own_call_site_is_allowlisted"), anti_vacuity: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::real_workspace_ledger_balances"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-017", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/pre-delete-citation-check/tests/killed_child.rs::killed_git_produces_refusal_not_success"), known_good: Some("crates/pre-delete-citation-check/tests/killed_child.rs::working_git_with_no_deletions_passes"), anti_vacuity: Some("NOT_APPLICABLE: an empty staged-deletion set is the valid clean input"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-018", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Ci }, referent: Some(".github/workflows/gate.yml::gate"), known_bad: Some("crates/path-literal-guard/tests/repo_wide.rs::unreadable_input_is_refused_and_restores_to_a_clean_scan"), known_good: Some("crates/path-literal-guard/tests/repo_wide.rs::zero_home_path_literals_across_crates_src"), anti_vacuity: Some("crates/path-literal-guard/tests/repo_wide.rs::staged_mode_over_the_real_repo_equals_the_sweep"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-019", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Test }, referent: Some("crates/no-shell-gate/tests/gate.rs::planted_shell_is_red_then_green_after_delete"), known_bad: Some("crates/no-shell-gate/tests/gate.rs::planted_shell_is_red_then_green_after_delete"), known_good: Some("crates/no-shell-gate/tests/gate.rs::clean_list_passes"), anti_vacuity: Some("crates/no-shell-gate/tests/gate.rs::empty_scan_set_is_an_error_not_a_pass"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-020", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Test }, referent: Some("crates/no-shell-gate/tests/gate.rs::clean_list_passes"), known_bad: Some("crates/no-shell-gate/tests/gate.rs::planted_shell_is_red_then_green_after_delete"), known_good: Some("crates/no-shell-gate/tests/gate.rs::clean_list_passes"), anti_vacuity: Some("crates/no-shell-gate/tests/gate.rs::empty_scan_set_is_an_error_not_a_pass"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-021", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Test }, referent: Some("crates/state-wildcard-lint/tests/specimens.rs::mutation_removing_state_wildcard_is_green"), known_bad: Some("crates/state-wildcard-lint/tests/specimens.rs::known_bad_state_wildcard_is_flagged"), known_good: Some("crates/state-wildcard-lint/tests/specimens.rs::wildcard_on_integer_and_string_passes"), anti_vacuity: Some("crates/state-wildcard-lint/tests/specimens.rs::empty_or_unreadable_workspace_is_an_error"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-022", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Test }, referent: Some("crates/no-shell-gate/tests/gate.rs::empty_scan_set_is_an_error_not_a_pass"), known_bad: Some("crates/no-shell-gate/tests/gate.rs::planted_shell_is_red_then_green_after_delete"), known_good: Some("crates/no-shell-gate/tests/gate.rs::clean_list_passes"), anti_vacuity: Some("crates/no-shell-gate/tests/gate.rs::empty_scan_set_is_an_error_not_a_pass"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-023", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Test }, referent: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::ratchet_refuses_new_debt_slack_and_undeclared"), known_bad: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::ratchet_refuses_new_debt_slack_and_undeclared"), known_good: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::kernel_own_call_site_is_allowlisted"), anti_vacuity: Some("crates/kernel-bypass-gate/tests/kernel_bypass.rs::real_workspace_ledger_balances"), owner: None, dies_when: None },
    GateIdentifier { id: "GATE-024", status: GateIdentifierStatus::Wired { kind: GateReferentKind::Test }, referent: Some("crates/no-shell-gate/tests/gate_reachability.rs::positive_control_runs_real_hook_and_refuses_staged_shell"), known_bad: Some("crates/no-shell-gate/tests/gate_reachability.rs::removing_ci_trigger_flips_gate_to_unreachable"), known_good: Some("crates/no-shell-gate/tests/gate_reachability.rs::known_good_fixture_reports_ci_trigger_and_unwired_gate"), anti_vacuity: Some("crates/no-shell-gate/tests/gate_reachability.rs::empty_gate_set_is_an_error_not_a_pass"), owner: None, dies_when: None },
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
    Manifest,
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
        Some("toml") => Some(SourceKind::Manifest),
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
            if kind == SourceKind::Manifest
                && path.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml")
            {
                continue;
            }
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
    if kind == SourceKind::Rust {
        return text_structure::code_only(contents).into_owned();
    }
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
            if (kind == SourceKind::Workflow || kind == SourceKind::Manifest) && byte == b'#' {
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
    let cleaned = if source.kind == SourceKind::Rust && strip_tests {
        strip_test_code(&without_comments)
    } else {
        without_comments
    };
    if source.path == Path::new("crates/omp-orchestrator/src/lib.rs") {
        remove_advisory_registry(cleaned)
    } else {
        cleaned
    }
}

/// The existing advisory registry is the authority, not a caller of every name it records.
/// Remove only that literal table from the source scan; preserve real command/use sites in
/// the same file. This is the anti-self-reference boundary for the reachability census.
fn remove_advisory_registry(mut source: String) -> String {
    let Some(start) = source.find("pub const ADVISORY_ALLOWANCE") else {
        return source;
    };
    let Some(relative_end) = source[start..].find("];" ) else {
        return source;
    };
    let end = start + relative_end + 2;
    source.replace_range(start..end, "");
    source
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

fn allowance_verdict(
    lane: &Lane,
    caller: Option<CallerHit>,
    allowance_reason: Option<&str>,
) -> Result<Option<CallerHit>, String> {
    match (caller, allowance_reason) {
        (Some(hit), Some(reason)) => Err(format!(
            "{} IS now invoked by {}:{}, but it is still declared unwired in UNWIRED_LANE_ALLOWANCE ({reason}). Remove the declaration",
            lane.name,
            hit.path.display(),
            hit.line,
        )),
        (Some(hit), None) => Ok(Some(hit)),
        (None, Some(_reason)) => Ok(None),
        (None, None) => Err(format!("UNWIRED LANE: {}", lane.name)),
    }
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
        let caller = find_caller(lane, sources, strip_tests)?;
        let allowance_reason = allowance
            .iter()
            .find(|(name, _)| *name == lane.name)
            .map(|(_, reason)| *reason);
        if let Some(hit) = allowance_verdict(lane, caller, allowance_reason)? {
            hits.push(hit);
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
fn assert_test_referent(root: &Path, field: &str, referent: &str) {
    let Some((path, function)) = referent.split_once("::") else {
        panic!("{field} referent must be path::function: {referent}");
    };
    let source = fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("{field} path {path} unreadable: {error}"));
    let needle = format!("fn {function}");
    assert!(
        source
            .lines()
            .any(|line| line.trim_start().starts_with("fn ") && line.contains(&needle)),
        "{field} referent {referent} names no existing test function"
    );
}

fn assert_ci_referent(root: &Path, field: &str, referent: &str) {
    let Some((path, job)) = referent.split_once("::") else {
        panic!("{field} CI referent must be workflow::job: {referent}");
    };
    let workflow = fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("{field} workflow {path} unreadable: {error}"));
    assert!(
        workflow.contains(&format!("  {job}:")),
        "{field} referent {referent} names no workflow job"
    );
}

fn assert_leg_referent(root: &Path, field: &str, referent: &str) {
    if referent.starts_with("NOT_APPLICABLE:") {
        assert!(
            referent.len() > "NOT_APPLICABLE:".len(),
            "{field} non-applicable leg needs a reason"
        );
        return;
    }
    assert_test_referent(root, field, referent);
}

#[test]
fn every_plan_gate_identifier_has_a_referent_or_dies_when() {
    let root = repo_root();
    assert_eq!(
        GATE_IDENTIFIER_REFERENTS.len(),
        24,
        "the plan declares exactly GATE-001 through GATE-024"
    );
    let mut seen = std::collections::BTreeSet::new();
    let mut wired = 0usize;
    let mut declared = 0usize;
    for (index, gate) in GATE_IDENTIFIER_REFERENTS.iter().enumerate() {
        let expected = format!("GATE-{:03}", index + 1);
        assert_eq!(gate.id, expected, "gate identifiers must be contiguous");
        assert!(seen.insert(gate.id), "duplicate gate identifier {}", gate.id);
        match gate.status {
            GateIdentifierStatus::Wired { kind } => {
                wired += 1;
                let referent = gate.referent.expect("wired gate needs a referent");
                match kind {
                    GateReferentKind::Test => assert_test_referent(&root, gate.id, referent),
                    GateReferentKind::Ci => assert_ci_referent(&root, gate.id, referent),
                }
                assert_leg_referent(
                    &root,
                    &format!("{} known_bad", gate.id),
                    gate.known_bad.expect("wired gate needs known-bad"),
                );
                assert_leg_referent(
                    &root,
                    &format!("{} known_good", gate.id),
                    gate.known_good.expect("wired gate needs known-good"),
                );
                assert_leg_referent(
                    &root,
                    &format!("{} anti_vacuity", gate.id),
                    gate.anti_vacuity.expect("wired gate needs anti-vacuity or N/A"),
                );
                assert!(gate.owner.is_none() && gate.dies_when.is_none());
            }
            GateIdentifierStatus::DeclaredNotWired => {
                declared += 1;
                assert!(
                    gate.referent.is_none()
                        && gate.known_bad.is_none()
                        && gate.known_good.is_none()
                        && gate.anti_vacuity.is_none(),
                    "declared-not-wired {} must not carry a fake referent or test leg",
                    gate.id
                );
                assert!(
                    gate.owner.is_some_and(|owner| !owner.trim().is_empty()),
                    "declared-not-wired {} needs an owner",
                    gate.id
                );
                assert!(
                    gate.dies_when
                        .is_some_and(|reason| reason.starts_with("Dies when") && reason.len() > 20),
                    "declared-not-wired {} needs a dies-when reason",
                    gate.id
                );
            }
        }
    }
    assert_eq!(wired, 14, "technical/property identifiers wired");
    assert_eq!(declared, 10, "future business identifiers declared, not wired");
}

#[test]
fn every_declared_lane_has_a_production_caller() {
    let sources = collect_sources(&repo_root()).expect("production sources must be readable");

    // The lane set is DERIVED from the workspace (bead -0hk): a hand-listed
    // DECLARED_LANES is the defect this crate exists to prevent. An empty or
    // unreadable derivation is an error, never a pass.
    let lanes = derive_lanes(&repo_root()).expect("lane derivation must be readable and non-empty");
    validate_allowance_rows(UNWIRED_LANE_ALLOWANCE, "unwired-lane")
        .expect("unwired allowances must carry owner and dies_when");
    let advisory = advisory_allowance(&repo_root()).expect("ADVISORY_ALLOWANCE must be readable");
    let mut allowance_rows: Vec<(String, String)> = unwired_allowance_refs()
        .into_iter()
        .map(|(name, reason)| (name.to_owned(), reason.to_owned()))
        .collect();
    allowance_rows.extend(advisory);
    let allowance_refs: Vec<(&str, &str)> = allowance_rows
        .iter()
        .map(|(name, reason)| (name.as_str(), reason.as_str()))
        .collect();
    validate_allowance(&lanes, &allowance_refs).expect("allowance must be valid");

    let positive = find_caller(&positive_control(), &sources, STRIP_TEST_CODE)
        .expect("positive-control search must run")
        .expect("known wired checkout action must be found");
    assert_eq!(positive.path, PathBuf::from(".github/workflows/gate.yml"));
    let workflow = std::fs::read_to_string(repo_root().join(&positive.path))
        .expect("positive-control file must be readable");
    let cited = workflow
        .lines()
        .nth(positive.line.saturating_sub(1))
        .expect("positive-control line must exist");
    assert!(
        cited.contains("actions/checkout"),
        "positive control must cite the needle, not a stale line number (got line {} text {:?})",
        positive.line,
        cited
    );

    let hits = check_wiring(&lanes, &sources, &allowance_refs, STRIP_TEST_CODE)
        .expect("every workspace lane must be wired or carry a named allowance reason");
    let allowlisted = allowance_refs.len();
    // Allowance means "do not fail if unwired", not "subtract from the hit
    // count". An allowlisted lane that HAS a caller still produces a hit.
    // left==81/right==20 was a stale line-cite on the positive control, then
    // 71 vs 68 was this off-by-allowance accounting. Do not paper it with a
    // blanket exemption.
    assert!(
        hits.len() <= lanes.len(),
        "cannot have more hits than derived lanes: hits={} lanes={}",
        hits.len(),
        lanes.len()
    );
    assert!(
        hits.len() >= lanes.len().saturating_sub(allowlisted),
        "wired hits {} below lanes {} minus allowances {}",
        hits.len(),
        lanes.len(),
        allowlisted
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
fn allowance_verdict_covers_four_wiring_combinations() {
    let lane = Lane {
        name: "matrix-lane".to_owned(),
        needle_hyphen: "matrix-lane".to_owned(),
        needle_underscore: "matrix_lane".to_owned(),
    };
    let hit = CallerHit {
        path: PathBuf::from("src/production.rs"),
        line: 7,
    };

    let wired_declared = allowance_verdict(&lane, Some(hit.clone()), Some("stale reason"))
        .expect_err("wired lane with an allowance must be refused");
    assert!(wired_declared.contains("matrix-lane IS now invoked"));
    assert!(wired_declared.contains("Remove the declaration"));

    let wired_undeclared = allowance_verdict(&lane, Some(hit), None)
        .expect("wired lane without an allowance must pass")
        .expect("wired caller must be returned");
    assert_eq!(wired_undeclared.path, PathBuf::from("src/production.rs"));

    assert!(
        allowance_verdict(&lane, None, Some("valid reason"))
            .expect("unwired declared lane is an allowed exception")
            .is_none()
    );
    let unwired_undeclared = allowance_verdict(&lane, None, None)
        .expect_err("unwired undeclared lane must be refused");
    assert_eq!(unwired_undeclared, "UNWIRED LANE: matrix-lane");
}

#[test]
fn wired_allowance_is_red_and_removing_declaration_is_green() {
    let lane = Lane {
        name: "planted-wired-lane".to_owned(),
        needle_hyphen: "planted-wired-lane".to_owned(),
        needle_underscore: "planted_wired_lane".to_owned(),
    };
    let source = rust_source(
        "src/production.rs",
        "fn run() { invoke(\"planted-wired-lane\"); }\n",
    );
    let stale = [("planted-wired-lane", "stale reason")];
    let error = check_wiring(&[lane.clone()], &[source.clone()], &stale, STRIP_TEST_CODE)
        .expect_err("a declared wired allowance must be RED");
    assert!(error.contains("planted-wired-lane IS now invoked"), "{error}");
    assert!(error.contains("Remove the declaration"), "{error}");
    let clean = check_wiring(&[lane], &[source], &[], STRIP_TEST_CODE)
        .expect("removing the stale declaration must restore GREEN");
    assert_eq!(clean.len(), 1);
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
        check_wiring(&lanes, &[], &unwired_allowance_refs(), STRIP_TEST_CODE),
        Err("ERROR: production caller scan set is empty".to_owned())
    );
    assert_eq!(
        check_wiring(&[], &[], &unwired_allowance_refs(), STRIP_TEST_CODE),
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
    validate_allowance(&lanes, &unwired_allowance_refs()).expect("allowance must validate");
    validate_allowance_rows(UNWIRED_LANE_ALLOWANCE, "unwired-lane")
        .expect("real allowance rows must carry owner and dies_when");
    assert!(
        validate_allowance(&lanes, &[("not-a-workspace-crate", "a reason")]).is_err(),
        "an allowance row naming an undeclared lane must be rejected"
    );
    assert!(
        validate_allowance(&lanes, &[("no-shell-gate", "")]).is_err(),
        "an allowance row without a reason must be rejected"
    );
    for (name, reason, owner, dies_when) in UNWIRED_LANE_ALLOWANCE {
        assert!(!reason.trim().is_empty(), "allowance {name} has an empty reason");
        assert!(!owner.trim().is_empty(), "allowance {name} has an empty owner");
        assert!(!dies_when.trim().is_empty(), "allowance {name} has an empty dies_when");
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

fn validate_allowance_rows(
    rows: &[(&str, &str, &str, &str)],
    leg: &str,
) -> Result<(), String> {
    for (subject, reason, owner, dies_when) in rows {
        if subject.trim().is_empty() {
            return Err(format!("{leg} allowance row has an empty subject"));
        }
        if reason.trim().len() < 8 {
            return Err(format!(
                "{leg} allowance row '{subject}' carries no reason — a bare path silences nothing"
            ));
        }
        if owner.trim().is_empty() {
            return Err(format!("{leg} allowance row '{subject}' has an empty owner"));
        }
        if dies_when.trim().is_empty() {
            return Err(format!("{leg} allowance row '{subject}' has an empty dies_when"));
        }
    }
    Ok(())
}

fn inherits_workspace_lints(manifest: &str) -> bool {
    let mut in_lints = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "[lints]" {
            in_lints = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_lints = false;
            continue;
        }
        if in_lints && (trimmed == "workspace = true" || trimmed == "workspace=true") {
            return true;
        }
    }
    false
}

fn crate_forbids_unsafe(root: &Path, manifest: &str) -> bool {
    if manifest
        .lines()
        .any(|line| {
            let line = line.trim();
            line == "unsafe_code = \"forbid\"" || line == "unsafe_code=\"forbid\""
        })
    {
        return true;
    }
    if !inherits_workspace_lints(manifest) {
        return false;
    }
    let Ok(workspace) = fs::read_to_string(root.join("Cargo.toml")) else {
        return false;
    };
    workspace.contains("unsafe_code = \"forbid\"")
}

// ── LEG 2: SURFACE DECLARED ─────────────────────────────────────────────────
const SURFACE_ALLOWANCE: &[(&str, &str, &str, &str)] = &[];

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
    validate_allowance_rows(SURFACE_ALLOWANCE, "leg2-surface").expect("allowance rows must validate");

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
const FORBID_ALLOWANCE: &[(&str, &str, &str, &str)] = &[];

#[test]
fn every_crate_declares_the_forbid_lint() {
    let root = repo_root();
    let crates = workspace_crate_names(&root);
    assert!(!crates.is_empty(), "ANTI-VACUITY: zero crates scanned");
    validate_allowance_rows(FORBID_ALLOWANCE, "leg3-forbid").expect("allowance rows must validate");

    let allowed: std::collections::HashSet<_> = FORBID_ALLOWANCE.iter().map(|(c, _, _, _)| *c).collect();
    let mut missing = Vec::new();
    for name in &crates {
        let manifest = root.join("crates").join(name).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|e| panic!("{} unreadable: {}", manifest.display(), e));
        let has_forbid = crate_forbids_unsafe(&root, &text);
        if !has_forbid && !allowed.contains(name.as_str()) {
            missing.push(name.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "MISSING forbid(unsafe_code) in [lints.rust] or workspace inherit: {:?} — every crate in an asupersync repo must forbid unsafe",
        missing
    );
}

// ── LEG 5: NO PUBLIC-TYPE-NAME COLLISIONS ──────────────────────────────────
const COLLISION_ALLOWANCE: &[(&str, &str, &str, &str)] = &[
    ("Finding", "finding and finding-dispatch both model a scan result; unification is -232 scope, not this gate", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("LintReport", "state-wildcard-lint and path-literal-guard predate the shared crate; same -232 scope", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Violation", "three gate crates declare it; aliasing to a shared type is a cross-crate refactor owned by the integrator", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Observation", "REQUIRES A DECISION not an allowance: tick-monitor produces what omp-orchestrator consumes and each declares an incompatible struct — the free_capacity seam (filter FIXED -oco; seam still open, 09 M1)", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("GateError", "no-shell-gate declares GitFailed/empty-scan for a FILE-EXTENSION scan; \
     porting-gate declares EmptyCandidates/InvalidCandidate/Io/Metadata for a CRATE-ARRIVAL \
     check. Same name, disjoint domains, no shared caller. Dies when a workspace error trait \
     exists; until then unifying them would couple two gates that share nothing but a suffix", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("DispatchIntent", "dispatch-claim-fence declares an enum (Bead/Broadcast/Correction) for the fence; ack-spine declares a struct (bead_id/pane_id/session) for the ledger — different domains, same name. Dies when omp-types provides the shared vocabulary", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("AllowRow", "path-literal-guard and state-wildcard-lint each own an allowlist row type; dies when a shared allowance schema lands", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Candidate", "sender-identity and silent-success-census name unrelated candidates; dies when omp-types owns Candidate", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Config", "cargo-lane-budget and crate-soundness-verify configs are disjoint; dies when a workspace Config type exists", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("ConfigError", "admission-reason and inbox-monitor parse different configs; dies when a shared error trait exists", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Decision", "decision-ledger / kernel-only-operator-hook / refill-idle-panes; HD row vs hook decision vs refill decision. Dies when omp-types Decision lands", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("EventPage", "agent-mail-native and inbox-monitor page different event stores; dies when mail EventPage is canonical", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("GateReport", "kernel-bypass-gate and preregistration-gate reports are gate-local; dies when GateReport lives in omp-types", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("GateVerdict", "crate-atom-gate / staged-build-gate / wired-but-inert-guard; dies when a shared GateVerdict exists", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("LedgerError", "ack-spine / admission-reason / decision-ledger / orchestration-tick-gate; dies when LedgerError is one type", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Lifecycle", "omp-rpc-session redeclares omp-types Lifecycle; dies when rpc-session re-exports omp-types", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Liveness", "bead-holder and tick-monitor; dies when PaneLiveness from omp-types is the only name", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Outcome", "agent-mail-native / lifecycle-event / tick-monitor; dies when asupersync Outcome is the only Outcome", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("PacketError", "agent-mail-native and omp-orchestrator packet errors; dies when PacketError is canonical", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("PaneObservation", "omp-orchestrator redeclares omp-types PaneObservation; dies when the supervisor re-exports omp-types", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("PaneRow", "pane-truth and refill-idle-panes; dies when pane-truth is the sole PaneRow", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("ParseError", "inbox-monitor and kernel-only-operator-hook; dies when a shared parse error exists", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Report", "asupersync-conformance / cargo-lane-budget / tick-monitor; dies when Report is namespaced per crate or unified", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Row", "crate-atom-gate and decision-ledger; dies when Row is not a public type name", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Rule", "admission-reason and composer-typed; dies when Rule is crate-private or unified", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Rules", "admission-reason and composer-typed; dies when Rules is crate-private or unified", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("ScanError", "asupersync-conformance and wired-but-inert-guard; dies when ScanError is crate-private", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("ScanMode", "path-literal-guard and state-wildcard-lint; dies when ScanMode is shared", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Stage", "agent-mail-native and tick-monitor lifecycle stage; dies when omp-types owns Stage", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("Verdict", "no-shell-gate / path-literal-guard / state-wildcard-lint; dies when omp-types Verdict exists", "type-vocabulary-owner", "when omp-types owns the shared type"),
    ("AppendOutcome", "decision-ledger appends human-decision rows while ntm-fleet-monitor appends lifecycle events; distinct domains", "type-vocabulary-owner", "when omp-types owns the shared append vocabulary"),
    ("BeadRecord", "blocker-taxonomy and s1-coverage project different tracker row shapes; no shared consumer", "type-vocabulary-owner", "when omp-types owns BeadRecord"),
    ("BlockerKind", "blocker-taxonomy classifies tracker blockers while ntm-fleet-monitor classifies lifecycle blockers", "type-vocabulary-owner", "when omp-types owns blocker vocabulary"),
    ("CensusReport", "silent-success-census and worker-oracle-gate report different census populations", "type-vocabulary-owner", "when omp-types owns the shared census report"),
    ("CheckError", "decision-ledger, r1-breadth-gate, and response-envelope-check have disjoint check domains", "type-vocabulary-owner", "when omp-types owns a shared check error"),
    ("Classification", "bead-availability, salvage-taxonomy, silent-success-census, and worker-oracle-gate classify different evidence", "type-vocabulary-owner", "when omp-types owns classification vocabulary"),
    ("ClosedBead", "grader-attribution-gate and pre-delete-citation-check consume different closed-bead projections", "type-vocabulary-owner", "when omp-types owns the closed-bead schema"),
    ("Config", "cargo-lane-budget, crate-soundness-verify, and omp-orchestrator configs are disjoint boundaries", "type-vocabulary-owner", "when omp-types owns workspace Config"),
    ("CrateVerdict", "gate-runner and staged-build-gate verdicts cover different execution scopes", "type-vocabulary-owner", "when omp-types owns a shared crate verdict"),
    ("GateCensus", "no-shell-gate and omp-orchestrator census rows have different gate authorities", "type-vocabulary-owner", "when omp-types owns GateCensus"),
    ("LifecycleEvent", "lifecycle-event journal records differ from ntm-fleet-monitor lifecycle model events", "type-vocabulary-owner", "when omp-types owns lifecycle events"),
    ("Predicate", "ompo-start startup predicates differ from silent-success-census oracle predicates", "type-vocabulary-owner", "when omp-types owns predicate vocabulary"),
    ("Receipt", "dispatch-saga transport receipt differs from orchestration-tick-gate journal receipt", "type-vocabulary-owner", "when omp-types owns receipt vocabulary"),
    ("RosterError", "extraction-roster errors differ from gate-runner metadata/allowance errors", "type-vocabulary-owner", "when omp-types owns roster errors"),
    ("TickVerdict", "omp-idle-dispatch decisions differ from orchestration-tick-gate observations", "type-vocabulary-owner", "when omp-types owns tick verdict vocabulary"),
];

#[test]
fn no_public_type_name_collisions_across_crates() {
    let root = repo_root();
    let crates = workspace_crate_names(&root);
    assert!(!crates.is_empty(), "ANTI-VACUITY: zero crates scanned");
    validate_allowance_rows(COLLISION_ALLOWANCE, "leg5-collision").expect("collision rows must validate");

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
        .map(|(n, _, _, _)| n.to_owned())
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

    fn validate_leg2_allowance(rows: &[(&str, &str, &str, &str)]) -> Result<(), String> {
        for (subject, reason, owner, dies_when) in rows {
            if subject.trim().is_empty()
                || reason.trim().len() < 8
                || owner.trim().is_empty()
                || dies_when.trim().is_empty()
            {
                return Err("leg2-surface allowance requires subject, reason, owner, and dies_when".to_owned());
            }
        }
        Ok(())
    }

    fn validate_leg3_allowance(rows: &[(&str, &str, &str, &str)]) -> Result<(), String> {
        for (subject, reason, owner, dies_when) in rows {
            if subject.trim().is_empty()
                || reason.trim().len() < 8
                || owner.trim().is_empty()
                || dies_when.trim().is_empty()
            {
                return Err("leg3-asupersync allowance requires subject, reason, owner, and dies_when".to_owned());
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

    fn validate_leg5_allowance(rows: &[(&str, &str, &str, &str)]) -> Result<(), String> {
        for (subject, reason, owner, dies_when) in rows {
            if subject.trim().is_empty()
                || reason.trim().len() < 8
                || owner.trim().is_empty()
                || dies_when.trim().is_empty()
            {
                return Err("leg5-collision allowance requires subject, reason, owner, and dies_when".to_owned());
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

    const CANONICAL_ALLOWANCE: &[(&str, &str)] = &[
        ("PaneObservation", "omp-orchestrator still declares its own PaneObservation; dies when it re-exports omp-types"),
        ("Lifecycle", "omp-rpc-session still declares its own Lifecycle; dies when it re-exports omp-types"),
    ];

    #[test]
    fn every_leg_has_an_independent_allowance_validator() {
        assert!(validate_leg2_allowance(&[("crate", "named reason", "owner", "dies when condition")]).is_ok());
        assert!(validate_leg3_allowance(&[("crate", "named reason", "owner", "dies when condition")]).is_ok());
        assert!(validate_leg4_allowance(&[("Type", "named reason")]).is_ok());
        assert!(validate_leg5_allowance(&[("Type", "named reason", "owner", "dies when condition")]).is_ok());
        assert!(validate_leg2_allowance(&[("crate", "", "owner", "dies when condition")]).is_err());
        assert!(validate_leg3_allowance(&[("crate", "", "owner", "dies when condition")]).is_err());
        assert!(validate_leg4_allowance(&[("Type", "")]).is_err());
        assert!(validate_leg5_allowance(&[("Type", "", "owner", "dies when condition")]).is_err());
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

    const ASUPERSYNC_SCAN_ALLOWANCE: &[(&str, &str, &str, &str)] = &[
        ("ack-spine", "syntactic async/cx-first mismatch; dies when every async fn takes &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("agent-mail-native", "syntactic async/cx-first mismatch; dies when every async fn takes &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("asupersync-conformance", "scanner crate itself has async helpers without Cx; dies when its async fns take &Cx", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("crate-soundness-verify", "async/cx-first mismatch; dies when its async fns take &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("extraction-roster", "async/cx-first mismatch; dies when its async fns take &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("omp-orchestrator", "async/cx-first mismatch on the supervisor; dies when every async fn takes &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("omp-rpc-session", "async/cx-first mismatch; dies when every async fn takes &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("plan-assemble", "async/cx-first mismatch; dies when every async fn takes &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("preregistration-gate", "async/cx-first and spawn-triage drift; dies when scan is clean", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("staged-build-gate", "async/cx-first mismatch; dies when every async fn takes &Cx first", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("finding", "workspace-lints inherit forbid; syntactic scan misses it until scanner reads workspace.lints", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("finding-dispatch", "workspace-lints inherit forbid; syntactic scan misses it until scanner reads workspace.lints", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("lifecycle-event", "workspace-lints inherit forbid; syntactic scan misses it until scanner reads workspace.lints", "asupersync-port-owner", "when all async functions take &Cx first"),
        ("lifecycle-monitor", "workspace-lints inherit forbid; syntactic scan misses it until scanner reads workspace.lints", "asupersync-port-owner", "when all async functions take &Cx first"),
    ];

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
        validate_allowance_rows(ASUPERSYNC_SCAN_ALLOWANCE, "leg3-asupersync-scan").expect("allowance rows must validate");
        let allowed: HashSet<&str> = ASUPERSYNC_SCAN_ALLOWANCE.iter().map(|(n, _, _, _)| *n).collect();
        let mut violations = Vec::new();
        for row in &report.crates {
            let manifest_path = root.join("crates").join(&row.name).join("Cargo.toml");
            let manifest = std::fs::read_to_string(&manifest_path).unwrap_or_default();
            let forbid_ok = row.forbid_unsafe || crate_forbids_unsafe(&root, &manifest);
            if !forbid_ok {
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
        let unallowed: Vec<_> = violations
            .into_iter()
            .filter(|violation| {
                let crate_name = violation.split_whitespace().next().unwrap_or("");
                !allowed.contains(crate_name)
            })
            .collect();
        assert!(
            unallowed.is_empty(),
            "LEG3 ASUPERSYNC CONFORMANCE failed: {unallowed:?}"
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


#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataMember {
    lane: Lane,
    cargo_callers: Vec<String>,
    has_bin: bool,
    has_lib: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReachabilityClass {
    CargoPathDependency,
    WiredByNonCargoEdge,
    NoCallerAtAll,
    LegitimatelyTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReachabilityRow {
    name: String,
    class: ReachabilityClass,
    cargo_callers: Vec<String>,
    non_cargo_caller: Option<CallerHit>,
}

fn metadata_members(root: &Path) -> Result<Vec<MetadataMember>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
        .current_dir(root)
        .env("RCH_ENABLED", "false")
        .env("RCH_CARGO_WRAPPER_BYPASS", "1")
        .output()
        .map_err(|error| format!("ERROR: cargo metadata did not start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ERROR: cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("ERROR: cargo metadata emitted invalid JSON: {error}"))?;
    let packages = document["packages"]
        .as_array()
        .ok_or_else(|| "ERROR: cargo metadata omitted packages".to_owned())?;
    if packages.is_empty() {
        return Err("ERROR: cargo metadata returned an empty package set".to_owned());
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut members = Vec::with_capacity(packages.len());
    let mut dependency_edges = Vec::<(String, String)>::new();
    for package in packages {
        let name = package["name"]
            .as_str()
            .ok_or_else(|| "ERROR: cargo metadata package omitted name".to_owned())?
            .to_owned();
        if !seen.insert(name.clone()) {
            return Err(format!("ERROR: cargo metadata duplicated package {name}"));
        }
        let mut has_bin = false;
        let mut has_lib = false;
        for target in package["targets"].as_array().into_iter().flatten() {
            for kind in target["kind"].as_array().into_iter().flatten() {
                match kind.as_str() {
                    Some("bin") => has_bin = true,
                    Some("lib" | "proc-macro") => has_lib = true,
                    _ => {}
                }
            }
        }
        for dependency in package["dependencies"].as_array().into_iter().flatten() {
            if dependency["kind"].as_str() == Some("dev") || dependency["path"].as_str().is_none() {
                continue;
            }
            let target = dependency["name"]
                .as_str()
                .ok_or_else(|| format!("ERROR: {name} has a path dependency without a name"))?;
            dependency_edges.push((name.clone(), target.to_owned()));
        }
        members.push(MetadataMember {
            lane: Lane {
                needle_hyphen: name.clone(),
                needle_underscore: name.replace('-', "_"),
                name,
            },
            cargo_callers: Vec::new(),
            has_bin,
            has_lib,
        });
    }
    let indexes: std::collections::BTreeMap<String, usize> = members
        .iter()
        .enumerate()
        .map(|(index, member)| (member.lane.name.clone(), index))
        .collect();
    for (caller, target) in dependency_edges {
        let Some(index) = indexes.get(target.as_str()) else {
            continue;
        };
        members[*index].cargo_callers.push(caller);
    }
    for member in &mut members {
        member.cargo_callers.sort();
        member.cargo_callers.dedup();
    }
    members.sort_by(|left, right| left.lane.name.cmp(&right.lane.name));
    Ok(members)
}

fn advisory_allowance(root: &Path) -> Result<std::collections::BTreeMap<String, String>, String> {
    let path = root.join("crates/omp-orchestrator/src/lib.rs");
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("ERROR: read advisory registry {}: {error}", path.display()))?;
    let start = source
        .find("pub const ADVISORY_ALLOWANCE")
        .ok_or_else(|| "ERROR: ADVISORY_ALLOWANCE declaration is missing".to_owned())?;
    let body = source[start..]
        .split_once("];" )
        .map(|(body, _)| body)
        .ok_or_else(|| "ERROR: ADVISORY_ALLOWANCE is unterminated".to_owned())?;
    let mut rows = std::collections::BTreeMap::new();
    for line in body.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("(\"") else { continue };
        let Some((name, reason)) = rest.split_once("\", \"") else {
            return Err(format!("ERROR: malformed advisory row {line}"));
        };
        let reason = reason
            .strip_suffix("\"),")
            .ok_or_else(|| format!("ERROR: malformed advisory reason {line}"))?;
        if name.is_empty() || reason.trim().is_empty() {
            return Err(format!("ERROR: advisory row lacks name or reason {line}"));
        }
        if rows.insert(name.to_owned(), reason.to_owned()).is_some() {
            return Err(format!("ERROR: duplicate advisory row {name}"));
        }
    }
    if rows.is_empty() {
        return Err("ERROR: ADVISORY_ALLOWANCE is empty or unreadable".to_owned());
    }
    Ok(rows)
}


fn classify_member(
    member: &MetadataMember,
    sources: &[CallerSource],
    strip_tests: bool,
) -> Result<ReachabilityRow, String> {
    if sources.is_empty() {
        return Err("ERROR: production caller scan set is empty".to_owned());
    }
    if !member.cargo_callers.is_empty() {
        return Ok(ReachabilityRow {
            name: member.lane.name.clone(),
            class: ReachabilityClass::CargoPathDependency,
            cargo_callers: member.cargo_callers.clone(),
            non_cargo_caller: None,
        });
    }
    if let Some(hit) = find_caller(&member.lane, sources, strip_tests)? {
        return Ok(ReachabilityRow {
            name: member.lane.name.clone(),
            class: ReachabilityClass::WiredByNonCargoEdge,
            cargo_callers: Vec::new(),
            non_cargo_caller: Some(hit),
        });
    }
    let class = if member.has_bin && !member.has_lib {
        ReachabilityClass::LegitimatelyTerminal
    } else {
        ReachabilityClass::NoCallerAtAll
    };
    Ok(ReachabilityRow {
        name: member.lane.name.clone(),
        class,
        cargo_callers: Vec::new(),
        non_cargo_caller: None,
    })
}

fn enforce_reachability(
    members: &[MetadataMember],
    rows: &[ReachabilityRow],
    advisory: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    if members.is_empty() || rows.is_empty() {
        return Err("ERROR: reachability census has an empty member or row set".to_owned());
    }
    if members.len() != rows.len() {
        return Err(format!(
            "ERROR: reachability rows={} do not cover metadata members={}",
            rows.len(),
            members.len()
        ));
    }
    let member_names: std::collections::BTreeSet<_> =
        members.iter().map(|member| member.lane.name.as_str()).collect();
    let stale: Vec<_> = advisory
        .keys()
        .filter(|name| !member_names.contains(name.as_str()))
        .cloned()
        .collect();
    if !stale.is_empty() {
        return Err(format!("ERROR: advisory registry names absent members {stale:?}"));
    }
    let unallowlisted: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.class == ReachabilityClass::NoCallerAtAll && !advisory.contains_key(&row.name)
        })
        .map(|row| row.name.clone())
        .collect();
    if !unallowlisted.is_empty() {
        return Err(format!(
            "UNWIRED CRATE: no caller at all and absent from ADVISORY_ALLOWANCE {unallowlisted:?}"
        ));
    }
    Ok(())
}

fn run_reachability_census(root: &Path) -> Result<Vec<ReachabilityRow>, String> {
    let members = metadata_members(root)?;
    let sources = collect_sources(root)?;
    let advisory = advisory_allowance(root)?;
    let rows: Vec<_> = members
        .iter()
        .map(|member| classify_member(member, &sources, STRIP_TEST_CODE))
        .collect::<Result<_, _>>()?;
    let stale_wired: Vec<_> = advisory
        .keys()
        .filter(|name| rows.iter().any(|row| row.name == **name && row.class != ReachabilityClass::NoCallerAtAll))
        .cloned()
        .collect();
    let pre_no_caller: Vec<_> = rows
        .iter()
        .filter(|row| row.class == ReachabilityClass::NoCallerAtAll)
        .map(|row| row.name.as_str())
        .collect();
    eprintln!("REACHABILITY_PRECHECK no_caller_names={pre_no_caller:?} advisory_stale_wired={stale_wired:?}");
    enforce_reachability(&members, &rows, &advisory)?;
    let mut counts = std::collections::BTreeMap::<&'static str, usize>::new();
    for row in &rows {
        let label = match row.class {
            ReachabilityClass::CargoPathDependency => "cargo_path_dependency",
            ReachabilityClass::WiredByNonCargoEdge => "wired_by_non_cargo_edge",
            ReachabilityClass::NoCallerAtAll => "no_caller_at_all",
            ReachabilityClass::LegitimatelyTerminal => "legitimately_terminal",
        };
        *counts.entry(label).or_default() += 1;
    }
    let no_caller: Vec<_> = rows
        .iter()
        .filter(|row| row.class == ReachabilityClass::NoCallerAtAll)
        .map(|row| row.name.as_str())
        .collect();
    eprintln!(
        "CRATE_REACHABILITY_CENSUS members={} cargo_path_dependency={} wired_by_non_cargo_edge={} no_caller_at_all={} legitimately_terminal={} no_caller_names={no_caller:?}",
        rows.len(),
        counts.get("cargo_path_dependency").copied().unwrap_or_default(),
        counts.get("wired_by_non_cargo_edge").copied().unwrap_or_default(),
        counts.get("no_caller_at_all").copied().unwrap_or_default(),
        counts.get("legitimately_terminal").copied().unwrap_or_default(),
    );
    Ok(rows)
}

#[test]
fn every_metadata_member_has_a_reachability_classification() {
    let rows = run_reachability_census(&repo_root()).expect("reachability census must be complete");
    assert!(!rows.is_empty(), "census cannot pass with an empty row set");
    let positive = rows
        .iter()
        .find(|row| row.name == "subprocess-contract")
        .expect("positive-control crate must be a workspace member");
    assert_ne!(
        positive.class,
        ReachabilityClass::NoCallerAtAll,
        "POSITIVE_CONTROL=subprocess-contract:found must be wired"
    );
    eprintln!("POSITIVE_CONTROL=subprocess-contract:found class={:?}", positive.class);
}

#[test]
fn planted_unwired_member_is_red_then_green() {
    let member = MetadataMember {
        lane: Lane {
            name: "planted-unwired-member".to_owned(),
            needle_hyphen: "planted-unwired-member".to_owned(),
            needle_underscore: "planted_unwired_member".to_owned(),
        },
        cargo_callers: Vec::new(),
        has_bin: true,
        has_lib: true,
    };
    let sources = [rust_source("src/other.rs", "fn run() {}\n")];
    let row = classify_member(&member, &sources, STRIP_TEST_CODE).expect("fixture scan");
    assert_eq!(row.class, ReachabilityClass::NoCallerAtAll);
    let empty = std::collections::BTreeMap::new();
    assert!(enforce_reachability(&[member.clone()], &[row.clone()], &empty).is_err());
    let mut allowance = std::collections::BTreeMap::new();
    allowance.insert(member.lane.name.clone(), "planted known-bad row".to_owned());
    assert!(enforce_reachability(&[member], &[row], &allowance).is_ok());
}

#[test]
fn reachability_census_rejects_empty_inputs() {
    let member = MetadataMember {
        lane: Lane {
            name: "empty-input".to_owned(),
            needle_hyphen: "empty-input".to_owned(),
            needle_underscore: "empty_input".to_owned(),
        },
        cargo_callers: Vec::new(),
        has_bin: true,
        has_lib: true,
    };
    assert!(classify_member(&member, &[], STRIP_TEST_CODE).is_err());
    assert!(enforce_reachability(&[], &[], &std::collections::BTreeMap::new()).is_err());
}

#[test]
fn allowance_rows_require_owner_and_dies_when() {
    let valid = [("subject", "valid reason", "owner", "dies when condition")];
    validate_allowance_rows(&valid, "xm0n.8").expect("complete allowance row");

    let ownerless = [("planted-ownerless", "valid reason", "", "dies when condition")];
    let owner_error = validate_allowance_rows(&ownerless, "xm0n.8")
        .expect_err("ownerless allowance row must be refused");
    assert!(owner_error.contains("planted-ownerless"));
    assert!(owner_error.contains("owner"));

    let deathless = [("planted-deathless", "valid reason", "owner", "")];
    let death_error = validate_allowance_rows(&deathless, "xm0n.8")
        .expect_err("deathless allowance row must be refused");
    assert!(death_error.contains("planted-deathless"));
    assert!(death_error.contains("dies_when"));

    let reasonless = [("short-reason", "seven!", "owner", "dies when condition")];
    assert!(
        validate_allowance_rows(&reasonless, "xm0n.8").is_err(),
        "reason length below eight must remain refused"
    );
}
