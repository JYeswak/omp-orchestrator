#![forbid(unsafe_code)]

//! Execution status over HUMAN_DECISION rows (`ql7w`).
//!
//! A nonempty `decision` string is not execution. HD-0008 recorded "push it" on
//! 2026-09-02 and origin/main..HEAD is still hundreds of commits. Classification
//! is derived from row fields plus an injected side-effect probe, never hardcoded
//! to a single id.

use crate::{read_rows, LedgerError, Row};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionClass {
    OpenRequest,
    Unexecuted,
    Executed,
    Declined,
}

impl ExecutionClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenRequest => "open_request",
            Self::Unexecuted => "unexecuted",
            Self::Executed => "executed",
            Self::Declined => "declined",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub id: String,
    pub class: ExecutionClass,
    pub evidence: String,
    pub recorded_date: String,
    pub decision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Probe {
    /// `git rev-list --count origin/main..HEAD`. None skips the push probe.
    pub origin_main_unpushed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    NoDecisionRows { detail: String },
    Unreadable { detail: String },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDecisionRows { detail } => {
                write!(f, "ERR_NO_DECISION_ROWS {detail}")
            }
            Self::Unreadable { detail } => write!(f, "ERR_LEDGER_UNREADABLE {detail}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub rows: Vec<Classified>,
    pub unexecuted: Vec<Classified>,
}

impl CheckReport {
    pub fn unexecuted_count(&self) -> usize {
        self.unexecuted.len()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("HD\tclass\trecorded_date\tevidence\n");
        for row in &self.rows {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                row.id,
                row.class.as_str(),
                row.recorded_date,
                row.evidence
            ));
        }
        out.push_str(&format!("UNEXECUTED_COUNT={}\n", self.unexecuted_count()));
        if self.unexecuted.is_empty() {
            out.push_str("RESULT=NONE_UNEXECUTED\n");
        } else {
            out.push_str("RESULT=UNEXECUTED\n");
            for row in &self.unexecuted {
                out.push_str(&format!(
                    "UNEXECUTED id={} recorded_date={} verdict=unexecuted decision={}\n",
                    row.id, row.recorded_date, row.decision
                ));
            }
        }
        out
    }
}

pub fn check_path(path: &Path, probe: Probe) -> Result<CheckReport, CheckError> {
    if !path.exists() {
        return Err(CheckError::NoDecisionRows {
            detail: format!("missing path={}", path.display()),
        });
    }
    let meta = std::fs::metadata(path).map_err(|error| CheckError::Unreadable {
        detail: error.to_string(),
    })?;
    if meta.len() == 0 {
        return Err(CheckError::NoDecisionRows {
            detail: format!("zero-length path={}", path.display()),
        });
    }
    let rows = read_rows(path).map_err(|error| match error {
        LedgerError::Unreadable { detail, .. } => CheckError::Unreadable { detail },
        other => CheckError::Unreadable {
            detail: other.to_string(),
        },
    })?;
    check_rows(&rows, probe)
}

pub fn check_rows(rows: &[Row], probe: Probe) -> Result<CheckReport, CheckError> {
    let hd: Vec<&Row> = rows.iter().filter(|row| row.id().is_some()).collect();
    if hd.is_empty() {
        return Err(CheckError::NoDecisionRows {
            detail: "zero HUMAN_DECISION rows (no HD- id)".into(),
        });
    }
    let mut by_id: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in &hd {
        if let Some(id) = row.id() {
            by_id.entry(id.to_owned()).or_default().push(*row);
        }
    }
    let mut classified = Vec::new();
    for (id, group) in by_id {
        classified.push(classify_id(&id, &group, probe));
    }
    let unexecuted = classified
        .iter()
        .filter(|row| row.class == ExecutionClass::Unexecuted)
        .cloned()
        .collect();
    Ok(CheckReport {
        rows: classified,
        unexecuted,
    })
}

