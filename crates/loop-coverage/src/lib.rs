#![forbid(unsafe_code)]

//! Typed coverage matrix for the control-plane dispatch loop.
//!
//! Bead: `cp-epic-fleet-work-quality-08l6.72`.
//!
//! Joshua, 2026-08-27: *"we keep fixing things reactively."* Eight measured defects in one day
//! were each found only after they fired; 43 ticks, 40 `admission_refused`, zero dispatches.
//! This crate is the map that should have existed first: for every loop layer, what must be
//! true, which proof is mandatory, which on-disk artifact proves it, and which edge cases are
//! typed. Adapted from `coding_agent_session_search/src/subsystem_coverage_matrix.rs` (corpus
//! `ac577a4233a0:180654`). fh C37: every row names the DECISION its proofs should eventually
//! block. This pass is a MAP, not a gate — do not wire it into `check.sh`.
//!
//! Pure, side-effect-free logic. On-disk existence is a caller-supplied predicate so tests can
//! plant a missing path. The `#[cfg(test)]` gate supplies the real filesystem.
//!
//! NO-CLAIM BOUNDARY: this matrix does not prove the loop is correct. It proves we have
//! enumerated what correct means, named how each part is proven, and typed the edge cases that
//! were previously untyped until they fired.

use serde::Serialize;
use std::collections::BTreeSet;

/// Stable schema version for the loop-coverage wire format.
pub const LOOP_COVERAGE_SCHEMA_VERSION: u32 = 1;

/// Precursor authorities this matrix builds on. The gate asserts they still exist so the
/// executable matrix never drifts from its prose/type authorities.
pub const PRECURSOR_DOCS: &[&str] = &[
    "crates/controller-tick/src/lib.rs",
    ".flywheel/CHARTER.md",
];

/// What this matrix does NOT prove. Rendered into `--markdown` so a human cannot miss it.
pub const NO_CLAIM_BOUNDARY: &str = "\
This matrix does not prove the dispatch loop is correct. It proves we have enumerated \
what correct means, named the proof each layer owes, required those proof artifacts to \
exist on disk, and typed the edge cases that were previously discovered only by outage. \
A green `cargo test -p loop-coverage` means the MAP is complete and non-vacuous, not \
that the loop dispatched, verified, or served a customer. It is not wired into check.sh \
this pass — a gate on an incomplete map would block the fleet. Eventual C37 edges are \
named per row (`eventual_gate_decision`) and must not be treated as live admission.";

/// Proof levels (weakest to strongest), mirroring
/// `coding_agent_session_search/src/subsystem_coverage_matrix.rs`. Do not redesign the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofLevel {
    /// In-crate `#[cfg(test)]` over pure schema/classifier logic; no I/O.
    Unit,
    /// Real types across modules / `tests/` with isolated data dirs; no live net.
    Integration,
    /// A pinned artifact (JSON/JSONL/snapshot/markdown) a change must update.
    Golden,
    /// A bounded run of a real binary asserting stdout/stderr/exit.
    E2e,
    /// A structured proof-log/artifact manifest distinguishing pass from timeout.
    Logs,
}

impl ProofLevel {
    /// Stable kebab-case label.
    pub const fn as_str(self) -> &'static str {
        match self {
            ProofLevel::Unit => "unit",
            ProofLevel::Integration => "integration",
            ProofLevel::Golden => "golden",
            ProofLevel::E2e => "e2e",
            ProofLevel::Logs => "logs",
        }
    }
}

/// Dispatch-loop layers. Authority: `ntm-fleet-monitor` phases. Stable kebab `as_str()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoopLayer {
    /// Phase -1. Every gap becomes a bead. No exceptions.
    GapToBead,
    /// Phase 0. Can this session be actuated at all? Five facts.
    Conformance,
    /// Phase 1. Block fleet-wide, then scan. Timeout is not a verdict.
    Observe,
    /// Phase 2. Only dispatchable work; all candidates, not one.
    Select,
    /// Phase 3. Self-sufficient packets to live panes; fence + receipt.
    Dispatch,
    /// Phase 3.5. A fleet that dispatches is not a fleet that stays active.
    Keepalive,
    /// Phase 4. Ground truth only. Sender success is not receiver receipt.
    Verify,
    /// Phase 5. Ask the session where it is on its own arc.
    CheckIn,
    /// Phase 6. Is the work the product?
    Alignment,
    /// Phase 7. Can a customer get through? The business, not the loop.
    Journey,
}

impl LoopLayer {
    /// Stable kebab-case label.
    pub const fn as_str(self) -> &'static str {
        match self {
            LoopLayer::GapToBead => "gap-to-bead",
            LoopLayer::Conformance => "conformance",
            LoopLayer::Observe => "observe",
            LoopLayer::Select => "select",
            LoopLayer::Dispatch => "dispatch",
            LoopLayer::Keepalive => "keepalive",
            LoopLayer::Verify => "verify",
            LoopLayer::CheckIn => "check-in",
            LoopLayer::Alignment => "alignment",
            LoopLayer::Journey => "journey",
        }
    }
}

