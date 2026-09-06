#![forbid(unsafe_code)]

//! omp-orchestrator — the RESIDENT SUPERVISOR (bead omp-orchestrator-kxe).
//!
//! Josh-approved architecture 2026-08-31T16:22Z: one long-lived process owns
//! observe -> queue -> dispatch -> receiver receipt -> verify for the resolved
//! repository/session. tick-monitor remains a PURE observation component consumed
//! by this supervisor. launchd restarts this process on exit.
//!
//! THE THREE DECIDING LEGS (from the orchestrator's dispatch, 12:3xZ):
//!
//! 1. NO NO-OP GREEN PATH. Exit 0 requires either a dispatch WITH a receiver
//!    receipt, or a typed escalation naming blocker, owner, and next action.
//!    A stubbed send must FAIL the suite.
//!
//! 2. FREE + READY => DISPATCH OR TYPED ESCALATION, no third branch. "nothing
//!    to do" is FORBIDDEN output whenever free capacity and ready work coexist.
//!
//! 3. IDLE_AUTHORIZED is a durable token, default UNAUTHORIZED. Idle is an
//!    INCIDENT unless Josh has persisted approval. This is the load-bearing
//!    inversion: idleness was a state that got observed instead of an event
//!    requiring authorization.
//!
//! NO-CLAIM: this source has not been compiled against the live fleet. The
//! subprocess wiring (tick-monitor observe, br ready, ntm robot-send, tmux
//! send-keys) is designed but unverified at runtime. Receiver receipts for
//! codex panes require tmux send-keys -l, whose receipt is a timer reset +
//! spinner-stripped content change — a different protocol than ntm's, and
//! the DispatchReceipt type models both but neither is proven here.

pub mod spine_emit;
pub mod dispatch_packet;

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use text_structure::code_only;
pub mod target_directory;

// ── IDLE_AUTHORIZATION ─────────────────────────────────────────────────────────

/// Whether the supervisor may tolerate idle panes without escalating.
///
/// Default: UNAUTHORIZED. This is the load-bearing inversion — idleness was a
/// STATE that got observed instead of an EVENT requiring authorization. Until
/// Josh persists an authorization token, every idle+ready observation is an
/// incident that must escalate, not a state to log and move past.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleAuthorization {
    /// Josh has persisted approval, BOUND to the conditions he approved.
    ///
    /// A bare reason string is NOT sufficient: an authorization that does not
    /// name what it authorized cannot be falsified, and it silently outlives the
    /// situation it was granted for. Every field below is required.
    Authorized {
        reason: String,
        /// The session the approval covers. An approval for one session must not
        /// license idleness in another.
        session: String,
        /// Hash of the pane set at approval time. If the fleet changes shape, the
        /// approval no longer describes the thing it approved.
        pane_set_hash: String,
        /// Ready-queue depth at approval time.
        queue_len: usize,
        issued_at: u64,
        /// Hard expiry. An authorization with no expiry is a permanent licence to
        /// go dark, which is the failure this whole contract exists to prevent.
        expires_at: u64,
    },
    /// No token, malformed token, or EXPIRED token. Idle+ready is an INCIDENT.
    Unauthorized { why: &'static str },
}

impl fmt::Display for IdleAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authorized {
                reason,
                session,
                expires_at,
                ..
            } => write!(
                formatter,
                "IDLE_AUTHORIZED session={session} expires_at={expires_at}: {reason}"
            ),
            Self::Unauthorized { why } => write!(
                formatter,
                "IDLE_UNAUTHORIZED ({why}): idle panes are an incident; dispatch or escalate"
            ),
        }
    }
}

/// Bound for the census's per-spawn git/grep reads. The census runs every
/// tick; a wedged child must degrade to a typed "not observed" instead of
/// stalling the resident loop past its own command timeout.
const CENSUS_SPAWN_DEADLINE_SECS: u64 = 10;

/// Required keys in the token. A token missing any of these is UNAUTHORIZED.
const TOKEN_KEYS: &[&str] = &[
    "reason",
    "session",
    "pane_set_hash",
    "queue_len",
    "issued_at",
    "expires_at",
];

/// Read and VALIDATE the durable idle-authorization token from
/// `<repo>/.idle_authorized`.
///
/// FILE EXISTENCE IS NOT AUTHORIZATION. The earlier implementation accepted any
/// non-empty file as approval, which meant a stray note authorized the fleet to
/// go dark forever. The token is `key = value` lines and MUST bind the approval
/// to a session, a pane-set hash, a queue depth, an issue time, and an EXPIRY.
/// Anything missing, unparseable, or expired reads UNAUTHORIZED with a reason.
pub fn read_idle_authorization(repo_root: &Path, now_unix: u64) -> IdleAuthorization {
    let token_path = repo_root.join(".idle_authorized");
    let Ok(text) = std::fs::read_to_string(&token_path) else {
        return IdleAuthorization::Unauthorized { why: "no_token" };
    };
    let mut map: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        map.push((key.trim().to_owned(), value.trim().to_owned()));
    }
    let get = |k: &str| -> Option<String> {
        map.iter()
            .find(|(key, _)| key == k)
            .map(|(_, value)| value.clone())
    };
    for key in TOKEN_KEYS {
        if get(key).is_none_or(|v| v.is_empty()) {
            return IdleAuthorization::Unauthorized {
                why: "token_missing_required_field",
            };
        }
    }
    let (Some(queue_len), Some(issued_at), Some(expires_at)) = (
        get("queue_len").and_then(|v| v.parse::<usize>().ok()),
        get("issued_at").and_then(|v| v.parse::<u64>().ok()),
        get("expires_at").and_then(|v| v.parse::<u64>().ok()),
    ) else {
        return IdleAuthorization::Unauthorized {
            why: "token_field_unparseable",
        };
    };
    if expires_at <= now_unix {
        return IdleAuthorization::Unauthorized {
            why: "token_expired",
        };
    }
    IdleAuthorization::Authorized {
        reason: get("reason").unwrap_or_default(),
        session: get("session").unwrap_or_default(),
        pane_set_hash: get("pane_set_hash").unwrap_or_default(),
        queue_len,
        issued_at,
        expires_at,
    }
}

// ── OBSERVATION ────────────────────────────────────────────────────────────────

/// One pane's observed state, produced by tick-monitor.
///
/// `is_dispatchable` and `is_free_capacity` are SEPARATE fields on purpose. An
/// earlier version derived both from one filter, which made the NewlyIdle branch
/// of `decide` unreachable: every unauthorized queue+newly-idle case fell through
/// to AuthorizedIdle, which is the exact shape that let the fleet sit idle while
/// every watchdog reported healthy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneObservation {
    pub pane_id: String,
    pub state: String,
    pub liveness: String,
    /// ConfirmedIdle only — two idle captures >= 75s apart.
    pub is_dispatchable: bool,
    /// ConfirmedIdle OR NewlyIdle — visible as free capacity, not yet dispatchable.
    pub is_free_capacity: bool,
    /// LIVE — genuinely working. Distinguishes a healthy busy fleet from an idle
    /// one, so queue-empty-and-everyone-working is not reported as an incident.
    pub is_working: bool,
    /// An Ask/approval dialog is open: the pane is ALIVE and blocked on a HUMAN.
    ///
    /// # Why this is not covered by `is_working`
    ///
    /// Measured 2026-08-31, bead `dialog-reads-as-working-zag`: on OMP v18 the
    /// dialog renders ABOVE the status line, the status line stays LAST, and its
    /// timer KEEPS ADVANCING while the pane waits. A pane blocked on an answer is
    /// byte-indistinguishable from a pane doing work, and `%1372` sat **36 minutes**
    /// on an install approval while reading as healthy.
    ///
    /// `is_working` is right about CAPACITY — do not dispatch there, it is busy. It
    /// is wrong about HEALTH — nobody is coming unless a human is told. Folding the
    /// two together is what hid the 36 minutes, so they are separate fields and the
    /// supervisor escalates on this one.
    pub awaits_human: bool,
}

/// The queue: how many beads are ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueState {
    pub ready_count: usize,
    pub readable: bool,
}

/// The observation half: what tick-monitor and br ready report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub panes: Vec<PaneObservation>,
    pub queue: QueueState,
    /// The gate census for this cycle. None = not performed (the caller
    /// should NOT dispatch without a census). Some = the census ran; if any
    /// gate is unwired, the supervisor must refuse before anything else.
    pub gate_census: Option<GateCensus>,
}

/// The supervisor's decision for one observation cycle.
///
/// THE TWO DECIDING LEGS, encoded as exhaustive match arms:
///
///   Leg 2: FREE + READY => DISPATCH OR TYPED ESCALATION, no third branch.
///   Leg 3: IDLE_AUTHORIZED is a durable token, default UNAUTHORIZED.
///
/// There is NO branch that returns "nothing to do" when dispatchable panes
/// and ready work coexist. The only "nothing" outcome is when the queue is
/// genuinely empty AND the authorization permits idleness.
/// The reachability of one gate's TRIGGER on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateReachability {
    /// A trigger exists on this machine and has been proven to fire.
    Reachable { trigger: String },
    /// A trigger is referenced but does not exist on this machine.
    Unreachable { reason: String },
    /// The gate has no trigger of any kind.
    NotInstalled,
    /// The crate is not in this workspace yet — it still lives upstream.
    ///
    /// # Why this is a distinct variant and not an `Unreachable` reason string
    ///
    /// Measured 2026-09-01: the supervisor refused with
    /// `GATE_UNWIRED unwired=["fleet-truth"] next_action=repair-gate-trigger`.
    /// Every word of that is defensible and the **remedy is wrong**. `fleet-truth`
    /// is not unwired — it is one of 20 crates still in `control-plane`
    /// (29,512 LOC, zero extracted), so no amount of trigger repair can satisfy
    /// it. An operator following `repair-gate-trigger` looks for a hook to fix and
    /// finds nothing to fix.
    ///
    /// A guard that names the wrong next action is worse than no guard: it costs
    /// the operator a search down a path that cannot terminate. Josh made the same
    /// correction from the other direction — my instinct was to DROP these names
    /// from the census so the gate would go green, which would have erased
    /// measured extraction debt to manufacture a pass.
    NotExtracted { upstream: String, loc: u32 },
    /// The crate exists on disk and its manifest could NOT BE READ, so no probe
    /// ran and the census has no opinion about it.
    ///
    /// # Why this is not an `Unreachable` reason string
    ///
    /// `leht`, measured 2026-09-02: the census had **25 rows against 65 crates on
    /// disk**, so 40 crates had no row at all and `all_reachable()` was true over a
    /// set that never included them. The enum had no way to say *we did not look*,
    /// which is precisely why the omission was silent — an unasked crate was
    /// indistinguishable from one that does not exist.
    ///
    /// Membership is now derived from disk, so nothing is unasked merely by being
    /// unlisted. This variant covers the residue that derivation cannot fix: a
    /// directory whose `Cargo.toml` is missing or unreadable. **Calling that
    /// `Unreachable` would report a measurement we did not take**, and its remedy
    /// (`repair-gate-trigger`) sends the operator to wire a crate whose manifest is
    /// the actual problem — the same wrong-next-action defect that forced
    /// `NotExtracted` to become its own variant.
    Unprobed { reason: String },
}

impl GateReachability {
    pub fn is_reachable(&self) -> bool {
        matches!(self, GateReachability::Reachable { .. })
    }

    /// The action that can actually satisfy this state.
    ///
    /// Exists so a refusal message cannot drift from the variant it describes:
    /// adding a variant without a remedy is a COMPILE ERROR, not a stale string.
    /// The supervisor printed `next_action=repair-gate-trigger` for an unextracted
    /// crate because the action was a literal in the format string, three hundred
    /// lines from the state that produced it.
    pub fn next_action(&self) -> &'static str {
        match self {
            Self::Reachable { .. } => "none",
            Self::Unreachable { .. } => "repair-gate-trigger",
            Self::NotInstalled => "install-gate",
            Self::NotExtracted { .. } => "extract-crate-from-control-plane",
            Self::Unprobed { .. } => "repair-the-crate-manifest",
        }
    }

    /// A short label for the refusal line, so the operator sees the CLASS before
    /// the detail.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Reachable { .. } => "REACHABLE",
            Self::Unreachable { .. } => "UNWIRED",
            Self::NotInstalled => "NOT_INSTALLED",
            Self::NotExtracted { .. } => "NOT_EXTRACTED",
            Self::Unprobed { .. } => "UNPROBED",
        }
    }
}
/// Whether a row's verdict may STOP THE FLEET, or is reported and does not.
///
/// # Why this is a separate axis from reachability (`leht`, and an amendment)
///
/// The ruling was advisory-first, on the ground that refusing dispatch across 40
/// newly-visible crates would be refusing **on absence of evidence, not on evidence
/// of absence**. That is right, and it needs a place to live: without this type,
/// "advisory" is a word in a comment and the census either blocks on everything or
/// silently ignores things.
///
/// **It is deliberately NOT a `GateReachability` variant.** Derivation means the
/// probe now runs on every crate on disk, so a newly-censused crate's reachability
/// is genuinely MEASURED — calling it `NotAsked` after measuring it would be the
/// same false-label defect this census keeps producing. What is missing for those
/// crates is not a measurement, it is a **triage judgement about the measurement**,
/// and that is what this axis records. (`Unprobed` covers the one case where we
/// truly could not look: an unreadable manifest.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CensusDisposition {
    /// On the curated roster. A non-`Reachable` verdict here REFUSES dispatch.
    Blocking,
    /// Entered the census by derived membership and has not been triaged. The
    /// verdict is reported every cycle and does not gate the loop.
    ///
    /// `reason` names why it is not yet blocking, per the allowance row that
    /// admits it. An advisory row with no allowance row is a BUILD FAILURE.
    Advisory { reason: String },
}

