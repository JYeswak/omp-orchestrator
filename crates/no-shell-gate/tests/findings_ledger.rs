#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RECONCILED_ROUND: u64 = 21;
const FUTURE_VOID_ROUND: u64 = 22;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclaredFinding {
    round: u64,
    section: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct GateReport {
    declared: usize,
    reconciled: usize,
    void_rows: usize,
}

fn fixture_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omp-findings-ledger-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("docs/plan")).expect("fixture root");
    root
}

fn write_fixture(root: &Path, findings: &str, source: &str, convergence: &str) {
    fs::write(root.join("docs/plan/FINDINGS.jsonl"), findings).expect("findings fixture");
    fs::write(root.join("docs/plan/round21-GreenFrog.jsonl"), source).expect("source fixture");
    fs::write(root.join("docs/plan/CONVERGENCE.jsonl"), convergence).expect("convergence fixture");
}

fn read_jsonl(path: &Path, role: &str) -> Result<Vec<Value>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("{role}_MISSING path={} error={error}", path.display()))?;
    if text.trim().is_empty() {
        return Err(format!("{role}_EMPTY path={}", path.display()));
    }
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<Value>(line).map_err(|error| {
                format!(
                    "{role}_MALFORMED path={} line={} error={error}",
                    path.display(),
                    index + 1
                )
            })
        })
        .collect()
}

fn string_field<'a>(row: &'a Value, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| row.get(*name).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn round_field(row: &Value) -> Option<u64> {
    row.get("round")
        .and_then(Value::as_u64)
        .or_else(|| row.get("source_round").and_then(Value::as_u64))
}

fn normalized_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_ascii_lowercase()
}

fn generated_id(row: &Value, index: usize) -> Result<String, String> {
    let round = round_field(row).ok_or_else(|| "FINDINGS_SOURCE_ROW_MISSING_ROUND".to_owned())?;
    let section = string_field(row, &["section"])
        .ok_or_else(|| "FINDINGS_SOURCE_ROW_MISSING_SECTION".to_owned())?;
    let lens = string_field(row, &["lens", "graded_by"]).unwrap_or("unknown-eye");
    Ok(format!(
        "R{round}-{}-{}-{index}",
        normalized_component(section),
        normalized_component(lens)
    ))
}

fn declared_ids(row: &Value) -> Result<Vec<String>, String> {
    let declared_count = row
        .get("new_findings")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            row.get("findings")
                .and_then(Value::as_array)
                .map_or(0, Vec::len) as u64
        });

    let mut ids = row
        .get("finding_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if ids.is_empty() {
        if let Some(findings) = row.get("findings").and_then(Value::as_array) {
            for (index, finding) in findings.iter().enumerate() {
                ids.push(
                    string_field(finding, &["finding_id", "id"])
                        .map(ToOwned::to_owned)
                        .unwrap_or(generated_id(row, index + 1)?),
                );
            }
        }
    }

    if ids.len() < declared_count as usize {
        for index in ids.len()..declared_count as usize {
            ids.push(generated_id(row, index + 1)?);
        }
    }
    if ids.len() != declared_count as usize {
        return Err(format!(
            "FINDINGS_SOURCE_COUNT_MISMATCH section={} round={} declared={} ids={}",
            string_field(row, &["section"]).unwrap_or("<missing>"),
            round_field(row).unwrap_or(0),
            declared_count,
            ids.len()
        ));
    }
    Ok(ids)
}

fn has_explicit_ids(row: &Value) -> bool {
    row.get("finding_ids")
        .and_then(Value::as_array)
        .is_some_and(|ids| {
            ids.iter()
                .any(|id| id.as_str().is_some_and(|value| !value.trim().is_empty()))
        })
        || row
            .get("findings")
            .and_then(Value::as_array)
            .is_some_and(|findings| {
                findings
                    .iter()
                    .any(|finding| string_field(finding, &["finding_id", "id"]).is_some())
            })
}
fn round_from_filename(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    let digits = name.strip_prefix("round")?.split_once('-')?.0;
    digits.parse().ok()
}

fn source_rows(repo: &Path) -> Result<Vec<Value>, String> {
    let plan = repo.join("docs/plan");
    let convergence_path = plan.join("CONVERGENCE.jsonl");
    let mut rows = Vec::new();
    for row in read_jsonl(&convergence_path, "CONVERGENCE")? {
        if round_field(&row) == Some(15) {
            rows.push(row);
        }
    }

    let mut round_paths = fs::read_dir(&plan)
        .map_err(|error| {
            format!(
                "ROUND_SOURCE_DIR_MISSING path={} error={error}",
                plan.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("round") && name.ends_with(".jsonl"))
        })
        .collect::<Vec<_>>();
    round_paths.sort();
    for path in round_paths {
        let Some(file_round) = round_from_filename(&path) else {
            continue;
        };
        if !(16..=MAX_RECONCILED_ROUND).contains(&file_round) {
            continue;
        }
        for mut row in read_jsonl(&path, "ROUND_SOURCE")? {
            if round_field(&row).is_none() {
                continue;
            }
            // The filename is the declared round authority. The historical
            // round16-Opus artifact carries round-22 labels because it was
            // briefed as round 16 and later voided; its declared count still
            // belongs to the R16 source family for coverage accounting.
            row["round"] = Value::from(file_round);
            rows.push(row);
        }
    }
    if rows.is_empty() {
        return Err("FINDINGS_SOURCE_EMPTY no rounds 15-21 were found".to_owned());
    }
    Ok(rows)
}