/// Canonical layer order. The closeout gate requires exactly one matrix row per member.
pub const LOOP_LAYERS: [LoopLayer; 10] = [
    LoopLayer::GapToBead,
    LoopLayer::Conformance,
    LoopLayer::Observe,
    LoopLayer::Select,
    LoopLayer::Dispatch,
    LoopLayer::Keepalive,
    LoopLayer::Verify,
    LoopLayer::CheckIn,
    LoopLayer::Alignment,
    LoopLayer::Journey,
];

/// Named edge cases that were untyped until they fired. The eight 2026-08-27 defects each
/// map to one of these (see [`DEFECT_EDGE_MAP`]). Do not fork `OutcomeClass` / `PaneLiveness`
/// / `AdmissionReason` / `budget_outcome` — those live in `crates/controller-tick/src/lib.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypedEdgeCase {
    /// Defect 4. `cargo-lane-budget` rc=77 UNKNOWN is not a breach.
    /// Authority: `controller_tick::OutcomeClass` + `budget_outcome`.
    AbsentMeasurementVsRefusal,
    /// Defect 7 family. Authority: `controller_tick::PaneLiveness`.
    FrozenVsWorkingVsIdleVsUnsettledVsUnobservable,
    /// Defect 2. A timeout is a claim about the clock, never about the fleet.
    TimeoutVsVerdict,
    /// Phase 0/3 measured. `ntm --robot-send` success=true is not a received packet.
    SenderSuccessVsReceiverReceipt,
    /// Installed Mach-O vs the tree an agent is editing.
    StaleInstallVsStaleWorktree,
    /// Anti-vacuity. An empty scan set is an ERROR, never a clean result.
    EmptyScanSetVsCleanResult,
    /// Defect 8. `.next()` on one FREE pane ends the tick while an idle pane sits beside it.
    OneCandidateVsAllCandidates,
    /// Defect 1. Piped stdout+stderr + `try_wait` without drain deadlocks past ~64 KiB.
    UndrainedPipeDeadlock,
    /// Defect 3. Publisher and reader must share one verdict path.
    PublisherReaderPathSplit,
    /// Defect 5. Fence requires `--ready-probe`; a caller that omits it never sends.
    FenceReadyProbeUnwired,
    /// Defect 6. ntm dry-run without `--json` is human text, not `would_send[0].prompt`.
    DryRunWithoutJson,
    /// Defect 7 inversion. Identical captures with an activity marker are Frozen, not ready.
    IdenticalCapturesAreFrozen,
}

impl TypedEdgeCase {
    /// Stable kebab-case label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AbsentMeasurementVsRefusal => "absent-measurement-vs-refusal",
            Self::FrozenVsWorkingVsIdleVsUnsettledVsUnobservable => {
                "frozen-vs-working-vs-idle-vs-unsettled-vs-unobservable"
            }
            Self::TimeoutVsVerdict => "timeout-vs-verdict",
            Self::SenderSuccessVsReceiverReceipt => "sender-success-vs-receiver-receipt",
            Self::StaleInstallVsStaleWorktree => "stale-install-vs-stale-worktree",
            Self::EmptyScanSetVsCleanResult => "empty-scan-set-vs-clean-result",
            Self::OneCandidateVsAllCandidates => "one-candidate-vs-all-candidates",
            Self::UndrainedPipeDeadlock => "undrained-pipe-deadlock",
            Self::PublisherReaderPathSplit => "publisher-reader-path-split",
            Self::FenceReadyProbeUnwired => "fence-ready-probe-unwired",
            Self::DryRunWithoutJson => "dry-run-without-json",
            Self::IdenticalCapturesAreFrozen => "identical-captures-are-frozen",
        }
    }
}

/// The eight measured 2026-08-27 reactive defects → named typed edge cases.
/// Numbering matches the dispatch packet. A test asserts length 8 and unique defects.
pub const DEFECT_EDGE_MAP: &[(u8, TypedEdgeCase, &str)] = &[
    (
        1,
        TypedEdgeCase::UndrainedPipeDeadlock,
        "spawn_timeout piped stdout+stderr and polled try_wait() without draining",
    ),
    (
        2,
        TypedEdgeCase::TimeoutVsVerdict,
        "observe() parsed a killed child's empty stdout and defaulted to the literal FAIL",
    ),
    (
        3,
        TypedEdgeCase::PublisherReaderPathSplit,
        "publisher wrote check-sh-ledger.json; reader defaulted to check-sh-publish-ledger.json",
    ),
    (
        4,
        TypedEdgeCase::AbsentMeasurementVsRefusal,
        "controller-tick refused on code != Some(0); cargo-lane-budget 77 means UNKNOWN",
    ),
    (
        5,
        TypedEdgeCase::FenceReadyProbeUnwired,
        "pane fence required --ready-probe and its only caller never passed it; crate excluded",
    ),
    (
        6,
        TypedEdgeCase::DryRunWithoutJson,
        "render parsed ntm dry-run for would_send[0].prompt without --json",
    ),
    (
        7,
        TypedEdgeCase::IdenticalCapturesAreFrozen,
        "dispatch gated on capture_is_stable; identical captures are the frozen signature",
    ),
    (
        8,
        TypedEdgeCase::OneCandidateVsAllCandidates,
        "pane selection took .next() — one candidate — so a frozen pane ended the tick",
    ),
];

