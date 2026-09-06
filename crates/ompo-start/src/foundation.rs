#![forbid(unsafe_code)]

//! L5 OUT: append a `stage=S1` row to `docs/plan/FOUNDATION.jsonl` whose
//! `output_refs` name `.omp-orchestrator/inception.json`.

use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub const SCHEMA_VERSION: &str = "journey_foundation.v1";
pub const STAGE: &str = "S1";
pub const INCEPTION_REF: &str = ".omp-orchestrator/inception.json";
pub const SOURCE: &str = "l5-foundation-append";

pub fn s1_row() -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "stage": STAGE,
        "input_refs": ["L5 portal write", "L5-READBACK"],
        "output_refs": [INCEPTION_REF, "S2 planning foundation"],
        "owner": "S1 L5 implementation lane",
        "crates": [{
            "name": "ompo-start",
            "role": "mechanism",
            "exists": true,
            "must_be_created": false
        }],
        "gates": [{
            "gate": "s2-gate refuses L5-METRIC-READBACK-OK=0",
            "known_bad_leg": "absent inception.json"
        }],
        "numbers": ["L5-METRIC-READBACK-OK"],
        "known": ["S1 OUT writes inception.json then appends this row"],
        "unknown": ["whether inception.json has been fsync+readback-ok on this host"],
        "gaps": ["inception.json may still be absent; s2-gate stays red"],
        "source": SOURCE
    })
}

fn already_appended(jsonl: &str) -> bool {
    jsonl.lines().filter(|line| !line.trim().is_empty()).any(|line| {
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|row| row.get("source").and_then(Value::as_str).map(str::to_owned))
            .as_deref()
            == Some(SOURCE)
    })
}

/// Append the L5 S1 row unless this writer has already landed it.
/// Returns true when a row was written.
pub fn append_s1_foundation(path: &Path) -> io::Result<bool> {
    let existing = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    if already_appended(&existing) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut line = serde_json::to_string(&s1_row()).map_err(io::Error::other)?;
    line.push('\n');
    file.write_all(line.as_bytes())?;
    file.flush()?;
    Ok(true)
}

pub fn s1_rows_citing_inception(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row.get("stage").and_then(Value::as_str) == Some(STAGE))
        .filter(|row| {
            row.get("output_refs")
                .and_then(Value::as_array)
                .map(|refs| {
                    refs.iter().any(|r| {
                        r.as_str()
                            .map(|s| s.contains("inception.json"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .collect()
}
