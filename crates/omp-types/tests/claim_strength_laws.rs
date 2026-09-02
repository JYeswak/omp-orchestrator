#![forbid(unsafe_code)]
//! One property test per law, five laws. Contract: `docs/contracts/claim_strength_contract.md` §5.
//!
//! **A law without a test is a comment.**
//!
//! # Why these are EXHAUSTIVE and not sampled
//!
//! The carrier set is finite and closed: six levels, 36 ordered pairs, 216 ordered triples. So
//! L3 and L4 are generated over the *entire* input space rather than over a sample of it. In the
//! vocabulary this type defines, a sampled property test of these laws would be a
//! `Statistical` claim; enumerating the carrier makes them `BoundedModel` — exhaustive over a
//! bounded domain, with the bound being `ClaimStrength::ALL` and that bound being the whole type.
//!
//! This is strictly stronger than sampling, and it is only available because the carrier is
//! small. It is stated here rather than assumed, because "generated inputs" is often taken to
//! mean "randomly sampled" and a reader is entitled to know which one this is.
//!
//! Anti-vacuity is asserted in every test: a generator that yielded nothing would make each law
//! pass over zero cases and report identically to a law that holds.

use omp_types::ClaimStrength::{
    self, Benchmark, BoundedModel, Invariant, Proof, Slo, Statistical,
};
use std::str::FromStr;

/// The generator. Every test draws from here, so a change to the carrier reaches every law.
fn all() -> Vec<ClaimStrength> {
    ClaimStrength::ALL.to_vec()
}

fn pairs() -> Vec<(ClaimStrength, ClaimStrength)> {
    all().into_iter().flat_map(|a| all().into_iter().map(move |b| (a, b))).collect()
}

fn triples() -> Vec<(ClaimStrength, ClaimStrength, ClaimStrength)> {
    all()
        .into_iter()
        .flat_map(|a| {
            all()
                .into_iter()
                .flat_map(move |b| all().into_iter().map(move |c| (a, b, c)))
        })
        .collect()
}

// ───────────────────────────────────────────────────────────────────── L1

/// L1 · TOTAL ORDER. Any two strengths are comparable; no incomparable pair exists.
///
/// An incomparable pair would make `justifies` undecidable for that pair, and a gate that cannot
/// decide fails open or closed arbitrarily.
#[test]
fn l1_the_order_is_total_so_no_incomparable_pair_exists() {
    let ps = pairs();
    assert_eq!(ps.len(), 36, "ANTI-VACUITY: expected all 36 ordered pairs, got {}", ps.len());

    for (a, b) in ps {
        // `partial_cmp` on a total order is never None — that is the definition being tested.
        let ord = a.partial_cmp(&b);
        assert!(ord.is_some(), "{a} and {b} are incomparable");

        // exactly one of <, ==, > holds
        let trichotomy = [a < b, a == b, a > b];
        let holds = trichotomy.iter().filter(|x| **x).count();
        assert_eq!(holds, 1, "trichotomy broken for {a} vs {b}: {trichotomy:?}");

        // and Ord agrees with rank, so the two definitions of the order cannot diverge
        assert_eq!(
            a.cmp(&b),
            a.rank().cmp(&b.rank()),
            "Ord and rank disagree for {a} vs {b}"
        );
    }

    // ranks are dense 1..=6, so no future level can be inserted by renumbering
    let mut ranks: Vec<u8> = all().into_iter().map(ClaimStrength::rank).collect();
    ranks.sort_unstable();
    assert_eq!(ranks, vec![1, 2, 3, 4, 5, 6], "ranks must be dense and ascending");
}

// ───────────────────────────────────────────────────────────────────── L2

/// L2 · ANTISYMMETRY. `a >= b && b >= a` implies `a == b`.
///
/// Without it two distinct levels could mutually justify each other and the order would carry a
/// cycle — a benchmark could reach an invariant in two steps.
#[test]
fn l2_mutual_justification_implies_equality() {
    let ps = pairs();
    assert_eq!(ps.len(), 36, "ANTI-VACUITY: expected 36 pairs");
    let mut mutual = 0usize;

    for (a, b) in ps {
        if a.justifies(b) && b.justifies(a) {
            mutual += 1;
            assert_eq!(a, b, "L2 VIOLATED: {a} and {b} justify each other but are not equal");
        }
    }
    // exactly the six reflexive pairs may be mutual; more would mean a collapsed level
    assert_eq!(
        mutual, 6,
        "expected exactly the 6 reflexive pairs to be mutually justifying, found {mutual}"
    );
}

// ───────────────────────────────────────────────────────────────────── L3