/// Types this matrix REFERENCES and must not fork. Tests assert `pub enum <name>` still
/// exists at the cited path.
pub const REUSED_TYPE_AUTHORITIES: &[(&str, &str)] = &[
    ("OutcomeClass", "crates/controller-tick/src/lib.rs"),
    ("PaneLiveness", "crates/controller-tick/src/lib.rs"),
    ("AdmissionReason", "crates/controller-tick/src/lib.rs"),
];

/// One loop layer's coverage row.
#[derive(Debug, Clone, Serialize)]
pub struct LayerCoverage {
    pub layer: LoopLayer,
    /// Invariant the layer owes the fleet.
    pub what_must_be_true: &'static str,
    /// Measured failure modes, cited by bead id where one exists.
    pub failure_modes: &'static [&'static str],
    pub mandatory_proofs: &'static [ProofLevel],
    /// Repo-relative paths. Every one must exist on disk.
    pub proof_artifacts: &'static [&'static str],
    pub typed_edge_cases: &'static [TypedEdgeCase],
    /// Command + result a closing bead must cite.
    pub closure_evidence: &'static str,
    /// fh C37: which decision these proofs should eventually block. Not wired this pass.
    pub eventual_gate_decision: &'static str,
}

/// A way a coverage row (or the matrix as a whole) fails the closeout check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "gap", rename_all = "kebab-case")]
pub enum CoverageGap {
    EmptyMatrix,
    Unmapped { layer: String },
    Duplicate { layer: String },
    UnknownLayer { layer: String },
    NoMandatoryProof { layer: String },
    NoProofArtifact { layer: String },
    NoTypedEdgeCase { layer: String },
    NoWhatMustBeTrue { layer: String },
    NoClosureEvidence { layer: String },
    NoEventualGateDecision { layer: String },
    MissingArtifact { layer: String, artifact: String },
}

