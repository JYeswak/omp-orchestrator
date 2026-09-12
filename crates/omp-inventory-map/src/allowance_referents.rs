//! Typed dies_when referents, resolved against derived data (bead
//! omp-orchestrator-poumg.3).
//!
//! A dies_when is a CLAIM ABOUT THE CODE, and nothing was checking it: rows
//! carried crate and trait names that do not exist while reading as MORE
//! rigorous than a bare row. This module resolves what the rows ALREADY
//! carry in structured form -- the `a+b` crate pair -- against the
//! inventory's own records: both crates still declare the collision name.
//!
//! DELIBERATELY NO parallel table. A hand-kept referent list beside the
//! derivable rows is the fourth-census defect (conductor ruling 2026-09-12):
//! it drifts toward a smaller, greener population. The pair IS the typed
//! data, written by the row's adjudicator in the same edit as the row, so
//! there is nothing here to drift.
//!
//! Only the LIVE branch is checked, which is what separates the two verdicts
//! the bead requires apart: "this row's pair no longer holds the name"
//! (fires here) versus "this row can never expire" (a row whose pair never
//! held it -- which this check would ALSO catch on first run, and did not,
//! which is why the six current rows stay green). A dead OR-branch naming
//! something nonexistent is correctly ignored: the row still expires via
//! its pair.
//!
//! PATHS are not resolved: without row-carried fields there is nothing to
//! resolve them from, and parsing prose for paths would re-acquire the
//! defect. Stated as the boundary, not hidden in it.
//!
//! GuardDecision is exempt by name ([`REFERENT_EXEMPT`], pinned exact): it
//! was adjudicated with its prose frozen. Production calls
//! [`check_referents`] from `check()`, gated on adjudicability beside the
//! STALE block -- a scratch tree cannot convict a row.

use crate::types_inventory::{TypeInventory, ALLOWED_COLLISIONS};

/// Rows exempt from referent resolution, by name. Exactly GuardDecision,
/// pinned exact by a leg: the exemption list is itself a claim about scope
/// and grows only by a commit that edits both the list and its leg.
pub const REFERENT_EXEMPT: &[&str] = &["GuardDecision"];

/// Split an `a+b` pair into its two crates. `None` when the pair is
/// malformed -- which is itself a fault, never a skip.
fn split_pair(pair: &str) -> Option<(&str, &str)> {
    let (first, second) = pair.split_once('+')?;
    if first.is_empty() || second.is_empty() || second.contains('+') {
        return None;
    }
    Some((first, second))
}

/// Resolve every non-exempt row's pair against an inventory's own records.
/// Pure over inputs: no filesystem, no git, no prose parsing. The
/// adjudicability gate lives in the CALLER (`check()` beside the STALE
/// block), not here, so fixture legs can exercise resolution without faking
/// a workspace.
pub fn check_referents(inv: &TypeInventory) -> Vec<String> {
    check_referents_rows(inv, ALLOWED_COLLISIONS)
}

