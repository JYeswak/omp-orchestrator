#![forbid(unsafe_code)]

//! Typed actions + approval/non-approval waves for `/ntm-fleet-monitor`.
//!
//! Bead: `cp-ntm-fleet-monitor-typed-xtask-h3chb`.
//!
//! The skill is the authority for WHAT the phases are. This crate is the authority
//! for whether a proposed action at a phase is Autonomous, Required (one of the
//! three human blockers, or the AGENTS.md gate-weaken carve-out), or Refuse
//! (safety, not taste). It classifies. It does not send, recycle, spend, or
//! deploy.
//!
//! Reuses [`loop_coverage::LoopLayer`] — do not fork the phase list.
//! Pane dispatchability is a CALLER-SUPPLIED fact from
//! `controller_tick::PaneLiveness::is_dispatchable`. This crate does not re-derive
//! Working/Frozen/Idle; that fork is what cp-rfx78 cost.
//!
//! NO-CLAIM BOUNDARY: a green `cargo test -p ntm-fleet-monitor` proves the
//! classifier matches the policy fixtures. It does not prove the next tick
//! dispatched, that a pane was idle, or that a customer was served.
//!
//! [`WaveVerdict::apply_allowed`] is a runtime check a caller may simply not
//! call. [`approved::Approved`] closes that gap at the type level: an executor
//! that takes `Approved` cannot be handed a `Required` or `Refuse` wave,
//! because no expression outside [`approved`] constructs one.

pub mod approved;
pub mod bead_lifecycle;
pub mod ntm;
pub use approved::{Approved, NotApproved};
pub use ntm::{
    parse_activity_json, ActivityError, ActivitySnapshot, AgentKind, AgentObservation,
    EvidenceFreshness, OmpVariant, Readiness, SignalState,
};

use loop_coverage::LoopLayer;

/// Schema version for the wave wire format.
pub const WAVE_SCHEMA_VERSION: u32 = 1;

/// What this crate does not prove. Rendered by `--selftest` and the xtask.
pub const NO_CLAIM_BOUNDARY: &str = "\
This crate classifies a proposed fleet action. It does not send, recycle, spend, \
or deploy. A green test run proves the Autonomous/Required/Refuse split matches \
the fixtures (including false-gate vs real blocker). It does not prove the next \
tick dispatched, that PaneLiveness was measured, or that a customer was served. \
Live cron still invokes ~/.local/bin/fleet-monitor; this crate is not that binary.";

/// A proposed fleet-loop action. Stable kebab `as_str()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypedAction {
    /// Phase 1. Fleet-wide scan. Never a mutation.
    ObserveScan,
    /// Phase 2. Pick dispatchable work. Never a send.
    SelectQueue,
    /// Phase 3. Send a complete packet to a live idle pane.
    DispatchPacket,
    /// Phase 4. Ground-truth verify (commit / bead / receipt).
    VerifyReceipt,
    /// Phase 3.5. Recycle a Frozen pane. Skill: reversible, no human approval.
    RecycleFrozen,
    /// Phase 3.5. Interrupt a Wedged queued-message pane.
    InterruptWedged,
    /// Phase 3.5. Re-`/goal` a Goal-achieved idle pane.
    ReGoalIdle,
    /// Phase -1. File the finding as a bead. The act of filing is autonomous.
    FileFindingBead,
    /// Report a finding in chat/docs with no bead. Always Refuse (PHASE -1).
    ReportFindingWithoutBead,
    /// Weaken, disable, or bypass a safety gate. Escalation-only (AGENTS.md carve-out).
    WeakenGate,
    /// Move money or incur a bill.
    SpendMoney,
    /// Issue, buy, or authorize a credential only Joshua can grant.
    ObtainCredentials,
    /// Ship to a buyer, a client, or the public.
    DeployPublic,
}

