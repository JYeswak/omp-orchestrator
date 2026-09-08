#![forbid(unsafe_code)]

//! Dynamic probe contract for the inventory map.
//!
//! The historical name remains for the public module path, but the implementation no longer
//! compares live counts with transcribed absolutes. Each required surface is judged by
//! its own probe evidence: known, non-empty output is healthy; missing, unknown, or zero output
//! is an explicit mismatch.

use crate::{ProbeEvidence, ProbeState};
use serde_json::Value;
use std::fmt;

/// Subjects whose live evidence must be present before the map can claim `OK`.
pub const REQUIRED_PROBES: &[&str] = &[
    "omp_version",
    "omp_help_cli_commands",
    "omp_type_roots",
    "omp_type_declarations",
    "omp_rpc_handlers",
    "omp_rpc_slash_commands",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeMismatch {
    pub name: String,
    pub state: ProbeState,
    pub observed: Option<usize>,
}

/// Find missing, unknown, and empty required probe evidence.
#[must_use]
pub fn mismatches(probes: &[ProbeEvidence]) -> Vec<ProbeMismatch> {
    REQUIRED_PROBES
        .iter()
        .filter_map(|name| {
            let evidence = probes.iter().find(|probe| probe.name == *name);
            match evidence {
                Some(probe)
                    if probe.state == ProbeState::Known
                        && probe.observed.is_some_and(|count| count > 0) => None,
                Some(probe) => Some(ProbeMismatch {
                    name: (*name).to_owned(),
                    state: probe.state,
                    observed: probe.observed,
                }),
                None => Some(ProbeMismatch {
                    name: (*name).to_owned(),
                    state: ProbeState::Unknown,
                    observed: None,
                }),
            }
        })
        .collect()
}

#[must_use]
pub fn format_mismatches(rows: &[ProbeMismatch]) -> String {
    rows.iter()
        .map(|row| {
            format!(
                "COUNT_PROBE_UNMEASURED name={} state={:?} observed={:?}",
                row.name, row.state, row.observed
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TwinError {
    EmptyRequiredProbeList,
    EnvelopeEmpty,
    NoStatus,
    NoProbes,
    MalformedProbes,
    MismatchClaimedOk { name: String },
    UnknownWithoutNamedProbe,
    UnknownNamesMatchingProbe { name: String },
}

impl fmt::Display for TwinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequiredProbeList => formatter.write_str("COUNT_PROBE_EMPTY_REQUIRED_SET"),
            Self::EnvelopeEmpty => formatter.write_str("COUNT_PROBE_ENVELOPE_EMPTY"),
            Self::NoStatus => formatter.write_str("COUNT_PROBE_NO_STATUS"),
            Self::NoProbes => formatter.write_str("COUNT_PROBE_NO_EVIDENCE"),
            Self::MalformedProbes => formatter.write_str("COUNT_PROBE_MALFORMED_EVIDENCE"),
            Self::MismatchClaimedOk { name } => {
                write!(formatter, "COUNT_PROBE_MISMATCH_CLAIMED_OK name={name}")
            }
            Self::UnknownWithoutNamedProbe => {
                formatter.write_str("COUNT_PROBE_UNKNOWN_WITHOUT_NAMED_PROBE")
            }
            Self::UnknownNamesMatchingProbe { name } => {
                write!(formatter, "COUNT_PROBE_UNKNOWN_NAMES_MATCHING_PROBE name={name}")
            }
        }
    }
}

impl std::error::Error for TwinError {}

fn names_from_error(error: Option<&str>) -> Vec<String> {
    let Some(error) = error else {
        return Vec::new();
    };
    error
        .split(';')
        .filter_map(|part| part.split("name=").nth(1))
        .filter_map(|name| name.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

/// Validate the robot envelope's dynamic probe evidence. `guard=false` is the mutation leg.
pub fn evaluate_envelope_guarded(envelope: &Value, guard: bool) -> Result<(), TwinError> {
    if REQUIRED_PROBES.is_empty() {
        return Err(TwinError::EmptyRequiredProbeList);
    }
    if envelope.is_null() || envelope.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(TwinError::EnvelopeEmpty);
    }
    let status = envelope
        .get("status")
        .and_then(Value::as_str)
        .ok_or(TwinError::NoStatus)?;
    let probes_value = envelope
        .pointer("/data/probes")
        .or_else(|| envelope.get("probes"))
        .ok_or(TwinError::NoProbes)?;
    let probes: Vec<ProbeEvidence> =
        serde_json::from_value(probes_value.clone()).map_err(|_| TwinError::MalformedProbes)?;
    let rows = mismatches(&probes);
    if !guard {
        return Ok(());
    }
    let named = names_from_error(envelope.get("error").and_then(Value::as_str));
    if status == "OK" {
        return rows.first().map_or(Ok(()), |row| {
            Err(TwinError::MismatchClaimedOk {
                name: row.name.clone(),
            })
        });
    }
    if status == "UNKNOWN" {
        if rows.is_empty() || named.is_empty() {
            return Err(TwinError::UnknownWithoutNamedProbe);
        }
        if let Some(name) = named
            .iter()
            .find(|name| !rows.iter().any(|row| &row.name == *name))
        {
            return Err(TwinError::UnknownNamesMatchingProbe { name: name.clone() });
        }
        if let Some(row) = rows
            .iter()
            .find(|row| !named.iter().any(|name| name == &row.name))
        {
            return Err(TwinError::UnknownNamesMatchingProbe {
                name: row.name.clone(),
            });
        }
    }
    Ok(())
}

pub fn evaluate_envelope(envelope: &Value) -> Result<(), TwinError> {
    evaluate_envelope_guarded(envelope, true)
}
