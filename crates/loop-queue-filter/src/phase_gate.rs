//! Make the jplf phase order BINDING on dispatch (`omp-orchestrator-block-non-arc-behind-s0-f3g5`).
//!
//! # Why a selector predicate and not 881 dependency edges
//!
//! The arc `jplf.1 <- jplf.2 <- … <- jplf.8` already encodes the order correctly, and it binds
//! exactly the 38 arc beads. The other ~881 rows carry no phase dependency, so the order is
//! unenforceable by construction and throughput accrues entirely outside the program. The
//! rejected alternative was adding `blocks` edges from every non-arc bead onto `jplf.1`:
//!
//! - it repeats `DagIntegrator2` at 30x scale — 101 inverting edges in one hour, 28 of them
//!   UNSUPPORTED, five of which strangled the whole canonical-CLI family;
//! - `AGENTS.md` forbids the shape outright: *"An epic OWNS its leaves via parent-child — NEVER a
//!   `blocks` edge onto its own leaf"*, with 13 of the first 30 unassigned open beads strangled
//!   that way, four P0;
//! - 881 edges are not reversible in practice, where this is one constant.
//!
//! # THE GATE SHIPS DISABLED, and that is a decision rather than caution
//!
//! [`PHASE_GATE_ENABLED`] is `false`. Turning it on is a conductor action taken against a measured
//! precondition, not a side effect of this landing — because a gate that admits nothing is
//! `HD-0016`'s total halt with extra steps.
//!
//! **The precondition is stated on the LEAVES, deliberately.** An earlier draft of it read "until
//! `jplf.1` is selectable", and that is unsatisfiable by construction: `jplf.1` is an *epic*, and
//! `br ready` excludes epics fleet-wide because a container is not work and its PageRank
//! accumulates from every child. A precondition keyed on it would never fire and would park this
//! gate forever — the never-fires class this repository keeps paying for. See
//! [`SWITCH_ON_PRECONDITION`].
//!
//! # NO-CLAIM
//!
//! This makes the phase order binding on dispatch when enabled. It does NOT make arc beads visible
//! to `br ready` (that is the selector-visibility class), does not unblock Phase 0, does not close
//! any arc bead, and does not prove the arc is the right decomposition — only that work outside it
//! cannot be dispatched while the phase is open.

use std::collections::BTreeSet;

use admission_reason::Withheld;

/// Every arc member's id starts with this. The arc is a NAMING convention in the tracker, which is
/// itself a weakness worth stating: membership is not a graph fact, so a renamed bead silently
/// leaves the arc.
pub const ARC_PREFIX: &str = "omp-orchestrator-jplf";

/// The phase whose completion releases the gate.
pub const GATED_PHASE: &str = "jplf.1";

/// THE ONE-LINE REVERSAL (acceptance item 9). Flip this constant to disable the gate entirely;
/// [`apply_phase_gate`] then admits the pre-gate candidate set unchanged, which
/// `the_disabled_path_admits_the_pre_gate_set_unchanged` proves rather than asserts.
pub const PHASE_GATE_ENABLED: bool = false;

/// The condition under which a conductor may set [`PHASE_GATE_ENABLED`] to `true`.
///
/// Keyed on non-epic arc members, never on `jplf.1` itself. Stated as a string so it ships in the
/// binary and cannot drift from the doc comment that explains it.
pub const SWITCH_ON_PRECONDITION: &str =
    "at least one NON-EPIC arc member is present in `br ready`; never keyed on jplf.1, which is an \
     epic and is correctly excluded from br ready fleet-wide, so a precondition naming it can \
     never fire";

/// Known-good arc census. A gate whose arc collapsed to zero must ERROR, not withhold everything,
/// and a bare `!= 0` check cannot tell a collapse from a rename. Measured 2026-09-07: 38.
pub const ARC_CENSUS_POSITIVE_CONTROL: usize = 38;

