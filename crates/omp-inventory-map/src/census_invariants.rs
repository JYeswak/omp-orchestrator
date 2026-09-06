//! Anti-vacuity check for inventory census invariants.
//!
//! A census that repeats one `must_be_true` / `negative_evidence` value across
//! more than one row is not a census. INV-2026-08-31 is the retained known-bad
//! (n=183, distinct=1). lwdo.2 raises the floor: every non-structural row
//! carries a contract about *that* surface, so distinct count equals row
//! count, and crate `what_it_provides` / `inputs` may not be cargo-metadata
//! scanner provenance.
//!
//! NO-CLAIM: uniqueness plus a named contract sentence does not prove the
//! sentence is true at runtime. Doctor wiring is the reachable trigger.


use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// Count of inventory row kinds the scanner emits today.
///
/// `cli_command`, `type_root`, `declaration`, `rpc_handler`, `slash_command`,
/// `omp_method`, `transport`, `workspace_crate`.
pub const ROW_KIND_COUNT: usize = 8;

/// Named kinds the live collector emits. A census whose distinct invariant
/// sets are fewer than this count is still vacuous even if not literally 1.
pub const ROW_KINDS: [&str; ROW_KIND_COUNT] = [
    "cli_command",
    "type_root",
    "declaration",
    "rpc_handler",
    "slash_command",
    "omp_method",
    "transport",
    "workspace_crate",
];

/// Opt-out for an invariant that is true by construction, not by measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VacuityMode {
    Structural,
}

impl VacuityMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
        }
    }
}

/// One census row as the checker sees it. Production rows and planted
/// artifacts both project into this view so the checker does not depend on
/// the full [`crate::InventoryRow`] construction site.
#[derive(Debug, Clone)]
pub struct CensusInvariantRow {
    pub id: String,
    pub kind: String,
    pub must_be_true: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub vacuity_mode: Option<VacuityMode>,
    pub vacuity_reason: Option<String>,
    pub what_it_provides: String,
    pub inputs: Vec<String>,
}

impl CensusInvariantRow {
    fn structural(&self) -> bool {
        matches!(self.vacuity_mode, Some(VacuityMode::Structural))
            && self
                .vacuity_reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty())
    }
}

/// Failures the census invariant gate can emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CensusInvariantError {
    EmptyCensus,
    BlankInvariant { id: String, field: &'static str },
    VacuousInvariantSet {
        field: &'static str,
        kind: String,
        n: usize,
        distinct: usize,
        repeated: String,
    },
    CrateInvariantOmitsIdentifier {
        id: String,
        crate_name: String,
    },
    StructuralVacuityMissingReason {
        id: String,
    },
    ScannerProvenance {
        id: String,
        field: &'static str,
        value: String,
    },
}

impl fmt::Display for CensusInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCensus => formatter.write_str("EMPTY_CENSUS inventory rows is empty"),
            Self::BlankInvariant { id, field } => write!(
                formatter,
                "EMPTY_INVARIANT id={id} field={field} requires a non-empty contract"
            ),
            Self::VacuousInvariantSet {
                field,
                kind,
                n,
                distinct,
                repeated,
            } => write!(
                formatter,
                "VACUOUS_INVARIANT_SET field={field} kind={kind} n={n} distinct={distinct} repeated={repeated}"
            ),
            Self::CrateInvariantOmitsIdentifier { id, crate_name } => write!(
                formatter,
                "CRATE_INVARIANT_OMITS_IDENTIFIER id={id} crate={crate_name}"
            ),
            Self::StructuralVacuityMissingReason { id } => {
                write!(
                    formatter,
                    "STRUCTURAL_VACUITY_MISSING_REASON id={id} vacuity_mode=structural requires a non-empty reason"
                )
            }
            Self::ScannerProvenance { id, field, value } => write!(
                formatter,
                "SCANNER_PROVENANCE id={id} field={field} value={value} — row contract must not be cargo-metadata provenance"
            ),
        }
    }
}

impl std::error::Error for CensusInvariantError {}

fn encode_set(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| values.join("\n"))
}

fn check_field(
    rows: &[CensusInvariantRow],
    field: &'static str,
    accessor: impl Fn(&CensusInvariantRow) -> &[String],
) -> Result<(), CensusInvariantError> {
    let mut by_kind: BTreeMap<&str, Vec<&CensusInvariantRow>> = BTreeMap::new();
    for row in rows {
        if row.structural() {
            continue;
        }
        by_kind.entry(row.kind.as_str()).or_default().push(row);
    }

    let measurable: Vec<&CensusInvariantRow> =
        rows.iter().filter(|row| !row.structural()).collect();
    let (global_n, global_distinct, global_sample) = uniqueness(&measurable, &accessor);
    if global_n > 1 && global_distinct < global_n {
        return Err(CensusInvariantError::VacuousInvariantSet {
            field,
            kind: "all".to_owned(),
            n: global_n,
            distinct: global_distinct,
            repeated: global_sample,
        });
    }

    for (kind, members) in by_kind {
        if members.len() <= 1 {
            continue;
        }
        let (n, distinct, sample) = uniqueness(&members, &accessor);
        if distinct < n {
            return Err(CensusInvariantError::VacuousInvariantSet {
                field,
                kind: kind.to_owned(),
                n,
                distinct,
                repeated: sample,
            });
        }
    }
    Ok(())
}

fn uniqueness(
    rows: &[&CensusInvariantRow],
    accessor: impl Fn(&CensusInvariantRow) -> &[String],
) -> (usize, usize, String) {
    let mut seen = BTreeSet::new();
    let mut repeated = String::new();
    for row in rows {
        let encoded = encode_set(accessor(row));
        if !seen.insert(encoded.clone()) && repeated.is_empty() {
            repeated = encoded;
        }
    }
    (rows.len(), seen.len(), repeated)
}

