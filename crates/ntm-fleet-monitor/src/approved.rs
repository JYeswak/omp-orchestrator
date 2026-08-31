//! Type-level approval: a human-gated action cannot be *constructed* on the
//! autonomous execution path, so it cannot reach an executor at all.
//!
//! # Why a newtype and not a bool
//!
//! `WaveVerdict::apply_allowed()` is a *runtime* branch. The caller is free to
//! ignore it, and xtask's own executor guarded exactly one call site with
//! `if w.verdict.apply_allowed()`. Delete that `if` — a refactor, a merge, a
//! tired afternoon — and `SpendMoney` executes. Nothing in the type system
//! objects, and no test that asserts on `classify()` alone would notice,
//! because `classify` still returns the correct verdict. The defect is not in
//! the classification; it is in the *distance* between classifying and obeying.
//!
//! This module removes that distance. [`Approved`] is a wrapper whose field is
//! private to this module and whose only constructor,
//! [`Approved::authorize`], returns `Err` for anything that is not
//! [`WaveVerdict::Autonomous`]. An executor that takes `Approved` therefore
//! *cannot be handed* a `Required` or `Refuse` wave: there is no expression
//! that produces one. This is the same guarantee that makes an absent
//! measurement a refusal rather than a green light — the bad state is
//! unrepresentable, not merely untested.
//!
//! # What this does NOT claim
//!
//! It does not prove an executor is correct, that a pane was measured
//! honestly, or that `classify` assigns the right verdict — those are
//! [`classify`](crate::classify)'s tests. It proves exactly one thing: **an
//! action whose verdict is not `Autonomous` cannot be passed to a function
//! that demands `Approved`.** This pass has no mutating executor by design;
//! the `xtask` operator frontend uses this constructor before it reports an
//! applyable wave, and any future executor must take `Approved`, not `Wave`.

use crate::{ApprovalKind, Refusal, Wave, WaveVerdict};

/// Why an [`Approved`] could not be minted. Mirrors the non-autonomous half of
/// [`WaveVerdict`], so a caller can report the reason without re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotApproved {
    /// One of the three human blockers, or the gate-weaken carve-out.
    /// Escalate: this is a decision, not a fact to fix.
    Required { kind: ApprovalKind },
    /// A safety refusal. NOT an escalation — fix the fact and re-classify.
    /// Asking a human to approve an unmeasured pane is how false FREE ships.
    Refused { reason: Refusal },
}

impl NotApproved {
    /// Stable kebab-case wire name for the class.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Required { .. } => "required",
            Self::Refused { .. } => "refuse",
        }
    }

    /// True when a human decision could unblock this.
    ///
    /// A `Refused` wave is deliberately NOT escalatable: the remedy is to take
    /// the second capture or file the bead, never to ask for permission to skip
    /// it. Routing a refusal to a human converts a fixable fact into a prompt.
    pub const fn is_escalatable(self) -> bool {
        matches!(self, Self::Required { .. })
    }
}

/// A wave proven `Autonomous` at construction time.
///
/// The inner field is private, so the ONLY way to obtain one outside this
/// module is [`Approved::authorize`], which refuses every non-autonomous
/// verdict. Executors take this type instead of [`Wave`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Approved(Wave);

impl Approved {
    /// The sole constructor. `Err` for anything not [`WaveVerdict::Autonomous`].
    ///
    /// Deliberately takes an already-classified [`Wave`] rather than an
    /// `Intent`: minting must not be able to re-run classification with
    /// different facts than the caller reported.
    pub const fn authorize(wave: Wave) -> Result<Self, NotApproved> {
        match wave.verdict {
            WaveVerdict::Autonomous => Ok(Self(wave)),
            WaveVerdict::Required { kind } => Err(NotApproved::Required { kind }),
            WaveVerdict::Refuse { reason } => Err(NotApproved::Refused { reason }),
        }
    }

    /// Read-only view of the authorized wave.
    ///
    /// Returns a copy, never `&mut`: a caller must not be able to swap the
    /// action after authorization and keep the proof.
    pub const fn wave(self) -> Wave {
        self.0
    }

    /// The authorized action.
    pub const fn action(self) -> crate::TypedAction {
        self.0.action
    }