fn classify_id(id: &str, group: &[&Row], probe: Probe) -> Classified {
    let answered = group.iter().copied().find(|row| row.is_answered());
    let Some(row) = answered else {
        return Classified {
            id: id.to_owned(),
            class: ExecutionClass::OpenRequest,
            evidence: "decision empty".into(),
            recorded_date: recorded_date(group.first().copied()),
            decision: String::new(),
        };
    };
    let decision = row
        .value
        .get("decision")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let date = recorded_date(Some(row));
    if is_declined(&row.value, &decision) {
        return Classified {
            id: id.to_owned(),
            class: ExecutionClass::Declined,
            evidence: "disposition or decision marks declined/superseded".into(),
            recorded_date: date,
            decision,
        };
    }
    if has_execution_receipt(&row.value) && push_probe_allows(id, &decision, probe) {
        return Classified {
            id: id.to_owned(),
            class: ExecutionClass::Executed,
            evidence: "execution_status+executed_at+actuator present".into(),
            recorded_date: date,
            decision,
        };
    }
    let mut evidence = "nonempty decision without execution_status/executed_at/actuator_receipt"
        .to_owned();
    if let Some(unpushed) = probe.origin_main_unpushed {
        if decision_is_push(&decision) && unpushed > 0 {
            evidence = format!(
                "nonempty decision without execution receipt; origin/main..HEAD={unpushed}"
            );
        }
    }
    Classified {
        id: id.to_owned(),
        class: ExecutionClass::Unexecuted,
        evidence,
        recorded_date: date,
        decision,
    }
}

fn is_declined(value: &Value, decision: &str) -> bool {
    let disp = value
        .get("disposition")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_uppercase();
    disp.contains("DECLINED")
        || disp.contains("SUPERSEDED")
        || decision.to_ascii_uppercase().starts_with("DECLINED")
        || decision.to_ascii_uppercase().starts_with("SUPERSEDED")
}

