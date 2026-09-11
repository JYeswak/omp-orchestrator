#![forbid(unsafe_code)]

//! L5 portal-row CONTRACT predicates: pure, total, and side-effect free.
//!
//! The portal row is already WRITTEN -- `ompo portal --json` emits `.data.sources`,
//! `.data._alerts` and `.data.one_next_action` today. What did not exist is anything
//! that REFUSES a malformed one. A field that is merely present is a decoration; a
//! field with a predicate behind it is a contract. This module is the predicate half.
//!
//! Three laws, one per bead, and each is written so that the DEGENERATE
//! implementation fails it:
//!
//! * [`validate_sources`] (van0) -- an ABSENT source must not read as available.
//!   The population is checked against an EXPECTED SET, because a source that is
//!   simply missing from the map is otherwise indistinguishable from a healthy one:
//!   nothing iterates it, so nothing complains. An empty map is an ERROR, never a pass.
//!   And `reason_code` must be non-empty whenever `available` is false, or the field
//!   is decorative exactly when it is needed.
//! * [`alerts_are_complete`] (cqwo) -- every alert carries severity, summary AND a
//!   non-empty action. An alert with no action is the same defect class as a refusal
//!   with no remedy. The known-good is an EMPTY array on a healthy system, so the law
//!   is cross-checked against the sources: a degraded source with no alert is
//!   `SilentDegradation`. Without that clause an emitter hard-wired to `[]` passes.
//! * [`one_next_action_is_object`] (qcev) -- EXACTLY ONE action, carried as an object.
//!   Cardinality is checked in all three directions -- zero refuses, two refuses, one
//!   passes -- and the singleton ARRAY refuses too, because "a list is a protocol bug"
//!   is a statement about the CARRIER, not only about the count. A predicate that only
//!   asks "is there at least one" satisfies the title and not the contract.
//!
//! The source names are MEASURED, not assumed. `ompo portal --json | jq '.data.sources | keys'`
//! returns `["agent-mail","ntm","tick-monitor"]`; the bead text's "ntm,tick,mail" is
//! shorthand for those three, not their literal keys.

use serde_json::Value;

/// The source population a portal row must account for, as measured from the live
/// emitter. Set EQUALITY, not containment: a new source must be added here
/// deliberately, and a vanished one must refuse rather than quietly shrink the scan.
pub const EXPECTED_SOURCES: [&str; 3] = ["agent-mail", "ntm", "tick-monitor"];

/// Ways `.data.sources` can fail the L5 contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceDefect {
    /// The whole field is not a JSON object.
    NotAnObject,
    /// Zero sources. An empty scan set is an ERROR, never a pass.
    EmptyScan,
    /// An expected source is absent. Absence must not read as health.
    MissingSource { name: &'static str },
    /// A source nobody expects appeared; the population drifted unannounced.
    UnknownSource { name: String },
    /// A row is not an object.
    RowNotAnObject { source: String },
    /// A required key is absent from a row.
    MissingField { source: String, field: &'static str },
    /// `available` or `fresh` is present but not a boolean.
    NotABool { source: String, field: &'static str },
    /// `reason_code` is present but not a string.
    ReasonNotAString { source: String },
    /// `available` is false and `reason_code` is empty. The one case the field
    /// exists for is the one case it was not filled in.
    UnavailableWithoutReason { source: String },
    /// A source claims freshness while reporting itself unavailable. Missing must
    /// never collapse into healthy, in either direction.
    UnavailableButFresh { source: String },
}

impl std::fmt::Display for SourceDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnObject => write!(f, "L5_SOURCES_NOT_AN_OBJECT"),
            Self::EmptyScan => write!(
                f,
                "L5_SOURCES_EMPTY_SCAN — zero sources is an ERROR, never a pass"
            ),
            Self::MissingSource { name } => write!(
                f,
                "L5_SOURCE_ABSENT source={name} — an absent source must not read as available"
            ),
            Self::UnknownSource { name } => write!(f, "L5_SOURCE_UNEXPECTED source={name}"),
            Self::RowNotAnObject { source } => write!(f, "L5_SOURCE_ROW_NOT_AN_OBJECT source={source}"),
            Self::MissingField { source, field } => {
                write!(f, "L5_SOURCE_MISSING_FIELD source={source} field={field}")
            }
            Self::NotABool { source, field } => {
                write!(f, "L5_SOURCE_FIELD_NOT_BOOL source={source} field={field}")
            }
            Self::ReasonNotAString { source } => {
                write!(f, "L5_SOURCE_REASON_NOT_A_STRING source={source}")
            }
            Self::UnavailableWithoutReason { source } => write!(
                f,
                "L5_SOURCE_UNAVAILABLE_WITHOUT_REASON source={source} — reason_code must be populated whenever available is false"
            ),
            Self::UnavailableButFresh { source } => write!(
                f,
                "L5_SOURCE_UNAVAILABLE_BUT_FRESH source={source} — an unavailable source cannot be fresh"
            ),
        }
    }
}

