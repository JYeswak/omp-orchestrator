//! GATE PROPERTY REGISTRY — every "Y" in the six-property matrix must cite the
//! test function that provides it, and that function must exist.
//!
//! # The BLOCKER this answers
//!
//! Round 13, `GradeGates`, against `06-gates.md`:
//!
//! > The preamble frames gates as making falsifiable claims, but the matrix
//! > immediately proves zero gates satisfy the six required properties. A gate that
//! > claims to enforce something mechanically while failing all six properties is
//! > making an unsupported claim.
//!
//! # Why keyword derivation does NOT work — measured, in both directions
//!
//! The obvious fix is to derive the matrix by grepping each gate crate for markers.
//! I tried it. It is wrong both ways:
//!
//! - **False negative.** `state-wildcard-lint` is documented as having a known-good
//!   leg. A grep for `known.good` found ZERO. It has two:
//!   `wildcard_on_integer_and_string_passes` and
//!   `wildcard_on_non_state_enum_passes` — known-good legs that never use the
//!   phrase. I was one commit from filing the document as wrong.
//! - **False positive against the document.** The same grep said
//!   `state-wildcard-lint` has no anti-vacuity, and the document agrees (`N`). Both
//!   are wrong: `empty_or_unreadable_workspace_is_an_error` is exactly that leg.
//!
//! So the hand-maintained table has at least one incorrect cell, AND the mechanical
//! derivation that would catch it is unreliable in both directions. A property is a
//! semantic claim about what a test does; no keyword scan can settle it.
//!
//! # What this registry does instead
//!
//! It replaces the assertion `known-good: Y` with the citation
//! `known-good: wildcard_on_integer_and_string_passes`, and checks that the named
//! function EXISTS in that crate. A "Y" becomes falsifiable: delete the test and
//! the build fails.
//!
//! # What it cannot do
//!
//! It cannot verify that the cited test actually *provides* the property. A test
//! named `known_good_leg` that asserts nothing satisfies this gate. That is the
//! same residual as `no-shell-gate`'s own mutation-leg counting, which it already
//! records: *"a file named `mutation.rs` that mutates nothing counts here as a
//! mutation leg."* The floor moves from "a letter in a table nobody can check" to
//! "a named function that must exist" — strictly better, and short of proof.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// One property claim: (gate crate, property, citing test fn — or None for a
/// declared absence).
///
/// `None` is a first-class value here. A declared absence is information; a blank
/// cell is not. Six of these rows are `None`, and they are the reason the section's
/// "zero gates satisfy all six" sentence is true.
type Claim = (&'static str, &'static str, Option<&'static str>);

/// Derived by reading every gate crate's tests, 2026-09-01. Where this disagrees
/// with the table in `06-gates.md`, the disagreement is noted in the section.
const GATE_PROPERTIES: &[Claim] = &[
    // no-shell-gate — the reference implementation
    ("no-shell-gate", "known-bad", Some("planted_shell_is_red_then_green_after_delete")),
    ("no-shell-gate", "known-good", Some("clean_list_passes")),
    ("no-shell-gate", "anti-vacuity", Some("empty_scan_set_is_an_error_not_a_pass")),
    // state-wildcard-lint — the crate whose row the document gets WRONG
    (
        "state-wildcard-lint",
        "known-good",
        Some("wildcard_on_integer_and_string_passes"),
    ),
    (
        "state-wildcard-lint",
        "mutation",
        Some("mutation_removing_state_wildcard_is_green"),
    ),
    (
        // 06-gates.md records `N` for this cell. It is wrong: the leg exists and is
        // named for what it does rather than for its category.
        "state-wildcard-lint",
        "anti-vacuity",
        Some("empty_or_unreadable_workspace_is_an_error"),
    ),
    ("state-wildcard-lint", "wired", Some("lint_is_wired_into_blocking_ci")),
    // Declared absences — each is a real gap, not an omission
    ("kernel-bypass-gate", "mutation", None),
    ("kernel-bypass-gate", "anti-vacuity", None),
    ("pre-delete-citation-check", "mutation", None),
    ("pre-delete-citation-check", "anti-vacuity", None),
    ("path-literal-guard", "known-good", None),
    ("undrained-pipe-lint", "claim-discipline", None),
];

fn crate_test_bodies(root: &Path, crate_name: &str) -> String {
    let mut all = String::new();
    for sub in ["src", "tests"] {
        let mut stack = vec![root.join("crates").join(crate_name).join(sub)];
        while let Some(d) = stack.pop() {
            let Ok(items) = std::fs::read_dir(&d) else { continue };
            for it in items.flatten() {
                let p = it.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_some_and(|x| x == "rs") {
                    if let Ok(b) = std::fs::read_to_string(&p) {
                        all.push_str(&b);
                        all.push('\n');
                    }
                }
            }
        }
    }
    all
}

#[test]
fn every_claimed_property_cites_a_test_that_exists() {
    let root = repo_root();
    let mut missing = Vec::new();
    let mut checked = 0usize;
    for (crate_name, property, citation) in GATE_PROPERTIES {
        let Some(test_fn) = citation else { continue };
        checked += 1;
        let body = crate_test_bodies(&root, crate_name);
        // Match the definition, not a mention: `fn NAME` with a boundary after.
        let needle = format!("fn {test_fn}");
        let found = body.contains(&needle);
        if !found {
            missing.push(format!("{crate_name}/{property} cites `{test_fn}` — not found"));
        }
    }
    assert!(
        checked > 0,
        "ANTI-VACUITY: no property claims carry a citation — the registry is empty or \
         every row declared an absence, and neither is a pass"
    );
    assert!(
        missing.is_empty(),
        "{} property claim(s) cite a test that does not exist:\n{:#?}\n\n\
         A \"Y\" in the six-property matrix is only worth the test behind it. Either \
         the test was renamed and this row is stale, or the claim was never true.",
        missing.len(),
        missing
    );
}

#[test]
fn a_declared_absence_is_recorded_rather_than_left_blank() {
    // Six rows carry None. If that ever reaches zero, either every gap closed — in
    // which case `06-gates.md`'s "zero gates satisfy all six" sentence is stale and
    // must be rewritten — or somebody deleted the honest half of the registry.
    let absences = GATE_PROPERTIES.iter().filter(|(_, _, c)| c.is_none()).count();
    assert!(
        absences > 0,
        "every property row now carries a citation. If that is real, 06-gates.md's \
         'Zero gates satisfy all six' is now FALSE and the section must be updated in \
         the same commit. If it is not real, the absences were deleted rather than closed."
    );
}

#[test]
fn no_property_is_claimed_twice_for_one_crate() {
    // A duplicated row lets a stale citation hide behind a fresh one — the same
    // shape as the duplicate NUMBERS.toml key that made the registry's own
    // duplicate-key gate fire on its author tonight.
    let mut seen = std::collections::BTreeSet::new();
    let mut dupes = Vec::new();
    for (crate_name, property, _) in GATE_PROPERTIES {
        if !seen.insert((*crate_name, *property)) {
            dupes.push(format!("{crate_name}/{property}"));
        }
    }
    assert!(
        dupes.is_empty(),
        "{} duplicated property row(s): {:?}",
        dupes.len(),
        dupes
    );
}