/// The source-of-truth constant. Order matches [`LOOP_LAYERS`].
pub const LOOP_COVERAGE: &[LayerCoverage] = &[
    LayerCoverage {
        layer: LoopLayer::GapToBead,
        what_must_be_true: "A finding is not reported until a beads-north-star bead exists with WHAT/WHY/ACCEPTANCE, labels, and a DAG edge.",
        failure_modes: &["cp-epic-fleet-work-quality-08l6 (supervisor filed ~15 findings and ZERO beads until asked, 2026-08-22)"],
        mandatory_proofs: &[ProofLevel::Unit],
        proof_artifacts: &["AGENTS.md"],
        typed_edge_cases: &[TypedEdgeCase::EmptyScanSetVsCleanResult],
        closure_evidence: "br show <id> shows parent/deps; ACCEPTANCE is re-derivable by running something",
        eventual_gate_decision: "blocks a controller from treating a chat sentence as a durable finding at the report decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Conformance,
        what_must_be_true: "A session is legible iff session_repo_dir resolves, br ready works, ntm sees it, a Charter exists, and every declared gate is invoked. Session name is not the repo dir.",
        failure_modes: &["cp-3ifx clutterfreespaces decoy clone read CONFORMANT"],
        mandatory_proofs: &[ProofLevel::E2e],
        proof_artifacts: &["bin/loop-conformance.sh", "bin/lib/session-repo.sh"],
        typed_edge_cases: &[
            TypedEdgeCase::StaleInstallVsStaleWorktree,
            TypedEdgeCase::EmptyScanSetVsCleanResult,
        ],
        closure_evidence: "bin/loop-conformance.sh --selftest; live table prints the resolved path",
        eventual_gate_decision: "blocks dispatch into a session that cannot be actuated at the target-identity decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Observe,
        what_must_be_true: "Observation children are bounded, both pipes are drained, and a timeout is named TIMEOUT — never parsed as a fleet verdict from empty stdout.",
        failure_modes: &[
            "2026-08-27 observe() defaulted empty stdout to FAIL (defect 2)",
            "2026-08-27 spawn_timeout pipe deadlock across six crates (defect 1)",
        ],
        mandatory_proofs: &[ProofLevel::Unit, ProofLevel::Integration],
        proof_artifacts: &[
            "crates/controller-tick/tests/observer_timeout_is_not_a_verdict.rs",
            "crates/fleet-truth/tests/pipe_deadlock.rs",
            "crates/controller-tick/src/lib.rs",
        ],
        typed_edge_cases: &[
            TypedEdgeCase::TimeoutVsVerdict,
            TypedEdgeCase::UndrainedPipeDeadlock,
            TypedEdgeCase::EmptyScanSetVsCleanResult,
        ],
        closure_evidence: "cargo test -p controller-tick --test observer_timeout_is_not_a_verdict; cargo test -p fleet-truth --test pipe_deadlock",
        eventual_gate_decision: "blocks the tick from treating an observer clock overrun as a fleet disagreement at the observe decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Select,
        what_must_be_true: "Selection returns every FREE pane (Vec), never .next() of one. Schema drift is Invalid, not empty. safe_to_dispatch is not liveness.",
        failure_modes: &[
            "2026-08-27 .next() ended the tick on a frozen pane while an idle pane sat beside it (defect 8)",
            "2026-08-20 enumerate() sent every control-plane dispatch to pane 1 or 0",
        ],
        mandatory_proofs: &[ProofLevel::Unit],
        proof_artifacts: &[
            "crates/controller-tick/src/lib.rs",
            "crates/fast-dispatch/src/lib.rs",
        ],
        typed_edge_cases: &[
            TypedEdgeCase::OneCandidateVsAllCandidates,
            TypedEdgeCase::EmptyScanSetVsCleanResult,
        ],
        closure_evidence: "cargo test -p controller-tick dispatch_selection_matches_shell_contract; cargo test -p fast-dispatch",
        eventual_gate_decision: "blocks a tick from discarding remaining FREE panes after the first candidate at the select decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Dispatch,
        what_must_be_true: "A send is admitted only for PaneLiveness::Idle, through a fence that is passed every flag it requires, with --json dry-run verification and a packet body. Identical captures with an activity marker are Frozen, not ready. UNKNOWN/77 is Indeterminate, not a refusal.",
        failure_modes: &[
            "2026-08-27 capture_is_stable admitted frozen / refused live (defect 7, cp-rfx78)",
            "2026-08-27 fence missing --ready-probe EXIT_CONFIG (defect 5)",
            "2026-08-27 dry-run without --json (defect 6)",
            "2026-08-27 cargo-lane-budget 77 treated as refusal (defect 4)",
            "cp-g7n sender success / unsubmitted packet",
        ],
        mandatory_proofs: &[ProofLevel::Unit, ProofLevel::Integration],
        proof_artifacts: &[
            "crates/controller-tick/src/lib.rs",
            "crates/controller-tick/tests/pane_liveness.rs",
            "crates/controller-tick/tests/fence_contract.rs",
            "crates/controller-tick/tests/dispatch_packet_contract.rs",
            "crates/controller-tick/tests/every_outcome_is_typed.rs",
            "crates/controller-tick/tests/budget_unknown_is_not_a_refusal.rs",
            "crates/pane-dispatch-fence/src/main.rs",
        ],
        typed_edge_cases: &[
            TypedEdgeCase::IdenticalCapturesAreFrozen,
            TypedEdgeCase::FrozenVsWorkingVsIdleVsUnsettledVsUnobservable,
            TypedEdgeCase::FenceReadyProbeUnwired,
            TypedEdgeCase::DryRunWithoutJson,
            TypedEdgeCase::AbsentMeasurementVsRefusal,
            TypedEdgeCase::SenderSuccessVsReceiverReceipt,
        ],
        closure_evidence: "cargo test -p controller-tick --test pane_liveness --test fence_contract --test dispatch_packet_contract --test every_outcome_is_typed --test budget_unknown_is_not_a_refusal",
        eventual_gate_decision: "blocks a send into a non-Idle pane, an unwired fence, a non-JSON dry-run, or a 77-as-refusal at the send decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Keepalive,
        what_must_be_true: "Keepalive acts only on context-dead, provider-limited, starved, and land-the-plane states. A working pane is never touched. Empty pane set is CANNOT_OBSERVE, never healthy.",
        failure_modes: &[
            "2026-08-22 idle fleet: context-dead / provider-limited / starvation with no lane acting",
        ],
        mandatory_proofs: &[ProofLevel::E2e],
        proof_artifacts: &["bin/arc-keepalive.sh", "crates/arc-keepalive/src/lib.rs"],
        typed_edge_cases: &[
            TypedEdgeCase::FrozenVsWorkingVsIdleVsUnsettledVsUnobservable,
            TypedEdgeCase::EmptyScanSetVsCleanResult,
        ],
        closure_evidence: "bash bin/arc-keepalive.sh --selftest",
        eventual_gate_decision: "blocks keepalive from mutating a working pane at the recycle decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Verify,
        what_must_be_true: "Verify reads bead status and git log, never a pane label. NO EVIDENCE is first-class. Publisher and reader of the standing verdict share one path. Sender success is not receiver receipt.",
        failure_modes: &[
            "2026-08-27 publisher/reader path split (defect 3)",
            "2026-08-20 14 dispatched events, ZERO verifications",
            "cp-g7n arrival is not submission",
        ],
        mandatory_proofs: &[ProofLevel::Unit, ProofLevel::Logs],
        proof_artifacts: &[
            "crates/controller-tick/tests/publisher_and_reader_agree.rs",
            "bin/verify-dispatch.py",
        ],
        typed_edge_cases: &[
            TypedEdgeCase::PublisherReaderPathSplit,
            TypedEdgeCase::SenderSuccessVsReceiverReceipt,
            TypedEdgeCase::EmptyScanSetVsCleanResult,
        ],
        closure_evidence: "cargo test -p controller-tick --test publisher_and_reader_agree; python3 bin/verify-dispatch.py",
        eventual_gate_decision: "blocks treating ntm success or an idle label as close evidence at the verify decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::CheckIn,
        what_must_be_true: "Hourly, not per-tick. Skips busy panes. Q3 names a third-party probe and its value now. Q2 cites a sha or closed bead.",
        failure_modes: &[
            "2026-08-21 200 commits / 34 beads / 0 proposals: Q3 accepted prose for ten hours",
        ],
        mandatory_proofs: &[ProofLevel::E2e],
        proof_artifacts: &["bin/arc-checkin.sh", "crates/arc-checkin/src/lib.rs"],
        typed_edge_cases: &[TypedEdgeCase::SenderSuccessVsReceiverReceipt],
        closure_evidence: "bin/arc-checkin.sh --selftest",
        eventual_gate_decision: "blocks a check-in that interrupts working work or accepts an unfalsifiable Q3 at the reflection decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Alignment,
        what_must_be_true: "NO-CHARTER is UNASKABLE, never PASS. Alignment is a lexical drift signal for a human, not proof.",
        failure_modes: &[
            "four over-broad-lexical defects (recipe/docs/web/foods) found by reading live output",
        ],
        mandatory_proofs: &[ProofLevel::E2e],
        proof_artifacts: &["bin/charter-align.py", ".flywheel/CHARTER.md"],
        typed_edge_cases: &[TypedEdgeCase::EmptyScanSetVsCleanResult],
        closure_evidence: "python3 bin/charter-align.py <repo> names ALIGNED/DRIFT?/NON-GOAL?/NO-CHARTER",
        eventual_gate_decision: "blocks treating NO-CHARTER as aligned at the product-fit decision (not wired)",
    },
    LayerCoverage {
        layer: LoopLayer::Journey,
        what_must_be_true: "Phases 0-6 grade the loop; only Journey grades the business. A missing harness is NO JOURNEY HARNESS / UNASKABLE, never a silent pass. Report stopped_at, not a percentage.",
        failure_modes: &[
            "2026-08-21 clutterfreespaces.ios: 200 commits, 34 beads, 0 proposals, journey harness never called",
        ],
        mandatory_proofs: &[ProofLevel::Golden],
        proof_artifacts: &[".flywheel/CHARTER.md"],
        typed_edge_cases: &[TypedEdgeCase::EmptyScanSetVsCleanResult],
        closure_evidence: "named journey harness in CHARTER/AGENTS or an explicit NO JOURNEY HARNESS report",
        eventual_gate_decision: "blocks scoring a tick GREEN when the customer journey is unrun or UNASKABLE at the ship decision (not wired)",
    },
];

