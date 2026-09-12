//! Mutation minimality: the discriminator is the reddened leg SET, not the spelling.
//!
//! 6we9q raised the packet floor to `file.rs:N`. That still cannot see a named
//! site that disables a SUPERSET of the property (dwj4v: `validate_workflows`
//! at a precise line also dropped serde's parse). This module compares the
//! property's DECLARED legs to the legs the mutation actually reddened.
//!
//! Three Ok verdicts, three remedies:
//! - [`MutationMinimality::Minimal`] — sets equal → MUTATION-VERIFIED is admissible
//! - [`MutationMinimality::Superset`] — reddened ⊃ declared → the mutation is wrong
//! - [`MutationMinimality::Inert`] — reddened empty, declared nonempty → subject not load-bearing
//!
//! BAR ATTACK (15rl7): the acceptance names no verdict for SUBSET (reddened ⊂
//! declared) or INCOMPARABLE (neither subset). Collapsing those to MINIMAL or
//! SUPERSET would launder an incomplete mutation. They are errors, not a fourth
//! Ok. Empty DECLARED is an error even when reddened is also empty — `{ } == { }`
//! is not MINIMAL.

use std::collections::BTreeSet;
use std::fmt;

/// Poumg known-good: exactly `live_wrapper` reddened.
pub const POUMG_LIVE_WRAPPER: &str = "live_wrapper_reddens_on_broken_row_repairs_clean";

/// dwj4v property leg (the route). Coarse mutation also reddened parse + scanner.
pub const DWJ4V_ROUTE_LEG: &str = "validate_workflows_route";
pub const DWJ4V_SERDE_PARSE: &str = "serde_duplicate_block_mapping";
pub const DWJ4V_BLOCK_SCANNER: &str = "block_scanner_downstream";

/// Result of comparing declared vs reddened leg sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationMinimality {
    Minimal,
    Superset,
    Inert,
}

impl MutationMinimality {
    /// Stable token a mutation report prints.
    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "MINIMAL",
            Self::Superset => "SUPERSET",
            Self::Inert => "INERT",
        }
    }
}

/// Shapes the three Ok verdicts do not name. Fail closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationMinimalityError {
    EmptyDeclared,
    Subset {
        missing: BTreeSet<String>,
    },
    Incomparable {
        extra: BTreeSet<String>,
        missing: BTreeSet<String>,
    },
}

impl fmt::Display for MutationMinimalityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDeclared => write!(
                formatter,
                "EMPTY_DECLARED reason=declared_leg_set_must_be_nonempty"
            ),
            Self::Subset { missing } => write!(
                formatter,
                "SUBSET missing={missing:?} reason=reddened_is_proper_subset_of_declared"
            ),
            Self::Incomparable { extra, missing } => write!(
                formatter,
                "INCOMPARABLE extra={extra:?} missing={missing:?} reason=sets_are_not_nested"
            ),
        }
    }
}

