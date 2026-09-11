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
        let mut flag = |field: &'static str| -> Result<bool, SourceDefect> {
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