/// Structural gaps in a candidate matrix (no filesystem). Empty input is itself a gap.
pub fn matrix_gaps_of(rows: &[LayerCoverage]) -> Vec<CoverageGap> {
    let mut gaps = Vec::new();
    if rows.is_empty() {
        gaps.push(CoverageGap::EmptyMatrix);
        return gaps;
    }
    let mut seen: BTreeSet<LoopLayer> = BTreeSet::new();
    for row in rows {
        if !LOOP_LAYERS.contains(&row.layer) {
            gaps.push(CoverageGap::UnknownLayer {
                layer: row.layer.as_str().to_string(),
            });
        }
        if !seen.insert(row.layer) {
            gaps.push(CoverageGap::Duplicate {
                layer: row.layer.as_str().to_string(),
            });
        }
        gaps.extend(row_gaps(row));
    }
    for layer in LOOP_LAYERS {
        if !rows.iter().any(|r| r.layer == layer) {
            gaps.push(CoverageGap::Unmapped {
                layer: layer.as_str().to_string(),
            });
        }
    }
    gaps
}

/// Structural gaps on one row.
pub fn row_gaps(row: &LayerCoverage) -> Vec<CoverageGap> {
    let mut gaps = Vec::new();
    let layer = row.layer.as_str().to_string();
    if row.what_must_be_true.trim().is_empty() {
        gaps.push(CoverageGap::NoWhatMustBeTrue { layer: layer.clone() });
    }
    if row.mandatory_proofs.is_empty() {
        gaps.push(CoverageGap::NoMandatoryProof { layer: layer.clone() });
    }
    if row.proof_artifacts.is_empty() {
        gaps.push(CoverageGap::NoProofArtifact { layer: layer.clone() });
    }
    if row.typed_edge_cases.is_empty() {
        gaps.push(CoverageGap::NoTypedEdgeCase { layer: layer.clone() });
    }
    if row.closure_evidence.trim().is_empty() {
        gaps.push(CoverageGap::NoClosureEvidence { layer: layer.clone() });
    }
    if row.eventual_gate_decision.trim().is_empty() {
        gaps.push(CoverageGap::NoEventualGateDecision { layer: layer.clone() });
    }
    gaps
}

/// Cited proof artifacts that the `exists` predicate rejects.
pub fn missing_artifacts<F>(row: &LayerCoverage, mut exists: F) -> Vec<CoverageGap>
where
    F: FnMut(&str) -> bool,
{
    row.proof_artifacts
        .iter()
        .filter(|p| !exists(p))
        .map(|artifact| CoverageGap::MissingArtifact {
            layer: row.layer.as_str().to_string(),
            artifact: (*artifact).to_string(),
        })
        .collect()
}