/// One declared exception, with the reason it exists.
///
/// A predicate that "happens to let things through" fails acceptance item 2. Every entry is a
/// named row with a one-line justification a reviewer can disagree with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    /// An exact bead id, or a class token matched as an id substring.
    pub selector: &'static str,
    /// Why this row or class is exempt. Never empty — see [`exception_set_is_complete`].
    pub reason: &'static str,
    /// `true` when `selector` names a CLASS matched by substring rather than one bead.
    pub is_class: bool,
}

/// The exception set. **Explicit, enumerated, and non-empty** — an empty set is a total halt,
/// which is `HD-0016` and fails acceptance item 2.
pub const EXCEPTION_SET: &[Exception] = &[
    Exception {
        selector: "omp-orchestrator-selector-blind-to-unpartitioned-open-kt0m",
        reason: "the arc-visibility defect itself: gating on the arc while the arc is unselectable \
                 dispatches nothing. NOTE 2026-09-07: kt0m is now CLOSED, so this entry is inert \
                 rather than load-bearing — kept because the CONDITION it protected against \
                 persists under different causes (epic exclusion, live blockers, parent-child \
                 invisibility) and a reader must not conclude the risk expired with the bead",
        is_class: false,
    },
    Exception {
        selector: "grade",
        reason: "grading lane, per HD-0015: classes that provably do not read a plan document \
                 remain dispatchable under a typed DEGRADED verdict. Withholding grading would \
                 starve the only path that CLOSES arc beads",
        is_class: true,
    },
    Exception {
        selector: "hygiene",
        reason: "hygiene lane, per HD-0015, same clause as grading",
        is_class: true,
    },
    Exception {
        selector: "unblock",
        reason: "the Phase 0 unblock path: the work that makes the gated phase completable cannot \
                 itself be gated behind that phase, which would be circular",
        is_class: true,
    },
];

/// Is every exception entry usable? An entry with an empty reason is the shape that turns a
/// declared list back into an opaque predicate.
#[must_use]
pub fn exception_set_is_complete() -> bool {
    !EXCEPTION_SET.is_empty()
        && EXCEPTION_SET
            .iter()
            .all(|e| !e.selector.trim().is_empty() && !e.reason.trim().is_empty())
}

/// Refusals, distinct from withholdings: a broken instrument must never look like a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// The arc census is zero. ANTI-VACUITY: withholding everything because the arc vanished is
    /// indistinguishable from a correct total halt, and the first probe of this graph returned
    /// "0 edges" from guessed field names — the instrument, not the graph.
    ArcCensusZero,
    /// No candidates at all. "Nothing to dispatch" and "I could not read the queue" are opposite
    /// conditions with opposite remedies.
    EmptyCandidateSet,
    /// The declared exception set is empty or malformed, so the gate cannot be trusted to admit
    /// the paths that make the phase completable.
    ExceptionSetIncomplete,
}

impl GateError {
    /// Distinct code per cause. A leg matching only `rc != 0` goes green on unrelated breakage.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::ArcCensusZero => 30,
            Self::EmptyCandidateSet => 31,
            Self::ExceptionSetIncomplete => 32,
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ArcCensusZero => "arc_census_zero",
            Self::EmptyCandidateSet => "empty_candidate_set",
            Self::ExceptionSetIncomplete => "exception_set_incomplete",
        }
    }
}

impl std::fmt::Display for GateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArcCensusZero => write!(
                formatter,
                "PHASE_GATE_ERROR arc_census_zero — the arc has no members, so every withhold \
                 decision would be vacuous; expected around {ARC_CENSUS_POSITIVE_CONTROL}"
            ),
            Self::EmptyCandidateSet => write!(
                formatter,
                "PHASE_GATE_ERROR empty_candidate_set — an unreadable or empty queue is not \
                 'nothing to dispatch'"
            ),
            Self::ExceptionSetIncomplete => write!(
                formatter,
                "PHASE_GATE_ERROR exception_set_incomplete — a gate with no usable exception set \
                 is a total halt (HD-0016), not a phase order"
            ),
        }
    }
}

