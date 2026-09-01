#![forbid(unsafe_code)]
//! FOUNDATION GATE — `docs/plan/FOUNDATION.jsonl` must be complete and non-vacuous.
//!
//! Bead `omp-orchestrator-7sd`. `SCHEMAS.toml [artifacts.journey_foundation]` declared this
//! artifact and its note states the rule this file enforces: *"Every KNOWN has a
//! command/result or a NUMBERS.toml key; every UNKNOWN has an owner, resolving experiment,
//! and cost; every GAP has a cost if left open. No blank epistemic cells."*
//!
//! The schema row existed and the file did not (`ls docs/plan/FOUNDATION.jsonl` -> No such
//! file, 2026-09-01). A declared reader over an absent artifact is the same
//! BUILT-but-not-WIRED shape the repo already carries rules about, so the gate lands with the
//! artifact rather than after it.
//!
//! # What this refuses
//!
//! 1. A row missing any `required` field, or a required array that is not an array.
//! 2. An `unknown` entry with no resolving experiment or no cost — a question with no
//!    falsifier is not an UNKNOWN, it is a wish.
//! 3. A `gap` entry with no cost-if-left-open or no owner — an unowned, uncosted gap never
//!    gets closed and never gets argued about either.
//! 4. A `crates[]` entry naming a directory absent from `crates/` that is NOT marked
//!    `must_be_created`. Existence is READ FROM DISK here, so a row cannot assert a crate
//!    into being.
//! 5. A `numbers[]` entry that is a BARE FIGURE rather than a registry key. This is the
//!    defect `NUMBERS.toml` exists to kill, restated one layer up.
//! 6. An empty or unreadable file — ANTI-VACUITY. A foundation that was never checked must
//!    never report like one that passed.
//!
//! # What this does NOT claim
//!
//! A complete row is an AGREED build, not a CORRECT one. Every predicate here is structural:
//! it proves the stage's author filled in the cell, never that the cell is true. `exists` is
//! the single exception and it is only a directory test — it does not prove the crate
//! compiles, is wired, or does what the row says.

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Fields `SCHEMAS.toml [artifacts.journey_foundation]` marks `required`.
const REQUIRED: &[&str] = &[
    "schema_version", "stage", "input_refs", "output_refs", "owner", "crates", "gates",
    "numbers", "known", "unknown", "gaps",
];

/// Required fields whose value must be a JSON array.
const REQUIRED_ARRAYS: &[&str] = &[
    "input_refs", "output_refs", "crates", "gates", "numbers", "known", "unknown", "gaps",
];

/// Keys accepted beyond `required`.
///
/// `evidence` and `status` are `SCHEMAS.toml`'s declared `optional` set. `source` is NOT in
/// that set and is mandated by 7sd ACCEPTANCE 2 ("a `source` field per row names the
/// 12-journey heading it was lifted from"), so it carries a NAMED ALLOWANCE ROW here rather
/// than being silently absorbed — the same shape `wired_lanes.rs` uses, where an exception is
/// a named row with a reason and never silence.
const KEY_ALLOWANCE: &[(&str, &str)] = &[
    ("evidence", "SCHEMAS.toml optional"),
    ("status", "SCHEMAS.toml optional"),
    (
        "source",
        "mandated by bead omp-orchestrator-7sd ACCEPTANCE 2 and ABSENT from SCHEMAS.toml \
         [artifacts.journey_foundation].optional; the divergence is reported to the schema \
         owner, not resolved by weakening this gate. Dies when SCHEMAS.toml adds `source` to \
         its optional list, or when 7sd's acceptance drops the field.",
    ),
];

const STAGES: &[&str] = &["S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9"];

/// Which predicates run. Exists so the MUTATION leg can disable exactly one and show the
/// known-bad specimen stops being caught — a leg that stays red under mutation is not
/// attributable to the check it claims to test.
#[derive(Debug, Clone, Copy)]
struct Checks {
    unknown_needs_experiment_and_cost: bool,
    gap_needs_cost_and_owner: bool,
    crate_absence_needs_must_be_created: bool,
    numbers_must_not_be_bare_figures: bool,
}

impl Default for Checks {
    fn default() -> Self {
        Self {
            unknown_needs_experiment_and_cost: true,
            gap_needs_cost_and_owner: true,
            crate_absence_needs_must_be_created: true,
            numbers_must_not_be_bare_figures: true,
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives two levels below repo root")
        .to_path_buf()
}

fn crates_on_disk(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Ok(entries) = fs::read_dir(root.join("crates")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    out.insert(name.to_owned());
                }
            }
        }
    }
    out
}