/// Scanner-provenance template lwdo.2 refuses on crate rows.
#[must_use]
pub fn scanner_provides_template(crate_name: &str) -> String {
    format!("Workspace crate {crate_name} from cargo metadata")
}

fn is_scanner_provides(value: &str) -> bool {
    value.contains(" from cargo metadata") || value.starts_with("Workspace crate ")
}

fn is_scanner_input(value: &str) -> bool {
    value.contains("cargo metadata")
}

/// Refuse an empty census, a blank per-row contract, and one-distinct-value invariant set.
///
/// Structural rows with a reason are accepted and ignored by the distinct
/// count. `vacuity_mode=structural` without a reason is an error, not a pass.
pub fn check_census_invariants(
    rows: &[CensusInvariantRow],
) -> Result<(), CensusInvariantError> {
    if rows.is_empty() {
        return Err(CensusInvariantError::EmptyCensus);
    }
    for row in rows {
        for (field, values) in [
            ("must_be_true", row.must_be_true.as_slice()),
            ("negative_evidence", row.negative_evidence.as_slice()),
        ] {
            if values.is_empty() || values.iter().all(|value| value.trim().is_empty()) {
                return Err(CensusInvariantError::BlankInvariant {
                    id: row.id.clone(),
                    field,
                });
            }
        }
        if matches!(row.vacuity_mode, Some(VacuityMode::Structural))
            && row
                .vacuity_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(CensusInvariantError::StructuralVacuityMissingReason {
                id: row.id.clone(),
            });
        }
        if row.kind == "workspace_crate" {
            let crate_name = row
                .id
                .strip_prefix("crate:")
                .unwrap_or(row.id.as_str());
            let joined = row.must_be_true.join("\n");
            if !joined.contains(crate_name) {
                return Err(CensusInvariantError::CrateInvariantOmitsIdentifier {
                    id: row.id.clone(),
                    crate_name: crate_name.to_owned(),
                });
            }
            if !row.what_it_provides.is_empty() && is_scanner_provides(&row.what_it_provides) {
                return Err(CensusInvariantError::ScannerProvenance {
                    id: row.id.clone(),
                    field: "what_it_provides",
                    value: row.what_it_provides.clone(),
                });
            }
            if let Some(input) = row.inputs.iter().find(|item| is_scanner_input(item)) {
                return Err(CensusInvariantError::ScannerProvenance {
                    id: row.id.clone(),
                    field: "inputs",
                    value: input.clone(),
                });
            }
        }
    }
    check_field(rows, "must_be_true", |row| &row.must_be_true)?;
    check_field(rows, "negative_evidence", |row| &row.negative_evidence)?;
    Ok(())
}

/// Per-row contracts. Identity is the surface name (cli command, crate, …).
/// A row whose invariant is reusable verbatim by another row has not written
/// an invariant.
pub fn invariants_for_kind(kind: &str, identity: &str) -> (Vec<String>, Vec<String>) {
    match kind {
        "cli_command" => (
            vec![format!("omp --help lists {identity}")],
            vec![format!(
                "a missing COMMANDS block is UNKNOWN, not a healthy zero for {identity}"
            )],
        ),
        "type_root" => (
            vec![format!(
                "installed dist/types contains directory {identity}"
            )],
            vec![format!(
                "type_root {identity} is not inferred by grepping this repository's Rust sources"
            )],
        ),
        "declaration" => (
            vec![format!(
                "installed dist/types lists declaration {identity}"
            )],
            vec![format!(
                "declaration {identity} is not counted from docs/plan prose"
            )],
        ),
        "rpc_handler" => (
            vec![format!(
                "installed cli.js dispatch handler has case {identity}"
            )],
            vec![format!(
                "rpc_handler {identity} is not the retired 81/17 method pair"
            )],
        ),
        "slash_command" => (
            vec![format!("omp --mode=rpc startup stream lists {identity}")],
            vec![format!(
                "slash_command {identity} is not treated as a discovered count"
            )],
        ),
        "omp_method" => (
            vec![format!("installed cli.js bundle contains {identity}")],
            vec![format!(
                "omp_method {identity} is not reconstructed from tmux pane scrapes"
            )],
        ),
        "transport" => (
            vec![format!(
                "omp --help documents --mode including {identity}"
            )],
            vec![format!(
                "a missing --mode probe is UNKNOWN, not an invented default for {identity}"
            )],
        ),
        "workspace_crate" => crate_row_contract(identity),
        other => (
            vec![format!(
                "{other}:{identity} carries a per-row contract, not the scanner provenance template"
            )],
            vec![format!(
                "{other}:{identity} does not reuse a shared NO_SOURCE_GREP negative-evidence string"
            )],
        ),
    }
}

fn crate_row_contract(name: &str) -> (Vec<String>, Vec<String>) {
    let must = match name {
        "ack-spine" => {
            "crate ack-spine exposes an ack detector reachable from finding-dispatch".to_owned()
        }
        "ack-stage" => {
            "crate ack-stage admits exactly one ACK comment form as delivery evidence".to_owned()
        }
        "finding-dispatch" => {
            "crate finding-dispatch is a reachable caller of the ack detector".to_owned()
        }
        "omp-inventory-map" => {
            "crate omp-inventory-map owns generation and the doctor inventory gate".to_owned()
        }
        other => format!(
            "crate {other} exposes a workspace surface named {other}, not a cargo-metadata provenance string"
        ),
    };
    (
        vec![must],
        vec![format!(
            "crate {name} is not inferred by grepping crate names in AGENTS.md"
        )],
    )
}