impl TypedAction {
    /// Stable kebab-case wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObserveScan => "observe-scan",
            Self::SelectQueue => "select-queue",
            Self::DispatchPacket => "dispatch-packet",
            Self::VerifyReceipt => "verify-receipt",
            Self::RecycleFrozen => "recycle-frozen",
            Self::InterruptWedged => "interrupt-wedged",
            Self::ReGoalIdle => "re-goal-idle",
            Self::FileFindingBead => "file-finding-bead",
            Self::ReportFindingWithoutBead => "report-finding-without-bead",
            Self::WeakenGate => "weaken-gate",
            Self::SpendMoney => "spend-money",
            Self::ObtainCredentials => "obtain-credentials",
            Self::DeployPublic => "deploy-public",
        }
    }

    /// Parse a kebab wire name. Unknown is None, never a silent Autonomous.
    pub fn parse(s: &str) -> Option<Self> {
        ALL_ACTIONS.iter().copied().find(|a| a.as_str() == s)
    }

    /// Skill phase this action belongs to.
    pub const fn phase(self) -> LoopLayer {
        match self {
            Self::FileFindingBead | Self::ReportFindingWithoutBead => LoopLayer::GapToBead,
            Self::ObserveScan => LoopLayer::Observe,
            Self::SelectQueue => LoopLayer::Select,
            Self::DispatchPacket => LoopLayer::Dispatch,
            Self::VerifyReceipt => LoopLayer::Verify,
            Self::RecycleFrozen | Self::InterruptWedged | Self::ReGoalIdle => LoopLayer::Keepalive,
            Self::WeakenGate | Self::SpendMoney | Self::ObtainCredentials | Self::DeployPublic => {
                LoopLayer::Dispatch
            }
        }
    }
}

/// Exhaustive action set. Anti-vacuity walks this.
pub const ALL_ACTIONS: [TypedAction; 13] = [
    TypedAction::ObserveScan,
    TypedAction::SelectQueue,
    TypedAction::DispatchPacket,
    TypedAction::VerifyReceipt,
    TypedAction::RecycleFrozen,
    TypedAction::InterruptWedged,
    TypedAction::ReGoalIdle,
    TypedAction::FileFindingBead,
    TypedAction::ReportFindingWithoutBead,
    TypedAction::WeakenGate,
    TypedAction::SpendMoney,
    TypedAction::ObtainCredentials,
    TypedAction::DeployPublic,
];

/// The three policy blockers plus the AGENTS.md gate-weaken carve-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalKind {
    Spend,
    Credentials,
    Deploy,
    GateWeaken,
}

impl ApprovalKind {
    /// Stable kebab-case wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spend => "spend",
            Self::Credentials => "credentials",
            Self::Deploy => "deploy",
            Self::GateWeaken => "gate-weaken",
        }
    }
}

/// Why a wave was refused. Safety, not taste. Never "ask Joshua".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// Caller said the pane is not Idle. Sending here is the cp-rfx78 FALSE FREE.
    PaneNotDispatchable,
    /// PHASE -1. A finding with no bead is lost.
    FindingWithoutBead,
    /// Dispatch packet missing required markers (Objective/Target/Scope/Acceptance/Stop).
    PacketIncomplete,
    /// One capture cannot separate Working from Frozen.
    SingleCaptureLiveness,
    /// Unknown kebab action. Fail closed, never Autonomous.
    UnknownAction,
}

impl Refusal {
    /// Stable kebab-case wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PaneNotDispatchable => "pane-not-dispatchable",
            Self::FindingWithoutBead => "finding-without-bead",
            Self::PacketIncomplete => "packet-incomplete",
            Self::SingleCaptureLiveness => "single-capture-liveness",
            Self::UnknownAction => "unknown-action",
        }
    }
}

/// Approval / non-approval for one wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveVerdict {
    /// Reversible local work. Proceed and record. Standing Autonomy Grant.
    Autonomous,
    /// One of the three human blockers, or gate-weaken. Do not execute.
    Required { kind: ApprovalKind },
    /// Safety refusal. Not an escalation. Fix the fact, do not ask Joshua.
    Refuse { reason: Refusal },
}