fn read_rows(path: &Path) -> Result<Vec<Value>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("FOUNDATION_UNREADABLE path={} {error}", path.display()))?;
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line)
            .map_err(|error| format!("FOUNDATION_ROW_NOT_JSON line={} {error}", index + 1))?;
        rows.push(row);
    }
    Ok(rows)
}

fn nonempty(row: &Value, key: &str) -> bool {
    row.get(key).and_then(Value::as_str).is_some_and(|v| !v.trim().is_empty())
}

/// A registry KEY is an identifier. A BARE FIGURE is anything numeric.
///
/// `12-journey` S5 F5 states the rule this encodes: *"declaring a number for a stage that has
/// never executed is a figure with no derivation, which is the defect this field exists to
/// kill."* So the field carries keys; the value lives in `NUMBERS.toml` behind a command.
fn is_registry_key(value: &str) -> bool {
    let v = value.trim();
    !v.is_empty()
        && v.parse::<f64>().is_err()
        && v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn validate(rows: &[Value], on_disk: &BTreeSet<String>, checks: Checks) -> Result<usize, Vec<String>> {
    // ANTI-VACUITY: an empty scan set is an ERROR, never a pass.
    if rows.is_empty() {
        return Err(vec![
            "FOUNDATION_EMPTY: zero rows read — a foundation that was never checked reports \
             identically to one that passed"
                .to_owned(),
        ]);
    }

    let mut errors = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for (index, row) in rows.iter().enumerate() {
        let stage = row
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or("<missing>")
            .to_owned();
        let label = format!("row={} stage={stage}", index + 1);

        for key in REQUIRED {
            if row.get(*key).is_none() || row.get(*key) == Some(&Value::Null) {
                errors.push(format!("FOUNDATION_ROW_MISSING_FIELD {label} field={key}"));
            }
        }
        for key in REQUIRED_ARRAYS {
            if let Some(value) = row.get(*key) {
                if !value.is_array() {
                    errors.push(format!("FOUNDATION_ROW_FIELD_NOT_ARRAY {label} field={key}"));
                }
            }
        }
        if let Some(map) = row.as_object() {
            for key in map.keys() {
                let known = REQUIRED.contains(&key.as_str())
                    || KEY_ALLOWANCE.iter().any(|(allowed, _)| allowed == key);
                if !known {
                    errors.push(format!("FOUNDATION_ROW_UNDECLARED_FIELD {label} field={key}"));
                }
            }
        }
        if !seen.insert(stage.clone()) {
            errors.push(format!("FOUNDATION_STAGE_DUPLICATED {label}"));
        }

        if checks.unknown_needs_experiment_and_cost {
            for (u, entry) in row.get("unknown").and_then(Value::as_array).map_or(&[][..], |v| v).iter().enumerate() {
                if !nonempty(entry, "question") {
                    errors.push(format!("FOUNDATION_UNKNOWN_NO_QUESTION {label} unknown={}", u + 1));
                }
                if !nonempty(entry, "experiment") {
                    errors.push(format!(
                        "FOUNDATION_UNKNOWN_NO_EXPERIMENT {label} unknown={} — a question with \
                         no falsifier is a wish, not an UNKNOWN",
                        u + 1
                    ));
                }
                if !nonempty(entry, "cost") {
                    errors.push(format!("FOUNDATION_UNKNOWN_NO_COST {label} unknown={}", u + 1));
                }
            }
        }

        if checks.gap_needs_cost_and_owner {
            for (g, entry) in row.get("gaps").and_then(Value::as_array).map_or(&[][..], |v| v).iter().enumerate() {
                if !nonempty(entry, "gap") {
                    errors.push(format!("FOUNDATION_GAP_NO_TEXT {label} gap={}", g + 1));
                }
                if !nonempty(entry, "cost_if_left_open") {
                    errors.push(format!("FOUNDATION_GAP_NO_COST {label} gap={}", g + 1));
                }
                if !nonempty(entry, "owner") {
                    errors.push(format!("FOUNDATION_GAP_NO_OWNER {label} gap={}", g + 1));
                }
            }
        }

        if checks.crate_absence_needs_must_be_created {
            for entry in row.get("crates").and_then(Value::as_array).map_or(&[][..], |v| v) {
                let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() {
                    errors.push(format!("FOUNDATION_CRATE_NO_NAME {label}"));
                    continue;
                }
                let role = entry.get("role").and_then(Value::as_str).unwrap_or("");
                if !matches!(role, "mechanism" | "thin_caller") {
                    errors.push(format!(
                        "FOUNDATION_CRATE_ROLE_INVALID {label} crate={name} role={role}"
                    ));
                }
                let declared_exists = entry.get("exists").and_then(Value::as_bool);
                let actually_exists = on_disk.contains(name);
                if declared_exists != Some(actually_exists) {
                    errors.push(format!(
                        "FOUNDATION_CRATE_EXISTS_MISDECLARED {label} crate={name} \
                         declared={declared_exists:?} on_disk={actually_exists}"
                    ));
                }
                let must_create = entry
                    .get("must_be_created")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !actually_exists && !must_create {
                    errors.push(format!(
                        "FOUNDATION_CRATE_ABSENT_NOT_MARKED {label} crate={name} — absent from \
                         crates/ and not marked must_be_created"
                    ));
                }
            }
        }

        if checks.numbers_must_not_be_bare_figures {
            for entry in row.get("numbers").and_then(Value::as_array).map_or(&[][..], |v| v) {
                match entry.as_str() {
                    Some(key) if is_registry_key(key) => {}
                    Some(bad) => errors.push(format!(
                        "FOUNDATION_NUMBER_IS_BARE_FIGURE {label} value={bad:?} — numbers[] \
                         carries NUMBERS.toml keys, never values"
                    )),
                    None => errors.push(format!(
                        "FOUNDATION_NUMBER_NOT_A_KEY {label} value={entry} — a figure declared \
                         here has no derivation"
                    )),
                }
            }
        }

        for entry in row.get("gates").and_then(Value::as_array).map_or(&[][..], |v| v) {
            if !nonempty(entry, "gate") {
                errors.push(format!("FOUNDATION_GATE_NO_TEXT {label}"));
            }
            if !nonempty(entry, "known_bad_leg") {
                errors.push(format!(
                    "FOUNDATION_GATE_NO_KNOWN_BAD_FIELD {label} — must name the test function or \
                     say NONE; silence is not the same claim as NONE"
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(rows.len())
    } else {
        Err(errors)
    }
}

// ─────────────────────────────────────────────────────────────────── the legs

/// KNOWN-GOOD. The real artifact must pass every predicate.
///
/// An attack-only suite ships an over-strict gate, and an over-strict gate gets routed
/// around — a slower death than no gate. `state-wildcard-lint` reached 89% false positives
/// without this leg.
#[test]
fn foundation_rows_are_complete_and_non_vacuous() {
    let root = repo_root();
    let path = root.join("docs/plan/FOUNDATION.jsonl");
    let rows = read_rows(&path).expect("FOUNDATION.jsonl must be readable");
    let on_disk = crates_on_disk(&root);
    assert!(
        !on_disk.is_empty(),
        "ANTI-VACUITY: crates/ read as empty, so every crate-existence check would pass \
         vacuously"
    );

    let count = validate(&rows, &on_disk, Checks::default())
        .unwrap_or_else(|errors| panic!("FOUNDATION.jsonl is invalid:\n  {}", errors.join("\n  ")));

    assert_eq!(count, STAGES.len(), "expected one row per stage S1..S9");
    let stages: BTreeSet<&str> = rows
        .iter()
        .filter_map(|row| row.get("stage").and_then(Value::as_str))
        .collect();
    let want: BTreeSet<&str> = STAGES.iter().copied().collect();
    assert_eq!(stages, want, "every stage S1..S9 exactly once");
}

/// ANTI-VACUITY. Zero rows is an ERROR, not a pass.
#[test]
fn an_empty_foundation_is_an_error_not_a_pass() {
    let errors = validate(&[], &crates_on_disk(&repo_root()), Checks::default())
        .expect_err("an empty foundation must be refused");
    assert!(
        errors.iter().any(|e| e.starts_with("FOUNDATION_EMPTY")),
        "the refusal must name vacuity: {errors:?}"
    );
}

/// A planted row carrying a bare figure and an experiment-less unknown.
fn known_bad_row() -> Value {
    serde_json::json!({
        "schema_version": "journey_foundation.v1",
        "stage": "S5",
        "input_refs": ["S4"],
        "output_refs": ["S6"],
        "owner": "planted",
        "crates": [{"name": "definitely-not-a-crate", "role": "mechanism", "exists": false}],
        "gates": [{"gate": "planted", "known_bad_leg": "NONE"}],
        "numbers": ["469"],
        "known": ["planted"],
        "unknown": [{"question": "does it work?"}],
        "gaps": [{"gap": "planted"}]
    })
}

/// FIRES-ON-KNOWN-BAD, and the refusal must NAME THE ROW.
#[test]
fn the_gate_refuses_a_planted_row_and_names_it() {
    let rows = vec![known_bad_row()];
    let errors = validate(&rows, &crates_on_disk(&repo_root()), Checks::default())
        .expect_err("the planted row must be refused");

    let joined = errors.join("\n");
    for expected in [
        "FOUNDATION_NUMBER_IS_BARE_FIGURE",
        "FOUNDATION_UNKNOWN_NO_EXPERIMENT",
        "FOUNDATION_UNKNOWN_NO_COST",
        "FOUNDATION_GAP_NO_COST",
        "FOUNDATION_GAP_NO_OWNER",
        "FOUNDATION_CRATE_ABSENT_NOT_MARKED",
    ] {
        assert!(joined.contains(expected), "missing {expected} in:\n{joined}");
    }
    assert!(
        errors.iter().all(|e| e.contains("row=1") && e.contains("stage=S5")),
        "every refusal must name the row it came from:\n{joined}"
    );
}

/// MUTATION. Drop the experiment predicate; the experiment-less unknown must stop being
/// caught. A leg that stays red under mutation is not attributable to the check it names.
#[test]
fn dropping_the_experiment_check_stops_catching_the_planted_unknown() {
    let rows = vec![known_bad_row()];
    let on_disk = crates_on_disk(&repo_root());

    let before = validate(&rows, &on_disk, Checks::default()).expect_err("baseline must be red");
    assert!(
        before.iter().any(|e| e.starts_with("FOUNDATION_UNKNOWN_NO_EXPERIMENT")),
        "baseline must catch the experiment-less unknown: {before:?}"
    );

    let mutated = Checks { unknown_needs_experiment_and_cost: false, ..Checks::default() };
    let after = validate(&rows, &on_disk, mutated).expect_err("other predicates still fire");
    assert!(
        !after.iter().any(|e| e.starts_with("FOUNDATION_UNKNOWN_NO_")),
        "with the check disabled the unknown must go unreported — otherwise some other \
         predicate is doing the work and the leg proves nothing: {after:?}"
    );
    assert!(
        after.iter().any(|e| e.starts_with("FOUNDATION_NUMBER_IS_BARE_FIGURE")),
        "the mutation must be SCOPED: the bare-figure predicate is independent and must \
         still fire: {after:?}"
    );
}

/// A row may not assert a crate into existence: `exists` is read from disk.
#[test]
fn a_row_cannot_declare_a_crate_that_is_not_on_disk_as_existing() {
    let mut row = known_bad_row();
    row["crates"] = serde_json::json!([
        {"name": "definitely-not-a-crate", "role": "mechanism", "exists": true,
         "must_be_created": true}
    ]);
    let errors = validate(&[row], &crates_on_disk(&repo_root()), Checks::default())
        .expect_err("a lied-about crate must be refused");
    assert!(
        errors.iter().any(|e| e.starts_with("FOUNDATION_CRATE_EXISTS_MISDECLARED")),
        "{errors:?}"
    );
}

/// An undeclared field is refused, so the allowance list cannot be bypassed by adding keys.
#[test]
fn an_undeclared_field_is_refused_and_the_allowance_is_explicit() {
    let root = repo_root();
    let rows = read_rows(&root.join("docs/plan/FOUNDATION.jsonl")).expect("readable");
    let mut row = rows[0].clone();
    row["smuggled"] = Value::from("not in any allowance row");
    let errors = validate(&[row], &crates_on_disk(&root), Checks::default())
        .expect_err("an undeclared field must be refused");
    assert!(
        errors.iter().any(|e| e.contains("FOUNDATION_ROW_UNDECLARED_FIELD")
            && e.contains("field=smuggled")),
        "{errors:?}"
    );
    // and every allowance row carries a reason, so an exception is never silence
    for (key, reason) in KEY_ALLOWANCE {
        assert!(reason.len() >= 8, "allowance row {key} has no real reason");
    }
}