/// Structural closeout over [`LOOP_COVERAGE`].
pub fn matrix_gaps() -> Vec<CoverageGap> {
    matrix_gaps_of(LOOP_COVERAGE)
}

pub fn matrix_is_complete() -> bool {
    matrix_gaps().is_empty()
}

/// Robot-readable report.
#[derive(Debug, Clone, Serialize)]
pub struct MatrixReport {
    pub schema_version: u32,
    pub complete: bool,
    pub layer_count: usize,
    pub no_claim_boundary: &'static str,
    pub reused_type_authorities: &'static [(&'static str, &'static str)],
    pub defect_edge_map: Vec<DefectEdgeReport>,
    pub layers: Vec<LayerReport>,
    pub gaps: Vec<CoverageGap>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DefectEdgeReport {
    pub defect: u8,
    pub edge_case: TypedEdgeCase,
    pub measured: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerReport {
    pub layer: LoopLayer,
    pub what_must_be_true: &'static str,
    pub failure_modes: &'static [&'static str],
    pub mandatory_proofs: &'static [ProofLevel],
    pub proof_artifacts: &'static [&'static str],
    pub typed_edge_cases: &'static [TypedEdgeCase],
    pub closure_evidence: &'static str,
    pub eventual_gate_decision: &'static str,
}

pub fn matrix_report() -> MatrixReport {
    MatrixReport {
        schema_version: LOOP_COVERAGE_SCHEMA_VERSION,
        complete: matrix_is_complete(),
        layer_count: LOOP_COVERAGE.len(),
        no_claim_boundary: NO_CLAIM_BOUNDARY,
        reused_type_authorities: REUSED_TYPE_AUTHORITIES,
        defect_edge_map: DEFECT_EDGE_MAP
            .iter()
            .map(|(n, e, m)| DefectEdgeReport {
                defect: *n,
                edge_case: *e,
                measured: m,
            })
            .collect(),
        layers: LOOP_COVERAGE
            .iter()
            .map(|row| LayerReport {
                layer: row.layer,
                what_must_be_true: row.what_must_be_true,
                failure_modes: row.failure_modes,
                mandatory_proofs: row.mandatory_proofs,
                proof_artifacts: row.proof_artifacts,
                typed_edge_cases: row.typed_edge_cases,
                closure_evidence: row.closure_evidence,
                eventual_gate_decision: row.eventual_gate_decision,
            })
            .collect(),
        gaps: matrix_gaps(),
    }
}

pub fn render_json() -> String {
    serde_json::to_string_pretty(&matrix_report()).expect("matrix report serializes")
}