/// True when a row is fully healthy, by the SAME condition the portal emitter uses to
/// decide whether to raise an alert: available AND fresh AND carrying an age.
/// Defined once so the alert cross-check cannot drift from the sources check.
#[must_use]
pub fn source_is_healthy(row: &Value) -> bool {
    row.get("available").and_then(Value::as_bool) == Some(true)
        && row.get("fresh").and_then(Value::as_bool) == Some(true)
        && row.get("age_ms").is_some_and(|age| !age.is_null())
}

/// Names of the sources that are NOT fully healthy, in sorted order.
#[must_use]
pub fn degraded_sources(sources: &Value) -> Vec<String> {
    let Some(map) = sources.as_object() else {
        return Vec::new();
    };
    let mut names: Vec<String> = map
        .iter()
        .filter(|(_, row)| !source_is_healthy(row))
        .map(|(name, _)| name.clone())
        .collect();
    names.sort();
    names
}

/// van0: per-source available/fresh/reason_code, with absence treated as a refusal.
///
/// # Errors
/// Returns the first [`SourceDefect`] found, in a deterministic order: shape, then
/// population, then per-row fields walked in key order.
pub fn validate_sources(sources: &Value) -> Result<(), SourceDefect> {
    let map = sources.as_object().ok_or(SourceDefect::NotAnObject)?;
    if map.is_empty() {
        return Err(SourceDefect::EmptyScan);
    }
    for expected in EXPECTED_SOURCES {
        if !map.contains_key(expected) {
            return Err(SourceDefect::MissingSource { name: expected });
        }
    }
    for name in map.keys() {
        if !EXPECTED_SOURCES.contains(&name.as_str()) {
            return Err(SourceDefect::UnknownSource { name: name.clone() });
        }
    }
    for (name, row) in map {
        let row = row.as_object().ok_or_else(|| SourceDefect::RowNotAnObject {
            source: name.clone(),
        })?;
        let flag = |field: &'static str| -> Result<bool, SourceDefect> {
            match row.get(field) {
                None => Err(SourceDefect::MissingField {
                    source: name.clone(),
                    field,
                }),
                Some(Value::Bool(value)) => Ok(*value),
                Some(_) => Err(SourceDefect::NotABool {
                    source: name.clone(),
                    field,
                }),
            }
        };
        let available = flag("available")?;
        let fresh = flag("fresh")?;
        let reason = match row.get("reason_code") {
            None => {
                return Err(SourceDefect::MissingField {
                    source: name.clone(),
                    field: "reason_code",
                })
            }
            Some(Value::String(reason)) => reason.as_str(),
            Some(_) => {
                return Err(SourceDefect::ReasonNotAString {
                    source: name.clone(),
                })
            }
        };
        if row.get("age_ms").is_none() {
            return Err(SourceDefect::MissingField {
                source: name.clone(),
                field: "age_ms",
            });
        }
        if !available && reason.trim().is_empty() {
            return Err(SourceDefect::UnavailableWithoutReason {
                source: name.clone(),
            });
        }
        if !available && fresh {
            return Err(SourceDefect::UnavailableButFresh {
                source: name.clone(),
            });
        }
    }
    Ok(())
}

