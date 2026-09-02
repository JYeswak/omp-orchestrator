#![forbid(unsafe_code)]

//! Cross-section authority gate.
//!
//! This gate validates the checked-in authority registry, not the runtime product. It refuses
//! missing finding dispositions, duplicate IDs, absent commands, missing owners, and status claims
//! that have no explicit PROJECTED/EXISTS/UNPROVEN boundary. The registry is the single current
//! disposition overlay; round ledgers remain historical evidence.

use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};

use serde_json::Value;

const REQUIRED_FINDINGS: &[&str] = &[
    "R19-00-open-census-scope",
    "R19-00-board-status-denominator",
    "R19-01-citation-drift",
    "R19-02-expected-slash-ownership",
    "R19-04-board-arithmetic",
    "R19-04-generator-acceptance",
    "R19-05-extraction-state",
    "R19-06-gate-matrix-authority",
    "R19-06-gate-inventory-self-contradiction",
    "R19-07-canonical-cli",
    "R19-07-doctor-down-exit",
    "R19-07-help-evidence",
    "R19-09-m2-grep-vacuity",
    "R19-12-done-signal-vacuity",
    "R19-12-close-evidence-gate",
    "R19-04-board-sum",
    "R19-04-generator-no-acceptance",
    "R19-05-dispatcher-extraction-state",
    "R19-06-gate-cell-drift",
    "R19-07-help-denominator",
    "R19-07-doctor-exit-semantics",
    "R19-09-pipeline-grep-vacuity",
    "R19-11-lifecycle-citation",
    "R19-11-Q13-owner-gap",
    "R19-12-s6-gate-absent",
    "R19-12-current-crate-denominator",
    "R19-12-s9-schema-ids",
    "R20-X-NUMBERS-notes-drift",
    "R20-X-surface-map-identity-drift",
    "R20-X-decision-schema-note-stale",
    "R20-09-checker-crate-count-stale",
    "R20-09-seam-citations-drift",
    "R20-10-ephemeral-artifacts-called-durable",
    "R20-10-gap-count-arithmetic",
    "R20-11-install-denominator-stale",
    "R20-11-surface-map-provenance-stale",
    "R20-11-cardinality-citation-drift",
    "R20-12-s9-schema-ids",
    "R21-04-mirror-entry-mislabel",
    "R21-05-source-citation-drift",
    "R21-05-transport-scar-unretained",
    "R21-05-heartbeat-count-stale",
    "R21-05-refusal-count-unproven",
    "R21-07-pane-truth-ghost-obsolete",
    "R21-07-exit-citation-drift",
    "R21-07-seven-adapter-residue",
    "R21-07-malformed-component-name",
    "R21-08-version-denominator-drift",
    "R21-08-source-anchor-drift",
    "R21-08-stale-surface-currentness",
    "R21-08-supervisor-drift-unproven",
    "R21-09-seam-citation-drift",
    "R21-09-unattended-duration-unproven",
    "R21-10-ambient-search-root",
    "R21-X-wire-artifact-unregistered",
    "R21-X-addressability-pass-unproven",
    "R21-X-unsafe-number-command-weak",
];

const ALLOWED_DISPOSITIONS: &[&str] = &["FIXED", "DUPLICATE", "FALSE_POSITIVE", "LINKED_CHILD"];
const ALLOWED_STATUSES: &[&str] = &["EXISTS", "PROJECTED", "UNPROVEN", "HISTORICAL"];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn required_string(row: &Value, key: &str, line: usize) -> Result<String, String> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("CROSS_SECTION_ROW_MISSING_{key} line={line}"))
}

fn validate_evidence_digest(row: &Value, line: usize) -> Result<(), String> {
    let digest = required_string(row, "evidence_sha256", line)?;
    if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "CROSS_SECTION_EVIDENCE_INVALID_SHA line={line} digest={digest}"
        ));
    }
    Ok(())
}