fn set_of(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Poumg declared = reddened = {live_wrapper}.
pub fn poumg_declared() -> BTreeSet<String> {
    set_of(&[POUMG_LIVE_WRAPPER])
}

/// Poumg reddened set (identical).
pub fn poumg_reddened() -> BTreeSet<String> {
    poumg_declared()
}

/// dwj4v declared: the workflow-route property only.
pub fn dwj4v_declared() -> BTreeSet<String> {
    set_of(&[DWJ4V_ROUTE_LEG])
}

/// dwj4v coarse reddened: route + serde parse + block scanner.
pub fn dwj4v_coarse_reddened() -> BTreeSet<String> {
    set_of(&[DWJ4V_ROUTE_LEG, DWJ4V_SERDE_PARSE, DWJ4V_BLOCK_SCANNER])
}

/// Compare declared vs reddened. Empty declared is an error, never MINIMAL.
pub fn classify_mutation_minimality(
    declared: &BTreeSet<String>,
    reddened: &BTreeSet<String>,
) -> Result<MutationMinimality, MutationMinimalityError> {
    if declared.is_empty() {
        return Err(MutationMinimalityError::EmptyDeclared);
    }
    if reddened.is_empty() {
        return Ok(MutationMinimality::Inert);
    }
    if reddened == declared {
        return Ok(MutationMinimality::Minimal);
    }
    if declared.is_subset(reddened) {
        return Ok(MutationMinimality::Superset);
    }
    if reddened.is_subset(declared) {
        return Err(MutationMinimalityError::Subset {
            missing: declared.difference(reddened).cloned().collect(),
        });
    }
    Err(MutationMinimalityError::Incomparable {
        extra: reddened.difference(declared).cloned().collect(),
        missing: declared.difference(reddened).cloned().collect(),
    })
}

/// Production instruction, and the single call site that keeps classify wired.
///
/// Exercises poumg MINIMAL, dwj4v SUPERSET, empty-declared ERROR, and nonempty
/// declared + empty reddened INERT. If any fixture regresses, grading packets
/// cannot render.
pub fn grading_instruction() -> String {
    let poumg = classify_mutation_minimality(&poumg_declared(), &poumg_reddened())
        .expect("poumg live_wrapper is MINIMAL");
    assert_eq!(poumg, MutationMinimality::Minimal);
    let dwj4v = classify_mutation_minimality(&dwj4v_declared(), &dwj4v_coarse_reddened())
        .expect("dwj4v coarse is SUPERSET");
    assert_eq!(dwj4v, MutationMinimality::Superset);
    let empty = classify_mutation_minimality(&BTreeSet::new(), &BTreeSet::new());
    assert!(
        matches!(empty, Err(MutationMinimalityError::EmptyDeclared)),
        "empty declared must not be MINIMAL: {empty:?}"
    );
    let inert = classify_mutation_minimality(&poumg_declared(), &BTreeSet::new())
        .expect("nonempty declared + empty reddened is INERT");
    assert_eq!(inert, MutationMinimality::Inert);
    format!(
        "\nMUTATION MINIMALITY (fhsyv; call site mutation_minimality::classify_mutation_minimality):\n\
         Report DECLARED leg set and REDDENED leg set from the mutation run.\n\
         {} = sets equal; only this supports MUTATION-VERIFIED.\n\
         {} = reddened strictly contains declared; the mutation is wrong.\n\
         {} = reddened empty; the subject is not load-bearing.\n\
         Empty DECLARED is an ERROR, not a trivially-equal match.\n",
        MutationMinimality::Minimal.label(),
        MutationMinimality::Superset.label(),
        MutationMinimality::Inert.label(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poumg_live_wrapper_is_minimal() {
        let verdict =
            classify_mutation_minimality(&poumg_declared(), &poumg_reddened()).expect("poumg");
        assert_eq!(verdict, MutationMinimality::Minimal);
        assert_eq!(verdict.label(), "MINIMAL");
    }

    #[test]
    fn dwj4v_coarse_named_site_is_superset() {
        let verdict = classify_mutation_minimality(&dwj4v_declared(), &dwj4v_coarse_reddened())
            .expect("dwj4v");
        assert_eq!(verdict, MutationMinimality::Superset);
        assert_eq!(verdict.label(), "SUPERSET");
        assert_ne!(verdict, MutationMinimality::Minimal);
    }

    #[test]
    fn empty_declared_is_error_even_when_reddened_is_empty() {
        let error = classify_mutation_minimality(&BTreeSet::new(), &BTreeSet::new())
            .expect_err("empty declared");
        assert_eq!(error, MutationMinimalityError::EmptyDeclared);
        assert!(error.to_string().contains("EMPTY_DECLARED"));
    }

    #[test]
    fn empty_declared_with_reddened_legs_is_still_error() {
        let error = classify_mutation_minimality(&BTreeSet::new(), &poumg_declared())
            .expect_err("empty declared wins");
        assert_eq!(error, MutationMinimalityError::EmptyDeclared);
    }

    #[test]
    fn nonempty_declared_empty_reddened_is_inert() {
        let verdict =
            classify_mutation_minimality(&poumg_declared(), &BTreeSet::new()).expect("inert");
        assert_eq!(verdict, MutationMinimality::Inert);
        assert_eq!(verdict.label(), "INERT");
    }

    #[test]
    fn subset_is_error_not_minimal() {
        let declared = set_of(&[POUMG_LIVE_WRAPPER, DWJ4V_ROUTE_LEG]);
        let reddened = set_of(&[POUMG_LIVE_WRAPPER]);
        let error = classify_mutation_minimality(&declared, &reddened).expect_err("subset");
        assert!(matches!(error, MutationMinimalityError::Subset { .. }));
    }

    #[test]
    fn incomparable_is_error_not_superset() {
        let declared = set_of(&[POUMG_LIVE_WRAPPER]);
        let reddened = set_of(&[DWJ4V_ROUTE_LEG]);
        let error = classify_mutation_minimality(&declared, &reddened).expect_err("incomparable");
        assert!(matches!(
            error,
            MutationMinimalityError::Incomparable { .. }
        ));
    }

    #[test]
    fn three_ok_labels_are_distinct() {
        let labels = [
            MutationMinimality::Minimal.label(),
            MutationMinimality::Superset.label(),
            MutationMinimality::Inert.label(),
        ];
        let unique: BTreeSet<_> = labels.into_iter().collect();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn grading_instruction_runs_the_fixtures() {
        let text = grading_instruction();
        assert!(text.contains("MINIMAL"));
        assert!(text.contains("SUPERSET"));
        assert!(text.contains("INERT"));
        assert!(text.contains("classify_mutation_minimality"));
        assert!(text.contains("Empty DECLARED"));
    }
}