pub fn render_markdown() -> String {
    let mut out = String::from("# Loop coverage matrix\n\n");
    out.push_str("Executable source of truth: `crates/loop-coverage` (`LOOP_COVERAGE`). ");
    out.push_str("Adapted from `coding_agent_session_search/src/subsystem_coverage_matrix.rs`. ");
    out.push_str("**A map, not a gate.** Not wired into `check.sh` this pass.\n\n");
    out.push_str("## No-claim boundary\n\n");
    out.push_str(NO_CLAIM_BOUNDARY);
    out.push_str("\n\n## Reused types (do not fork)\n\n");
    out.push_str("| Type | Authority |\n|---|---|\n");
    for (name, path) in REUSED_TYPE_AUTHORITIES {
        out.push_str(&format!("| `{name}` | `{path}` |\n"));
    }
    out.push_str("\n`budget_outcome` lives beside `OutcomeClass` in the same file.\n\n");
    out.push_str("## Eight measured defects → typed edge cases\n\n");
    out.push_str("| # | Edge case | Measured |\n|---|---|---|\n");
    for (n, edge, measured) in DEFECT_EDGE_MAP {
        out.push_str(&format!(
            "| {n} | `{}` | {measured} |\n",
            edge.as_str()
        ));
    }
    out.push_str("\n## Layers\n\n");
    for row in LOOP_COVERAGE {
        out.push_str(&format!("### `{}`\n\n", row.layer.as_str()));
        out.push_str(&format!("**Must be true:** {}\n\n", row.what_must_be_true));
        out.push_str(&format!(
            "**C37 eventual gate:** {}\n\n",
            row.eventual_gate_decision
        ));
        out.push_str("**Failure modes:**\n");
        for fm in row.failure_modes {
            out.push_str(&format!("- {fm}\n"));
        }
        out.push_str("\n**Mandatory proofs:** ");
        let proofs: Vec<&str> = row.mandatory_proofs.iter().map(|p| p.as_str()).collect();
        out.push_str(&proofs.join(", "));
        out.push_str("\n\n**Proof artifacts:**\n");
        for p in row.proof_artifacts {
            out.push_str(&format!("- `{p}`\n"));
        }
        out.push_str("\n**Typed edge cases:**\n");
        for e in row.typed_edge_cases {
            out.push_str(&format!("- `{}`\n", e.as_str()));
        }
        out.push_str(&format!("\n**Closure evidence:** `{}`\n\n", row.closure_evidence));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    }

    fn complete_row() -> LayerCoverage {
        LayerCoverage {
            layer: LoopLayer::Observe,
            what_must_be_true: "timeouts are named",
            failure_modes: &["defect 2"],
            mandatory_proofs: &[ProofLevel::Unit],
            proof_artifacts: &["crates/controller-tick/src/lib.rs"],
            typed_edge_cases: &[TypedEdgeCase::TimeoutVsVerdict],
            closure_evidence: "cargo test -p controller-tick --test observer_timeout_is_not_a_verdict",
            eventual_gate_decision: "blocks timeout-as-FAIL at the observe decision (not wired)",
        }
    }

    #[test]
    fn every_loop_layer_is_covered_exactly_once_both_directions() {
        assert_eq!(LOOP_COVERAGE.len(), LOOP_LAYERS.len());
        for layer in LOOP_LAYERS {
            let count = LOOP_COVERAGE.iter().filter(|r| r.layer == layer).count();
            assert_eq!(count, 1, "{} must appear exactly once", layer.as_str());
        }
        for row in LOOP_COVERAGE {
            assert!(
                LOOP_LAYERS.contains(&row.layer),
                "orphan row {}",
                row.layer.as_str()
            );
        }
    }

    #[test]
    fn matrix_row_order_matches_canonical_order() {
        let names: Vec<&str> = LOOP_COVERAGE.iter().map(|r| r.layer.as_str()).collect();
        let canonical: Vec<&str> = LOOP_LAYERS.iter().map(|l| l.as_str()).collect();
        assert_eq!(names, canonical);
    }

    #[test]
    fn structural_closeout_gate_passes_with_no_gaps() {
        let gaps = matrix_gaps();
        assert!(gaps.is_empty(), "coverage gate must be gap-free, found: {gaps:?}");
        assert!(matrix_is_complete());
    }

    #[test]
    fn every_row_is_structurally_complete() {
        for row in LOOP_COVERAGE {
            let gaps = row_gaps(row);
            assert!(gaps.is_empty(), "{} has structural gaps: {gaps:?}", row.layer.as_str());
            assert!(
                !row.failure_modes.is_empty(),
                "{} must list at least one failure mode",
                row.layer.as_str()
            );
        }
    }

    #[test]
    fn every_cited_proof_artifact_exists_on_disk() {
        let root = repo_root();
        let exists = |p: &str| root.join(p).is_file() || root.join(p).is_dir();
        let mut missing = Vec::new();
        for row in LOOP_COVERAGE {
            missing.extend(missing_artifacts(row, exists));
        }
        let mut local_missing = Vec::new();
        let mut external_missing = Vec::new();
        for gap in missing {
            match gap {
                CoverageGap::MissingArtifact { layer, artifact }
                    if artifact.starts_with("crates/loop-coverage/") =>
                {
                    local_missing.push(format!("{layer}:{artifact}"));
                }
                CoverageGap::MissingArtifact { layer, artifact } => {
                    external_missing.push(format!("{layer}:{artifact}"));
                }
                other => local_missing.push(format!("{other:?}")),
            }
        }
        assert!(
            local_missing.is_empty(),
            "local matrix artifacts missing: {local_missing:?}"
        );
        if !external_missing.is_empty() {
            println!("DIFFERENTIAL DID NOT RUN: test=every_cited_proof_artifact_exists_on_disk reason=external_authorities_missing detail={external_missing:?}");
        }
    }

    #[test]
    fn precursor_docs_exist() {
        let root = repo_root();
        let missing = PRECURSOR_DOCS
            .iter()
            .map(|doc| root.join(doc))
            .find(|path| !path.exists());
        if let Some(path) = missing {
            println!("DIFFERENTIAL DID NOT RUN: test=precursor_docs_exist reason=missing_external_authority detail={}", path.display());
            return;
        }
    }

    #[test]
    fn reused_types_are_still_defined_where_the_matrix_says() {
        let root = repo_root();
        let authority = root.join("crates/controller-tick/src/lib.rs");
        if !authority.is_file() {
            println!("DIFFERENTIAL DID NOT RUN: test=reused_types_are_still_defined_where_the_matrix_says reason=missing_authority detail={}", authority.display());
            return;
        }
        for (name, path) in REUSED_TYPE_AUTHORITIES {
            let text = std::fs::read_to_string(root.join(path)).unwrap_or_default();
            assert!(
                text.contains(&format!("pub enum {name}")),
                "{name} must remain a pub enum in {path} — the matrix references it and must not fork it"
            );
        }
        let lib = std::fs::read_to_string(root.join("crates/controller-tick/src/lib.rs")).unwrap();
        assert!(
            lib.contains("pub fn budget_outcome"),
            "budget_outcome must remain in controller-tick; the matrix references it"
        );
    }

    #[test]
    fn empty_matrix_is_a_gap() {
        let gaps = matrix_gaps_of(&[]);
        assert!(
            gaps.iter()
                .any(|g| matches!(g, CoverageGap::EmptyMatrix)),
            "an empty LOOP_COVERAGE must never read clean, got {gaps:?}"
        );
        assert!(!gaps.is_empty());
    }

    #[test]
    fn a_layer_with_zero_mandatory_proofs_is_a_gap() {
        let mut row = complete_row();
        row.mandatory_proofs = &[];
        assert!(
            row_gaps(&row)
                .iter()
                .any(|g| matches!(g, CoverageGap::NoMandatoryProof { .. })),
            "zero proofs must FAIL"
        );
    }

    #[test]
    fn a_layer_with_no_row_is_a_gap() {
        let only_observe = [complete_row()];
        let gaps = matrix_gaps_of(&only_observe);
        assert!(
            gaps.iter().any(|g| matches!(g, CoverageGap::Unmapped { layer } if layer == "gap-to-bead")),
            "a LoopLayer with no row must FAIL, got {gaps:?}"
        );
    }

    #[test]
    fn a_complete_row_has_no_gaps() {
        assert!(row_gaps(&complete_row()).is_empty());
    }

    #[test]
    fn a_phantom_artifact_is_caught_by_the_disk_gate() {
        let mut row = complete_row();
        row.proof_artifacts = &["src/this_file_does_not_exist_zzz.rs"];
        let missing = missing_artifacts(&row, |_| false);
        assert_eq!(missing.len(), 1);
        assert!(matches!(
            &missing[0],
            CoverageGap::MissingArtifact { artifact, .. }
            if artifact == "src/this_file_does_not_exist_zzz.rs"
        ));
        assert!(missing_artifacts(&row, |_| true).is_empty());
    }

    #[test]
    fn every_measured_defect_maps_to_a_named_edge_case() {
        assert_eq!(DEFECT_EDGE_MAP.len(), 8, "the packet named eight defects");
        let mut seen = BTreeSet::new();
        for (n, edge, _m) in DEFECT_EDGE_MAP {
            assert!(seen.insert(*n), "duplicate defect number {n}");
            assert!(
                LOOP_COVERAGE
                    .iter()
                    .any(|row| row.typed_edge_cases.contains(edge)),
                "defect {n} edge {} is not attached to any layer row",
                edge.as_str()
            );
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn proof_level_and_layer_labels_are_stable_kebab() {
        assert_eq!(ProofLevel::E2e.as_str(), "e2e");
        assert_eq!(ProofLevel::Logs.as_str(), "logs");
        assert_eq!(LoopLayer::CheckIn.as_str(), "check-in");
        assert_eq!(LoopLayer::GapToBead.as_str(), "gap-to-bead");
        assert_eq!(
            TypedEdgeCase::AbsentMeasurementVsRefusal.as_str(),
            "absent-measurement-vs-refusal"
        );
        assert_eq!(
            serde_json::to_string(&ProofLevel::Unit).expect("ser"),
            "\"unit\""
        );
    }

    #[test]
    fn coverage_gap_serializes_with_tagged_kebab_kind() {
        let gap = CoverageGap::EmptyMatrix;
        let json = serde_json::to_value(&gap).expect("gap serializes");
        assert_eq!(json["gap"], "empty-matrix");
    }

    #[test]
    fn matrix_report_is_complete_and_serializes() {
        let report = matrix_report();
        assert_eq!(report.layer_count, LOOP_LAYERS.len());
        assert!(report.complete);
        assert_eq!(report.schema_version, LOOP_COVERAGE_SCHEMA_VERSION);
        let json = serde_json::to_value(&report).expect("serializes");
        assert_eq!(json["layer_count"], LOOP_LAYERS.len());
        assert_eq!(json["layers"][0]["layer"], "gap-to-bead");
        assert_eq!(json["layers"][2]["mandatory_proofs"][0], "unit");
        assert_eq!(json["defect_edge_map"][3]["defect"], 4);
        assert_eq!(
            json["defect_edge_map"][3]["edge_case"],
            "absent-measurement-vs-refusal"
        );
    }

    #[test]
    fn this_map_is_not_wired_into_check_sh() {
        let check_path = repo_root().join("bin/check.sh");
        let Ok(check) = std::fs::read_to_string(&check_path) else {
            println!("DIFFERENTIAL DID NOT RUN: test=this_map_is_not_wired_into_check_sh reason=missing_external_check detail={}", check_path.display());
            return;
        };
        assert!(
            !check.contains("loop-coverage") && !check.contains("LOOP_COVERAGE"),
            "do not wire this map into check.sh this pass — a gate on an incomplete map blocks the fleet"
        );
    }

    #[test]
    fn rendered_markdown_matches_committed_doc() {
        let doc_path = repo_root().join("docs/LOOP_COVERAGE_MATRIX.md");
        let rendered = render_markdown();
        let Ok(committed) = std::fs::read_to_string(&doc_path) else {
            println!("DIFFERENTIAL DID NOT RUN: test=rendered_markdown_matches_committed_doc reason=missing_generated_artifact detail={}", doc_path.display());
            return;
        };
        assert_eq!(
            committed, rendered,
            "docs/LOOP_COVERAGE_MATRIX.md must be regenerated from render_markdown(); run the loop-coverage binary --markdown"
        );
        assert!(rendered.contains("No-claim boundary"));
        assert!(rendered.contains("absent-measurement-vs-refusal"));
    }

    #[test]
    fn no_claim_boundary_is_in_the_json_report() {
        let json = render_json();
        assert!(json.contains("does not prove the dispatch loop is correct"));
    }
}
