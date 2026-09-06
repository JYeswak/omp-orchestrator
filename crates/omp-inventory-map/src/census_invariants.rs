//! Anti-vacuity check for inventory census invariants.
//!
//! A census that repeats one `must_be_true` / `negative_evidence` value across
//! more than one row is not a census: the four-field discipline passed
//! syntactically and said nothing about the subject. INV-2026-08-31 is the
//! retained known-bad (n=183, distinct=1).
//!
//! Partition is by `kind`. A row may declare `vacuity_mode=structural` with a
//! non-empty reason; those rows are excluded from the distinct-count and are
//! carried through the envelope unchanged.
//!
//! NO-CLAIM: this refuses identical invariant *sets*. It does not prove a
//! remaining distinct string is a true contract of the surface (lwdo.2).

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
}

impl fmt::Display for CensusInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCensus => formatter.write_str("EMPTY_CENSUS inventory rows is empty"),
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

    let mut global: BTreeSet<String> = BTreeSet::new();
    let mut global_n = 0usize;
    let mut global_sample = String::new();
    for row in rows.iter().filter(|row| !row.structural()) {
        let encoded = encode_set(accessor(row));
        if global_n == 0 {
            global_sample = encoded.clone();
        }
        global.insert(encoded);
        global_n += 1;
    }
    if global_n > 1 && global.len() == 1 {
        return Err(CensusInvariantError::VacuousInvariantSet {
            field,
            kind: "all".to_owned(),
            n: global_n,
            distinct: 1,
            repeated: global_sample,
        });
    }

    for (kind, members) in by_kind {
        if members.len() <= 1 {
            continue;
        }
        let mut distinct = BTreeSet::new();
        let mut sample = String::new();
        for row in &members {
            let encoded = encode_set(accessor(row));
            if distinct.is_empty() {
                sample = encoded.clone();
            }
            distinct.insert(encoded);
        }
        if distinct.len() == 1 {
            return Err(CensusInvariantError::VacuousInvariantSet {
                field,
                kind: kind.to_owned(),
                n: members.len(),
                distinct: 1,
                repeated: sample,
            });
        }
    }
    Ok(())
}

/// Refuse an empty census and a one-distinct-value invariant set.
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
        }
    }
    check_field(rows, "must_be_true", |row| &row.must_be_true)?;
    check_field(rows, "negative_evidence", |row| &row.negative_evidence)?;
    Ok(())
}

/// Kind-specific invariant templates. Crate rows embed the crate identifier;
/// every other kind shares one template so a live scan still trips
/// `VACUOUS_INVARIANT_SET` on the repeated kind-level value until lwdo.2
/// rewrites per-row contracts.
pub fn invariants_for_kind(kind: &str, identity: &str) -> (Vec<String>, Vec<String>) {
    match kind {
        "cli_command" => (
            vec!["cli_command rows are enumerated from the omp --help COMMANDS block".to_owned()],
            vec![
                "a missing COMMANDS block is UNKNOWN, not a healthy zero-command census".to_owned(),
            ],
        ),
        "type_root" => (
            vec!["type_root rows are directories under the installed dist/types tree".to_owned()],
            vec!["type_root rows are not inferred by grepping this repository's Rust sources".to_owned()],
        ),
        "declaration" => (
            vec!["declaration rows are top-level .d.ts files beside dist/types".to_owned()],
            vec!["declaration rows are not counted from docs/plan prose".to_owned()],
        ),
        "rpc_handler" => (
            vec!["rpc_handler rows are case labels in the installed cli.js dispatch handler".to_owned()],
            vec!["rpc_handler rows are not the retired 81/17 method pair".to_owned()],
        ),
        "slash_command" => (
            vec!["slash_command rows come from the omp --mode=rpc startup stream".to_owned()],
            vec!["slash_command expected_slash_commands is not treated as a discovered count".to_owned()],
        ),
        "omp_method" => (
            vec!["omp_method rows are omp/* strings in the installed cli.js bundle".to_owned()],
            vec!["omp_method rows are not reconstructed from tmux pane scrapes".to_owned()],
        ),
        "transport" => (
            vec!["transport rows are the documented --mode=<text|json|rpc|rpc-ui> flag".to_owned()],
            vec!["a missing --mode probe is UNKNOWN, not an invented default transport".to_owned()],
        ),
        "workspace_crate" => (
            vec![format!(
                "workspace crate {identity} is listed by cargo metadata --format-version 1 --no-deps"
            )],
            vec![format!(
                "workspace crate {identity} is not inferred by grepping crate names in AGENTS.md"
            )],
        ),
        other => (
            vec![format!(
                "{other} rows carry a kind-specific must_be_true, not the scanner provenance template"
            )],
            vec![format!(
                "{other} rows do not reuse the shared NO_SOURCE_GREP negative-evidence string"
            )],
        ),
    }
}