impl std::error::Error for GateError {}

/// The gate's decision over one candidate set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    /// Candidates the selector may still offer, in input order.
    pub admitted: Vec<String>,
    /// Candidates withheld, each with its typed reason.
    pub withheld: Vec<(String, Withheld)>,
    /// Whether the gate actually ran, so a caller can tell "admitted everything" from
    /// "the gate is off". Those are the same admitted set and different facts.
    pub gate_active: bool,
}

impl GateOutcome {
    /// The operator line. States both counts and whether the gate was active, because an admitted
    /// count alone cannot distinguish a released phase from a disabled gate.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "PHASE_GATE active={} admitted={} withheld={} phase={} precondition=\"{}\"",
            self.gate_active,
            self.admitted.len(),
            self.withheld.len(),
            GATED_PHASE,
            SWITCH_ON_PRECONDITION,
        )
    }
}

/// Is this candidate exempt from the gate?
#[must_use]
pub fn is_exempt(candidate: &str) -> bool {
    EXCEPTION_SET.iter().any(|e| {
        if e.is_class {
            candidate.contains(e.selector)
        } else {
            candidate == e.selector
        }
    })
}

/// Is this candidate an arc member?
///
/// THE BOUNDARY IS LOAD-BEARING, and my own acceptance leg caught this before it landed. A bare
/// `starts_with(ARC_PREFIX)` also matches `omp-orchestrator-jplf-lookalike-not-arc`, which would
/// FALSE-ADMIT a non-arc bead straight through the gate — the same substring-versus-token defect
/// that made `std::thread::spawn` double-count inside `thread::spawn` and turned 14 sites into a
/// published 20. Membership is the prefix at a real separator, or the arc root exactly.
#[must_use]
pub fn is_arc_member(candidate: &str) -> bool {
    candidate == ARC_PREFIX
        || candidate
            .strip_prefix(ARC_PREFIX)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// Apply the phase gate.
///
/// `arc_census` is passed in rather than derived here so the anti-vacuity check keys on what the
/// caller actually observed; deriving it from `candidates` would make the census a function of the
/// queue and could report zero for a healthy arc that simply is not offered today — which is the
/// live state, measured: 0 of 37 non-terminal arc members appear in `br ready`.
///
/// # Errors
///
/// [`GateError::EmptyCandidateSet`], [`GateError::ArcCensusZero`] and
/// [`GateError::ExceptionSetIncomplete`] — each a refusal, never an empty admitted set.
pub fn apply_phase_gate(
    candidates: &[String],
    arc_census: usize,
    phase_complete: bool,
    enabled: bool,
) -> Result<GateOutcome, GateError> {
    if candidates.is_empty() {
        return Err(GateError::EmptyCandidateSet);
    }
    if !enabled {
        // The disabled path is a straight passthrough, and it reports `gate_active=false` so the
        // admitted set is never mistaken for a gated one.
        return Ok(GateOutcome {
            admitted: candidates.to_vec(),
            withheld: Vec::new(),
            gate_active: false,
        });
    }
    if !exception_set_is_complete() {
        return Err(GateError::ExceptionSetIncomplete);
    }
    // Checked only on the ACTIVE path: an arc census of zero cannot make a passthrough wrong, and
    // erroring there would make the disable flag unusable in exactly the emergency it exists for.
    if arc_census == 0 {
        return Err(GateError::ArcCensusZero);
    }

    let mut admitted = Vec::new();
    let mut withheld = Vec::new();
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if phase_complete || is_arc_member(candidate) || is_exempt(candidate) {
            admitted.push(candidate.clone());
        } else {
            withheld.push((
                candidate.clone(),
                Withheld::PhaseGate {
                    phase: GATED_PHASE.to_owned(),
                    blocking_bead: format!("{ARC_PREFIX}.1"),
                },
            ));
        }
    }
    Ok(GateOutcome {
        admitted,
        withheld,
        gate_active: true,
    })
}