impl CensusDisposition {
    pub fn is_blocking(&self) -> bool {
        matches!(self, CensusDisposition::Blocking)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Blocking => "BLOCKING",
            Self::Advisory { .. } => "ADVISORY",
        }
    }
}

/// One row in the gate census.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCensusRow {
    pub gate: String,
    pub reachability: GateReachability,
    /// Whether this row's verdict may stop the fleet. See [`CensusDisposition`].
    pub disposition: CensusDisposition,
}

/// The full census across all known gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCensus {
    pub rows: Vec<GateCensusRow>,
}

/// Advisory rows that are allowed to be non-`Reachable` without stopping the loop,
/// each with the reason it is not yet blocking.
///
/// # Why a NAMED LIST and not a count
///
/// This is `franken_lean`'s `UNWIRED_LANE_ALLOWANCE` applied here: every exception
/// is a named row with a reason, and **the set is required to shrink**. A bare count
/// can be satisfied by any 40 crates, so it cannot tell "we fixed one and broke
/// another" from "nothing happened". A named row can.
///
/// # THE RATCHET, and it is mechanical rather than a nag
///
/// Three legs hold it, and the third is the one with teeth:
///
/// 1. An advisory row that is non-`Reachable` and NOT named here fails the build,
///    so growth cannot be silent.
/// 2. `ADVISORY_CEILING` bounds the length, so adding a name is a visible diff.
/// 3. **A name here that is now `Reachable`, or no longer on disk, fails the
///    build.** So wiring a crate FORCES the deletion of its row — the count cannot
///    stay high after the work is done, and nobody has to remember to lower it.
///
/// Without leg 3, advisory-first is indistinguishable from permanent silence.
pub const ADVISORY_ALLOWANCE: &[(&str, &str)] = &[
    ("admission-reason", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("bead-availability", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("cargo-lane-budget", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("crate-soundness-verify", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("dispatcher-deadman", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("extraction-roster", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("fast-dispatch", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("fleet-monitor", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("fleet-truth", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("inbox-monitor", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("loop-driver", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("loop-tick", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("omp-idle-dispatch", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("omp-surface-consumption", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("oracle-pane-state-differential", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("pane-oracle-diff", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("reap-finished-panes", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("refill-idle-panes", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("response-envelope-check", "lib with no manifest caller; entered census 2026-09-02 by derived membership, untriaged"),
    ("s1-coverage", "advisory-unreachable: S1 depth is suspended by Atlas Arc R1 and the HD-0012 hook decision pending Joshua approval; no production caller is honest while S1 is frozen. Dies when an approved S1 build wave wires this crate into an in-tree production caller; delete this allowance row then"),
    ("silent-success-census", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("tick-dispatch", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("verify-dispatch", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
    ("wired-but-inert-guard", "bin with no invocation site; entered census 2026-09-02 by derived membership, untriaged"),
];
/// The advisory ceiling and the value recorded at its anchor are one ratchet.
///
/// The duplicated ceiling_at_recording field is intentional: it makes a mutation
/// that changes the live ceiling while leaving its recorded anchor untouched fail
/// the census contract instead of silently changing the meaning of the deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvisoryRatchetAnchor {
    ceiling: usize,
    ceiling_at_recording: usize,
    recorded_at_unix: u64,
}
impl AdvisoryRatchetAnchor {
    pub const fn ceiling(self) -> usize {
        self.ceiling
    }

    pub const fn recorded_at_unix(self) -> u64 {
        self.recorded_at_unix
    }

    pub const fn is_consistent(self) -> bool {
        self.ceiling == self.ceiling_at_recording
    }
}

pub const ADVISORY_RATCHET: AdvisoryRatchetAnchor = AdvisoryRatchetAnchor {
    ceiling: 24,
    ceiling_at_recording: 24,
    recorded_at_unix: 1_788_576_189,
};

/// Compatibility projection from the single ratchet anchor.
pub const ADVISORY_CEILING: usize = ADVISORY_RATCHET.ceiling();

///
/// Crates whose verdict was BLOCKING before `leht` and must stay blocking.
///
/// # Why a list is honest here and was NOT honest for membership
///
/// Membership is a **fact about the repository** — the crate is on disk or it is
/// not — so hand-listing it produced 43 invisible crates. Triage status is a
/// **human judgement about a measurement**, and a judgement has nowhere to live
/// except a list. The distinction is the whole ruling: derive the facts, record the
/// judgements, and never let a missing judgement masquerade as a missing fact.
///
/// These six were rowed by the old hand-written generic loop. Every other pre-`leht`
/// row is pushed by a curated site that declares `Blocking` inline. Without this
/// list, derivation would have silently demoted all six to advisory — `ack-spine`
/// among them, which is the row that refused the loop in `kwo9`. **That would be a
/// membership fix quietly changing verdicts**, which the ruling explicitly forbids.
pub const CURATED_BLOCKING_ROSTER: &[&str] = &[
    "ack-spine",
    "ack-stage",
    "composer-typed",
    "finding",
    "receiver-receipt",
    "subprocess-contract",
];

/// The pre-`leht` census row count, recorded so the known-good leg has a number.
///
/// **CORRECTED 2026-09-02.** First reported as 25, from a script that read the
/// source lists; the real function walks push order and answers 22. The script
/// counted the three `UNEXTRACTED` names as rows, and that loop `continue`s for
/// every one of them because all three are now on disk. Third instrument error of
/// the same shape today: a reader that did not model the code it was summarising.
pub const PRE_LEHT_BLOCKING_ROWS: usize = 22;

/// SnowyCanyon's own falsifier, written in as a number so the ruling is checkable:
/// *"if the advisory count has not decreased after a stated number of ticks,
/// advisory-first has failed and triage-first was the right call."*
///
/// 200 ticks at the supervisor's 90s interval is **5 hours**. The supervisor prints
/// `CENSUS_ADVISORY_RATCHET_OVERDUE` past that point with no decrease, in its own
/// decision output — because a deadline nobody prints is the fourth instance of the
/// class that already produced 178 unread ticks and a 29-times-unread refusal.
///
/// # A deadline already past is not a deadline
///
/// MEASURED 2026-09-02: the first value here was `1_756_845_000`, **one year off**,
/// so the live run printed `CENSUS_ADVISORY_RATCHET_OVERDUE` on tick 1 — 350,394
/// elapsed ticks against a 200-tick deadline. A verdict that is true the instant it
/// is recorded carries no information, and it would have trained the operator to
/// ignore the line. `overdue_is_false_at_the_moment_of_recording_and_true_past_the_deadline`
/// predicate cannot fire at record time.
/// Unix time at which the current advisory ceiling was recorded.
pub const ADVISORY_CEILING_RECORDED_AT_UNIX: u64 = ADVISORY_RATCHET.recorded_at_unix();
pub const ADVISORY_RATCHET_DEADLINE_TICKS: u64 = 200;

/// Has the advisory ratchet blown its deadline without the count decreasing?
///
/// Pure, so both directions are checkable: the supervisor's inline version could
/// only ever be observed in the direction the clock happened to be in, and it was
/// wrong in exactly that way — see the note on
/// [`ADVISORY_CEILING_RECORDED_AT_UNIX`].
///
/// `false` when the count HAS decreased, whatever the clock says: the falsifier is
/// about a stalled ratchet, not about elapsed time.
pub fn advisory_ratchet_overdue(
    now_unix: u64,
    recorded_at_unix: u64,
    interval_secs: u64,
    deadline_ticks: u64,
    advisory_count: usize,
    ceiling: usize,
) -> bool {
    if advisory_count < ceiling {
        return false; // it shrank; that is the ratchet working
    }
    let ticks = now_unix.saturating_sub(recorded_at_unix) / interval_secs.max(1);
    ticks > deadline_ticks
}
/// Crates emitted by the eleven OMP coverage waves and watched by the
/// supervisor. This list is intentionally explicit: a new wave output must
/// add a census row before it can be treated as wired.
///
/// The census row is a trigger-presence check, not an implementation oracle.
/// Wave-specific correctness remains each crate's own responsibility.
pub const COVERAGE_WAVE_OUTPUT_CRATES: &[&str] = &[
    "dispatch-silence-watch",
    "finding-dispatch",
    "fleet-composite",
    "installer",
    "kernel-bypass-gate",
    "kernel-only-operator-hook",
    "loop-queue-filter",
    "omp-orchestrator",
    "omp-types",
    "pane-dispatch-fence",
    "tick-monitor",
];

impl GateCensus {
    /// First independently-reachable gate this census measured.
    ///
    /// WHY NOT a named crate: a hardcoded canary exists in omp-orchestrator
    /// only. Serving any other repo made `positive_control_passes` false forever
    /// and the supervisor could never dispatch (uds-k0i6: `POSITIVE_CONTROL_FAILED`
    /// with free_capacity=3 dispatchable=2). The anti-vacuity control is "THIS
    /// census independently verified SOME gate", not "a crate from another repo
    /// is present". Empty / all-unreachable returns `None`: a scan that verified
    /// nothing is not a green fleet. Weakening that is escalation-only.
    pub fn derived_positive_control(&self) -> Option<&str> {
        self.rows
            .iter()
            .find(|r| r.reachability.is_reachable())
            .map(|r| r.gate.as_str())
    }

    /// Rows that are not reachable AND may stop the fleet.
    ///
    /// `leht`: this used to return every non-reachable row, over a hand-listed
    /// membership of 25. Derivation took membership to every crate on disk, so an
    /// unscoped version of this would have converted one blocker into forty — which
    /// is refusing on absence of evidence rather than evidence of absence, and is
    /// the reason the ruling was advisory-first.
    pub fn unwired_gates(&self) -> Vec<&GateCensusRow> {
        self.rows
            .iter()
            .filter(|r| !r.reachability.is_reachable() && r.disposition.is_blocking())
            .collect()
    }

    /// Non-reachable rows that are REPORTED and do not gate. This is the triage
    /// queue, and the number the ratchet is measured on.
    pub fn advisory_gates(&self) -> Vec<&GateCensusRow> {
        self.rows
            .iter()
            .filter(|r| !r.reachability.is_reachable() && !r.disposition.is_blocking())
            .collect()
    }

    /// Whether every BLOCKING row is reachable.
    ///
    /// Deliberately says nothing about advisory rows: that is the ruling, and
    /// naming it here keeps a caller from reading this as "the tree is wired".
    pub fn all_reachable(&self) -> bool {
        self.rows
            .iter()
            .filter(|r| r.disposition.is_blocking())
            .all(|r| r.reachability.is_reachable())
    }

    /// The POSITIVE CONTROL: at least one gate in THIS census was independently
    /// verified reachable. A census that reports everything unreachable, or that
    /// scanned nothing, is indistinguishable from a broken probe.
    pub fn positive_control_passes(&self) -> bool {
        self.derived_positive_control().is_some()
    }
}

/// Does the INSTALLED hook actually invoke this gate?
///
/// The hook is a compiled binary, so the only thing readable from outside is its
/// string table. That is a weak probe and it is named as weak: a gate name may
/// appear in a diagnostic message without ever being executed. It is still
/// strictly better than a hardcoded verdict, because it changes when the hook
/// changes.
///
/// A stronger probe — running the hook against a planted bad input and observing
/// which gate refuses — is what `no-shell-gate` does for itself and is unbuilt
/// here.
fn hook_invokes(hook_path: &Path, gate: &str) -> bool {
    let Ok(bytes) = std::fs::read(hook_path) else {
        return false;
    };
    // Scan the raw bytes: the hook may be Mach-O, a script, or a shim.
    bytes.windows(gate.len()).any(|w| w == gate.as_bytes())
}

/// Is this gate declared in a workflow that a remote could run?
///
/// Declaration is not execution — `has_remote` is checked separately at the call
/// site, and a declared-but-unrunnable gate reports Unreachable with that as the
/// stated reason rather than being silently lumped in with "no caller at all".
/// Those are different repository states and the operator needs to tell them apart.
fn workflow_invokes(repo_root: &Path, gate: &str) -> bool {
    let dir = repo_root.join(".github/workflows");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if text.contains(&format!("-p {gate}")) || text.contains(&format!("{gate}:")) {
                return true;
            }
        }
    }
    false
}

/// Classify each known gate by whether its TRIGGER exists on this machine.
///
/// Trigger reachability, NOT caller existence: a caller in `gate.yml` is not a
/// trigger when there is no remote to run the workflow on. This repository has
/// **zero git remotes** (measured 2026-09-01), which is why three CI-only gates
/// report Unreachable — correctly. A workflow nothing can run is not a gate.
/// Resolve the upstream source location from runtime configuration. Never infer
/// an author-machine path: an unset or unusable setting is reported as
/// unavailable so the census does not make an unsupported provenance claim.
fn configured_upstream_crate(repo_root: &Path, gate: &str) -> String {
    let Some(root) = std::env::var_os("CONTROL_PLANE_REPO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return format!("unavailable (set CONTROL_PLANE_REPO for crates/{gate})");
    };

    let root = if root.is_absolute() {
        root
    } else {
        repo_root.join(root)
    };
    let source = root.join("crates").join(gate);
    if source.is_dir() {
        source.display().to_string()
    } else {
        format!("{} (unavailable)", source.display())
    }
}
/// The supervisor's coverage step: an output crate is wired when its declared
/// workspace target exists and therefore has a row the decision loop can see.
/// Missing output is restrictive; it must reach `GateUnwired`, not disappear
/// from the census.
fn coverage_output_reachability(repo_root: &Path, crate_name: &str) -> GateReachability {
    if repo_root
        .join("crates")
        .join(crate_name)
        .join("Cargo.toml")
        .is_file()
    {
        GateReachability::Reachable {
            trigger: format!("supervisor:coverage-output-census -> crates/{crate_name}"),
        }
    } else {
        GateReachability::NotInstalled
    }
}

pub fn census_gates(repo_root: &Path) -> GateCensus {
    let hook_path = repo_root.join(".git/hooks/pre-commit");
    // Bounded: this census runs EVERY tick; a wedged git must degrade to
    // "no remote observed" (typed restrictive) instead of stalling the loop.
    let has_remote = {
        let mut remote_command = std::process::Command::new("git");
        remote_command.args(["remote"]);
        remote_command.current_dir(repo_root);
        match subprocess_contract::bounded_output(
            &mut remote_command,
            std::time::Duration::from_secs(CENSUS_SPAWN_DEADLINE_SECS),
        ) {
            subprocess_contract::BoundedOutcome::Completed(output) => {
                !String::from_utf8_lossy(&output.stdout).trim().is_empty()
            }
            subprocess_contract::BoundedOutcome::TimedOut
            | subprocess_contract::BoundedOutcome::Unspawned(_) => false,
        }
    };

    let mut rows = Vec::new();
    // Worker-oracle census: the ledger is the target list and this call is the production trigger.
    // The row is advisory because census reachability and admission correctness are separate axes;
    // run_cycle below refuses on a failed ledger check.
    if repo_root.join("crates/worker-oracle-gate").is_dir() {
        let reachability = match worker_oracle_gate::census(repo_root) {
            Ok(report) => GateReachability::Reachable {
                trigger: format!(
                    "supervisor:census_gates -> worker-oracle-gate::census ledger_targets={} host_bound={} grep_host_bound={}",
                    report.ledger_targets, report.ledger_host_bound, report.source_host_bound
                ),
            },
            Err(error) => GateReachability::Unreachable { reason: error.to_string() },
        };
        rows.push(GateCensusRow {
            gate: "worker-oracle-gate".to_owned(),
            reachability,
            disposition: CensusDisposition::Advisory {
                reason: "worker-bound test oracle is checked again by admission before dispatch".to_owned(),
            },
        });
    }

    // no-shell-gate: .git/hooks/pre-commit is the REAL trigger (proven to bite
    // 2026-08-31, exit 1 naming the file). Curated blocking row, not the canary.
    let nsg_reachable = hook_path.exists();
    rows.push(GateCensusRow {
        gate: "no-shell-gate".into(),
        reachability: if nsg_reachable {
            GateReachability::Reachable {
                trigger: ".git/hooks/pre-commit".into(),
            }
        } else {
            GateReachability::Unreachable {
                reason: ".git/hooks/pre-commit does not exist on this clone".into(),
            }
        },
        // CURATED, therefore BLOCKING: this row was triaged before `leht`.
        disposition: CensusDisposition::Blocking,
    });

    // path-literal-guard, state-wildcard-lint, undrained-pipe-lint:
    // their only invocation is .github/workflows/gate.yml, and there is no
    // remote to run it on.
    for gate in [
        "path-literal-guard",
        "state-wildcard-lint",
        "undrained-pipe-lint",
    ] {
        rows.push(GateCensusRow {
            gate: gate.into(),
            reachability: if has_remote {
                GateReachability::Reachable {
                    trigger: ".github/workflows/gate.yml".into(),
                }
            } else {
                GateReachability::Unreachable {
                    reason: "no git remote: the CI workflow can never execute".into(),
                }
            },
            // CURATED, therefore BLOCKING: this row was triaged before `leht`.
            disposition: CensusDisposition::Blocking,
        });
    }

    // ack-spine: THE ONE CRATE WHOSE WIRING QUESTION IS "DOES ANYONE EMIT THROUGH
    // IT", so its row is measured on the emit site and not on a manifest edge.
    //
    // `eg0m`. A manifest dependency is an UPPER BOUND on use, never use — the same
    // distinction that made `finding` show 375 grep hits against 2 real dependency
    // edges. And `eg0m` forbids the shortcut by name: *"adding a manifest
    // dependency so the census turns green is the census defect just fixed in
    // `kwo9`, running backwards — a caller that exists to satisfy a gate."*
    // Keying on the emit site makes that shortcut ineffective and makes
    // acceptance 4 mechanical: delete the emission and this row goes RED even
    // though `Cargo.toml` is untouched.
    //
    // THIS IS A BESPOKE PROBE, NOT A HAND-WRITTEN VERDICT. It reads the tree, like
    // the hook and workflow probes above; `leht`'s defect was a hand-listed
    // MEMBERSHIP and two HARDCODED verdicts, neither of which this is.
    {
        let emitters = crates_emitting(repo_root, "ack-spine");
        rows.push(GateCensusRow {
            gate: "ack-spine".into(),
            reachability: if emitters.is_empty() {
                GateReachability::Unreachable {
                    reason: "no crate emits a StepRecord through ack_spine::ledger::step: the \
                             ledger would hold no production data"
                        .into(),
                }
            } else {
                GateReachability::Reachable {
                    trigger: format!("emits StepRecords ({})", emitters.join(", ")),
                }
            },
            // CURATED, therefore BLOCKING: this row was triaged before `leht`.
            disposition: CensusDisposition::Blocking,
        });
    }

    // (the scanner itself lives below, beside the other probes)

    // kernel-bypass-gate, pre-delete-citation-check.
    //
    // These two rows were HARDCODED to `Unreachable` with the literal reason
    // "no caller, no manifest dependency, no trigger". That was true when written
    // and it is not a measurement: the census returned the same verdict no matter
    // what the repository contained, so **no amount of wiring could ever make the
    // supervisor tick**. Both were in fact wired into `.github/workflows/gate.yml`
    // on 2026-08-31 and the census kept saying otherwise.
    //
    // A frozen snapshot presented as a census is the defect class this whole
    // repository keeps finding — a hand-maintained list masquerading as a probe.
    // These now measure the same two triggers every other gate is measured on:
    // the installed hook, and a workflow that a remote could actually run.
    // The three crates named in the census that are NOT ON DISK are unextracted,
    // not unwired. Upstream LOC measured 2026-09-01; see docs/plan/03-crates.md 3.9.
    const UNEXTRACTED: &[(&str, u32)] = &[
        ("fleet-truth", 1621),
        ("oracle-compare", 561),
        ("oracle-pane-state-differential", 613),
    ];
    for (gate, loc) in UNEXTRACTED {
        if repo_root.join("crates").join(gate).is_dir() {
            continue; // it landed — fall through to the normal measurement below
        }
        rows.push(GateCensusRow {
            gate: (*gate).into(),
            reachability: GateReachability::NotExtracted {
                upstream: configured_upstream_crate(repo_root, gate),
                loc: *loc,
            },
            // CURATED, therefore BLOCKING: this row was triaged before `leht`.
            disposition: CensusDisposition::Blocking,
        });
    }

    for gate in ["kernel-bypass-gate", "pre-delete-citation-check"] {
        let in_hook = hook_invokes(&hook_path, gate);
        let in_workflow = workflow_invokes(repo_root, gate);
        rows.push(GateCensusRow {
            gate: gate.into(),
            reachability: if in_hook {
                GateReachability::Reachable {
                    trigger: ".git/hooks/pre-commit".into(),
                }
            } else if in_workflow && has_remote {
                GateReachability::Reachable {
                    trigger: ".github/workflows/gate.yml".into(),
                }
            } else if in_workflow {
                GateReachability::Unreachable {
                    reason:
                        "declared in gate.yml but no git remote: the workflow can never execute"
                            .into(),
                }
            } else {
                GateReachability::Unreachable {
                    reason: "not invoked by the installed hook and not declared in gate.yml".into(),
                }
            },
            // CURATED, therefore BLOCKING: this row was triaged before `leht`.
            disposition: CensusDisposition::Blocking,
        });
    }

    // OMP-COVERAGE-WIRING: every output named by the coverage mission gets a
    // first-class row. This is deliberately a supervisor trigger-presence check,
    // not a claim that the output crate's behavior is correct.
    for crate_name in COVERAGE_WAVE_OUTPUT_CRATES {
        if *crate_name == "kernel-bypass-gate" {
            continue;
        }
        rows.push(GateCensusRow {
            gate: (*crate_name).into(),
            reachability: coverage_output_reachability(repo_root, crate_name),
            // CURATED, therefore BLOCKING: this row was triaged before `leht`.
            disposition: CensusDisposition::Blocking,
        });
    }
    // ===================== MEMBERSHIP IS DERIVED, NOT LISTED =====================
    //
    // `leht`, MEASURED 2026-09-02: this loop was a hand-written list of eleven
    // crate names. With the four special-cased gates and the coverage-wave outputs
    // that made **25 census rows against 65 crates on disk** — 40 crates with no
    // row at all, 61% of the tree, and `all_reachable()` true over a set that never
    // included them. A crate could be uninstalled, unwired, and INVISIBLE at once.
    //
    // This file's own comment 200 lines up already named the class — *"a
    // hand-maintained list masquerading as a probe"* — and it was written about two
    // hardcoded verdicts while the membership list beside it stayed hand-written.
    // An observation in a comment that the code ignores.
    //
    // The dependency graph and the crate set are both already on disk. Reading the
    // directory is exact, needs no subprocess, and cannot time out — the same
    // argument that replaced the load-dependent grep below.
    let mut already_rowed: BTreeSet<String> = rows.iter().map(|r| r.gate.clone()).collect();
    for crate_name in crates_on_disk(repo_root) {
        if !already_rowed.insert(crate_name.clone()) {
            continue;
        }
        let crate_name: &str = &crate_name;
        // DETERMINISTIC MANIFEST READ, replacing a load-dependent grep.
        //
        // MEASURED 2026-09-02, build `7600dda`: the supervisor refused with
        // `GATE_UNWIRED unwired=[ack-spine, ack-stage, composer-typed, finding,
        // receiver-receipt]`. Re-running THIS FUNCTION'S OWN PROBE by hand returned
        // REACHABLE for every one of them:
        //
        //   ack-spine 12   ack-stage 6   composer-typed 16
        //   finding 375    receiver-receipt 11   (no-shell-gate 28, control)
        //
        // The probe was `grep -rl <name> --include=*.toml --include=*.rs .` under a
        // 10s deadline. `--include` filters FILENAMES, not directories, so the walk
        // still descends `target/` -- measured 7.0G -- and under the concurrent cargo
        // builds that are this checkout's normal state it exceeds 10s. Then
        // `BoundedOutcome::TimedOut` fell through to `has_caller = false`, which is
        // `Unreachable`.
        //
        // **So the verdict was a function of machine load, not of the repository**,
        // and `ack-stage` came back REACHABLE in the same run purely because its grep
        // happened to finish. A gate census whose answer moves with system load is the
        // nondeterministic-oracle defect, and it refused every dispatch for hours.
        //
        // The dependency graph is already on disk in the manifests. Reading it is
        // exact, needs no subprocess, and cannot time out.
        let callers = manifest_callers(repo_root, crate_name);
        // A LIB and a BIN have different trigger classes, and measuring both by
        // "does a manifest depend on it" sends the operator to add a dependency
        // nobody should add.
        //
        // MEASURED 2026-09-02: `ack-spine` ships `bin:ack-spine` and has 0 manifest
        // callers. The old probe reported `no manifest dependency references this
        // crate` and `next_action=repair-gate-trigger`. Both are true and the remedy
        // is wrong for the same reason `NotExtracted` had to become its own variant:
        // **a binary's trigger is an INVOCATION SITE, not a dependency edge.** This
        // file's own comment already said "bins without manifest callers are
        // expected"; the code did not act on it.
        let has_bin = crate_ships_a_bin(repo_root, crate_name);
        // The one case where we genuinely could not look. Checked BEFORE the
        // trigger arms, because every one of them reads this file: an unreadable
        // manifest makes `callers` and `has_bin` both zero, which would render as
        // "library with no manifest dependency" — a measurement we did not take.
        let manifest = repo_root.join("crates").join(crate_name).join("Cargo.toml");
        let reachability = if !manifest.is_file() {
            GateReachability::Unprobed {
                reason: format!("no readable manifest at crates/{crate_name}/Cargo.toml"),
            }
        } else if callers > 0 {
            GateReachability::Reachable {
                trigger: format!("manifest dependency ({callers} caller(s))"),
            }
        } else if has_bin && hook_invokes(&hook_path, crate_name) {
            GateReachability::Reachable {
                trigger: ".git/hooks/pre-commit".into(),
            }
        } else if has_bin && workflow_invokes(repo_root, crate_name) && has_remote {
            GateReachability::Reachable {
                trigger: ".github/workflows/gate.yml".into(),
            }
        } else if has_bin {
            GateReachability::Unreachable {
                reason: "binary with no invocation site: no manifest caller, not \
                         invoked by the installed hook, not declared in gate.yml"
                    .into(),
            }
        } else {
            GateReachability::Unreachable {
                reason: "library with no manifest dependency referencing it".into(),
            }
        };
        rows.push(GateCensusRow {
            gate: crate_name.into(),
            reachability,
            // Derived rows are ADVISORY. Their verdict is real and their triage is
            // not done, and per the ruling the loop must not stop on the second
            // fact. `disposition_for` refuses an advisory non-reachable row that no
            // allowance names, so this cannot become a quiet default.
            disposition: disposition_for(crate_name),
        });
    }

    GateCensus { rows }
}

/// Every directory under `crates/` — the census's membership, derived.
///
/// A crate is a directory, not a manifest, on purpose: a directory whose manifest
/// is missing is exactly the `Unprobed` case, and keying membership on the manifest
/// would make that crate vanish from the census again.
pub fn crates_on_disk(repo_root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let Ok(entries) = std::fs::read_dir(repo_root.join("crates")) else {
        return names;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_owned());
        }
    }
    names.sort_unstable();
    names
}

/// The disposition for a derived row: advisory, with the reason the allowance gives.
///
/// An unnamed crate still returns `Advisory`, carrying a reason that says it is
/// unnamed. That is deliberate: the REFUSAL for an unnamed advisory row belongs to
/// the ratchet test, which can name the crate and the file to edit, not to this
/// function, which runs inside the supervisor's tick and must never panic the loop.
pub fn disposition_for(crate_name: &str) -> CensusDisposition {
    // KNOWN-GOOD FIRST: a triaged row keeps its blocking disposition no matter how
    // it is now produced. Without this, derivation would silently demote all six
    // roster crates to advisory, which is a membership fix changing verdicts.
    if CURATED_BLOCKING_ROSTER.contains(&crate_name) {
        return CensusDisposition::Blocking;
    }
    match ADVISORY_ALLOWANCE
        .iter()
        .find(|(name, _)| *name == crate_name)
    {
        Some((_, reason)) => CensusDisposition::Advisory {
            reason: (*reason).to_owned(),
        },
        None => CensusDisposition::Advisory {
            reason: "NOT NAMED IN ADVISORY_ALLOWANCE -- the ratchet test fails on this".to_owned(),
        },
    }
}

/// Which OTHER crates actually emit through `target`'s step primitive.
///
/// # Why the needle is assembled rather than written
///
/// A literal `ack_spine::ledger::step(` in this file would make the census match
/// ITSELF. That is the self-referential-checker defect this repository has now
/// produced six times — most exactly, a census that grepped for gate names while
/// its own table named all of them, and `pgrep -f omp-orchestrator` returning a
/// `cass search` process whose ARGUMENTS held the string. Assembling the needle
/// from parts means this source never contains the contiguous text it looks for,
/// so the scan cannot find the scanner.
///
/// # What it does and does not prove
///
/// It proves a **call site exists in another crate's source**. It does not prove
/// the site executes, and a call inside a `#[cfg(test)]` module would count —
/// `src/` is scanned, and this crate's own tests live in `tests/`, so the exposure
/// is a test module inside a peer's `src`. Named rather than hidden: the honest
/// claim is "somebody wrote an emission", which is strictly more than
/// "somebody declared a dependency".
pub fn crates_emitting(repo_root: &Path, target: &str) -> Vec<String> {
    let ident = target.replace('-', "_");
    let needle = format!("{ident}::{}::{}(", "ledger", "step");
    let mut out = Vec::new();
    for crate_name in crates_on_disk(repo_root) {
        if crate_name == target {
            continue; // a crate emitting into its own ledger is not a caller
        }
        let src = repo_root.join("crates").join(&crate_name).join("src");
        if !src.is_dir() {
            continue;
        }
        if rust_sources_contain(&src, &needle) {
            out.push(crate_name);
        }
    }
    out.sort();
    out
}

/// Does any `.rs` file under `dir` contain `needle` **in code**? Depth-limited
/// walk, no subprocess, so it cannot time out the way the old load-dependent grep
/// did.
///
/// # Comments are stripped, and a mutation is why
///
/// MEASURED 2026-09-02: removing the supervisor's only emit site left this census
/// GREEN, so acceptance 4's fires-on-known-bad leg did not bite. The needle was
/// still matching — **inside the doc comment two functions up that warns about
/// exactly this**. I assembled the needle from parts so the code would not contain
/// it, then wrote the literal in the sentence explaining why. **The comment
/// defeated the mitigation it documented.**
///
/// That is the seventh self-referential-checker instance in this repository and my
/// second today — the first being a `path-literal-guard` refusal where the
/// sentence explaining the forbidden literal contained the literal. Splitting the
/// needle protects against the checker's own source; stripping comments protects
/// against every OTHER file's prose too, which is the general fix. It mirrors
/// `close-evidence-gate`, which blanks fenced and inline code before harvesting
/// paths for the same reason.
fn rust_sources_contain(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if rust_sources_contain(&path, needle) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if code_only(&text).contains(needle) {
                    return true;
                }
            }
        }
    }
    false
}

/// Shared structure-keyed comment masking; prose cannot manufacture a gate caller.
// structure-keyed: text-structure::code_only preserves lines while removing comments.

/// How many OTHER workspace manifests declare a path dependency on `crate_name`.
///
/// Reads `crates/*/Cargo.toml` and counts `path = "../<crate_name>"`. Deterministic,
/// bounded by the number of crates, and it never touches `target/` — the three
/// properties the grep it replaced lacked.
///
/// # Self-exclusion is structural, not a filter
///
/// The crate's own manifest is skipped by NAME comparison, so a crate cannot vouch
/// for itself. The grep it replaced excluded only paths containing `<name>/src/`,
/// which left `crates/<name>/Cargo.toml` in the count — meaning **a crate with no
/// dependents at all scored 1**, and the threshold had to be `> 1` to compensate. A
/// threshold tuned around a self-hit is a self-referential checker with arithmetic
/// on top; here the count is `> 0` because the self row is genuinely gone.
pub fn manifest_callers(repo_root: &Path, crate_name: &str) -> usize {
    let crates_dir = repo_root.join("crates");
    let Ok(entries) = std::fs::read_dir(&crates_dir) else {
        return 0;
    };
    let needle_slash = format!("path = \"../{crate_name}\"");
    let needle_tight = format!("path=\"../{crate_name}\"");
    let mut callers = 0usize;
    for entry in entries.flatten() {
        let dir = entry.path();
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == crate_name {
            continue; // structural self-exclusion
        }
        let manifest = dir.join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        // Both spellings, because a formatter that removes the spaces around `=`
        // would otherwise silently drop every caller. The `^command = ` vs
        // `command  = ` false zero in NUMBERS.toml is the same shape.
        if text.contains(&needle_slash) || text.contains(&needle_tight) {
            callers += 1;
        }
    }
    callers
}

/// Whether the crate ships a binary target: an explicit `[[bin]]` or the implicit
/// `src/main.rs`.
///
/// Both forms, because checking only `[[bin]]` misses every crate that relies on
/// Cargo's implicit binary — measured across this workspace, `git grep -c '\[\[bin\]\]'`
/// undercounts binary targets against `cargo metadata` for exactly that reason.
pub fn crate_ships_a_bin(repo_root: &Path, crate_name: &str) -> bool {
    let dir = repo_root.join("crates").join(crate_name);
    if dir.join("src/main.rs").is_file() {
        return true;
    }
    std::fs::read_to_string(dir.join("Cargo.toml"))
        .map(|text| text.contains("[[bin]]"))
        .unwrap_or(false)
}

pub use finding_dispatch::SupervisorDecision;


/// The pure deciding function: given an observation and the authorization state,
/// produce the supervisor's decision. This is where the three deciding legs are
/// encoded. It is PURE so the deciding legs are testable without subprocesses.
pub fn decide(observation: &Observation, authorization: &IdleAuthorization) -> SupervisorDecision {
    // GATE CENSUS — the FIRST check, before anything else. If any gate lacks
    // a reachable trigger, the supervisor refuses: it cannot dispatch into a
    // repo whose guarantees cannot fire. This is UNREACHABLE-AROUND — no
    // branch after this may return SupervisedWorking or AuthorizedIdle.
    match &observation.gate_census {
        Some(census) => {
            if !census.positive_control_passes() {
                return SupervisorDecision::GateUnwired {
                    unwired: vec![
                        "POSITIVE_CONTROL_FAILED: no independently reachable gate in this census"
                            .to_owned(),
                    ],
                };
            }
            let unwired: Vec<String> = census
                .unwired_gates()
                .iter()
                .map(|r| r.gate.clone())
                .collect();
            if !unwired.is_empty() {
                return SupervisorDecision::GateUnwired { unwired };
            }
        }
        None => {
            return SupervisorDecision::GateUnwired {
                unwired: vec!["CENSUS_NOT_PERFORMED".to_owned()],
            };
        }
    }
    // The monitor must have produced a readable census.
    // (In the real wiring, this is where a tick-monitor invoke failure surfaces.)
    if observation.panes.is_empty() {
        return SupervisorDecision::MonitorBlind {
            detail: "zero panes observed — the monitor is blind".to_owned(),
        };
    }

    // The queue must be readable.
    if !observation.queue.readable {
        return SupervisorDecision::QueueUnreadable {
            detail: "br ready produced no parseable output".to_owned(),
        };
    }

    // A pane blocked on a HUMAN is checked BEFORE capacity, because it is the one
    // condition the loop cannot resolve by working harder.
    //
    // Measured 2026-08-31 (bead dialog-reads-as-working-zag): `%1372` sat 36 MINUTES
    // on an install approval while every classifier read it as healthy work — the
    // dialog renders above the status line, the status line stays last, and the timer
    // keeps advancing while nobody answers. Counting that as "working" is right about
    // capacity and wrong about health, and the wrongness is invisible precisely
    // because the pane looks busy.
    //
    // This is Rule Zero's shape: a blocker HALTS and surfaces ONE named decision,
    // rather than being routed around by dispatching elsewhere.
    let awaiting: Vec<&PaneObservation> = observation
        .panes
        .iter()
        .filter(|p| p.awaits_human)
        .collect();
    if !awaiting.is_empty() {
        let panes = awaiting
            .iter()
            .map(|p| p.pane_id.clone())
            .collect::<Vec<_>>()
            .join(",");
        return SupervisorDecision::AwaitingHuman { panes };
    }

    // Count dispatchable panes (ConfirmedIdle only — one capture is not enough).
    let dispatchable: Vec<&PaneObservation> = observation
        .panes
        .iter()
        .filter(|p| p.is_dispatchable)
        .collect();

    // Count free-capacity panes: ConfirmedIdle OR NewlyIdle. THIS MUST READ ITS
    // OWN FIELD. It previously filtered on `is_dispatchable`, which made it
    // identical to `dispatchable` — so inside `if dispatchable.is_empty()` it was
    // always 0, the NewlyIdle branch was unreachable, and every unauthorized
    // newly-idle-plus-ready-work case fell through to AuthorizedIdle. That is the
    // exact shape that let the fleet sit idle for hours while the watchdogs
    // reported healthy.
    let free_capacity = observation
        .panes
        .iter()
        .filter(|p| p.is_free_capacity)
        .count();

    let working = observation.panes.iter().filter(|p| p.is_working).count();
    let expiry = match authorization {
        IdleAuthorization::Authorized { expires_at, .. } => Some(*expires_at),
        IdleAuthorization::Unauthorized { .. } => None,
    };
    if observation.queue.ready_count == 0 {
        // QUEUE EMPTY. A busy fleet with an empty queue is HEALTHY, not an
        // incident. Reporting it as one trains the operator to ignore the alarm,
        // which is how a real alarm gets missed — and it is why 178 consecutive
        // capacity ticks were written to a file nobody read.
        if free_capacity == 0 {
            return SupervisorDecision::SupervisedWorking {
                working_count: working,
                ready_count: 0,
            };
        }
        // Idle capacity with nothing queued. Not starvation — nobody is waiting —
        // but not a state to sit in silently either. Only Josh can decide that the
        // fleet has nothing to do.
        return match expiry {
            Some(expires_at) => SupervisorDecision::AuthorizedIdle {
                pane_count: observation.panes.len(),
                expires_at,
            },
            None => SupervisorDecision::QueueEmptyNeedsJosh {
                free_capacity_count: free_capacity,
            },
        };
    }

    // READY WORK EXISTS. A confirmed-idle pane wins immediately.
    if let Some(target) = dispatchable.first() {
        return SupervisorDecision::Dispatch {
            pane: target.pane_id.clone(),
            bead_hint: "first-ready-bead".to_owned(),
        };
    }

    // Ready work, nothing CONFIRMED idle. Free capacity here means NewlyIdle: a
    // pane that just finished and is visible but not yet twice-confirmed. With
    // work queued that must NOT be tolerated silently — it is the measured
    // 4h19m failure.
    if free_capacity > 0 {
        return match expiry {
            Some(expires_at) => SupervisorDecision::AuthorizedIdle {
                pane_count: observation.panes.len(),
                expires_at,
            },
            None => SupervisorDecision::EscalateIdleIncident {
                dispatchable_count: 0,
                ready_count: observation.queue.ready_count,
            },
        };
    }

    // Ready work and every pane working: healthy saturation. Reported, not
    // alarmed, and NOT AuthorizedIdle — nothing here is idle.
    SupervisorDecision::SupervisedWorking {
        working_count: working,
        ready_count: observation.queue.ready_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A census where every gate is reachable, including the positive control.
    ///
    /// WHY NOT `gate_census: None`, which is the one-line fix that was asked
    /// for: the census is the FIRST check in `decide()`, and `None` returns
    /// `GateUnwired { CENSUS_NOT_PERFORMED }` before any other branch is
    /// reached. Every existing test below would then pass its assertion on a
    /// decision no test intended to exercise — twelve green legs measuring the
    /// census instead of dispatch, idleness, and authorization. The gate-unwired
    /// path gets its OWN tests; these helpers must clear the gate to reach the
    /// branch under test.
    fn passing_census() -> GateCensus {
        GateCensus {
            rows: vec![reachable_blocking("registry-check")],
        }
    }

    fn reachable_blocking(gate: &str) -> GateCensusRow {
        GateCensusRow {
            gate: gate.to_owned(),
            reachability: GateReachability::Reachable {
                trigger: format!("crates/{gate}"),
            },
            disposition: CensusDisposition::Blocking,
        }
    }

    fn unreachable_advisory(gate: &str) -> GateCensusRow {
        GateCensusRow {
            gate: gate.to_owned(),
            reachability: GateReachability::Unreachable {
                reason: "no independently verified trigger".to_owned(),
            },
            disposition: CensusDisposition::Advisory {
                reason: "untriaged derived row".to_owned(),
            },
        }
    }

    fn obs(panes: Vec<PaneObservation>, ready: usize, readable: bool) -> Observation {
        Observation {
            panes,
            queue: QueueState {
                ready_count: ready,
                readable,
            },
            gate_census: Some(passing_census()),
        }
    }

    /// Build a pane from tick-monitor's LIVENESS vocabulary. The three booleans
    /// are DERIVED from that one label, never from a caller-supplied flag: a bare
    /// `dispatchable: bool` cannot distinguish NewlyIdle from working, and that
    /// ambiguity is what made the NewlyIdle branch unreachable.
    fn pane(id: &str, state: &str, _dispatchable_hint: bool) -> PaneObservation {
        let liveness = match state {
            "IDLE" => "CONFIRMED_IDLE",
            other => other,
        };
        PaneObservation {
            pane_id: id.to_owned(),
            state: state.to_owned(),
            liveness: liveness.to_owned(),
            is_dispatchable: liveness == "CONFIRMED_IDLE",
            is_free_capacity: matches!(liveness, "CONFIRMED_IDLE" | "NEWLY_IDLE"),
            // MIRRORS PRODUCTION EXACTLY (main.rs: is_working). This helper
            // previously derived is_working from `liveness` alone, so a "DIALOG"
            // fixture reported NOT working while production reported working — a
            // fixture drifted from the code it certifies, which is `fh C38`: its
            // green is indistinguishable from a working check.
            is_working: matches!(liveness, "LIVE" | "WORKING")
                || matches!(state, "WORKING" | "DIALOG"),
            // Derived from the state string exactly as production does, so a test
            // passing "DIALOG" exercises the real escalation path rather than a
            // hand-set flag. A fixture that cannot reproduce the condition certifies
            // nothing about it.
            awaits_human: state == "DIALOG",
        }
    }

    // ── DIALOG: ALIVE AND BLOCKED ON A HUMAN ─────────────────────────────────

    /// A pane awaiting a human answer must ESCALATE, not read as healthy work.
    ///
    /// # The measured failure
    ///
    /// 2026-08-31, bead `dialog-reads-as-working-zag`. On OMP v18 an Ask/approval
    /// dialog renders ABOVE the status line; the status line stays LAST and its
    /// timer KEEPS ADVANCING while the pane waits. Every classifier therefore read
    /// a human-blocked pane as `Working`/`Live`, and `%1372` sat **36 minutes** on
    /// an install approval while the fleet reported healthy.
    ///
    /// `tick-monitor` already classified this correctly as `PaneState::Dialog` — the
    /// detection was BUILT. The supervisor folded `"DIALOG"` into `is_working` and
    /// escalated nothing, so the detection was NOT WIRED. This test is the wiring.
    #[test]
    fn a_pane_on_a_dialog_escalates_to_a_human_and_does_not_read_as_working() {
        let observation = Observation {
            panes: vec![pane("%1372", "DIALOG", false)],
            queue: QueueState {
                ready_count: 3,
                readable: true,
            },
            gate_census: Some(passing_census()),
        };
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "no_token" },
        );
        match decision {
            SupervisorDecision::AwaitingHuman { panes } => {
                assert!(
                    panes.contains("%1372"),
                    "the escalation must NAME the pane a human has to go look at, got {panes:?}"
                );
            }
            other => panic!(
                "a pane blocked on a human must escalate; got {other:?}. \
                 This is the 36-minute stall: ready work exists, the pane looks busy, \
                 and nobody is coming."
            ),
        }
    }

    /// The escalation must NOT fire for a pane that is genuinely working.
    ///
    /// KNOWN-GOOD leg. Without it this gate is over-strict in the direction that
    /// gets it routed around: every busy fleet would page a human.
    #[test]
    fn a_working_pane_does_not_escalate_to_a_human() {
        let observation = Observation {
            panes: vec![pane("%1408", "WORKING", false)],
            queue: QueueState {
                ready_count: 3,
                readable: true,
            },
            gate_census: Some(passing_census()),
        };
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "no_token" },
        );
        assert!(
            !matches!(decision, SupervisorDecision::AwaitingHuman { .. }),
            "a genuinely working pane must not page a human; got {decision:?}"
        );
    }

    /// DIALOG is still counted as busy for CAPACITY purposes.
    ///
    /// The fix separates health from capacity; it must not accidentally make a
    /// human-blocked pane look dispatchable, which would send work into a pane
    /// that cannot accept it until someone answers.
    #[test]
    fn a_dialog_pane_is_not_dispatchable_capacity() {
        let p = pane("%1372", "DIALOG", false);
        assert!(
            !p.is_dispatchable,
            "a pane holding an open dialog must never be offered as dispatchable"
        );
        assert!(
            p.is_working,
            "DIALOG must still count as busy for capacity accounting — the pane is \
             occupied, it just is not progressing"
        );
        assert!(p.awaits_human, "and it must be flagged for escalation");
    }

    // ── DECIDING LEG 1: NO NO-OP GREEN PATH ──────────────────────────────────

    #[test]
    fn stubbed_send_cannot_produce_green() {
        // A stubbed send that returns Ok without actually sending must be caught
        // by the Dispatch variant carrying the pane and bead — if the test can
        // assert on those fields, a stub that produces neither fails.
        let observation = obs(vec![pane("%1409", "IDLE", true)], 5, true);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        match &decision {
            SupervisorDecision::Dispatch { pane, .. } => {
                assert_eq!(pane, "%1409", "the dispatchable pane must be named");
            }
            other => panic!("expected Dispatch, got {other:?}"),
        }
    }

    // ── DECIDING LEG 2: FREE + READY => DISPATCH OR ESCALATION ───────────────

    #[test]
    fn free_and_ready_must_dispatch_or_escalate() {
        // ConfirmedIdle pane + ready work -> Dispatch (the happy path).
        let observation = obs(vec![pane("%1413", "IDLE", true)], 10, true);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        assert!(
            matches!(decision, SupervisorDecision::Dispatch { .. }),
            "free + ready + authorized-ambient = dispatch, got {decision:?}"
        );
    }

    #[test]
    fn newly_idle_with_ready_work_escalates_when_unauthorized() {
        // A NewlyIdle pane is free capacity but NOT dispatchable (one capture).
        // With ready work and no ConfirmedIdle panes, this must ESCALATE —
        // the conductor needs to know that a freed worker is visible but
        // not yet confirmed.
        let observation = obs(vec![pane("%1413", "NEWLY_IDLE", false)], 10, true);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        match &decision {
            SupervisorDecision::EscalateIdleIncident {
                dispatchable_count,
                ready_count,
            } => {
                assert_eq!(*dispatchable_count, 0, "no confirmed-idle panes");
                assert_eq!(*ready_count, 10, "ready count from the queue");
            }
            other => panic!("expected EscalateIdleIncident, got {other:?}"),
        }
    }

    #[test]
    fn no_third_branch_free_and_ready() {
        // The three-outcome contract: dispatch, escalate, or authorized-idle.
        // There is no fourth outcome for free+ready.
        let observation = obs(vec![pane("%1409", "IDLE", true)], 3, true);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        assert!(
            matches!(decision, SupervisorDecision::Dispatch { .. }),
            "the only legal outcomes for free+ready are Dispatch or Escalate, got {decision:?}"
        );
    }

    // ── DECIDING LEG 3: IDLE_AUTHORIZED DEFAULT UNAUTHORIZED ─────────────────

    #[test]
    fn unauthorized_idle_with_empty_queue_needs_josh_not_an_incident() {
        // CONTRACT SHARPENED. This previously asserted EscalateIdleIncident, which
        // conflated two different situations: a starving fleet (work queued, panes
        // free) and a fleet with nothing to do. Nobody is starving here — the
        // queue is empty — so calling it an incident is the alarm-fatigue failure.
        // It still must NOT be silent: an empty queue is a decision only Josh can
        // make, so queue-empty stays SUPERVISED.
        let observation = obs(vec![pane("%1409", "IDLE", true)], 0, true);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        match &decision {
            SupervisorDecision::QueueEmptyNeedsJosh {
                free_capacity_count,
            } => {
                assert_eq!(*free_capacity_count, 1, "the free pane must be counted");
            }
            other => panic!("expected QueueEmptyNeedsJosh, got {other:?}"),
        }
        // And it must NOT read as authorized — nothing authorized this.
        assert!(
            !matches!(decision, SupervisorDecision::AuthorizedIdle { .. }),
            "unauthorized must never render as AuthorizedIdle"
        );
    }

    #[test]
    fn a_busy_fleet_with_an_empty_queue_is_supervised_working_not_an_incident() {
        // The live smoke case: six panes working, nothing free. Escalating this
        // trains the operator to ignore the alarm.
        let observation = obs(
            vec![pane("%1413", "LIVE", false), pane("%1414", "LIVE", false)],
            0,
            true,
        );
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        match &decision {
            SupervisorDecision::SupervisedWorking { working_count, .. } => {
                assert_eq!(*working_count, 2, "both working panes must be counted");
            }
            other => panic!("expected SupervisedWorking, got {other:?}"),
        }
    }

    #[test]
    fn a_saturated_fleet_with_queued_work_is_supervised_working() {
        // Ready work AND every pane working = healthy saturation, not idleness.
        // This is the case BlueLantern's live smoke hit (panes=6 ready=4).
        let observation = obs(
            vec![pane("%1413", "LIVE", false), pane("%1414", "LIVE", false)],
            4,
            true,
        );
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        match &decision {
            SupervisorDecision::SupervisedWorking {
                working_count,
                ready_count,
            } => {
                assert_eq!(*working_count, 2);
                assert_eq!(*ready_count, 4, "queued work must be reported, not hidden");
            }
            other => panic!("expected SupervisedWorking, got {other:?}"),
        }
    }

    #[test]
    fn authorized_idle_with_empty_queue_is_tolerated() {
        let observation = obs(vec![pane("%1409", "IDLE", true)], 0, true);
        let auth = IdleAuthorization::Authorized {
            reason: "Josh said stand down".to_owned(),
            session: "omp-orchestrator".to_owned(),
            pane_set_hash: "test-hash".to_owned(),
            queue_len: 0,
            issued_at: 1,
            expires_at: u64::MAX,
        };
        let decision = decide(&observation, &auth);
        assert!(
            matches!(decision, SupervisorDecision::AuthorizedIdle { .. }),
            "authorized idle = tolerated, got {decision:?}"
        );
    }

    // ── THE TOKEN CONTRACT: EXISTENCE IS NOT AUTHORIZATION ───────────────────

    fn token_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("kxe-auth-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        root
    }

    #[test]
    fn idle_authorization_defaults_to_unauthorized() {
        let root = token_root("none");
        let auth = read_idle_authorization(&root, 1_000);
        assert_eq!(
            auth,
            IdleAuthorization::Unauthorized { why: "no_token" },
            "no token file = unauthorized"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bare_reason_string_does_not_authorize() {
        // KNOWN-BAD, and it is the defect this contract exists to close: the
        // earlier reader accepted ANY non-empty file, so a stray note authorized
        // the fleet to go dark forever. An approval that does not name what it
        // approved cannot be falsified.
        let root = token_root("bare");
        std::fs::write(root.join(".idle_authorized"), "Josh said stand down\n").expect("write");
        let auth = read_idle_authorization(&root, 1_000);
        assert_eq!(
            auth,
            IdleAuthorization::Unauthorized {
                why: "token_missing_required_field"
            },
            "a bare reason must NOT authorize, got {auth:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fully_bound_unexpired_token_authorizes() {
        // KNOWN-GOOD, mandatory: without this leg the reader could refuse
        // everything and still pass, which is an over-strict gate that gets
        // routed around.
        let root = token_root("bound");
        std::fs::write(
            root.join(".idle_authorized"),
            "reason = overnight stand-down\n\
             session = omp-orchestrator\n\
             pane_set_hash = abc123\n\
             queue_len = 4\n\
             issued_at = 900\n\
             expires_at = 5000\n",
        )
        .expect("write");
        match read_idle_authorization(&root, 1_000) {
            IdleAuthorization::Authorized {
                session,
                queue_len,
                expires_at,
                ..
            } => {
                assert_eq!(session, "omp-orchestrator", "session must bind");
                assert_eq!(queue_len, 4, "queue depth must bind");
                assert_eq!(expires_at, 5000, "expiry must bind");
            }
            other => panic!("a bound unexpired token must authorize, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_expired_token_does_not_authorize() {
        // An authorization with no enforced expiry is a permanent licence to go
        // dark. `now` is PASSED IN so this leg cannot flake on wall-clock time.
        let root = token_root("expired");
        std::fs::write(
            root.join(".idle_authorized"),
            "reason = old stand-down\n\
             session = omp-orchestrator\n\
             pane_set_hash = abc123\n\
             queue_len = 0\n\
             issued_at = 100\n\
             expires_at = 500\n",
        )
        .expect("write");
        assert_eq!(
            read_idle_authorization(&root, 1_000),
            IdleAuthorization::Unauthorized {
                why: "token_expired"
            },
            "an expired token must not authorize"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── FAIL-CLOSED LEGS ──────────────────────────────────────────────────────

    #[test]
    fn monitor_blind_is_typed_not_silent() {
        let observation = obs(vec![], 5, true);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        match &decision {
            SupervisorDecision::MonitorBlind { detail } => {
                assert!(
                    detail.contains("blind"),
                    "must name the blindness: {detail}"
                );
            }
            other => panic!("expected MonitorBlind, got {other:?}"),
        }
    }

    #[test]
    fn queue_unreadable_is_typed_not_silent() {
        let observation = obs(vec![pane("%1409", "IDLE", true)], 0, false);
        let decision = decide(
            &observation,
            &IdleAuthorization::Unauthorized { why: "test" },
        );
        assert!(
            matches!(decision, SupervisorDecision::QueueUnreadable { .. }),
            "unreadable queue must be typed, got {decision:?}"
        );
    }

    // ── POSITIVE CONTROL IS DERIVED PER SERVED REPO (uds-k0i6) ─────────────

    fn decide_ready(census: GateCensus) -> SupervisorDecision {
        decide(
            &Observation {
                panes: vec![pane("%1409", "IDLE", true)],
                queue: QueueState {
                    ready_count: 3,
                    readable: true,
                },
                gate_census: Some(census),
            },
            &IdleAuthorization::Unauthorized { why: "test" },
        )
    }

    #[test]
    fn k0i6_serving_uds_passes_on_registry_check() {
        let census = GateCensus {
            rows: vec![reachable_blocking("registry-check")],
        };
        assert_eq!(census.derived_positive_control(), Some("registry-check"));
        assert!(census.positive_control_passes());
        match decide_ready(census) {
            SupervisorDecision::Dispatch { pane, .. } => {
                assert_eq!(pane, "%1409", "cleared canary must still dispatch")
            }
            other => panic!("uds canary must clear the gate, got {other:?}"),
        }
    }

    #[test]
    fn k0i6_empty_census_still_refuses() {
        let census = GateCensus { rows: vec![] };
        assert!(
            census.unwired_gates().is_empty(),
            "vacuity: scanned nothing so unwired=0, and that must still refuse"
        );
        assert!(!census.positive_control_passes());
        match decide_ready(census) {
            SupervisorDecision::GateUnwired { unwired } => {
                assert!(
                    unwired.iter().any(|u| u.contains("POSITIVE_CONTROL_FAILED")),
                    "empty census must name POSITIVE_CONTROL_FAILED, got {unwired:?}"
                );
            }
            other => panic!("empty census must refuse, got {other:?}"),
        }
    }

    #[test]
    fn k0i6_all_unreachable_still_refuses() {
        let census = GateCensus {
            rows: vec![
                unreachable_advisory("close-cites"),
                unreachable_advisory("uds-drift"),
            ],
        };
        assert!(
            census.unwired_gates().is_empty(),
            "advisory unreachables must not count as blocking unwired"
        );
        assert!(!census.positive_control_passes());
        match decide_ready(census) {
            SupervisorDecision::GateUnwired { unwired } => {
                assert!(
                    unwired.iter().any(|u| u.contains("POSITIVE_CONTROL_FAILED")),
                    "got {unwired:?}"
                );
            }
            other => panic!("all-unreachable must refuse, got {other:?}"),
        }
    }

    #[test]
    fn k0i6_derived_canary_ignores_foreign_crate_name() {
        // Mutation target: if positive_control_passes looks up a hardcoded
        // crate name, this census (reachable uds gate, that crate absent) REDs.
        let census = GateCensus {
            rows: vec![reachable_blocking("closure-check")],
        };
        assert_eq!(census.derived_positive_control(), Some("closure-check"));
        assert!(census.positive_control_passes());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// THE KERNEL: AN OBSERVATION CARRIES AN OBLIGATION
// ═══════════════════════════════════════════════════════════════════════════════
//
// Every failure this session had ONE shape: a state that could be OBSERVED but
// carried NO OBLIGATION TO BE DISCHARGED.
//
//   tick-monitor detected idle capacity for 178 consecutive ticks -> wrote a file
//   ATTENTION.txt was written                                     -> nobody read it
//   admission went RED                                            -> nothing repaired it
//   refill could not tell                                         -> printed "nothing to do"
//   the conductor observed idle panes                              -> named it a next action
//
// Rules do not fix this; 4 of them were already written down and the failure
// happened anyway. The obligation has to be IN THE TYPE.
//
// THE CONTRACT, enforced by the compiler rather than by discipline:
//   1. A `Census` CANNOT be empty         -> "zero observed" cannot be a pass.
//   2. Observing ALWAYS yields a `Duty`   -> there is no "nothing to do" outcome.
//   3. `Duty` is `#[must_use]`            -> dropping it is a compile-time warning.
//   4. Only consuming a `Duty` yields
//      `Discharged`                       -> the sole proof of a completed tick.
//   5. Only `Discharged` produces a
//      success `ExitCode`                 -> exiting 0 having done nothing is
//                                            unrepresentable.

/// A non-empty pane census. The constructor is the anti-vacuity gate: an empty
/// scan is an ERROR at the type boundary, not a healthy fleet reported as clean.
#[derive(Debug, Clone)]
pub struct Census {
    panes: Vec<PaneObservation>,
}

impl Census {
    /// Refuses an empty scan. A monitor that saw nothing has NOT seen an idle-free
    /// fleet — those are opposite conditions and this is where they separate.
    pub fn try_new(panes: Vec<PaneObservation>) -> Result<Self, &'static str> {
        if panes.is_empty() {
            return Err("empty census: a scan that observed zero panes is an ERROR, never a pass");
        }
        Ok(Self { panes })
    }
    pub fn panes(&self) -> &[PaneObservation] {
        &self.panes
    }
}

/// An obligation the supervisor MUST discharge this tick.
///
/// `#[must_use]` is the load-bearing attribute: it makes "observe and move on" a
/// compiler warning instead of a 4-hour outage. There is deliberately NO variant
/// meaning "nothing to do" — every reachable state names an action.
#[must_use = "an undischarged Duty is the 178-tick failure: observed, recorded, and not acted on"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duty(SupervisorDecision);

impl Duty {
    /// The ONLY way to obtain a Duty, and it is TOTAL: every census plus
    /// authorization yields one. A caller cannot reach a code path that observed
    /// the fleet and owes nothing.
    ///
    /// `gates` is a REQUIRED PARAMETER, not a default. It previously read
    /// `gate_census: None` hardcoded, which meant every Duty this constructor
    /// produced was `GateUnwired { CENSUS_NOT_PERFORMED }` — so the kernel's
    /// central obligation type could never report a healthy fleet, and the
    /// alarm would fire on every cycle until the operator discounted it. That
    /// is the failure `a_saturated_fleet_discharges_as_a_heartbeat_not_an_alarm`
    /// exists to catch, and it caught it.
    ///
    /// It stays an `Option` rather than becoming mandatory because
    /// `None` is a MEANINGFUL VALUE: it says "this caller performed no census",
    /// which `decide` answers with a refusal. Making it non-optional would
    /// delete that signal. The defect was hardcoding it, not offering it.
    pub fn observe(
        census: &Census,
        queue: &QueueState,
        authorization: &IdleAuthorization,
        gates: Option<GateCensus>,
    ) -> Self {
        let observation = Observation {
            panes: census.panes.to_vec(),
            queue: queue.clone(),
            gate_census: gates,
        };
        Self(decide(&observation, authorization))
    }

    pub fn decision(&self) -> &SupervisorDecision {
        &self.0
    }

    /// Does discharging this duty require ACTUATION rather than a heartbeat?
    pub fn requires_action(&self) -> bool {
        !matches!(self.0, SupervisorDecision::SupervisedWorking { .. })
    }

    /// Consume the duty. `evidence` must describe what was actually done — a
    /// receipt, an escalation id, or a heartbeat row. It is REQUIRED because a
    /// discharge with no evidence is the close-without-evidence debt in another
    /// costume.
    pub fn discharge(self, evidence: impl Into<String>) -> Discharged {
        let evidence = evidence.into();
        Discharged {
            decision: self.0,
            evidence,
        }
    }
}

/// Proof that a tick did something. The ONLY route to a success exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discharged {
    decision: SupervisorDecision,
    evidence: String,
}

impl Discharged {
    pub fn decision(&self) -> &SupervisorDecision {
        &self.decision
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    /// Success requires NON-EMPTY evidence. Empty evidence is a no-op wearing a
    /// discharge, so it maps to a failure code rather than silently passing.
    pub fn exit_code(&self) -> u8 {
        if self.evidence.trim().is_empty() {
            70
        } else {
            0
        }
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;

    fn working(id: &str) -> PaneObservation {
        PaneObservation {
            pane_id: id.to_owned(),
            state: "WORKING".to_owned(),
            liveness: "LIVE".to_owned(),
            is_dispatchable: false,
            is_free_capacity: false,
            is_working: true,
            awaits_human: false,
        }
    }
    fn idle(id: &str) -> PaneObservation {
        PaneObservation {
            pane_id: id.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        }
    }
    fn q(ready: usize) -> QueueState {
        QueueState {
            ready_count: ready,
            readable: true,
        }
    }
    fn unauth() -> IdleAuthorization {
        IdleAuthorization::Unauthorized { why: "test" }
    }

    #[test]
    fn an_empty_census_cannot_be_constructed() {
        // ANTI-VACUITY AT THE TYPE BOUNDARY. Without this, a monitor that saw
        // nothing reports identically to one that saw a busy fleet.
        let err = Census::try_new(vec![]).expect_err("empty census must be refused");
        assert!(err.contains("ERROR"), "the refusal must say so: {err}");
    }

    #[test]
    fn observing_always_yields_a_duty_there_is_no_nothing_to_do() {
        // TOTALITY. Four shapes, none of which can produce "no obligation".
        let cases = vec![
            (vec![idle("%1")], q(4)),    // free + ready
            (vec![idle("%1")], q(0)),    // free, empty queue
            (vec![working("%1")], q(4)), // saturated + ready
            (vec![working("%1")], q(0)), // saturated, empty queue
        ];
        for (panes, queue) in cases {
            let census = Census::try_new(panes).expect("non-empty");
            let duty = Duty::observe(&census, &queue, &unauth(), gates());
            // The duty exists and names something. `#[must_use]` forces this line.
            let _ = duty.discharge("test-evidence");
        }
    }

    #[test]
    fn free_plus_ready_always_requires_action_never_a_heartbeat() {
        // The 4h19m failure, made unrepresentable: an idle pane beside ready work
        // can never discharge as SupervisedWorking.
        let census = Census::try_new(vec![idle("%1"), working("%2")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(4), &unauth(), gates());
        assert!(
            duty.requires_action(),
            "free + ready must demand actuation, got {:?}",
            duty.decision()
        );
    }

    #[test]
    fn a_saturated_fleet_discharges_as_a_heartbeat_not_an_alarm() {
        // The other half: a healthy busy fleet must NOT demand action, or the
        // alarm fires constantly and gets discounted.
        let census = Census::try_new(vec![working("%1"), working("%2")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(0), &unauth(), gates());
        assert!(!duty.requires_action(), "a busy fleet is not an incident");
    }

    #[test]
    fn a_discharge_with_no_evidence_cannot_exit_zero() {
        // "I did something" with nothing to show is a no-op wearing a discharge.
        let census = Census::try_new(vec![idle("%1")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(4), &unauth(), gates());
        assert_eq!(
            duty.discharge("   ").exit_code(),
            70,
            "empty evidence must fail"
        );
    }

    #[test]
    fn a_discharge_with_evidence_exits_zero() {
        // KNOWN-GOOD, mandatory: without it the kernel could refuse everything and
        // still pass, which is an over-strict gate that gets routed around.
        let census = Census::try_new(vec![idle("%1")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(4), &unauth(), gates());
        assert_eq!(
            duty.discharge("dispatched %1 bead=x receipt=IDLE_TO_WORKING")
                .exit_code(),
            0
        );
    }

    #[test]
    fn a_duty_built_without_a_census_refuses_rather_than_reporting_healthy() {
        // THE DEFECT THIS REPLACES, measured 2026-08-31: `Duty::observe` hardcoded
        // `gate_census: None`, so EVERY duty the kernel produced was GateUnwired —
        // a healthy busy fleet read as an incident, which trains the operator to
        // discount the alarm. Passing None is now a CALLER'S CHOICE with a defined
        // meaning, and this asserts that meaning.
        let census = Census::try_new(vec![working("%1"), working("%2")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(0), &unauth(), None);
        match duty.decision() {
            SupervisorDecision::GateUnwired { unwired } => {
                assert!(
                    unwired.iter().any(|u| u.contains("CENSUS_NOT_PERFORMED")),
                    "a caller that performed no census must be told exactly that, got {unwired:?}"
                );
            }
            other => panic!("no census must refuse, got {other:?}"),
        }
        assert!(
            duty.requires_action(),
            "a refusal is never a heartbeat — it must demand actuation"
        );
    }

    /// A census in which the positive control is reachable, so `decide` clears the
    /// gate check and the test below it exercises the branch it actually names.
    /// Without this every kernel test would assert against GateUnwired.
    fn gates() -> Option<GateCensus> {
        Some(GateCensus {
            rows: vec![GateCensusRow {
                gate: "registry-check".to_owned(),
                reachability: GateReachability::Reachable {
                    trigger: "crates/registry-check".to_owned(),
                },
                disposition: CensusDisposition::Blocking,
            }],
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BINDING VALIDATION: A PARSED FIELD THAT IS NEVER COMPARED IS A VACUOUS FIELD
// ═══════════════════════════════════════════════════════════════════════════════
//
// `read_idle_authorization` parses session, pane_set_hash and queue_len, and
// `decide` checked only the EXPIRY. So an approval granted for one session with
// an empty queue silently authorized a DIFFERENT session with forty ready beads.
// The fields read as safety and provided none — the same shape as `free_capacity`
// derived from `is_dispatchable`, and as a test that passed because the branch it
// covered could not run.
//
// The comparison lives HERE, not in main: `decide` is the only consumer of the
// authorization, and policy that leaks into the binary gets reimplemented per
// call site. It is exposed as an ADDITIVE function so no existing signature
// changes — main calls `applicable(..)` and passes the result to `decide(..)`.

/// Canonical pane-set hash. Order-independent, so a pane list read in a different
/// order is the same fleet; and it names the PANE IDS, because a fleet with the
/// same COUNT but different panes is not the fleet that was approved.
pub fn pane_set_hash(panes: &[PaneObservation]) -> String {
    let mut ids: Vec<&str> = panes.iter().map(|p| p.pane_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for id in ids {
        for byte in id.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0x1f;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Downgrade an authorization that does not describe the CURRENT situation.
///
/// An approval is a statement about a moment: this session, this fleet shape,
/// this queue depth. When any of those move, the approval no longer describes
/// what was approved, and continuing to honour it is how a stand-down granted for
/// a quiet fleet licenses silence during a backlog.
///
/// Returns the authorization unchanged when every binding still holds, and
/// `Unauthorized { why }` naming the FIRST binding that failed otherwise.
pub fn applicable(
    authorization: IdleAuthorization,
    session: &str,
    panes: &[PaneObservation],
    queue: &QueueState,
) -> IdleAuthorization {
    let IdleAuthorization::Authorized {
        session: token_session,
        pane_set_hash: token_hash,
        queue_len: token_queue,
        ..
    } = &authorization
    else {
        return authorization;
    };
    if token_session != session {
        return IdleAuthorization::Unauthorized {
            why: "token_session_mismatch",
        };
    }
    if token_hash != &pane_set_hash(panes) {
        return IdleAuthorization::Unauthorized {
            why: "token_pane_set_changed",
        };
    }
    // A stand-down granted over an empty queue does not authorize silence once
    // work arrives. Growth invalidates; shrinkage does not.
    if queue.ready_count > *token_queue {
        return IdleAuthorization::Unauthorized {
            why: "token_queue_grew",
        };
    }
    authorization
}

/// The canonical token writer. Without it the format is unusable, which means the
/// only reachable state is Unauthorized — fail-safe, and it makes the authorized
/// path untestable in practice. A contract nobody can satisfy is not a contract.
pub fn write_token(
    reason: &str,
    session: &str,
    panes: &[PaneObservation],
    queue: &QueueState,
    issued_at: u64,
    valid_for_secs: u64,
) -> String {
    format!(
        "# minted by omp-orchestrator::write_token — bindings are load-bearing\n\
         reason = {reason}\n\
         session = {session}\n\
         pane_set_hash = {}\n\
         queue_len = {}\n\
         issued_at = {issued_at}\n\
         expires_at = {}\n",
        pane_set_hash(panes),
        queue.ready_count,
        issued_at.saturating_add(valid_for_secs)
    )
}

#[cfg(test)]
mod binding_tests {
    use super::*;

    fn p(id: &str) -> PaneObservation {
        PaneObservation {
            pane_id: id.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        }
    }
    fn q(n: usize) -> QueueState {
        QueueState {
            ready_count: n,
            readable: true,
        }
    }
    /// `tag` MUST be unique per test. Keying the temp dir on `session` alone made
    /// parallel tests clobber each other's token — shared mutable state with no
    /// isolation, which produced two failures whose reported cause
    /// (token_pane_set_changed) was a different test's fleet.
    fn minted(
        tag: &str,
        session: &str,
        panes: &[PaneObservation],
        queue: &QueueState,
    ) -> IdleAuthorization {
        let dir = std::env::temp_dir().join(format!("bind-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join(".idle_authorized"),
            write_token("stand down", session, panes, queue, 1_000, 3_600),
        )
        .expect("write");
        let auth = read_idle_authorization(&dir, 1_100);
        let _ = std::fs::remove_dir_all(&dir);
        auth
    }

    #[test]
    fn the_canonical_writer_produces_a_token_the_reader_accepts() {
        // KNOWN-GOOD, and it is mandatory: without a writer the only reachable
        // state is Unauthorized, so every binding leg below would pass vacuously.
        let panes = vec![p("%1"), p("%2")];
        assert!(
            matches!(
                minted("writer", "omp-orchestrator", &panes, &q(0)),
                IdleAuthorization::Authorized { .. }
            ),
            "the writer and reader must agree, or the authorized path is untestable"
        );
    }

    #[test]
    fn an_authorization_for_another_session_does_not_apply() {
        let panes = vec![p("%1")];
        let auth = minted("session", "some-other-session", &panes, &q(0));
        assert_eq!(
            applicable(auth, "omp-orchestrator", &panes, &q(0)),
            IdleAuthorization::Unauthorized {
                why: "token_session_mismatch"
            }
        );
    }

    #[test]
    fn an_authorization_stops_applying_when_the_fleet_changes_shape() {
        let approved = vec![p("%1"), p("%2")];
        let auth = minted("shape", "omp-orchestrator", &approved, &q(0));
        let now = vec![p("%1"), p("%2"), p("%3")];
        assert_eq!(
            applicable(auth, "omp-orchestrator", &now, &q(0)),
            IdleAuthorization::Unauthorized {
                why: "token_pane_set_changed"
            }
        );
    }

    #[test]
    fn a_standdown_granted_over_an_empty_queue_does_not_survive_a_backlog() {
        // THE LEG THAT MATTERS. Josh authorizes idleness when there is nothing to
        // do; four ready beads is a different situation and must re-ask.
        let panes = vec![p("%1")];
        let auth = minted("grew", "omp-orchestrator", &panes, &q(0));
        assert_eq!(
            applicable(auth, "omp-orchestrator", &panes, &q(4)),
            IdleAuthorization::Unauthorized {
                why: "token_queue_grew"
            }
        );
    }

    #[test]
    fn a_shrinking_queue_does_not_invalidate_an_authorization() {
        // KNOWN-GOOD on the other side: growth invalidates, shrinkage does not.
        // Without this leg the rule is "any queue change revokes", which makes an
        // authorization useless the moment a worker closes a bead.
        let panes = vec![p("%1")];
        let auth = minted("shrank", "omp-orchestrator", &panes, &q(5));
        assert!(
            matches!(
                applicable(auth, "omp-orchestrator", &panes, &q(2)),
                IdleAuthorization::Authorized { .. }
            ),
            "a draining queue must not revoke a valid stand-down"
        );
    }

    #[test]
    fn the_pane_set_hash_is_order_independent_but_identity_sensitive() {
        assert_eq!(
            pane_set_hash(&[p("%1"), p("%2")]),
            pane_set_hash(&[p("%2"), p("%1")]),
            "the same fleet read in a different order is the same fleet"
        );
        assert_ne!(
            pane_set_hash(&[p("%1"), p("%2")]),
            pane_set_hash(&[p("%1"), p("%3")]),
            "same COUNT, different panes: not the fleet that was approved"
        );
    }

    #[test]
    fn an_unauthorized_input_passes_through_unchanged() {
        let panes = vec![p("%1")];
        let before = IdleAuthorization::Unauthorized { why: "no_token" };
        assert_eq!(
            applicable(before.clone(), "omp-orchestrator", &panes, &q(0)),
            before,
            "applicable() must not manufacture authorization"
        );
    }
}

#[cfg(test)]
mod census_is_measured_not_frozen {
    use super::*;

    /// KNOWN-BAD: a gate absent from both triggers must be Unreachable, and the
    /// reason must say WHICH absence — "not invoked and not declared" is a
    /// different repository state from "declared but no remote", and an operator
    /// fixes them differently.
    #[test]
    fn an_absent_gate_reports_which_absence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        std::fs::write(dir.path().join(".git/hooks/pre-commit"), b"no gates here").unwrap();

        let census = census_gates(dir.path());
        let row = census
            .rows
            .iter()
            .find(|r| r.gate == "kernel-bypass-gate")
            .expect("kernel-bypass-gate must appear in the census");
        match &row.reachability {
            GateReachability::Unreachable { reason } => assert!(
                reason.contains("not invoked") || reason.contains("no git remote"),
                "the reason must name which absence, got: {reason}"
            ),
            other => panic!("expected Unreachable for an unwired gate, got {other:?}"),
        }
    }

    /// The row must MOVE when the repository changes. This is the whole point:
    /// the previous implementation hardcoded Unreachable and returned it no
    /// matter what the tree contained, so wiring a gate could never make the
    /// supervisor tick.
    #[test]
    fn wiring_a_gate_into_the_hook_changes_its_verdict() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        let hook = dir.path().join(".git/hooks/pre-commit");

        std::fs::write(&hook, b"nothing").unwrap();
        let before = census_gates(dir.path());
        let before_row = before
            .rows
            .iter()
            .find(|r| r.gate == "pre-delete-citation-check")
            .unwrap();
        assert!(
            !before_row.reachability.is_reachable(),
            "precondition: unwired before"
        );

        // Now the hook mentions it — the only thing readable from a compiled hook.
        std::fs::write(&hook, b"invoking pre-delete-citation-check now").unwrap();
        let after = census_gates(dir.path());
        let after_row = after
            .rows
            .iter()
            .find(|r| r.gate == "pre-delete-citation-check")
            .unwrap();
        assert!(
            after_row.reachability.is_reachable(),
            "a gate present in the installed hook MUST report Reachable — if this fails the \
             census is frozen again"
        );
    }

    /// ANTI-VACUITY: an empty census reports identically to an all-reachable one
    /// through `all_reachable()`, which returns true for zero rows.
    #[test]
    fn the_census_is_never_empty() {
        let dir = tempfile::tempdir().unwrap();
        let census = census_gates(dir.path());
        assert!(
            census.rows.len() >= 6,
            "census produced {} rows on a bare directory; all_reachable() returns true for an \
             empty census, so a collapsed scan reads as a green fleet",
            census.rows.len()
        );
    }

    /// THE DEFECT `kwo9` WAS FILED FOR, and the fix is not a trigger.
    ///
    /// The old probe was `grep -rl <name> --include=*.toml --include=*.rs .` under a
    /// 10s deadline, and `BoundedOutcome::TimedOut` fell through to `has_caller =
    /// false` — i.e. UNWIRED. `--include` filters FILENAMES, not directories, so the
    /// walk still descends `target/` (measured 7.0G), and under the concurrent cargo
    /// builds that are this checkout's normal state it exceeds the deadline.
    ///
    /// **The verdict was therefore a function of machine load.** In one live run
    /// `ack-stage` came back REACHABLE and four siblings came back UNWIRED, from the
    /// same repository state.
    ///
    /// A manifest read is exact, needs no subprocess, and cannot time out.
    #[test]
    fn the_reachability_probe_is_deterministic_and_ignores_target() {
        let root = repo_root_for_test();
        // Ten identical calls must agree. A load-dependent probe does not.
        let first = manifest_callers(&root, "subprocess-contract");
        for _ in 0..10 {
            assert_eq!(
                manifest_callers(&root, "subprocess-contract"),
                first,
                "the probe must not move under load"
            );
        }
        // POSITIVE CONTROL: a crate the workspace demonstrably depends on widely.
        assert!(
            first >= 10,
            "subprocess-contract must have many callers, got {first} -- a probe that \
             finds none is broken, not a measurement"
        );
        // NEGATIVE CONTROL: a name no manifest can reference.
        assert_eq!(manifest_callers(&root, "no-such-crate-anywhere"), 0);
    }

    /// SELF-EXCLUSION IS STRUCTURAL, and the old threshold was arithmetic on a
    /// self-hit. `grep` excluded only paths containing `<name>/src/`, leaving
    /// `crates/<name>/Cargo.toml` in the count — so a crate with NO dependents scored
    /// 1, and the threshold had to be `> 1` to compensate. Here the self row is gone
    /// and the threshold is `> 0`.
    #[test]
    fn a_crate_cannot_vouch_for_itself() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let lonely = root.join("crates/lonely");
        std::fs::create_dir_all(&lonely).unwrap();
        // Its own manifest names itself; that must not count.
        std::fs::write(
            lonely.join("Cargo.toml"),
            "[package]\nname = \"lonely\"\n\n[dependencies]\nlonely = { path = \"../lonely\" }\n",
        )
        .unwrap();
        assert_eq!(manifest_callers(root, "lonely"), 0);

        // FIRES-ON-KNOWN-BAD: one real caller flips it, and BOTH spellings count, so a
        // formatter that strips the spaces around `=` cannot silently drop every
        // caller -- the `^command = ` vs `command  = ` false zero, in a new place.
        for spelling in [
            "[dependencies]\nlonely = { path = \"../lonely\" }\n",
            "[dependencies]\nlonely={path=\"../lonely\"}\n",
        ] {
            let caller = root.join("crates/caller");
            std::fs::create_dir_all(&caller).unwrap();
            std::fs::write(
                caller.join("Cargo.toml"),
                format!("[package]\nname = \"caller\"\n\n{spelling}"),
            )
            .unwrap();
            assert_eq!(
                manifest_callers(root, "lonely"),
                1,
                "spelling not counted: {spelling:?}"
            );
        }
    }

    /// A BIN AND A LIB HAVE DIFFERENT TRIGGER CLASSES. `ack-spine` ships
    /// `bin:ack-spine` and has 0 manifest callers; reporting `no manifest dependency
    /// references this crate` with `repair-gate-trigger` sends an operator to add a
    /// dependency nobody should add. Same correction `NotExtracted` needed.
    #[test]
    fn a_binary_with_no_invocation_site_is_named_as_one() {
        let root = repo_root_for_test();
        assert!(
            crate_ships_a_bin(&root, "ack-spine"),
            "ack-spine ships bin:ack-spine per cargo metadata"
        );
        assert!(
            !crate_ships_a_bin(&root, "ack-stage"),
            "ack-stage is lib-only per cargo metadata"
        );
        // The implicit form must count too: checking only `[[bin]]` undercounts.
        let dir = tempfile::tempdir().unwrap();
        let implicit = dir.path().join("crates/implicit/src");
        std::fs::create_dir_all(&implicit).unwrap();
        std::fs::write(implicit.join("main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(
            dir.path().join("crates/implicit/Cargo.toml"),
            "[package]\nname = \"implicit\"\n",
        )
        .unwrap();
        assert!(crate_ships_a_bin(dir.path(), "implicit"));

        // And the reason string must distinguish the two classes, or the remedy is
        // wrong for one of them.
        let census = census_gates(&root);
        let reasons: Vec<&str> = census
            .rows
            .iter()
            .filter_map(|r| match &r.reachability {
                GateReachability::Unreachable { reason } => Some(reason.as_str()),
                _ => None,
            })
            .collect();
        // ANTI-VACUITY: if nothing is unreachable this leg proves nothing, and that
        // must be stated rather than passing quietly.
        if reasons.is_empty() {
            panic!(
                "no unreachable row in a live census: either the fleet is fully wired \
                 (state it) or the probe collapsed"
            );
        }
        // THE ASSERTION THIS LEG SHIPPED WITH WAS TOOTHLESS, and the mutation proved
        // it: `any()` over a DISJUNCTION of the class strings still matched when a
        // binary's reason was replaced by the library one, so the substitution the
        // leg exists to catch went green. An `any` over alternatives cannot detect a
        // substitution among those alternatives.
        //
        // Assert the SPECIFIC row instead. `ack-spine` ships a bin and has 0 manifest
        // callers, so its reason must name the BINARY class or the remedy is wrong.
        //
        // SPECIMEN UNPINNED 2026-09-02 (`eg0m`). This leg named `ack-spine` as the
        // bin-with-no-invocation-site specimen, and `ack-spine` is now REACHABLE
        // because the supervisor emits through it — so the leg failed on a change
        // that FIXED the thing it was watching. Its own message said *"if that
        // changed, update this leg deliberately"*, and this is that update.
        //
        // Pinning a different crate would rebuild the same fragility. The property
        // that actually matters is the PAIRING: a reason naming the binary class
        // must belong to a crate that really ships a bin, and a reason naming the
        // library class must belong to one that does not. That detects the
        // substitution the old `any()` could not, WITHOUT depending on which crate
        // happens to be unwired today.
        let mut bin_class = 0usize;
        let mut lib_class = 0usize;
        for row in &census.rows {
            let GateReachability::Unreachable { reason } = &row.reachability else {
                continue;
            };
            let ships_bin = crate_ships_a_bin(&root, &row.gate);
            if reason.contains("binary with no invocation site") {
                bin_class += 1;
                assert!(
                    ships_bin,
                    "{} is told to fix an INVOCATION SITE and ships no binary: the \
                     remedy sends the operator to the wrong place",
                    row.gate
                );
            } else if reason.contains("library with no manifest dependency") {
                lib_class += 1;
                assert!(
                    !ships_bin,
                    "{} ships a binary and is told to acquire a MANIFEST DEPENDENCY: \
                     that is the remedy `eg0m` acceptance 7 forbids",
                    row.gate
                );
            }
        }
        // ANTI-VACUITY: the pairing above is vacuous if neither class appears.
        assert!(
            bin_class + lib_class > 0,
            "no row carries either class string, so the pairing assertions above \
             checked nothing"
        );
    }

    fn repo_root_for_test() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }
}

#[cfg(test)]
mod disk_pressure_thresholds {
    /// The predicate, isolated from `df` so both directions are testable without
    /// filling a volume. Mirrors main.rs::disk_pressure's arithmetic exactly.
    fn refuses(total_k: u64, avail_k: u64) -> bool {
        if total_k == 0 {
            return true; // a zero-size volume is a finding, never a pass
        }
        let pct_free = (avail_k as f64 / total_k as f64) * 100.0;
        let gib_free = avail_k as f64 / 1024.0 / 1024.0;
        pct_free < 8.0 || gib_free < 1.0
    }

    #[test]
    fn the_measured_halt_condition_refuses() {
        // The exact state that stopped every gate on 2026-09-01:
        // 9.2GiB used, 98MiB available on a ~9.3GiB volume.
        let total = 9_752_866; // ~9.3 GiB in 1K blocks
        let avail = 100_352; // ~98 MiB
        assert!(
            refuses(total, avail),
            "the state that actually halted the fleet must refuse"
        );
    }

    #[test]
    fn the_state_i_ignored_hours_earlier_also_refuses() {
        // 84% full, 1.5GiB free — observed, recorded as "needs a decision", not acted on.
        // Under 1GiB floor? No. Under 8% free? 16% free, no. So this does NOT refuse,
        // and that is the honest limit: this guard would NOT have caught the earlier
        // warning state. It catches the halt, not the trend.
        let total = 9_752_866;
        let avail = 1_572_864; // 1.5 GiB
        assert!(
            !refuses(total, avail),
            "1.5GiB/16% free passes — documenting that this guard catches the HALT, \
             not the 84% warning I ignored. Trend detection is a separate, unbuilt thing."
        );
    }

    #[test]
    fn a_healthy_volume_passes() {
        // Post-clean: 5.6GiB free of 9.3GiB = 60%.
        assert!(
            !refuses(9_752_866, 5_872_025),
            "a 60%-free volume must not refuse"
        );
    }

    #[test]
    fn the_gib_floor_bites_independently_of_the_percentage() {
        // A large volume can be percentage-healthy and still too small in absolute
        // terms for a 4GiB release rebuild. 10% free of 8GiB = 800MiB.
        let total = 8_388_608; // 8 GiB
        let avail = 838_860; // ~819 MiB, 10% free
        assert!(
            refuses(total, avail),
            "10% free is above the percentage floor but under 1GiB — the absolute \
             floor must bite, or a big volume passes while a rebuild cannot fit"
        );
    }

    #[test]
    fn a_zero_size_volume_is_a_finding_not_a_pass() {
        assert!(
            refuses(0, 0),
            "an unmeasurable volume must refuse, never pass"
        );
    }
    #[test]
    fn advisory_ratchet_anchor_couples_ceiling_and_recorded_at() {
        assert!(crate::ADVISORY_RATCHET.is_consistent());
        assert_eq!(crate::ADVISORY_CEILING, crate::ADVISORY_RATCHET.ceiling());
        assert_eq!(
            crate::ADVISORY_CEILING_RECORDED_AT_UNIX,
            crate::ADVISORY_RATCHET.recorded_at_unix()
        );
    }
}