/// Ways `.data._alerts` can fail the L5 contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertDefect {
    /// The field is not a JSON array.
    NotAnArray,
    /// An entry is not an object.
    NotAnObject { index: usize },
    /// A required key is absent.
    MissingField { index: usize, field: &'static str },
    /// A required key is present but not a string.
    NotAString { index: usize, field: &'static str },
    /// A required key is a string but carries nothing. THE cqwo defect: an
    /// alert with an empty `action` announces a problem and withholds the
    /// remedy, which is the same defect class as a refusal with no remediation.
    Empty { index: usize, field: &'static str },
    /// `severity` is outside the emitted vocabulary.
    UnknownSeverity { index: usize, severity: String },
    /// A source is degraded and NO alert names it. Without this clause an
    /// emitter hard-wired to `[]` satisfies every per-entry rule above by
    /// having no entries to violate them.
    SilentDegradation { source: String },
}

/// The severities the portal emitter actually produces: `error` when a source
/// is unavailable, `warn` when it is available but not fresh.
pub const ALERT_SEVERITIES: [&str; 2] = ["error", "warn"];

/// The keys every alert must carry. `action` is last because it is the one the
/// bead is about, and the walk reports the first missing key in this order.
pub const ALERT_FIELDS: [&str; 3] = ["severity", "summary", "action"];

impl std::fmt::Display for AlertDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnArray => write!(f, "L5_ALERTS_NOT_AN_ARRAY"),
            Self::NotAnObject { index } => write!(f, "L5_ALERT_NOT_AN_OBJECT index={index}"),
            Self::MissingField { index, field } => {
                write!(f, "L5_ALERT_MISSING_FIELD index={index} field={field}")
            }
            Self::NotAString { index, field } => {
                write!(f, "L5_ALERT_FIELD_NOT_A_STRING index={index} field={field}")
            }
            Self::Empty { index, field } => write!(
                f,
                "L5_ALERT_EMPTY_FIELD index={index} field={field} — an alert with an empty {field} is incomplete"
            ),
            Self::UnknownSeverity { index, severity } => {
                write!(f, "L5_ALERT_UNKNOWN_SEVERITY index={index} severity={severity}")
            }
            Self::SilentDegradation { source } => write!(
                f,
                "L5_ALERT_SILENT_DEGRADATION source={source} — a degraded source with no alert is a silent failure"
            ),
        }
    }
}

/// cqwo: every alert is a complete `severity` + `summary` + `action` triple,
/// and every degraded source is actually alerted on.
///
/// The second half is what makes the first half load-bearing. Checked alone,
/// the per-entry rules are vacuously satisfied by an emitter that never emits;
/// checked against `sources`, an unalerted degradation is
/// [`AlertDefect::SilentDegradation`]. `source_is_healthy` is the SAME
/// predicate the emitter uses to decide whether to raise an alert, so the
/// cross-check cannot drift from the emit condition.
///
/// A degraded source is matched by NAME APPEARING IN `summary`, which is the
/// only linkage the emitted triple carries: the shipped summary is
/// `"{name} source is not fresh and available"`.
///
/// # Errors
/// Returns the first [`AlertDefect`]: shape, then per-entry fields in array
/// order, then unalerted degradations in sorted source order.
pub fn alerts_are_complete(alerts: &Value, sources: &Value) -> Result<(), AlertDefect> {
    let entries = alerts.as_array().ok_or(AlertDefect::NotAnArray)?;
    for (index, alert) in entries.iter().enumerate() {
        let row = alert
            .as_object()
            .ok_or(AlertDefect::NotAnObject { index })?;
        for field in ALERT_FIELDS {
            match row.get(field) {
                None => return Err(AlertDefect::MissingField { index, field }),
                Some(Value::String(text)) if text.trim().is_empty() => {
                    return Err(AlertDefect::Empty { index, field })
                }
                Some(Value::String(_)) => {}
                Some(_) => return Err(AlertDefect::NotAString { index, field }),
            }
        }
        let severity = row["severity"].as_str().unwrap_or_default();
        if !ALERT_SEVERITIES.contains(&severity) {
            return Err(AlertDefect::UnknownSeverity {
                index,
                severity: severity.to_owned(),
            });
        }
    }
    for source in degraded_sources(sources) {
        let alerted = entries.iter().any(|alert| {
            alert
                .get("summary")
                .and_then(Value::as_str)
                .is_some_and(|summary| summary.contains(&source))
        });
        if !alerted {
            return Err(AlertDefect::SilentDegradation { source });
        }
    }
    Ok(())
}

