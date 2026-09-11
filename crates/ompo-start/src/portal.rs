#![forbid(unsafe_code)]

//! Pure portal-row helpers. The umbrella command supplies live source data.

use crate::sha256_hex;

pub const SCHEMA_ID: &str = "ompo:portal:v1";

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
