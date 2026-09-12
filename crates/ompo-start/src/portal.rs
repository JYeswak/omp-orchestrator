#![forbid(unsafe_code)]

//! Pure portal-row helpers. The umbrella command supplies live source data.

use crate::sha256_hex;
use std::collections::HashSet;
use std::path::Path;


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

/// Required envelope fields of an observability block: the four a reader
/// depends on. `schema_id` versions the grammar, `sources` carries the
/// per-source ages, `readback_ok` carries the readback verdict, and
/// `data_hash` covers the other three. A block missing any of these is not a
/// short row — it is not a row at all.
pub const ENVELOPE_FIELDS: &[&str] = &["schema_id", "sources", "readback_ok", "data_hash"];

/// Refuse a portal JSON missing a required envelope field. A missing field
/// is a typed `Err` naming the field — never a partial object treated as
/// success, and never a defaulted value standing in for an absent reading
/// (the absent-collapses-to-a-value class: a missing `sources` defaulted to
/// empty would read as "no sources observed" rather than "no block").
pub fn require_envelope(row: &serde_json::Value) -> Result<(), String> {
    let object = row
        .as_object()
        .ok_or_else(|| "L5_ENVELOPE_NOT_OBJECT — a portal row is a JSON object".to_owned())?;
    for field in ENVELOPE_FIELDS {
        if !object.contains_key(*field) {
            return Err(format!(
                "L5_ENVELOPE_MISSING_FIELD field={field} — a portal JSON without its envelope is not a row"
            ));
        }
    }
    Ok(())
}

/// L5-CURSOR (enjg): optional cursor fields for snapshot-shaped portals.
///
/// A one-shot portal may omit cursor state; a snapshot must not invent it.
/// `latest_cursor` is a number when the position is known, else null WITH a
/// reason in `cursor_reason`. `replay_window` is a `[lo, hi]` pair when the
/// replay bounds are known, else null under the same reason. A null without
/// a reason is refused rather than rendered: a bare null says nothing about
/// WHY the position is unknown, and the cheaper rendering — a fabricated 0 —
/// would read as a real position (replay from the start) instead of as
/// ignorance. `None` therefore never becomes `0` on any path through here.
pub fn cursor_envelope(
    cursor: Option<u64>,
    window: Option<(u64, u64)>,
    reason: Option<&str>,
) -> Result<serde_json::Value, String> {
    if (cursor.is_none() || window.is_none()) && reason.is_none() {
        return Err(
            "L5_CURSOR_NULL_WITHOUT_REASON — a null cursor field without a reason explains nothing; refusing rather than rendering ignorance as data"
                .to_owned(),
        );
    }
    Ok(serde_json::json!({
        "latest_cursor": cursor,
        "replay_window": window.map(|(lo, hi)| vec![lo, hi]),
        "cursor_reason": reason,
    }))
}

/// L5-DECISIONS (4tq2): unpaid HD rows with age, NEVER omitted.
///
/// Empty array is the honest zero (no ledger, unreadable ledger, or nothing
/// owed). Omitting the field is the defect: a reader cannot tell "none owed"
/// from "the portal does not know about decisions". `age_s` is `now_s - ts`
/// for a numeric unix `ts`; rows whose `ts` cannot be parsed are dropped
/// rather than emitting a fabricated 0 (a 0-second age reads as "just asked").
/// A question is owed when it has an `HD-` id, is not answered in-row, and no
/// other row with a non-empty `decision` points at it via `answers`.
#[must_use]
pub fn decisions_owed(rows: &[decision_ledger::Row], now_s: u64) -> serde_json::Value {
    let mut settled: HashSet<String> = HashSet::new();
    for row in rows {
        if !row.is_answered() {
            continue;
        }
        if let Some(id) = row.id() {
            settled.insert(id.to_owned());
        }
        for target in answer_targets(row) {
            settled.insert(target);
        }
    }
    let mut owed = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for row in rows {
        let Some(id) = row.id() else {
            continue;
        };
        if !id.starts_with("HD-") || settled.contains(id) || !seen.insert(id.to_owned()) {
            continue;
        }
        let Some(asked_s) = ts_unix_s(row) else {
            continue;
        };
        let age_s = now_s.saturating_sub(asked_s);
        owed.push(serde_json::json!({ "id": id, "age_s": age_s }));
    }
    serde_json::Value::Array(owed)
}

