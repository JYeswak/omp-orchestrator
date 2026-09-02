#![forbid(unsafe_code)]

//! Reusable validator for the orchestration tick receipt contract.
//!
//! This crate validates rows that exist. It does not create the ledger or
//! claim that every supervisor tick emitted one; that producer is a separate
//! wiring concern.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The result class for a missing or empty ledger.
pub const NOTHING_TO_CHECK: &str = "NOTHING_TO_CHECK";

/// A receipt validation failure with a stable row/law marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The ledger is absent or contains no receipt rows.
    NothingToCheck(String),
    /// A physical row cannot be decoded as one JSON object.
    Invalid(String),
}

/// Read one JSON object per physical ledger line.
///
/// Missing, empty, and whitespace-only ledgers are errors. A multiline JSON
/// object is also an error: the ledger contract is JSONL, so the physical line
/// is the addressable evidence row.
pub fn read_ledger(path: &Path) -> Result<Vec<Value>, LedgerError> {
    let bytes = fs::read(path).map_err(|error| {
        LedgerError::NothingToCheck(format!(
            "{NOTHING_TO_CHECK}: orchestration ledger unavailable at {}: {error}",
            path.display()
        ))
    })?;
    parse_ledger(&bytes)
}

/// Parse staged ledger bytes without reading the worktree version.
pub fn parse_ledger(bytes: &[u8]) -> Result<Vec<Value>, LedgerError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(LedgerError::NothingToCheck(format!(
            "{NOTHING_TO_CHECK}: orchestration ledger has zero receipt rows"
        )));
    }

    let mut rows = Vec::new();
    for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let row = serde_json::from_slice(line).map_err(|error| {
            LedgerError::Invalid(format!(
                "LEDGER_ROW_INVALID row={} error={error}",
                index + 1
            ))
        })?;
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(LedgerError::NothingToCheck(format!(
            "{NOTHING_TO_CHECK}: orchestration ledger has zero receipt rows"
        )));
    }
    Ok(rows)
}

/// Validate one production-shaped orchestration receipt.
///
/// The six laws are named in the returned text. OC-L3 and OC-L5 are checked
/// when their receipt fields are present; a missing producer remains outside
/// this row validator's scope.
pub fn validate_receipt(receipt: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let observed = receipt.get("observed").and_then(Value::as_object);
    let free = observed
        .and_then(|value| value.get("free_capacity"))
        .and_then(Value::as_array)
        .map(|panes| {
            panes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    let dispatched = receipt
        .get("dispatched")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let refused = receipt
        .get("refused")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // OC-L1: every observed free pane is dispatched or refused exactly once.
    let mut named = BTreeMap::<String, usize>::new();
    for row in dispatched.iter().chain(refused.iter()) {
        if let Some(pane) = row.get("pane").and_then(Value::as_str) {
            *named.entry(pane.to_owned()).or_default() += 1;
        } else {
            errors.push("OC-L1: dispatch/refusal row has no pane".to_owned());
        }
    }
    for pane in &free {
        match named.get(pane).copied().unwrap_or(0) {
            0 => errors.push(format!(
                "OC-L1: free pane {pane} is neither dispatched nor refused"
            )),
            1 => {}
            count => errors.push(format!(
                "OC-L1: free pane {pane} is named {count} times; expected exactly once"
            )),
        }
    }
    for pane in named.keys().filter(|pane| !free.contains(*pane)) {
        errors.push(format!(
            "OC-L1: pane {pane} is named but was not observed free"
        ));
    }
    for row in &refused {
        let reason = row
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if reason.trim().is_empty() {
            let pane = row
                .get("pane")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            errors.push(format!("OC-L1: refusal for {pane} has no reason"));
        }
    }

    // OC-L2: dispatch can name only a claimed bead.
    for row in &dispatched {
        let bead = row
            .get("bead")
            .and_then(Value::as_str)
            .unwrap_or("<missing>");
        if row.get("claimed_first") != Some(&Value::Bool(true)) {
            errors.push(format!("OC-L2: dispatch names unclaimed bead {bead}"));
        }
    }

    // OC-L3: a row may not report edits to delegated paths.
    if let Some(edits) = receipt.get("edits").and_then(Value::as_array) {
        for edit in edits {
            let path = edit
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            errors.push(format!("OC-L3: orchestrator edited delegated path {path}"));
        }
    }

    // OC-L4: measured claims carry their producing command.
    if let Some(claims) = receipt.get("claims").and_then(Value::as_array) {
        for claim in claims {
            let figure = claim
                .get("figure")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let command = claim
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if figure.trim().is_empty() || command.trim().is_empty() {
                errors.push("OC-L4: measured claim is missing its producing command".to_owned());
            }
        }
    } else {
        errors.push("OC-L4: claims is not an array".to_owned());
    }

    // OC-L5: investigation and routing cannot share a tick with free capacity.
    if receipt.get("investigation").is_some_and(has_nonempty_value) && !free.is_empty() {
        errors.push(format!(
            "OC-L5: investigation row has non-zero free capacity ({})",
            free.len()
        ));
    }

    // OC-L6: destructive actions cite a prior measurement row.
    let tick = receipt.get("tick").and_then(Value::as_u64).unwrap_or(0);
    if let Some(actions) = receipt.get("destructive").and_then(Value::as_array) {
        for action in actions {
            let target = action
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            let evidence_row = action.get("evidence_row").and_then(Value::as_u64);
            if evidence_row.is_none_or(|row| row == 0 || row >= tick) {
                errors.push(format!(
                    "OC-L6: destructive action on {target} lacks a prior measurement row"
                ));
            }
        }
    } else {
        errors.push("OC-L6: destructive is not an array".to_owned());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn has_nonempty_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Number(_) => true,
    }
}

/// Render the stable human/machine-readable result for one receipt.
#[must_use]
pub fn report(receipt: &Value) -> String {
    match validate_receipt(receipt) {
        Ok(()) => "ORCHESTRATION_TICK_GATE status=CLEAN".to_owned(),
        Err(errors) => format!(
            "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}",
            errors.join("\n")
        ),
    }
}

/// Extract a stable law code from a validator error.
#[must_use]
pub fn law_code(error: &str) -> &str {
    error.split_once(':').map_or("UNKNOWN", |(law, _)| law)
}

/// Return the default canonical ledger path relative to a repository root.
#[must_use]
pub fn default_ledger_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".flywheel/orchestration-ticks.jsonl")
}