impl WaveVerdict {
    /// Stable kebab-case class name (not the inner kind).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Autonomous => "autonomous",
            Self::Required { .. } => "required",
            Self::Refuse { .. } => "refuse",
        }
    }

    /// Only Autonomous may be `--apply`'d by xtask.
    pub const fn apply_allowed(self) -> bool {
        matches!(self, Self::Autonomous)
    }
}

/// Caller-supplied facts. Liveness is measured OUTSIDE this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intent {
    pub action: TypedAction,
    /// From `PaneLiveness::is_dispatchable`. False unless the caller measured Idle.
    pub pane_dispatchable: bool,
    /// Two captures were taken. One screenshot is not a liveness claim.
    pub two_captures: bool,
    /// Dispatch packet carries Objective/Target/Scope/Acceptance/Stop.
    pub packet_complete: bool,
    /// PHASE -1: a bead exists for this finding.
    pub finding_has_bead: bool,
}

impl Intent {
    /// Conservative defaults: action set, everything else fail-closed.
    pub const fn new(action: TypedAction) -> Self {
        Self {
            action,
            pane_dispatchable: false,
            two_captures: false,
            packet_complete: false,
            finding_has_bead: false,
        }
    }
}

/// One wave: phase + action + verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wave {
    pub phase: LoopLayer,
    pub action: TypedAction,
    pub verdict: WaveVerdict,
}

/// Classify one proposed action. Pure. No I/O.
pub fn classify(intent: Intent) -> Wave {
    let action = intent.action;
    let phase = action.phase();
    let verdict = match action {
        TypedAction::WeakenGate => WaveVerdict::Required {
            kind: ApprovalKind::GateWeaken,
        },
        TypedAction::SpendMoney => WaveVerdict::Required {
            kind: ApprovalKind::Spend,
        },
        TypedAction::ObtainCredentials => WaveVerdict::Required {
            kind: ApprovalKind::Credentials,
        },
        TypedAction::DeployPublic => WaveVerdict::Required {
            kind: ApprovalKind::Deploy,
        },
        TypedAction::ReportFindingWithoutBead => WaveVerdict::Refuse {
            reason: Refusal::FindingWithoutBead,
        },
        TypedAction::DispatchPacket => {
            if !intent.two_captures {
                WaveVerdict::Refuse {
                    reason: Refusal::SingleCaptureLiveness,
                }
            } else if !intent.pane_dispatchable {
                WaveVerdict::Refuse {
                    reason: Refusal::PaneNotDispatchable,
                }
            } else if !intent.packet_complete {
                WaveVerdict::Refuse {
                    reason: Refusal::PacketIncomplete,
                }
            } else {
                WaveVerdict::Autonomous
            }
        }
        TypedAction::RecycleFrozen | TypedAction::InterruptWedged | TypedAction::ReGoalIdle => {
            if !intent.two_captures {
                WaveVerdict::Refuse {
                    reason: Refusal::SingleCaptureLiveness,
                }
            } else {
                WaveVerdict::Autonomous
            }
        }
        TypedAction::FileFindingBead
        | TypedAction::ObserveScan
        | TypedAction::SelectQueue
        | TypedAction::VerifyReceipt => WaveVerdict::Autonomous,
    };
    Wave {
        phase,
        action,
        verdict,
    }
}

/// Classify an unknown kebab. Fail closed.
pub fn classify_named(name: &str, intent: Intent) -> Wave {
    match TypedAction::parse(name) {
        Some(action) => classify(Intent { action, ..intent }),
        None => Wave {
            phase: LoopLayer::Observe,
            action: intent.action,
            verdict: WaveVerdict::Refuse {
                reason: Refusal::UnknownAction,
            },
        },
    }
}

