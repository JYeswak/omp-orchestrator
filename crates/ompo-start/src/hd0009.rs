//! Where `hd0009_status` gets its value, and who is allowed to say so.
//!
//! 812ax: the field advertised ledger provenance and was driven by an operator
//! flag, so it could lie in BOTH directions — the flag without a recorded
//! decision reported Ready, and a recorded decision without the flag reported
//! Blocked forever. A field with two possible sources and no stated precedence
//! IS the defect.
//!
//! # Precedence, stated once, here
//!
//! THE LEDGER IS AUTHORITATIVE. The flag is a DECLARED OVERRIDE: it can still
//! decide the step, but it must SAY that it is overriding, so a reader can
//! never mistake an operator assertion for a recorded decision.
//!
//! # Absent is not a value
//!
//! An absent or unreadable ledger is its own [`Authority`] variant, never a
//! silent "undecided". A missing ledger and a recorded non-decision cannot
//! share a representation, because they differ in exactly the case that
//! matters: whether anyone has looked. That is the absent-collapses-to-a-value
//! class this repo has paid for repeatedly (an absent age reading 0, an absent
//! source reading ALL FRESH, an empty set reading FULL).
//!
//! # NO-CLAIM
//!
//! This module resolves ONE row's decidedness. It does not claim the override
//! flag is wrongly named — one flag today drives both this predicate and the
//! spawn gate, which is tracked separately and deliberately untouched here.

use std::path::{Path, PathBuf};

/// The HD row this module answers for.
pub const HD_ID: &str = "HD-0009";

/// Where the resolved value came from. The whole point of the bead is that this
/// is REPORTED rather than inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authority {
    /// The ledger records an answered `HD-0009` row.
    LedgerDecided,
    /// The ledger is readable and records no ANSWERED `HD-0009` row. A merely
    /// ASKED question lands here, which is the trap below.
    LedgerUndecided,
    /// The operator asserted the decision. Names itself as an override so it is
    /// never read as provenance.
    FlagOverride,
    /// The ledger file is not present. A typed refusal, not a default.
    LedgerAbsent { path: String },
    /// The ledger is present and could not be read or parsed.
    LedgerUnreadable { path: String, detail: String },
}

impl Authority {
    /// The stable token a reader keys on.
    #[must_use]
    pub fn token(&self) -> &'static str {
        match self {
            Self::LedgerDecided => "ledger_decided",
            Self::LedgerUndecided => "ledger_undecided",
            Self::FlagOverride => "flag_override",
            Self::LedgerAbsent { .. } => "ledger_absent",
            Self::LedgerUnreadable { .. } => "ledger_unreadable",
        }
    }

    /// Is this authority a refusal rather than a reading? A refusal cannot
    /// decide the step, and it must not be mistaken for having decided it did
    /// not.
    #[must_use]
    pub fn is_refusal(&self) -> bool {
        matches!(self, Self::LedgerAbsent { .. } | Self::LedgerUnreadable { .. })
    }

    /// Human detail for the refusal variants; `None` when there is nothing to
    /// explain.
    #[must_use]
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::LedgerAbsent { path } => {
                Some(format!("HD0009_LEDGER_ABSENT path={path}"))
            }
            Self::LedgerUnreadable { path, detail } => {
                Some(format!("HD0009_LEDGER_UNREADABLE path={path} detail={detail}"))
            }
            _ => None,
        }
    }
}

/// A resolved decidedness plus the authority that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// Does the step count as decided?
    pub decided: bool,
    pub authority: Authority,
}

/// The ledger path this module reads, relative to a repo root.
#[must_use]
pub fn ledger_path(repo: &Path) -> PathBuf {
    repo.join("docs").join("decisions.jsonl")
}

/// Is this row an ANSWER to `HD-0009`?
///
/// ⛔ THE TRAP, and it is why row presence is the wrong predicate: the request
/// row carries `id = "HD-0009"` with an EMPTY `decision`, and the ANSWER is a
/// SEPARATE row keyed by `answers = "HD-0009"` with no `id` at all. So
/// `grep -c HD-0009` counts a question PLUS its answer, and resolving on
/// presence would report DECIDED for a question nobody has answered.
fn answers_hd0009(row: &decision_ledger::Row) -> bool {
    let names_it = row.id() == Some(HD_ID)
        || row.value.get("answers").and_then(serde_json::Value::as_str) == Some(HD_ID);
    names_it && row.is_answered()
}

/// Resolve from the ledger alone, with no override applied.
///
/// An absent file is a REFUSAL here, deliberately stricter than
/// `decision_ledger::read_rows`, which treats absence as an empty ledger. That
/// is the right call for a writer counting rows and the wrong one for a field
/// claiming provenance: "nobody has recorded anything" and "there is nothing to
/// read" are different facts, and only one of them is a reading.
#[must_use]
pub fn from_ledger(repo: &Path) -> Resolution {
    let path = ledger_path(repo);
    if !path.exists() {
        return Resolution {
            decided: false,
            authority: Authority::LedgerAbsent {
                path: path.display().to_string(),
            },
        };
    }
    match decision_ledger::read_rows(&path) {
        Ok(rows) => {
            let decided = rows.iter().any(answers_hd0009);
            Resolution {
                decided,
                authority: if decided {
                    Authority::LedgerDecided
                } else {
                    Authority::LedgerUndecided
                },
            }
        }
        Err(error) => Resolution {
            decided: false,
            authority: Authority::LedgerUnreadable {
                path: path.display().to_string(),
                detail: error.to_string(),
            },
        },
    }
}

/// Resolve with the operator override layered on top.
///
/// The override can only ever ADD a decision, and when it does it reports
/// itself as [`Authority::FlagOverride`] rather than borrowing the ledger's
/// authority. A ledger that already decided is NOT relabelled by the flag:
/// provenance beats an assertion that agrees with it.
#[must_use]
pub fn resolve(repo: &Path, override_flag: bool) -> Resolution {
    let ledger = from_ledger(repo);
    if ledger.decided || !override_flag {
        return ledger;
    }
    Resolution {
        decided: true,
        authority: Authority::FlagOverride,
    }
}
