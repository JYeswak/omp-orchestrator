#![forbid(unsafe_code)]

//! Count-twin invariant (06.16 / xm0n.7): for every observed/`expected_*` pair,
//! either the counts match or envelope `status` is `UNKNOWN` and the mismatching
//! pair is named. Closing slash_commands 0-vs-136 is out of scope.

use crate::InventoryCounts;
use serde_json::Value;
use std::fmt;

/// The seven twins pinned by `docs/plan/06-gates.md` §2.7 (f). Order is the
/// `InventoryCounts` field order. An empty list is a typed error, not a skip.
pub const COUNT_TWINS: &[(&str, &str)] = &[
    ("cli_commands", "expected_cli_commands"),
    ("type_roots", "expected_type_roots"),
    ("declarations", "expected_declarations"),
    ("rpc_handlers", "expected_rpc_handlers"),
    ("slash_commands", "expected_slash_commands"),
    ("omp_methods", "expected_omp_methods"),
    ("workspace_crates", "expected_workspace_crates"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwinMismatch {
    pub pair: String,
    pub observed: usize,
    pub expected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TwinError {
    EmptyPairList,
    EnvelopeEmpty,
    NoStatus,
    NoPairsFound,
    UnknownPairName(String),
    ExpectedWithoutObserved(String),
    MismatchClaimedOk {
        pair: String,
        observed: usize,
        expected: usize,
    },
    UnknownWithoutNamedPair,
    UnknownNamesMatchingPair {
        pair: String,
    },
}

impl fmt::Display for TwinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPairList => f.write_str("COUNT_TWIN_EMPTY_PAIR_LIST"),
            Self::EnvelopeEmpty => f.write_str("EnvelopeEmpty"),
            Self::NoStatus => f.write_str("COUNT_TWIN_NO_STATUS"),
            Self::NoPairsFound => f.write_str("NoPairsFound"),
            Self::UnknownPairName(name) => {
                write!(f, "COUNT_TWIN_UNKNOWN_PAIR pair={name}")
            }
            Self::ExpectedWithoutObserved(name) => {
                write!(f, "COUNT_TWIN_EXPECTED_WITHOUT_OBSERVED pair={name}")
            }
            Self::MismatchClaimedOk {
                pair,
                observed,
                expected,
            } => write!(
                f,
                "COUNT_TWIN_MISMATCH_CLAIMED_OK pair={pair} observed={observed} expected={expected}"
            ),
            Self::UnknownWithoutNamedPair => {
                f.write_str("COUNT_TWIN_UNKNOWN_WITHOUT_NAMED_PAIR")
            }
            Self::UnknownNamesMatchingPair { pair } => {
                write!(f, "COUNT_TWIN_UNKNOWN_NAMES_MATCHING_PAIR pair={pair}")
            }
        }
    }
}

pub fn mismatches(counts: &InventoryCounts) -> Vec<TwinMismatch> {
    COUNT_TWINS
        .iter()
        .filter_map(|(observed_name, _)| {
            let (observed, expected) = twin_values(counts, observed_name)?;
            (observed != expected).then_some(TwinMismatch {
                pair: (*observed_name).to_owned(),
                observed,
                expected,
            })
        })
        .collect()
}

pub fn format_mismatches(rows: &[TwinMismatch]) -> String {
    rows.iter()
        .map(|row| {
            format!(
                "COUNT_TWIN_MISMATCH pair={} observed={} expected={}",
                row.pair, row.observed, row.expected
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn twin_values(counts: &InventoryCounts, observed: &str) -> Option<(usize, usize)> {
    Some(match observed {
        "cli_commands" => (counts.cli_commands, counts.expected_cli_commands),
        "type_roots" => (counts.type_roots, counts.expected_type_roots),
        "declarations" => (counts.declarations, counts.expected_declarations),
        "rpc_handlers" => (counts.rpc_handlers, counts.expected_rpc_handlers),
        "slash_commands" => (counts.slash_commands, counts.expected_slash_commands),
        "omp_methods" => (counts.omp_methods, counts.expected_omp_methods),
        "workspace_crates" => (counts.workspace_crates, counts.expected_workspace_crates),
        _ => return None,
    })
}

fn named_pairs_from_error(error: Option<&str>) -> Vec<String> {
    let Some(error) = error else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut rest = error;
    while let Some(idx) = rest.find("pair=") {
        rest = &rest[idx + 5..];
        let name: String = rest
            .chars()
            .take_while(|ch| *ch == '_' || ch.is_ascii_alphabetic())
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

/// `guard` is the count-twin conditional. Tests invert it for the mutation leg.
pub fn evaluate_envelope_guarded(envelope: &Value, guard: bool) -> Result<(), TwinError> {
    if COUNT_TWINS.is_empty() {
        return Err(TwinError::EmptyPairList);
    }
    if envelope.is_null() || envelope.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(TwinError::EnvelopeEmpty);
    }
    let status = envelope
        .get("status")
        .and_then(Value::as_str)
        .ok_or(TwinError::NoStatus)?;
    let counts = envelope
        .pointer("/data/counts")
        .or_else(|| envelope.get("counts"))
        .ok_or(TwinError::NoPairsFound)?;
    let obj = counts.as_object().ok_or(TwinError::NoPairsFound)?;
    if obj.is_empty() {
        return Err(TwinError::NoPairsFound);
    }
    for key in obj.keys() {
        if let Some(rest) = key.strip_prefix("expected_") {
            if !COUNT_TWINS.iter().any(|(_, expected)| *expected == key) {
                return Err(TwinError::UnknownPairName(key.clone()));
            }
            if !obj.contains_key(rest) {
                return Err(TwinError::ExpectedWithoutObserved(rest.to_owned()));
            }
        }
    }
    for (observed, expected) in COUNT_TWINS {
        if obj.contains_key(*expected) && !obj.contains_key(*observed) {
            return Err(TwinError::ExpectedWithoutObserved((*observed).to_owned()));
        }
    }
    let parsed: InventoryCounts =
        serde_json::from_value(counts.clone()).map_err(|_| TwinError::NoPairsFound)?;
    let mismatch_rows = mismatches(&parsed);
    let named = named_pairs_from_error(envelope.get("error").and_then(Value::as_str));
    if !guard {
        return Ok(());
    }
    if status == "OK" {
        if let Some(row) = mismatch_rows.first() {
            return Err(TwinError::MismatchClaimedOk {
                pair: row.pair.clone(),
                observed: row.observed,
                expected: row.expected,
            });
        }
        return Ok(());
    }
    if status == "UNKNOWN" {
        if mismatch_rows.is_empty() {
            if let Some(name) = named.first() {
                return Err(TwinError::UnknownNamesMatchingPair {
                    pair: name.clone(),
                });
            }
            return Err(TwinError::UnknownWithoutNamedPair);
        }
        if named.is_empty() {
            return Err(TwinError::UnknownWithoutNamedPair);
        }
        for name in &named {
            if !mismatch_rows.iter().any(|row| &row.pair == name) {
                return Err(TwinError::UnknownNamesMatchingPair { pair: name.clone() });
            }
        }
        for row in &mismatch_rows {
            if !named.iter().any(|name| name == &row.pair) {
                return Err(TwinError::UnknownWithoutNamedPair);
            }
        }
        return Ok(());
    }
    Ok(())
}

pub fn evaluate_envelope(envelope: &Value) -> Result<(), TwinError> {
    evaluate_envelope_guarded(envelope, true)
}
