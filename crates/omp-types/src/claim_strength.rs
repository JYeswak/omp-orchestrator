//! `ClaimStrength` — a total order over kinds of evidence, and the justification rule.
//!
//! Contract: `docs/contracts/claim_strength_contract.md`. Read it first; this module implements
//! that document and does not restate its arguments.
//!
//! # The rule this type makes inexpressible
//!
//! `~/.claude/CLAUDE.md:210-213`, THE ARTIFACT RIGOR STANDARD layer L1, mined from
//! `frankengraphdb`:
//!
//! > invariant(6) > proof(5) > bounded_model(4) > statistical(3) > slo(2) > benchmark(1). A weaker
//! > claim may NEVER enforce or justify a stronger one: `rank(justifier) >= rank(claim)` or the
//! > build fails.
//!
//! In prose that is a sentence a reviewer must catch every time, forever. Here it is
//! [`ClaimStrength::justifies`], and a benchmark justifying an invariant is `false` by
//! construction.
//!
//! # Why this carrier is AUTHORED and not re-exported
//!
//! This crate's rule is that its contents are derived from `asupersync`, never authored. The
//! survey that licenses the exception is §2 of the contract. In one line: the near miss is
//! `asupersync::lab::oracle::evidence::EvidenceStrength`, and it cannot serve because it carries
//! an `Against` variant — a *magnitude* including evidence against the hypothesis, where this is a
//! *kind* order with no such element. `Severity` and `ProofStrength` each exist twice upstream
//! under one name (derivation trap #1) and neither is this concern.
//!
//! # NO-CLAIM
//!
//! Pinning the order does not make any caller obey it. Production call sites today: **zero**.
//! Phase 1 wires consumers. A green property test proves the law for the TYPE.

use core::fmt;
use core::str::FromStr;

/// How strong the evidence behind a claim is — six kinds of warrant, totally ordered.
///
/// The derived `Ord` is the order: variants are declared weakest-first so the discriminant
/// ordering and [`ClaimStrength::rank`] cannot disagree. Ranks are **dense** 1..=6 on purpose —
/// a gap would invite a future level to be inserted by renumbering, and renumbering is the silent
/// widening [`ClaimStrength`] exists to forbid (law L5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaimStrength {
    /// One measurement, one machine, one moment. No declared expectation.
    Benchmark = 1,
    /// A budget declared and monitored. A breach is a signal, not a refutation.
    Slo = 2,
    /// A sample with a stated test. True in distribution, not per-instance.
    Statistical = 3,
    /// Exhaustive over a bounded domain; silent outside the bound.
    BoundedModel = 4,
    /// A machine-checked derivation over a stated model.
    Proof = 5,
    /// Holds by construction: a counterexample is a compile or type error.
    Invariant = 6,
}

impl ClaimStrength {
    /// The whole carrier set, weakest first.
    ///
    /// Public so a test can **exhaust** the order rather than sample it. Six levels give 36
    /// ordered pairs and 216 ordered triples, which is a `BoundedModel`-strength claim about the
    /// laws — the bound being this array, and this array being the entire type.
    pub const ALL: [ClaimStrength; 6] = [
        ClaimStrength::Benchmark,
        ClaimStrength::Slo,
        ClaimStrength::Statistical,
        ClaimStrength::BoundedModel,
        ClaimStrength::Proof,
        ClaimStrength::Invariant,
    ];

    /// Position in the order, 1..=6. Total and injective; no failure mode.
    #[must_use]
    pub const fn rank(self) -> u8 {
        self as u8
    }

    /// **The load-bearing operation.** `true` iff this strength may justify a claim of `claim`
    /// strength — that is, iff `rank(self) >= rank(claim)`.
    ///
    /// ```
    /// use omp_types::ClaimStrength::{Benchmark, Invariant, Proof};
    /// assert!(Invariant.justifies(Proof));
    /// assert!(!Benchmark.justifies(Invariant));
    /// assert!(Proof.justifies(Proof)); // reflexive: equal strength justifies
    /// ```
    #[must_use]
    pub const fn justifies(self, claim: ClaimStrength) -> bool {
        self.rank() >= claim.rank()
    }
    /// The **meet**: a compound claim is only as strong as its weakest evidence.
    ///
    /// There is deliberately no `strongest_of`. Combining two pieces of evidence never produces a
    /// stronger warrant for the compound claim than the weaker of them — that is law L5 expressed
    /// as an absent function rather than as a rule.
    #[must_use]
    pub const fn weakest_of(self, other: ClaimStrength) -> ClaimStrength {
        if self.rank() <= other.rank() { self } else { other }
    }

    /// The registry spelling, matching the L1 prose and the TOML rows it describes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ClaimStrength::Benchmark => "benchmark",
            ClaimStrength::Slo => "slo",
            ClaimStrength::Statistical => "statistical",
            ClaimStrength::BoundedModel => "bounded_model",
            ClaimStrength::Proof => "proof",
            ClaimStrength::Invariant => "invariant",
        }
    }
}

impl fmt::Display for ClaimStrength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The spelling was not one of the six registry names.
///
/// Carries the offending input so a refusal names what it refused — a bare "invalid" is the
/// defect this repo files against its own gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownClaimStrength(pub String);

impl fmt::Display for UnknownClaimStrength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UNKNOWN_CLAIM_STRENGTH {:?} — expected one of benchmark, slo, statistical, \
             bounded_model, proof, invariant",
            self.0
        )
    }
}

impl std::error::Error for UnknownClaimStrength {}

impl FromStr for ClaimStrength {
    type Err = UnknownClaimStrength;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ClaimStrength::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| UnknownClaimStrength(s.to_owned()))
    }
}