    /// The phase this action belongs to.
    pub const fn phase(self) -> loop_coverage::LoopLayer {
        self.0.phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{classify, Intent, TypedAction, ALL_ACTIONS};

    /// Every fact true: the maximally-permissive intent.
    fn ready(action: TypedAction) -> Intent {
        Intent {
            action,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        }
    }

    #[test]
    fn autonomous_mints_and_carries_its_action() {
        let w = classify(ready(TypedAction::DispatchPacket));
        assert_eq!(w.verdict, WaveVerdict::Autonomous);
        let a = Approved::authorize(w).expect("autonomous must mint");
        assert_eq!(a.action(), TypedAction::DispatchPacket);
        assert_eq!(a.wave(), w);
    }

    /// ANTI-VACUITY. Without this, a constructor that refused *everything*
    /// would pass every negative test below and read perfectly clean — the
    /// stopped-clock shape that refuses all work and reports healthy.
    #[test]
    fn at_least_one_action_actually_mints() {
        let minted = ALL_ACTIONS
            .iter()
            .filter(|a| Approved::authorize(classify(ready(**a))).is_ok())
            .count();
        assert!(
            minted > 0,
            "no action can EVER be approved; the gate is a stopped clock"
        );
    }

    #[test]
    fn each_human_blocker_refuses_with_its_own_kind() {
        for (action, kind) in [
            (TypedAction::SpendMoney, ApprovalKind::Spend),
            (TypedAction::ObtainCredentials, ApprovalKind::Credentials),
            (TypedAction::DeployPublic, ApprovalKind::Deploy),
            (TypedAction::WeakenGate, ApprovalKind::GateWeaken),
        ] {
            let err = Approved::authorize(classify(ready(action)))
                .expect_err("a human blocker must never mint");
            assert_eq!(
                err,
                NotApproved::Required { kind },
                "{} must carry its own kind, not a generic denial",
                action.as_str()
            );
            assert!(err.is_escalatable(), "a human blocker is escalatable");
        }
    }

    /// The full-permission intent is the hostile case: every fact is true, so
    /// only the ACTION's own class can stop it. A gate that leans on missing
    /// facts would pass a weaker test and fail this one.
    #[test]
    fn no_human_blocker_mints_even_with_every_fact_true() {
        for action in ALL_ACTIONS {
            let is_blocker = matches!(
                action,
                TypedAction::SpendMoney
                    | TypedAction::ObtainCredentials
                    | TypedAction::DeployPublic
                    | TypedAction::WeakenGate
            );
            let minted = Approved::authorize(classify(ready(action))).is_ok();
            assert!(
                !(is_blocker && minted),
                "{} minted an Approved with all facts true",
                action.as_str()
            );
        }
    }

    #[test]
    fn a_safety_refusal_is_not_escalatable() {
        let mut intent = ready(TypedAction::DispatchPacket);
        intent.two_captures = false;
        let err = Approved::authorize(classify(intent)).expect_err("one capture must refuse");
        assert_eq!(
            err,
            NotApproved::Refused {
                reason: Refusal::SingleCaptureLiveness
            }
        );
        assert!(
            !err.is_escalatable(),
            "a refusal must NOT route to a human; the remedy is the second capture"
        );
    }

    /// cp-rfx78's FALSE FREE, at the type level: an unmeasured pane cannot
    /// produce the value an executor demands.
    #[test]
    fn false_free_cannot_mint() {
        let mut intent = ready(TypedAction::DispatchPacket);
        intent.pane_dispatchable = false;
        assert_eq!(
            Approved::authorize(classify(intent)),
            Err(NotApproved::Refused {
                reason: Refusal::PaneNotDispatchable
            })
        );
    }

    #[test]
    fn an_incomplete_packet_cannot_mint() {
        let mut intent = ready(TypedAction::DispatchPacket);
        intent.packet_complete = false;
        assert_eq!(
            Approved::authorize(classify(intent)),
            Err(NotApproved::Refused {
                reason: Refusal::PacketIncomplete
            })
        );
    }

    /// PHASE -1: a finding with no bead is lost, so it is refused rather than
    /// escalated — filing the bead is the fix, not permission to skip it.
    #[test]
    fn a_finding_without_a_bead_cannot_mint() {
        let err = Approved::authorize(classify(ready(TypedAction::ReportFindingWithoutBead)))
            .expect_err("a beadless finding must refuse");
        assert_eq!(
            err,
            NotApproved::Refused {
                reason: Refusal::FindingWithoutBead
            }
        );
        assert!(!err.is_escalatable());
    }

    /// TOTALITY. Every action lands in exactly one bucket, so a variant added
    /// later cannot slip through unclassified.
    #[test]
    fn every_action_is_either_approvable_or_carries_a_typed_denial() {
        for action in ALL_ACTIONS {
            match Approved::authorize(classify(ready(action))) {
                Ok(a) => assert_eq!(a.action(), action),
                Err(e) => assert!(
                    matches!(
                        e,
                        NotApproved::Required { .. } | NotApproved::Refused { .. }
                    ),
                    "{} produced an untyped denial",
                    action.as_str()
                ),
            }
        }
    }

    #[test]
    fn denial_wire_names_are_stable_kebab() {
        assert_eq!(
            NotApproved::Required {
                kind: ApprovalKind::Spend
            }
            .as_str(),
            "required"
        );
        assert_eq!(
            NotApproved::Refused {
                reason: Refusal::PacketIncomplete
            }
            .as_str(),
            "refuse"
        );
    }

    /// The authorized wave is carried verbatim. If `authorize` ever rewrote the
    /// action, the proof would attach to something the caller never classified.
    #[test]
    fn authorize_does_not_rewrite_the_wave() {
        for action in ALL_ACTIONS {
            let w = classify(ready(action));
            if let Ok(a) = Approved::authorize(w) {
                assert_eq!(a.wave(), w, "authorize mutated the wave it approved");
                assert_eq!(a.phase(), w.phase);
            }
        }
    }
}