/// The rows-parameterised core, so legs can name their own table: the live
/// table for resolution, synthetic tables for the vacuity arms. A leg that
/// only ever ran against the live table could not tell "resolves" from
/// "never looked".
fn check_referents_rows(inv: &TypeInventory, rows: &[(&str, &str, &str)]) -> Vec<String> {
    let mut errors = Vec::new();
    if rows.is_empty() {
        errors.push(
            "REFERENTS_MISSING — the allowance table is empty; a leg that \
             resolves zero rows and passes is indistinguishable from one that \
             works"
                .to_owned(),
        );
        return errors;
    }
    for (name, pair, _) in rows {
        if REFERENT_EXEMPT.contains(name) {
            continue;
        }
        let Some((left, right)) = split_pair(pair) else {
            errors.push(format!(
                "REFERENTS_MALFORMED row={name} pair={pair} — a pair that is \
                 not exactly two crates cannot resolve"
            ));
            continue;
        };
        for krate in [left, right] {
            let resolved = inv.crates.iter().any(|c| {
                c.crate_name == krate
                    && c.decls
                        .iter()
                        .any(|d| !d.in_test_module && d.name == *name)
            });
            if !resolved {
                errors.push(format!(
                    "REFERENT_UNRESOLVED row={name} referent={krate}::{name} — \
                     no such declaration in this inventory"
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types_inventory::{CrateTypes, TypeDecl, TypeKind};

    fn fixture_crate(krate: &str, decls: &[(&str, &str)]) -> CrateTypes {
        CrateTypes {
            crate_name: krate.to_owned(),
            decls: decls
                .iter()
                .map(|(rel, name)| TypeDecl {
                    crate_name: krate.to_owned(),
                    name: (*name).to_owned(),
                    kind: TypeKind::Struct,
                    rel_path: (*rel).to_owned(),
                    line: 1,
                    in_test_module: false,
                })
                .collect(),
            reexports: Vec::new(),
            has_lib: true,
        }
    }

    fn fixture_inventory(crates: Vec<CrateTypes>) -> TypeInventory {
        crate::types_inventory::assemble(crates)
    }

    /// KNOWN-BAD: drop one side of a live pair and the row reddens naming
    /// row AND crate. The inventory holds everything else, so exactly one
    /// fault can fire.
    #[test]
    fn a_dropped_pair_side_reddens_naming_row_and_crate() {
        let rows: &[(&str, &str, &str)] = &[("Finding", "finding+state-wildcard-lint", "")];
        let inv = fixture_inventory(vec![
            fixture_crate("finding", &[("src/lib.rs", "Finding")]),
            fixture_crate("state-wildcard-lint", &[] as &[(&str, &str)]),
        ]);
        let errors = check_referents_rows(&inv, rows);
        assert_eq!(errors.len(), 1, "exactly the dropped side must fire: {errors:?}");
        assert!(
            errors[0].contains("REFERENT_UNRESOLVED")
                && errors[0].contains("row=Finding")
                && errors[0].contains("state-wildcard-lint::Finding"),
            "the refusal must name row and referent: {errors:?}"
        );
    }

    /// KNOWN-GOOD: a held pair resolves silently.
    #[test]
    fn a_held_pair_resolves_silently() {
        let rows: &[(&str, &str, &str)] = &[("Finding", "finding+state-wildcard-lint", "")];
        let inv = fixture_inventory(vec![
            fixture_crate("finding", &[("src/lib.rs", "Finding")]),
            fixture_crate("state-wildcard-lint", &[("src/lib.rs", "Finding")]),
        ]);
        let errors = check_referents_rows(&inv, rows);
        assert!(errors.is_empty(), "a held pair must not fault: {errors:?}");
    }

    /// ANTI-VACUITY twice: an empty table errors, and a malformed pair
    /// faults rather than skipping. Either silence would pass a table this
    /// leg never looked at.
    #[test]
    fn empty_table_and_malformed_pairs_fail_closed() {
        let inv = fixture_inventory(vec![]);
        let errors = check_referents_rows(&inv, &[]);
        assert!(
            errors.iter().any(|e| e.contains("REFERENTS_MISSING")),
            "an empty table must fail: {errors:?}"
        );
        let bad: &[(&str, &str, &str)] = &[("Finding", "finding", "")];
        let errors = check_referents_rows(&inv, bad);
        assert!(
            errors.iter().any(|e| e.contains("REFERENTS_MALFORMED")),
            "a malformed pair must fault: {errors:?}"
        );
    }

    /// The exemption list is exact: GuardDecision and nothing else. A second
    /// exemption arriving silently would be the carve-out this schema exists
    /// to prevent -- it must edit this leg in the open.
    #[test]
    fn exemption_list_is_exactly_guard_decision() {
        assert_eq!(REFERENT_EXEMPT, &["GuardDecision"]);
    }
    /// LIVE-WRAPPER leg (poumg.3 fix unit): breaking a REAL row reddens
    /// through the production `check()` path, not just the rows-parameterised
    /// core. The fixture is shaped adjudicable (omp-inventory-map crate
    /// present, Declared source from assemble) WITHOUT pretending the lane
    /// is adjudicable -- on workers this source never occurs naturally, so
    /// the shape is fixtured, not discovered. RULE 11: both directions
    /// pinned -- the broken table reddens AND the repaired table passes, so
    /// the leg can neither go green by accident nor be collapsed by refactor.
    #[test]
    fn live_wrapper_reddens_on_broken_row_repairs_clean() {
        fn live_crates_without_finding() -> Vec<CrateTypes> {
            vec![
                fixture_crate("omp-inventory-map", &[("src/lib.rs", "InventoryMap")]),
                fixture_crate("finding", &[] as &[(&str, &str)]),
                fixture_crate("state-wildcard-lint", &[("src/lib.rs", "Finding"), ("src/lib.rs", "LintReport")]),
                fixture_crate("undrained-pipe-lint", &[("src/lib.rs", "LintReport"), ("src/lib.rs", "Violation")]),
                fixture_crate("no-shell-gate", &[("src/lib.rs", "Violation"), ("src/lib.rs", "GateError")]),
                fixture_crate("ack-spine", &[("src/spine.rs", "DispatchIntent")]),
                fixture_crate("dispatch-claim-fence", &[("src/lib.rs", "DispatchIntent")]),
                fixture_crate("porting-gate", &[("src/lib.rs", "GateError")]),
                fixture_crate("agent-mail-native", &[("src/packet.rs", "Authority")]),
                fixture_crate("ompo-start", &[("src/hd0009.rs", "Authority"), ("src/hd0009.rs", "Resolution")]),
                fixture_crate("refill-idle-panes", &[("src/lib.rs", "Resolution")]),
                fixture_crate("contabo-reclaim", &[("src/model.rs", "GuardDecision")]),
                fixture_crate("omp-host-tool-guard", &[("src/lib.rs", "GuardDecision")]),
            ]
        }
        let broken = fixture_inventory(live_crates_without_finding());
        let errs = broken.check().err().unwrap_or_default();
        assert!(
            errs.iter().any(|e| e.contains("REFERENT_UNRESOLVED") && e.contains("row=Finding")),
            "breaking a real row must redden through check(): {errs:?}"
        );
        let mut repaired = live_crates_without_finding();
        let finding = repaired.iter_mut().find(|c| c.crate_name == "finding").expect("finding present");
        *finding = fixture_crate("finding", &[("src/lib.rs", "Finding")]);
        let errs = fixture_inventory(repaired).check().err().unwrap_or_default();
        assert!(
            !errs.iter().any(|e| e.contains("REFERENT_")),
            "the repaired table must pass with no referent errors: {errs:?}"
        );
    }
}