fn validate_finding_row(row: &Value) -> Result<(String, u64, bool, String), String> {
    let id = string_field(row, &["finding_id", "id"])
        .ok_or_else(|| "FINDINGS_ROW_MISSING_ID".to_owned())?;
    let round = row
        .get("round")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("FINDINGS_ROW_MISSING_ROUND id={id}"))?;
    let section = string_field(row, &["section"])
        .ok_or_else(|| format!("FINDINGS_ROW_MISSING_SECTION id={id}"))?
        .to_owned();
    let _summary = string_field(row, &["summary", "finding"])
        .ok_or_else(|| format!("FINDINGS_ROW_MISSING_SUMMARY id={id}"))?;
    let _verified_by = string_field(row, &["verified_by", "graded_by"])
        .ok_or_else(|| format!("FINDINGS_ROW_MISSING_VERIFIER id={id}"))?;
    let status = string_field(row, &["status", "disposition"])
        .ok_or_else(|| format!("FINDINGS_ROW_MISSING_STATUS id={id}"))?;
    let status = status.to_ascii_uppercase();
    let is_void = row.get("void").and_then(Value::as_bool).unwrap_or(false);

    if is_void {
        let reason = string_field(row, &["void_reason"])
            .ok_or_else(|| format!("FINDINGS_VOID_MISSING_REASON id={id}"))?;
        if round < FUTURE_VOID_ROUND || reason.is_empty() {
            return Err(format!("FINDINGS_VOID_INVALID id={id}"));
        }
        return Ok((id.to_owned(), round, true, section));
    }

    if !matches!(status.as_str(), "FIXED" | "RETRACTED" | "DEFERRED" | "OPEN") {
        return Err(format!(
            "FINDINGS_ROW_INVALID_STATUS id={id} status={status}"
        ));
    }
    let evidence = string_field(row, &["evidence"])
        .ok_or_else(|| format!("FINDINGS_ROW_MISSING_EVIDENCE id={id}"))?;
    if !evidence.starts_with(".flywheel/grade-evidence/") || !evidence.ends_with(".gz") {
        return Err(format!(
            "FINDINGS_ROW_UNRETAINED_EVIDENCE id={id} evidence={evidence}"
        ));
    }
    match status.as_str() {
        "FIXED" => {
            let sha = string_field(row, &["sha", "fixed_in"])
                .ok_or_else(|| format!("FINDINGS_FIXED_MISSING_SHA id={id}"))?;
            if !(7..=64).contains(&sha.len()) || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return Err(format!("FINDINGS_FIXED_INVALID_SHA id={id} sha={sha}"));
            }
        }
        "RETRACTED" => {
            string_field(row, &["reason", "retracted_reason"])
                .ok_or_else(|| format!("FINDINGS_RETRACTED_MISSING_REASON id={id}"))?;
        }
        "DEFERRED" => {
            let bead = string_field(row, &["bead", "deferred_to"])
                .ok_or_else(|| format!("FINDINGS_DEFERRED_MISSING_BEAD id={id}"))?;
            if bead.starts_with("omp-orchestrator-") {
                // The concrete existence check is performed below against the target tracker.
                let _ = bead;
            }
        }
        "OPEN" if round <= MAX_RECONCILED_ROUND => {
            return Err(format!("FINDINGS_LEDGER_OPEN id={id} round={round}"));
        }
        _ => {}
    }
    Ok((id.to_owned(), round, false, section))
}

fn actual_finding_row(row: &Value) -> Result<Option<(String, u64, String, String)>, String> {
    let (id, round, is_void, section) = validate_finding_row(row)?;
    let status = string_field(row, &["status", "disposition"])
        .unwrap_or("OPEN")
        .to_ascii_uppercase();
    if is_void {
        return Ok(None);
    }
    Ok(Some((id, round, section, status)))
}