fn validate_finding(row: &Value, line: usize) -> Result<String, String> {
    let id = required_string(row, "finding_id", line)?;
    let disposition = required_string(row, "disposition", line)?;
    if !ALLOWED_DISPOSITIONS.contains(&disposition.as_str()) {
        return Err(format!(
            "CROSS_SECTION_FINDING_INVALID_DISPOSITION id={id} disposition={disposition}"
        ));
    }
    for key in [
        "authority_scope",
        "authority_command",
        "acceptance_command",
        "owner",
        "failure_result",
        "status",
        "notes",
        "evidence_sha256",
    ] {
        required_string(row, key, line)?;
    }
    let status = row.get("status").and_then(Value::as_str).unwrap();
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(format!(
            "CROSS_SECTION_FINDING_INVALID_STATUS id={id} status={status}"
        ));
    }
    if disposition == "FIXED" {
        let sha = required_string(row, "fixed_in", line)?;
        if !(7..=64).contains(&sha.len()) || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(format!(
                "CROSS_SECTION_FIXED_INVALID_SHA id={id} fixed_in={sha}"
            ));
        }
    }
    if disposition == "LINKED_CHILD" {
        required_string(row, "linked_bead", line)?;
    }
    validate_evidence_digest(row, line)?;
    Ok(id)
}

fn validate_authority(row: &Value, line: usize) -> Result<(), String> {
    for key in [
        "authority_key",
        "authority_scope",
        "authority_command",
        "acceptance_command",
        "owner",
        "failure_result",
        "status",
        "writer",
        "consumer",
        "evidence_sha256",
    ] {
        required_string(row, key, line)?;
    }
    let status = row.get("status").and_then(Value::as_str).unwrap();
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(format!(
            "CROSS_SECTION_AUTHORITY_INVALID_STATUS line={line} status={status}"
        ));
    }
    let authority_command = row
        .get("authority_command")
        .and_then(Value::as_str)
        .unwrap();
    let acceptance_command = row
        .get("acceptance_command")
        .and_then(Value::as_str)
        .unwrap();
    if authority_command == acceptance_command {
        return Err(format!(
            "CROSS_SECTION_AUTHORITY_COMMANDS_COLLAPSED line={line}"
        ));
    }
    if authority_command.contains("CROSS-SECTION-AUTHORITY") {
        return Err(format!(
            "CROSS_SECTION_AUTHORITY_SELF_SEARCH line={line} command={authority_command}"
        ));
    }
    if !acceptance_command.contains("cross_section_authority") {
        return Err(format!(
            "CROSS_SECTION_AUTHORITY_NO_EXECUTABLE_ACCEPTANCE line={line}"
        ));
    }
    validate_evidence_digest(row, line)?;
    Ok(())
}