/// Measured false-gate vs real blocker. Lexical on PURPOSE: the three categories
/// plus gate-weaken. A claim that does not name one is a false gate — Autonomous
/// to proceed from data, never Required.
///
/// Fixtures (AUTONOMY-POLICY.md 2026-08-20):
///   "purge APFS snapshots" → None (objects did not exist; never a human blocker)
///   "deploy to production" → Some(Deploy)
pub fn claimed_blocker_kind(claim: &str) -> Option<ApprovalKind> {
    let c = claim.to_ascii_lowercase();
    if c.contains("spend")
        || c.contains("purchas")
        || c.contains(" invoice")
        || c.contains("billing")
        || c.contains("pay ")
    {
        return Some(ApprovalKind::Spend);
    }
    if c.contains("credential")
        || c.contains("api key")
        || c.contains("api cred")
        || c.contains("rotate secret")
    {
        return Some(ApprovalKind::Credentials);
    }
    if c.contains("deploy")
        || c.contains("publish")
        || c.contains("public release")
        || c.contains("client production")
    {
        return Some(ApprovalKind::Deploy);
    }
    if (c.contains("weaken") || c.contains("bypass") || c.contains("disable"))
        && (c.contains("gate") || c.contains("hook") || c.contains("admission"))
    {
        return Some(ApprovalKind::GateWeaken);
    }
    None
}

/// Default action for a phase when the operator says "run this wave".
pub fn default_action(phase: LoopLayer) -> TypedAction {
    match phase {
        LoopLayer::GapToBead => TypedAction::FileFindingBead,
        LoopLayer::Conformance | LoopLayer::Observe => TypedAction::ObserveScan,
        LoopLayer::Select => TypedAction::SelectQueue,
        LoopLayer::Dispatch => TypedAction::DispatchPacket,
        LoopLayer::Keepalive => TypedAction::RecycleFrozen,
        LoopLayer::Verify => TypedAction::VerifyReceipt,
        LoopLayer::CheckIn | LoopLayer::Alignment | LoopLayer::Journey => TypedAction::ObserveScan,
    }
}

/// Plan one wave from a phase name + facts.
pub fn plan_wave(phase: LoopLayer, facts: Intent) -> Wave {
    classify(Intent {
        action: default_action(phase),
        ..facts
    })
}

/// Render one wave as a single ledger line.
pub fn render_wave(wave: Wave) -> String {
    match wave.verdict {
        WaveVerdict::Autonomous => format!(
            "wave phase={} action={} verdict=autonomous apply=yes",
            wave.phase.as_str(),
            wave.action.as_str()
        ),
        WaveVerdict::Required { kind } => format!(
            "wave phase={} action={} verdict=required kind={} apply=no",
            wave.phase.as_str(),
            wave.action.as_str(),
            kind.as_str()
        ),
        WaveVerdict::Refuse { reason } => format!(
            "wave phase={} action={} verdict=refuse reason={} apply=no",
            wave.phase.as_str(),
            wave.action.as_str(),
            reason.as_str()
        ),
    }
}

