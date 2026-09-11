#![forbid(unsafe_code)]

//! Pure portal-row helpers. The umbrella command supplies live source data.

use crate::sha256_hex;

pub const SCHEMA_ID: &str = "ompo:portal:v1";

/// The portal row's schema VERSION, beside its id.
///
/// This is a constant rather than a literal at the emit site because the row
/// previously spelled `"schema_version": "1"` inline in `ompo-doctor`'s
/// `run_portal`, which made the portal schema's own identity a SECOND source of
/// truth living in a different crate from `SCHEMA_ID`. The two halves of one
/// contract could then drift independently, and nothing in `ompo-start` could
/// assert what the emitted version actually was.
pub const SCHEMA_VERSION: &str = "1";

/// Hash an object after removing its self-referential data_hash field.
#[must_use]
pub fn data_hash_without_self(row: &serde_json::Value) -> String {
    let mut payload = row.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("data_hash");
    }
    let bytes = serde_json::to_vec(&payload).expect("portal row is JSON-serializable");
    sha256_hex(&bytes)
}

/// Add the content hash after the row has been assembled.
pub fn seal(mut row: serde_json::Value) -> Result<serde_json::Value, String> {
    if !row.is_object() {
        return Err("L5_PORTAL_ROW_NOT_OBJECT".to_owned());
    }
    row.as_object_mut()
        .expect("object checked")
        .remove("data_hash");
    let hash = data_hash_without_self(&row);
    row.as_object_mut()
        .expect("object checked")
        .insert("data_hash".to_owned(), serde_json::Value::String(hash));
    Ok(row)
}

/// Queue depth read from the JSONL mirror, never from a subprocess.
///
/// `br`/`bv` are absent on CI lanes (MISSING_EXECUTABLE at the gate runner),
/// so a row sourced from them would be UNOBSERVABLE exactly where the gate
/// looks. `.beads/issues.jsonl` is the same store they read. Terminal states
/// are closed + tombstone; everything else is still on the board. `by_status`
/// carries the full distribution so the depth definition is checkable here,
/// not trusted. Unparseable lines bound the result to PARTIAL; a missing or
/// unreadable mirror is UNOBSERVABLE with a reason -- never an empty depth
/// reading as "fine" (onl90).
pub fn queue_depth(repo_root: &std::path::Path) -> serde_json::Value {
    const TERMINAL: [&str; 2] = ["closed", "tombstone"];
    let path = repo_root.join(".beads").join("issues.jsonl");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return serde_json::json!({
                "state": "UNOBSERVABLE",
                "source": ".beads/issues.jsonl",
                "reason": format!("mirror unreadable: {error}"),
            })
        }
    };
    let mut by_status = std::collections::BTreeMap::new();
    let mut parsed = 0usize;
    let mut total = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        let Ok(row) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let status = row
            .get("status")
            .and_then(|status| status.as_str())
            .unwrap_or("?")
            .to_owned();
        *by_status.entry(status).or_insert(0usize) += 1;
        parsed += 1;
    }
    let depth: usize = by_status
        .iter()
        .filter(|(status, _)| !TERMINAL.contains(&status.as_str()))
        .map(|(_, count)| count)
        .sum();
    if parsed < total {
        serde_json::json!({
            "state": "PARTIAL",
            "source": ".beads/issues.jsonl",
            "bound_kind": "unparseable_lines",
            "bound_value": parsed,
            "lines_total": total,
            "depth": depth,
            "by_status": by_status,
        })
    } else {
        serde_json::json!({
            "state": "FULL",
            "source": ".beads/issues.jsonl",
            "depth": depth,
            "by_status": by_status,
        })
    }
}

/// Gate verdicts from the runner's aggregate line, never by re-running gates.
///
/// Source is `target/gate-runner-aggregate.line` (written AFTER a gate-runner
/// measurement, removed BEFORE it, so absence means "no verdict was produced"
/// -- gate-runner's own contract). Grammar mirrors
/// `gate_runner::ci_citation::parse_aggregate`: a `GATE_RUNNER crates=` line of
/// `key=value` u64 tokens with exactly crates/pass/fail/unmeasurable/short/
/// no_tests present and pass+fail+unmeasurable==crates. A missing file is
/// UNOBSERVABLE (no verdict to read); a malformed line is UNOBSERVABLE with
/// the parse reason (no valid verdict to read). Neither is a zero, and neither
/// passes anything (8vhx9).
pub fn gates_verdict(repo_root: &std::path::Path) -> serde_json::Value {
    const SOURCE: &str = "target/gate-runner-aggregate.line";
    let path = match std::env::var("GATE_RUNNER_AGGREGATE") {
        Ok(explicit) if !explicit.trim().is_empty() => std::path::PathBuf::from(explicit),
        _ => repo_root.join(SOURCE),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return serde_json::json!({
                "state": "UNOBSERVABLE",
                "source": SOURCE,
                "reason": format!("no aggregate verdict on disk (no run produced one here): {error}"),
            })
        }
    };
    match parse_gates_aggregate(&text) {
        Ok(values) => {
            let mut verdict = serde_json::json!({
                "state": "FULL",
                "source": SOURCE,
            });
            for (key, value) in &values {
                verdict[key] = serde_json::Value::from(*value);
            }
            verdict
        }
        Err(reason) => serde_json::json!({
            "state": "UNOBSERVABLE",
            "source": SOURCE,
            "reason": reason,
        }),
    }
}

/// Parse one aggregate verdict line. Pure: every refusal shape is unit-pinned
/// below, so the grammar cannot drift from `ci_citation::parse_aggregate`.
pub fn parse_gates_aggregate(
    text: &str,
) -> Result<std::collections::BTreeMap<String, u64>, String> {
    let line = text
        .lines()
        .filter(|line| line.starts_with("GATE_RUNNER crates="))
        .last()
        .ok_or_else(|| "no GATE_RUNNER crates= line".to_owned())?;
    let mut values = std::collections::BTreeMap::new();
    for token in line.split_whitespace().skip(1) {
        let (key, value) = token.split_once('=').ok_or_else(|| {
            format!("malformed token without '=': {token}")
        })?;
        let number: u64 = value.parse().map_err(|_| {
            format!("non-numeric value for {key}: {value}")
        })?;
        values.insert(key.to_owned(), number);
    }
    for key in [
        "crates",
        "pass",
        "fail",
        "unmeasurable",
        "short",
        "no_tests",
    ] {
        if !values.contains_key(key) {
            return Err(format!("missing required key: {key}"));
        }
    }
    if values["pass"] + values["fail"] + values["unmeasurable"] != values["crates"] {
        return Err("invariant violated: pass+fail+unmeasurable != crates".to_owned());
    }
    Ok(values)
}