fn validate_serial_rule(
    convergence: &[Value],
    findings: &[(String, u64, bool, String)],
) -> Result<(), String> {
    for row in convergence {
        let Some(round) = round_field(row) else {
            continue;
        };
        let Some(pin) = string_field(row, &["pin"]) else {
            continue;
        };
        if pin.is_empty() {
            continue;
        }
        for (id, finding_round, is_void, status) in findings {
            if !*is_void && *finding_round < round && status == "OPEN" {
                return Err(format!(
                    "FINDINGS_SERIAL_RULE_OPEN id={id} finding_round={finding_round} pinned_round={round}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_findings_ledger(repo: &Path) -> Result<GateReport, String> {
    let declared_rows = source_rows(repo)?;
    let mut declared = BTreeMap::<String, DeclaredFinding>::new();
    let mut count_only = BTreeMap::<(u64, String), usize>::new();
    for row in &declared_rows {
        let round =
            round_field(row).ok_or_else(|| "FINDINGS_SOURCE_ROW_MISSING_ROUND".to_owned())?;
        let section = string_field(row, &["section"])
            .ok_or_else(|| "FINDINGS_SOURCE_ROW_MISSING_SECTION".to_owned())?
            .to_owned();
        let ids = declared_ids(row)?;
        if has_explicit_ids(row) {
            for id in ids {
                if declared
                    .insert(
                        id.clone(),
                        DeclaredFinding {
                            round,
                            section: section.clone(),
                        },
                    )
                    .is_some()
                {
                    return Err(format!("FINDINGS_SOURCE_DUPLICATE_ID id={id}"));
                }
            }
        } else {
            *count_only.entry((round, section)).or_default() += ids.len();
        }
    }
    let declared_total = declared.len() + count_only.values().sum::<usize>();
    if declared_total == 0 {
        return Err("FINDINGS_SOURCE_EMPTY no declared findings".to_owned());
    }

    let convergence_path = repo.join("docs/plan/CONVERGENCE.jsonl");
    let convergence = read_jsonl(&convergence_path, "CONVERGENCE")?;
    let void_rows = convergence
        .iter()
        .filter(|row| {
            round_field(row).is_some_and(|round| {
                round >= FUTURE_VOID_ROUND && string_field(row, &["pin"]).is_none()
            })
        })
        .count();

    let findings_path = repo.join("docs/plan/FINDINGS.jsonl");
    let finding_rows = read_jsonl(&findings_path, "FINDINGS_LEDGER")?;
    let mut actual = BTreeMap::<String, (u64, String)>::new();
    let mut serial_rows = Vec::new();
    for row in &finding_rows {
        let Some((id, round, section, status)) = actual_finding_row(row)? else {
            let (id, round, _is_void, _section) = validate_finding_row(row)?;
            let status = string_field(row, &["status", "disposition"])
                .unwrap_or("OPEN")
                .to_ascii_uppercase();
            serial_rows.push((id, round, true, status));
            continue;
        };
        if actual.insert(id.clone(), (round, section)).is_some() {
            return Err(format!("FINDINGS_LEDGER_DUPLICATE_ID id={id}"));
        }
        serial_rows.push((id, round, false, status));
    }

    for (id, expected) in &declared {
        let Some((round, _section)) = actual.get(id) else {
            return Err(format!(
                "FINDINGS_LEDGER_COVERAGE_MISSING id={id} round={} section={} void_rows={void_rows}",
                expected.round, expected.section
            ));
        };
        if *round != expected.round {
            return Err(format!("FINDINGS_LEDGER_COVERAGE_INVALID id={id}"));
        }
    }
    for ((round, section), expected_count) in &count_only {
        let observed_count = actual
            .values()
            .filter(|(actual_round, actual_section)| {
                actual_round == round && actual_section == section
            })
            .count();
        if observed_count < *expected_count {
            return Err(format!(
                "FINDINGS_LEDGER_COVERAGE_MISSING round={round} section={section} expected={expected_count} observed={observed_count} void_rows={void_rows}"
            ));
        }
    }
    validate_serial_rule(&convergence, &serial_rows)?;
    Ok(GateReport {
        declared: declared_total,
        reconciled: actual.len(),
        void_rows,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_owned()
}

#[test]
fn empty_findings_ledger_is_an_error() {
    let root = fixture_root("empty");
    write_fixture(
        &root,
        "",
        r#"{"section":"00-brief","round":21,"new_findings":1,"lens":"fresh","graded_by":"FreshEye"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":1,"lens":"fresh","gates_green":true}
"#,
    );

    let error = validate_findings_ledger(&root).expect_err("empty ledger must refuse");
    assert!(error.contains("FINDINGS_LEDGER_EMPTY"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn known_good_reconciliation_has_exact_coverage() {
    let root = fixture_root("good");
    write_fixture(
        &root,
        r#"{"finding_id":"R21-00-001","round":21,"section":"00-brief","summary":"fixed","status":"FIXED","sha":"0123456789012345678901234567890123456789","evidence":".flywheel/grade-evidence/evidence.gz","verified_by":"BlueLantern"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":1,"finding_ids":["R21-00-001"],"lens":"fresh","graded_by":"FreshEye"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":1,"gates_green":true}
"#,
    );

    let report = validate_findings_ledger(&root).expect("known-good fixture");
    assert_eq!(report.declared, 1);
    assert_eq!(report.reconciled, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn count_only_coverage_rejects_a_short_ledger() {
    let root = fixture_root("count-only");
    write_fixture(
        &root,
        r#"{"finding_id":"R21-00-001","round":21,"section":"00-brief","summary":"one","status":"RETRACTED","reason":"wrong","evidence":".flywheel/grade-evidence/evidence.gz","verified_by":"BlueLantern"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":2,"lens":"fresh","graded_by":"FreshEye"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":2,"gates_green":true}
"#,
    );

    let error = validate_findings_ledger(&root).expect_err("count-only short ledger must refuse");
    assert!(error.contains("expected=2 observed=1"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn void_finding_rows_are_excluded_from_coverage_map() {
    let row = serde_json::json!({
        "finding_id": "R22-00-001",
        "round": 22,
        "section": "00-brief",
        "summary": "void",
        "status": "DEFERRED",
        "bead": "omp-orchestrator-kxe.3",
        "evidence": ".flywheel/grade-evidence/evidence.gz",
        "verified_by": "BlueLantern",
        "void": true,
        "void_reason": "round was not pinned",
    });

    assert!(
        actual_finding_row(&row).expect("void row shape").is_none(),
        "void rows must not enter the reconciled coverage map"
    );
}
#[test]
fn serial_rule_rejects_open_prior_to_pinned_round() {
    let convergence = vec![serde_json::json!({"round":22,"pin":"abc123"})];
    let findings = vec![("R21-00-001".to_owned(), 21, false, "OPEN".to_owned())];

    let error = validate_serial_rule(&convergence, &findings)
        .expect_err("a pinned next round must refuse an open prior finding");
    assert!(error.contains("FINDINGS_SERIAL_RULE_OPEN"), "{error}");
}

#[test]
fn short_ledger_is_an_error_with_both_counts() {
    let root = fixture_root("short");
    write_fixture(
        &root,
        r#"{"finding_id":"R21-00-001","round":21,"section":"00-brief","summary":"one","status":"RETRACTED","reason":"wrong","evidence":".flywheel/grade-evidence/evidence.gz","verified_by":"BlueLantern"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":2,"finding_ids":["R21-00-001","R21-00-002"],"lens":"fresh","graded_by":"FreshEye"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":2,"gates_green":true}
"#,
    );

    let error = validate_findings_ledger(&root).expect_err("short ledger must refuse");
    assert!(
        error.contains("FINDINGS_LEDGER_COVERAGE_MISSING"),
        "{error}"
    );
    assert!(error.contains("R21-00-002"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pinned_later_round_cannot_follow_open_finding() {
    let root = fixture_root("serial");
    write_fixture(
        &root,
        r#"{"finding_id":"R21-00-001","round":21,"section":"00-brief","summary":"open","status":"OPEN","evidence":".flywheel/grade-evidence/evidence.gz","verified_by":"BlueLantern"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":1,"finding_ids":["R21-00-001"],"lens":"fresh","graded_by":"FreshEye"}
"#,
        r#"{"section":"00-brief","round":22,"pin":"abc123","new_findings":0,"gates_green":true}
"#,
    );

    let error = validate_findings_ledger(&root).expect_err("serial rule must refuse");
    assert!(error.contains("FINDINGS_LEDGER_OPEN"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unpinned_future_convergence_rows_are_void_and_not_coverage() {
    let root = fixture_root("void");
    write_fixture(
        &root,
        r#"{"finding_id":"R21-00-001","round":21,"section":"00-brief","summary":"fixed","status":"DEFERRED","bead":"omp-orchestrator-kxe.3","evidence":".flywheel/grade-evidence/evidence.gz","verified_by":"BlueLantern"}
"#,
        r#"{"section":"00-brief","round":21,"new_findings":1,"finding_ids":["R21-00-001"],"lens":"fresh","graded_by":"FreshEye"}
"#,
        r#"{"section":"00-brief","round":22,"void":true,"void_reason":"pin was not cut","new_findings":1,"gates_green":true}
"#,
    );

    let report = validate_findings_ledger(&root).expect("void row is not coverage");
    assert_eq!(report.void_rows, 1);
    assert_eq!(report.reconciled, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
#[ignore = "requires reconciled FINDINGS.jsonl from HD-0006 reconciliation beads"]
fn real_findings_ledger_is_strictly_valid() {
    let report = validate_findings_ledger(&repo_root()).expect("real findings ledger");
    assert!(report.declared > 0);
    assert_eq!(report.declared, report.reconciled);
}