/// In-process selftest used by `#[test]` and `xtask fleet-monitor --selftest`.
/// Returns Ok(()) or the first failure string. No I/O.
pub fn selftest() -> Result<(), String> {
    let auto = |a| {
        classify(Intent {
            action: a,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        })
    };

    let want_auto = [
        TypedAction::ObserveScan,
        TypedAction::SelectQueue,
        TypedAction::VerifyReceipt,
        TypedAction::FileFindingBead,
        TypedAction::RecycleFrozen,
        TypedAction::InterruptWedged,
        TypedAction::ReGoalIdle,
        TypedAction::DispatchPacket,
    ];
    for a in want_auto {
        let w = auto(a);
        if w.verdict != WaveVerdict::Autonomous {
            return Err(format!(
                "{} want autonomous got {}",
                a.as_str(),
                w.verdict.as_str()
            ));
        }
        if !w.verdict.apply_allowed() {
            return Err(format!("{} autonomous must allow apply", a.as_str()));
        }
    }

    let req = [
        (TypedAction::SpendMoney, ApprovalKind::Spend),
        (TypedAction::ObtainCredentials, ApprovalKind::Credentials),
        (TypedAction::DeployPublic, ApprovalKind::Deploy),
        (TypedAction::WeakenGate, ApprovalKind::GateWeaken),
    ];
    for (a, k) in req {
        let w = auto(a);
        match w.verdict {
            WaveVerdict::Required { kind } if kind == k => {}
            other => {
                return Err(format!(
                    "{} want required/{} got {:?}",
                    a.as_str(),
                    k.as_str(),
                    other
                ))
            }
        }
        if w.verdict.apply_allowed() {
            return Err(format!("{} required must NOT allow apply", a.as_str()));
        }
    }

    let w = classify(Intent::new(TypedAction::ReportFindingWithoutBead));
    match w.verdict {
        WaveVerdict::Refuse {
            reason: Refusal::FindingWithoutBead,
        } => {}
        other => return Err(format!("report-without-bead want refuse got {other:?}")),
    }

    let w = classify(Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: true,
        two_captures: false,
        packet_complete: true,
        finding_has_bead: true,
    });
    match w.verdict {
        WaveVerdict::Refuse {
            reason: Refusal::SingleCaptureLiveness,
        } => {}
        other => return Err(format!("one-capture dispatch want refuse got {other:?}")),
    }

    let w = classify(Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: false,
        two_captures: true,
        packet_complete: true,
        finding_has_bead: true,
    });
    match w.verdict {
        WaveVerdict::Refuse {
            reason: Refusal::PaneNotDispatchable,
        } => {}
        other => return Err(format!("busy-pane dispatch want refuse got {other:?}")),
    }

    if claimed_blocker_kind("purge APFS snapshots + Time Machine exclusions").is_some() {
        return Err("false-gate APFS claim must not be a human blocker".into());
    }
    if claimed_blocker_kind("deploy to production") != Some(ApprovalKind::Deploy) {
        return Err("deploy claim must be Required/deploy".into());
    }

    let unknown = classify_named("not-a-real-action", Intent::new(TypedAction::ObserveScan));
    match unknown.verdict {
        WaveVerdict::Refuse {
            reason: Refusal::UnknownAction,
        } => {}
        other => return Err(format!("unknown action want refuse got {other:?}")),
    }

    // Keepalive without two captures is a screenshot, not a recycle authorization.
    // Deleting this guard used to leave cargo test GREEN — only a hand-run classify refused.
    for a in [
        TypedAction::RecycleFrozen,
        TypedAction::InterruptWedged,
        TypedAction::ReGoalIdle,
    ] {
        let w = classify(Intent::new(a));
        match w.verdict {
            WaveVerdict::Refuse {
                reason: Refusal::SingleCaptureLiveness,
            } => {}
            other => {
                return Err(format!(
                    "{} without two_captures want refuse/single-capture-liveness got {:?}",
                    a.as_str(),
                    other
                ))
            }
        }
        if w.verdict.apply_allowed() {
            return Err(format!(
                "{} without two_captures must NOT allow apply",
                a.as_str()
            ));
        }
    }

    let mut n_auto = 0;
    let mut n_req = 0;
    let mut n_ref = 0;
    for a in ALL_ACTIONS {
        match auto(a).verdict {
            WaveVerdict::Autonomous => n_auto += 1,
            WaveVerdict::Required { .. } => n_req += 1,
            WaveVerdict::Refuse { .. } => n_ref += 1,
        }
    }
    if n_auto == 0 || n_req == 0 || n_ref == 0 {
        return Err(format!(
            "anti-vacuity: need all three verdicts, got auto={n_auto} req={n_req} ref={n_ref}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_passes() {
        selftest().unwrap();
    }

    #[test]
    fn weaken_gate_is_never_autonomous() {
        let w = classify(Intent::new(TypedAction::WeakenGate));
        assert!(
            matches!(
                w.verdict,
                WaveVerdict::Required {
                    kind: ApprovalKind::GateWeaken
                }
            ),
            "AGENTS.md carve-out: weakening a gate is escalation-only, got {:?}",
            w.verdict
        );
        assert!(!w.verdict.apply_allowed());
    }

    #[test]
    fn mutation_weaken_gate_must_stay_required() {
        // If someone maps WeakenGate to Autonomous, this is the leg that goes RED.
        let w = classify(Intent {
            action: TypedAction::WeakenGate,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        });
        assert_ne!(w.verdict, WaveVerdict::Autonomous);
        assert_eq!(w.verdict.as_str(), "required");
    }

    #[test]
    fn mutation_recycle_without_two_captures_must_stay_refuse() {
        // If someone maps RecycleFrozen to Autonomous even with one capture, this goes RED.
        let w = classify(Intent::new(TypedAction::RecycleFrozen));
        assert_ne!(w.verdict, WaveVerdict::Autonomous);
        assert_eq!(w.verdict.as_str(), "refuse");
        assert_eq!(
            w.verdict,
            WaveVerdict::Refuse {
                reason: Refusal::SingleCaptureLiveness
            }
        );
    }

    #[test]
    fn every_action_has_a_unique_wire_name() {
        let mut seen = std::collections::BTreeSet::new();
        for a in ALL_ACTIONS {
            assert!(seen.insert(a.as_str()), "duplicate {}", a.as_str());
            assert_eq!(TypedAction::parse(a.as_str()), Some(a));
        }
        assert_eq!(seen.len(), ALL_ACTIONS.len());
    }

    #[test]
    fn apply_allowed_is_exactly_autonomous() {
        assert!(WaveVerdict::Autonomous.apply_allowed());
        assert!(!WaveVerdict::Required {
            kind: ApprovalKind::Spend
        }
        .apply_allowed());
        assert!(!WaveVerdict::Refuse {
            reason: Refusal::PaneNotDispatchable
        }
        .apply_allowed());
    }

    #[test]
    fn dispatch_needs_idle_and_two_captures_and_complete_packet() {
        let base = Intent {
            action: TypedAction::DispatchPacket,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        };
        assert_eq!(classify(base).verdict, WaveVerdict::Autonomous);
        assert_eq!(
            classify(Intent {
                two_captures: false,
                ..base
            })
            .verdict,
            WaveVerdict::Refuse {
                reason: Refusal::SingleCaptureLiveness
            }
        );
        assert_eq!(
            classify(Intent {
                pane_dispatchable: false,
                ..base
            })
            .verdict,
            WaveVerdict::Refuse {
                reason: Refusal::PaneNotDispatchable
            }
        );
        assert_eq!(
            classify(Intent {
                packet_complete: false,
                ..base
            })
            .verdict,
            WaveVerdict::Refuse {
                reason: Refusal::PacketIncomplete
            }
        );
    }

    #[test]
    fn false_gate_apfs_is_not_required() {
        assert_eq!(
            claimed_blocker_kind(
                "APPROVAL GATE: Joshua must dispose before purging APFS snapshots"
            ),
            None,
            "measured 2026-08-20: objects did not exist; parking this as Required is the defect"
        );
    }

    #[test]
    fn render_wave_states_apply_yes_or_no() {
        let auto = classify(Intent {
            action: TypedAction::ObserveScan,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        });
        let line = render_wave(auto);
        assert!(line.contains("verdict=autonomous"), "{line}");
        assert!(line.contains("apply=yes"), "{line}");
        let req = classify(Intent::new(TypedAction::SpendMoney));
        let line = render_wave(req);
        assert!(line.contains("verdict=required"), "{line}");
        assert!(line.contains("apply=no"), "{line}");
        assert!(line.contains("kind=spend"), "{line}");
    }
}