fn validate_registry(text: &str) -> Result<Vec<Value>, String> {
    if text.trim().is_empty() {
        return Err("CROSS_SECTION_AUTHORITY_EMPTY_REGISTRY".to_owned());
    }
    let mut rows = Vec::new();
    let mut finding_ids = BTreeSet::new();
    let mut authority_keys = BTreeSet::new();
    let mut fresh_reader = false;

    for (offset, line) in text.lines().enumerate() {
        let line_number = offset + 1;
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line).map_err(|error| {
            format!("CROSS_SECTION_AUTHORITY_INVALID_JSON line={line_number}: {error}")
        })?;
        if row.get("schema_version").and_then(Value::as_str) != Some("cross_section_authority.v1") {
            return Err(format!(
                "CROSS_SECTION_AUTHORITY_SCHEMA_MISMATCH line={line_number}"
            ));
        }
        match row.get("record_type").and_then(Value::as_str) {
            Some("finding") => {
                let id = validate_finding(&row, line_number)?;
                if !finding_ids.insert(id.clone()) {
                    return Err(format!("CROSS_SECTION_DUPLICATE_FINDING id={id}"));
                }
            }
            Some("authority") => {
                validate_authority(&row, line_number)?;
                let key = required_string(&row, "authority_key", line_number)?;
                if !authority_keys.insert(key.clone()) {
                    return Err(format!("CROSS_SECTION_DUPLICATE_AUTHORITY key={key}"));
                }
            }
            Some("fresh_reader") => {
                if fresh_reader {
                    return Err("CROSS_SECTION_DUPLICATE_FRESH_READER".to_owned());
                }
                fresh_reader = true;
                for key in [
                    "source_revision",
                    "command",
                    "result",
                    "evidence",
                    "evidence_sha256",
                    "owner",
                ] {
                    required_string(&row, key, line_number)?;
                }
                validate_evidence_digest(&row, line_number)?;
                if row.get("result").and_then(Value::as_str) != Some("ZERO_NEW") {
                    return Err("CROSS_SECTION_FRESH_READER_NOT_ZERO".to_owned());
                }
                let revision = row.get("source_revision").and_then(Value::as_str).unwrap();
                if revision.len() != 40 || !revision.chars().all(|ch| ch.is_ascii_hexdigit()) {
                    return Err(format!(
                        "CROSS_SECTION_FRESH_READER_UNPINNED revision={revision}"
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "CROSS_SECTION_AUTHORITY_UNKNOWN_RECORD_TYPE line={line_number}"
                ));
            }
        }
        rows.push(row);
    }

    for required in REQUIRED_FINDINGS {
        if !finding_ids.contains(*required) {
            return Err(format!("CROSS_SECTION_FINDING_MISSING id={required}"));
        }
    }
    if finding_ids.len() != REQUIRED_FINDINGS.len() {
        return Err(format!(
            "CROSS_SECTION_FINDING_SET_MISMATCH expected={} observed={}",
            REQUIRED_FINDINGS.len(),
            finding_ids.len()
        ));
    }
    for required in [
        "board_total",
        "generator_acceptance",
        "gate_matrix",
        "canonical_cli_doctor",
        "extraction_state",
        "lifecycle_foundation",
        "completion_close_evidence",
    ] {
        if !authority_keys.contains(required) {
            return Err(format!("CROSS_SECTION_AUTHORITY_MISSING key={required}"));
        }
    }
    if !fresh_reader {
        return Err("CROSS_SECTION_FRESH_READER_MISSING".to_owned());
    }
    Ok(rows)
}

fn registry_text() -> String {
    fs::read_to_string(repo_root().join("docs/plan/CROSS-SECTION-AUTHORITY.jsonl"))
        .expect("CROSS-SECTION-AUTHORITY.jsonl must exist")
}

#[test]
fn authority_registry_is_nonempty_unique_and_schema_valid() {
    let rows = validate_registry(&registry_text()).expect("authority registry must validate");
    let evidence = repo_root().join(".flywheel/grade-evidence/kxe5-cross-section.md.gz");
    let fresh_evidence = repo_root().join(".flywheel/grade-evidence/kxe5-fresh-reader.md.gz");
    assert!(
        evidence.is_file(),
        "retained kxe.5 evidence artifact is missing"
    );
    assert!(
        fresh_evidence.is_file(),
        "retained fresh-reader evidence artifact is missing"
    );
    assert!(
        fs::metadata(evidence).unwrap().len() > 0
            && fs::metadata(fresh_evidence).unwrap().len() > 0,
        "retained evidence artifact is empty"
    );
    assert!(
        rows.len() > REQUIRED_FINDINGS.len(),
        "authority rows are missing"
    );
}

#[test]
fn every_required_round19_round20_round21_finding_is_dispositioned() {
    validate_registry(&registry_text()).expect("every required finding must have one disposition");
}