/// Always an array. Missing/unreadable ledger → `[]`, never omitted.
#[must_use]
pub fn decisions_owed_from_repo(repo: &Path, now_s: u64) -> serde_json::Value {
    let path = repo.join("docs").join("decisions.jsonl");
    match decision_ledger::read_rows(&path) {
        Ok(rows) => decisions_owed(&rows, now_s),
        Err(_) => serde_json::json!([]),
    }
}

/// L5-METRIC-HOME (sheg): the expectation verdict for the `decisions_owed`
/// age metric. There is no xkr6 row shipping a duration-capable threshold
/// for this metric, so no numeric delta can be honestly computed: any number
/// here would be a homeless green. The verdict is therefore UNMEASURABLE
/// with the missing home named, in xkr6's own row vocabulary (`metric`,
/// `unit`, `expected`, `threshold`, `delta`, `verdict`). When xkr6 ships the
/// row, this function is where its expected/threshold land — not a parallel
/// schema beside it.
#[must_use]
pub fn decisions_owed_delta() -> serde_json::Value {
    serde_json::json!({
        "metric": "decisions_owed_age_s",
        "unit": "s",
        "expected": null,
        "threshold": null,
        "delta": "UNMEASURABLE",
        "verdict": "UNMEASURABLE",
        "reason": "no xkr6 expectation row ships a duration-capable threshold for decisions_owed age; refusing a numeric delta without a home",
    })
}

fn answer_targets(row: &decision_ledger::Row) -> Vec<String> {
    match row.value.get("answers") {
        Some(serde_json::Value::String(target)) if !target.is_empty() => vec![target.clone()],
        Some(serde_json::Value::Array(targets)) => targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|target| !target.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn ts_unix_s(row: &decision_ledger::Row) -> Option<u64> {
    match row.value.get("ts") {
        Some(serde_json::Value::Number(number)) => number.as_u64(),
        Some(serde_json::Value::String(text)) => text.parse().ok(),
        _ => None,
    }
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

/// The observability block a reader polls: ONE object carrying its own schema
/// id, the per-source ages, the readback verdict, and a content hash taken over
/// everything EXCEPT itself.
///
/// Four S1 readbacks land in this single object rather than four scattered
/// top-level keys -- `.observability.schema_id`, `.observability.sources`,
/// `.observability.readback_ok`, `.observability.data_hash` -- so the four
/// cannot drift apart: they are assembled once, hashed once, and the hash
/// covers the other three. A reader that trusts `data_hash` is therefore
/// trusting the schema id, the ages and the readback verdict together.
///
/// `L5_OBS_EMPTY_SOURCES_WHILE_LIVE` is what makes `sources` load-bearing: an
/// EMPTY source set while the verdict claims live is the known-bad, and it is
/// an ERROR here rather than an empty map reading as "fine". An empty scan set
/// otherwise reports identically to a complete one that found nothing.
pub fn observability(
    sources: &[crate::liveness::SourceVerdict],
    live: bool,
    readback_ok: bool,
) -> Result<serde_json::Value, String> {
    if sources.is_empty() && live {
        return Err(
            "L5_OBS_EMPTY_SOURCES_WHILE_LIVE reason=zero_sources_claimed_live".to_owned()
        );
    }
    // The per-source rows are the CANONICAL emitter's, not a third rendering of
    // one. `liveness::sources_json` already owns "what a source row looks like"
    // and carries `age_ms` plus the silent/gap fields; re-deriving those four
    // keys here would put a third emitter in the tree beside it and
    // ompo-doctor's local one, and two emitters that can disagree is a shape
    // this repo has already paid for. This block therefore contributes only the
    // AGGREGATE a reader cannot compute from one row.
    let rows = crate::liveness::sources_json(sources);
    // "min age or silent": the freshest age actually OBSERVED, or null when
    // every source is silent. Only `Some(age)` participates, so a source with
    // no reading can never contribute a 0 that would read as perfectly fresh.
    let min_age_ms = sources.iter().filter_map(|source| source.age_ms).min();
    // `readback_ok` is 0/1, not a bool: the acceptance reads it as a number
    // (`expect 0` while inception is absent), and a `false` rendering as the
    // string "false" would satisfy a truthiness test while failing that read.
    seal(serde_json::json!({
        "schema_id": SCHEMA_ID,
        "sources": rows,
        // Counted from the SLICE, never from the map: `sources_json` keys each
        // row by canonical AND short name, so the map length is an alias count
        // and would over-report the census.
        "source_count": sources.len(),
        "min_age_ms": min_age_ms,
        "live": live,
        "readback_ok": u8::from(readback_ok),
    }))
}