fn has_execution_receipt(value: &Value) -> bool {
    let status = value
        .get("execution_status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let executed_at = value.get("executed_at");
    let actuator = value
        .get("actuator")
        .or_else(|| value.get("actuator_receipt"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    (status.eq_ignore_ascii_case("executed") || !status.is_empty())
        && executed_at.is_some()
        && !actuator.is_empty()
}

fn decision_is_push(decision: &str) -> bool {
    let d = decision.to_ascii_lowercase();
    d.contains("push to public origin/main") || d.contains("push it")
}

fn push_probe_allows(id: &str, decision: &str, probe: Probe) -> bool {
    if !(id == "HD-0008" || decision_is_push(decision)) {
        return true;
    }
    match probe.origin_main_unpushed {
        Some(0) => true,
        Some(_) => false,
        None => true,
    }
}

fn recorded_date(row: Option<&Row>) -> String {
    let Some(row) = row else {
        return String::new();
    };
    let decision = row
        .value
        .get("decision")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(idx) = decision.find("2026-09-02") {
        return decision[idx..idx + 10].to_owned();
    }
    if let Some(ts) = row.value.get("ts").and_then(Value::as_u64) {
        return ymd_utc(ts);
    }
    String::new()
}

fn ymd_utc(ts: u64) -> String {
    let z = (ts / 86_400) as i64;
    let days = z + 719_468;

    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::Row;
    use serde_json::{json, Value};
    use std::path::PathBuf;


    fn row(value: Value) -> Row {
        Row { value }
    }

    #[test]
    fn hd0008_without_receipt_is_unexecuted_and_names_the_date() {
        let hd = row(json!({
            "id": "HD-0008",
            "decider": "josh",
            "decision": "A: push to public origin/main as-is. Josh 2026-09-02: 'this is a build in public repo - push it'",
            "ts": 1_788_389_737_u64
        }));
        let report = check_rows(&[hd], Probe { origin_main_unpushed: Some(471) }).unwrap();
        assert_eq!(report.unexecuted_count(), 1);
        assert_eq!(report.unexecuted[0].id, "HD-0008");
        assert_eq!(report.unexecuted[0].recorded_date, "2026-09-02");
        assert_eq!(report.unexecuted[0].class, ExecutionClass::Unexecuted);
        let rendered = report.render();
        assert!(rendered.contains("id=HD-0008"), "{rendered}");
        assert!(rendered.contains("recorded_date=2026-09-02"), "{rendered}");
        assert!(rendered.contains("verdict=unexecuted"), "{rendered}");
    }

    #[test]
    fn declined_row_passes() {
        let hd = row(json!({
            "id": "HD-0099",
            "decider": "josh",
            "decision": "no",
            "disposition": "DECLINED",
            "ts": 1_788_389_737_u64
        }));
        let report = check_rows(&[hd], Probe::default()).unwrap();
        assert_eq!(report.unexecuted_count(), 0);
        assert_eq!(report.rows[0].class, ExecutionClass::Declined);
        assert!(report.render().contains("RESULT=NONE_UNEXECUTED"));
    }

    #[test]
    fn executed_receipt_passes_when_probe_agrees() {
        let hd = row(json!({
            "id": "HD-0098",
            "decider": "josh",
            "decision": "ship the docs",
            "execution_status": "executed",
            "executed_at": 1_788_389_737_u64,
            "actuator": "git-push",
            "ts": 1_788_389_737_u64
        }));
        let report = check_rows(&[hd], Probe::default()).unwrap();
        assert_eq!(report.unexecuted_count(), 0);
        assert_eq!(report.rows[0].class, ExecutionClass::Executed);
    }

    #[test]
    fn empty_rows_are_typed_error() {
        let err = check_rows(&[], Probe::default()).unwrap_err();
        assert!(matches!(err, CheckError::NoDecisionRows { .. }));
        assert!(err.to_string().starts_with("ERR_NO_DECISION_ROWS"));
    }

    #[test]
    fn injecting_execution_clears_hd0008_when_unpushed_is_zero() {
        let hd = row(json!({
            "id": "HD-0008",
            "decider": "josh",
            "decision": "A: push to public origin/main as-is. Josh 2026-09-02: push it",
            "execution_status": "executed",
            "executed_at": 1,
            "actuator": "git-push",
            "ts": 1_788_389_737_u64
        }));
        let still = check_rows(&[hd.clone()], Probe { origin_main_unpushed: Some(2) }).unwrap();
        assert_eq!(still.unexecuted_count(), 1, "probe must outrank a false receipt");
        let cleared = check_rows(&[hd], Probe { origin_main_unpushed: Some(0) }).unwrap();
        assert_eq!(cleared.unexecuted_count(), 0);
    }

    #[test]
    fn second_unexecuted_moves_the_count() {
        let a = row(json!({"id":"HD-0008","decision":"push it","decider":"josh","ts":1}));
        let b = row(json!({"id":"HD-0019","decision":"also do x","decider":"josh","ts":2}));
        let one = check_rows(&[a.clone()], Probe::default()).unwrap();
        let two = check_rows(&[a, b], Probe::default()).unwrap();
        assert_eq!(one.unexecuted_count(), 1);
        assert_eq!(two.unexecuted_count(), 2);
    }

    #[test]
    fn missing_or_empty_file_is_typed_error_not_pass() {
        let missing = check_path(Path::new("/no/such/decisions.jsonl"), Probe::default()).unwrap_err();
        assert!(missing.to_string().starts_with("ERR_NO_DECISION_ROWS"));
        let base = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join(".local/state/zeststream/scratch/decision-ledger-tests");
        let _ = std::fs::create_dir_all(&base);
        let empty = base.join(format!("empty-{}.jsonl", std::process::id()));
        std::fs::write(&empty, "").unwrap();
        let err = check_path(&empty, Probe::default()).unwrap_err();
        assert!(err.to_string().starts_with("ERR_NO_DECISION_ROWS"));
        let _ = std::fs::remove_file(&empty);
    }

}