#[test]
fn fixed_pointers_exist_and_touch_authority_sources() {
    let root = repo_root();
    let rows = validate_registry(&registry_text()).expect("registry must validate");
    let mut checked = 0usize;
    for row in rows
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("finding"))
        .filter(|row| row.get("disposition").and_then(Value::as_str) == Some("FIXED"))
    {
        let id = row.get("finding_id").and_then(Value::as_str).unwrap();
        let sha = row.get("fixed_in").and_then(Value::as_str).unwrap();
        let output = Command::new("git")
            .args([
                "-C",
                root.to_str().unwrap(),
                "show",
                "--format=",
                "--name-only",
                sha,
            ])
            .output()
            .unwrap_or_else(|error| panic!("git show failed for {id} {sha}: {error}"));
        assert!(
            output.status.success(),
            "fixed pointer is missing: {id} {sha}"
        );
        let paths = String::from_utf8_lossy(&output.stdout);
        assert!(
            paths.contains("docs/plan/")
                || paths.contains("NUMBERS.toml")
                || paths.contains("SCHEMAS.toml"),
            "fixed pointer does not touch plan authority sources: {id} {sha}"
        );
        checked += 1;
    }
    assert!(checked > 0, "no FIXED pointers were checked");
}
#[test]
fn board_generator_and_completion_claims_have_independent_falsifiers() {
    let rows = validate_registry(&registry_text()).expect("registry must validate");
    let authority_keys: BTreeSet<_> = rows
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("authority"))
        .filter_map(|row| row.get("authority_key").and_then(Value::as_str))
        .collect();
    for key in [
        "board_total",
        "generator_acceptance",
        "completion_close_evidence",
    ] {
        assert!(authority_keys.contains(key), "missing authority row {key}");
    }
    let board_row = rows
        .iter()
        .find(|row| row.get("authority_key").and_then(Value::as_str) == Some("board_total"))
        .expect("board_total authority row");
    let board_command = board_row
        .get("authority_command")
        .and_then(Value::as_str)
        .unwrap();
    let output = Command::new("sh")
        .args(["-c", board_command])
        .current_dir(repo_root())
        .output()
        .expect("board authority command must spawn");
    assert!(
        output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "CROSS_SECTION_BOARD_AUTHORITY_FAILURE command={board_command} stderr={}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

#[test]
fn projected_and_unproven_product_claims_are_not_laundered_as_exists() {
    let rows = validate_registry(&registry_text()).expect("registry must validate");
    for row in rows
        .iter()
        .filter(|row| row.get("record_type").and_then(Value::as_str) == Some("authority"))
    {
        let key = row.get("authority_key").and_then(Value::as_str).unwrap();
        let status = row.get("status").and_then(Value::as_str).unwrap();
        if matches!(
            key,
            "canonical_cli_doctor" | "generator_acceptance" | "completion_close_evidence"
        ) {
            assert_ne!(status, "EXISTS", "{key} is not a shipped runtime claim");
        }
    }
}

#[test]
fn fresh_reader_receipt_is_pinned_and_zero_new() {
    let rows = validate_registry(&registry_text()).expect("registry must validate");
    let row = rows
        .iter()
        .find(|row| row.get("record_type").and_then(Value::as_str) == Some("fresh_reader"))
        .expect("fresh reader row");
    assert_eq!(row.get("result").and_then(Value::as_str), Some("ZERO_NEW"));
    assert!(row
        .get("command")
        .and_then(Value::as_str)
        .unwrap()
        .contains("git rev-parse HEAD"));
    let revision = row.get("source_revision").and_then(Value::as_str).unwrap();
    let output = Command::new("git")
        .args([
            "-C",
            repo_root().to_str().unwrap(),
            "cat-file",
            "-e",
            &format!("{revision}^{{commit}}"),
        ])
        .output()
        .expect("fresh-reader source revision probe must spawn");
    assert!(
        output.status.success(),
        "CROSS_SECTION_FRESH_READER_PIN_MISSING revision={revision}"
    );
}
#[test]
fn removing_a_required_finding_is_red_and_names_the_missing_id() {
    let original = registry_text();
    let removed = original.replacen(REQUIRED_FINDINGS[0], "R19-MUTATED", 1);
    let error = validate_registry(&removed).expect_err("missing finding must refuse");
    assert!(
        error.contains("CROSS_SECTION_FINDING_MISSING") && error.contains(REQUIRED_FINDINGS[0]),
        "mutation refusal must name the missing finding: {error}"
    );
}