/// L3 · TRANSITIVITY, over all 216 generated triples — not hand-picked cases.
///
/// Justification chains are the normal case (a claim justified by a claim justified by evidence);
/// non-transitivity would make a chain's validity depend on assembly order.
#[test]
fn l3_justification_is_transitive_over_every_triple() {
    let ts = triples();
    assert_eq!(ts.len(), 216, "ANTI-VACUITY: expected all 216 triples, got {}", ts.len());

    let mut antecedents = 0usize;
    for (a, b, c) in ts {
        if a.justifies(b) && b.justifies(c) {
            antecedents += 1;
            assert!(
                a.justifies(c),
                "L3 VIOLATED: {a} >= {b} and {b} >= {c} but {a} does not justify {c}"
            );
        }
    }
    // A vacuous pass is the failure mode here: if no triple satisfied the antecedent the
    // implication would hold trivially over 216 cases and prove nothing.
    assert!(
        antecedents >= 56,
        "ANTI-VACUITY: only {antecedents} triples satisfied the antecedent; the implication would \
         be near-vacuous. For a 6-element total order the count is C(6,3)+... = 56 strictly \
         descending plus all cases with equalities"
    );
}

// ───────────────────────────────────────────────────────────────────── L4

/// L4 · JUSTIFICATION, over all 36 generated pairs, in BOTH directions.
///
/// This is the law with teeth and the one prose cannot enforce. The negative direction is the
/// half that matters: for every pair where the justifier is weaker, `justifies` must be `false`.
#[test]
fn l4_justifies_holds_exactly_when_the_justifier_ranks_at_least_as_high() {
    let ps = pairs();
    assert_eq!(ps.len(), 36, "ANTI-VACUITY: expected 36 pairs");

    let mut positives = 0usize;
    let mut negatives = 0usize;
    for (j, c) in ps {
        let expected = j.rank() >= c.rank();
        assert_eq!(
            j.justifies(c),
            expected,
            "L4 VIOLATED: {j}(rank {}) justifies {c}(rank {}) returned {}, expected {expected}",
            j.rank(),
            c.rank(),
            j.justifies(c)
        );
        if expected {
            positives += 1;
        } else {
            negatives += 1;
        }
    }
    // BOTH arms must be populated. An all-true or all-false predicate would satisfy the loop.
    assert_eq!(positives, 21, "expected 21 justifying pairs, got {positives}");
    assert_eq!(negatives, 15, "expected 15 refusing pairs, got {negatives}");

    // The named case from the contract and from the L1 prose, asserted explicitly because it is
    // the sentence the type exists to make inexpressible.
    assert!(
        !Benchmark.justifies(Invariant),
        "a benchmark must NEVER justify an invariant"
    );
    assert!(!Slo.justifies(Proof));
    assert!(!Statistical.justifies(BoundedModel));
    assert!(Invariant.justifies(Benchmark));
    assert!(Proof.justifies(Proof), "equal strength justifies: the relation is reflexive");
}

// ───────────────────────────────────────────────────────────────────── L5

/// L5 · NO SILENT WIDENING. No operation raises a claim's strength.
///
/// Tested structurally over the whole carrier: `weakest_of` never exceeds either input, is the
/// true meet, and no public operation returns a strength strictly greater than every input.
#[test]
fn l5_no_operation_raises_a_strength() {
    let ps = pairs();
    assert_eq!(ps.len(), 36, "ANTI-VACUITY: expected 36 pairs");

    for (a, b) in ps {
        let w = a.weakest_of(b);
        assert!(w <= a && w <= b, "weakest_of({a}, {b}) = {w} exceeds an input");
        assert!(w == a || w == b, "weakest_of must return one of its inputs, got {w}");
        assert_eq!(w, a.min(b), "weakest_of must be the meet");
        assert_eq!(w, b.weakest_of(a), "weakest_of must be commutative");
        assert_eq!(w, w.weakest_of(w), "weakest_of must be idempotent");
    }

    // Round-tripping through the registry spelling must not change strength either — a widening
    // through serialisation would be exactly the silent kind.
    for a in all() {
        let back = ClaimStrength::from_str(a.as_str()).expect("registry spelling must round-trip");
        assert_eq!(back, a, "{a} did not round-trip through its registry spelling");
        assert_eq!(back.rank(), a.rank());
    }

    // An unknown spelling is a NAMED refusal carrying the input, not a silent default that would
    // let a typo land as some arbitrary strength.
    let err = ClaimStrength::from_str("guarantee").expect_err("must refuse an unknown spelling");
    assert!(
        err.to_string().contains("UNKNOWN_CLAIM_STRENGTH") && err.to_string().contains("guarantee"),
        "the refusal must name what it refused: {err}"
    );
}