/// Ways `.data.one_next_action` can fail the L5 contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextActionDefect {
    /// ZERO actions: the field is absent or null. A cursor with nowhere to go
    /// must say HUMAN HALT explicitly, never fall silent.
    Absent,
    /// The carrier is a LIST. Reported with its length so the zero-, one- and
    /// two-element cases are distinguishable in the message, but all three are
    /// the same refusal: "a list is a protocol bug" is a statement about the
    /// CARRIER, not only about the count. A predicate that merely counted
    /// would accept the singleton array.
    NotAnObject { len: usize },
    /// The field is neither object, array, nor null — a scalar cursor.
    NotAnObjectScalar,
    /// A required key is absent from the action object.
    MissingField { field: &'static str },
    /// A required key is present but not a string.
    NotAString { field: &'static str },
    /// A required key is an empty string.
    Empty { field: &'static str },
}

/// The keys an actionable cursor must carry.
pub const NEXT_ACTION_FIELDS: [&str; 2] = ["command", "reason_code"];

/// The reason_code that carries a HUMAN HALT instead of a machine command.
/// A halt is still ONE object with a populated `command`; the command is the
/// halt itself, so the shape never degrades into an absent cursor.
pub const HUMAN_HALT: &str = "HUMAN_HALT";

impl std::fmt::Display for NextActionDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => write!(
                f,
                "L5_ONE_NEXT_ABSENT — zero actions; a cursor with nowhere to go must say HUMAN_HALT"
            ),
            Self::NotAnObject { len } => write!(
                f,
                "L5_ONE_NEXT_IS_A_LIST len={len} — one_next_action is an object, never an array"
            ),
            Self::NotAnObjectScalar => write!(f, "L5_ONE_NEXT_NOT_AN_OBJECT"),
            Self::MissingField { field } => write!(f, "L5_ONE_NEXT_MISSING_FIELD field={field}"),
            Self::NotAString { field } => write!(f, "L5_ONE_NEXT_FIELD_NOT_A_STRING field={field}"),
            Self::Empty { field } => write!(f, "L5_ONE_NEXT_EMPTY_FIELD field={field}"),
        }
    }
}

/// qcev: EXACTLY ONE `{command, reason_code}`, carried as an object.
///
/// Cardinality is refused in all three directions — zero
/// ([`NextActionDefect::Absent`]), two ([`NextActionDefect::NotAnObject`]) and
/// the singleton array, which is the case a count-only predicate lets through.
/// One object passes.
///
/// # Errors
/// Returns the [`NextActionDefect`] describing the first failure: carrier
/// shape first, then the required keys in [`NEXT_ACTION_FIELDS`] order.
pub fn validate_one_next_action(action: &Value) -> Result<(), NextActionDefect> {
    let row = match action {
        Value::Null => return Err(NextActionDefect::Absent),
        Value::Array(items) => return Err(NextActionDefect::NotAnObject { len: items.len() }),
        Value::Object(row) => row,
        _ => return Err(NextActionDefect::NotAnObjectScalar),
    };
    for field in NEXT_ACTION_FIELDS {
        match row.get(field) {
            None => return Err(NextActionDefect::MissingField { field }),
            Some(Value::String(text)) if text.trim().is_empty() => {
                return Err(NextActionDefect::Empty { field })
            }
            Some(Value::String(_)) => {}
            Some(_) => return Err(NextActionDefect::NotAString { field }),
        }
    }
    Ok(())
}

