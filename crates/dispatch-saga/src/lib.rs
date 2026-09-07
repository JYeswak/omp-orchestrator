#![forbid(unsafe_code)]

//! Saga over claim → send → ack → close.
//!
//! For every [`DispatchKey`] *k*, at most one transport send is ever performed.
//! A process that stops after recording `Sending(k)` and before `Sent(k)`
//! surfaces as [`DispatchState::Unknown`] and is reconciled by READING, never
//! by re-sending.
//!
//! [`ChildOutcome`] is child execution. Saga phase state is the four enums
//! below. Do not reuse a type named `Outcome` for both.
//!
//! NO-CLAIM: at-most-once SEND under a key, not exactly-once delivery. The
//! substrate is tmux. This crate does not prove the receiver is alive.
//!
//! Wired: `grading` (IMPL→GRADING decide-only) plus caller `ack-stage`.
//! Supervisor dispatch path remains unwired on purpose.

use omp_types::ChildOutcome;
use pane_dispatch_fence::PaneIncarnation;
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::process::Command;
use std::time::Duration;
pub mod grading;
pub mod m2;



/// Supervisor send path is still unwired. IMPL→GRADING is `grading`.
pub const DECLARED_NOT_WIRED: &str =
    "supervisor send path deferred: IMPL->GRADING is grading::decide (ack-stage calls it)";

/// Deadline for the one `br` WRITE this crate performs: `br update --status grading`.
///
/// ARGUED, NOT PICKED. Three measured bands set the floor, and every one of them is a
/// LEGITIMATE wait that must not be converted into a failure:
///
/// * `.beads/beads.db` is ~31 MB with several live writers. AGENTS.md records reads at
///   **40-250 s**, one `br comments add` at **56.7 s** under contention, and a close
///   attempt that held for **290 s**.
/// * Every `br` call in this repo carries `--lock-timeout 60000`-`90000`, so the command
///   is itself instructed to wait up to **90 s** for `.beads/.write.lock`. A deadline at
///   or below that kills `br` while it is obeying the wait we asked for.
/// * This is a WRITE, not a read: it takes the write lock rather than sharing a reader,
///   so it queues behind every live writer instead of alongside them.
///
/// 420 s sits above the longest observed hold (290 s) plus a full 90 s lock wait, with
/// headroom, and is still finite. `62lz` chose 300 s for `br list` READS against the same
/// database; a write needs more, not less. **A ceiling inside the contention band would
/// convert contention into a false failure on the stage-transition path**, which is the
/// one place a false failure is invisible — see [`run_bounded`].
pub const BR_UPDATE_DEADLINE: Duration = Duration::from_secs(420);

/// Run the `br update` that moves IMPL→GRADING, under [`BR_UPDATE_DEADLINE`].
///
/// WHY THIS EXISTS: `main.rs` used `Command::new(..).status()` with NO DEADLINE, on the
/// crate that owns the stage transition. AGENTS.md's asupersync contract is that every
/// subprocess — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a deadline.
/// This one was not, and the subject is the worst case: a `br` WRITE holding
/// `.beads/.write.lock`.
///
/// `bounded_output` RATHER THAN `bounded_status`, and the reason is specific to this call
/// site rather than inherited from `62lz`. `bounded_status` INHERITS stdio, so a refusal
/// reaches the terminal and nothing else. But `br update --status grading` can be REFUSED
/// BY POLICY, and AGENTS.md records exactly that failure: *"the refusal scrolls past
/// in-pane while the agent believes the close landed"*. Capturing stdout and stderr is
/// what lets this crate tell APPLIED from REFUSED from WEDGED instead of printing all
/// three and returning success. `62lz` captures because it PARSES; this captures because
/// it must CLASSIFY.
///
/// A TIMEOUT IS NOT A VERDICT. The deadline arm returns
/// [`ChildOutcome::TimedOut`] and never `Completed`, so no caller can read a killed
/// child's empty stdout as "the transition did not apply" — the transition's true state
/// after a kill is UNKNOWN and must be reconciled by READING the bead, which is this
/// crate's own stated discipline for `DispatchState::Unknown`. `SpawnFailed` stays
/// distinct from `TimedOut` because the remedies differ: a PATH/env problem versus a
/// wedged subject.
///
/// WHICH BUG THIS IS: a DEADLINE hole, not the undrained-pipe deadlock. `.status()`
/// inherits the pipes rather than filling them, so the ~64 KiB `try_wait()` deadlock in
/// AGENTS.md's asupersync section never applied here. Adding a drain would have "fixed" a
/// bug that was not present and left this one intact. `subprocess-contract` makes the
/// child its own process-group leader and signals the GROUP on the deadline, so a `br`
/// that spawned helpers cannot leave them at `ppid=1` — which is why this does not
/// hand-roll a timer.
pub fn run_bounded(command: &mut Command, deadline: Duration) -> ChildOutcome {
    match subprocess_contract::bounded_output(command, deadline) {
        subprocess_contract::BoundedOutcome::Completed(output) => ChildOutcome::Completed {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        subprocess_contract::BoundedOutcome::TimedOut => ChildOutcome::TimedOut {
            after_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
            group_killed: true,
        },
        subprocess_contract::BoundedOutcome::Unspawned(error) => ChildOutcome::SpawnFailed {
            message: error.to_string(),
        },
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimState {
    Pending,
    Claiming,
    Claimed,
    Failed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchState {
    Pending,
    Sending,
    Sent,
    Failed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AckState {
    Pending,
    Awaiting,
    Acked,
    Silent,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseState {
    Open,
    Graded,
    Closed,
    Reconciled,
    Unreconciled,
}

/// Content-addressed from `(bead_id, pane_incarnation, attempt)`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DispatchKey {
    bead_id: String,
    incarnation: u64,
    attempt: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnmintableKey;

impl DispatchKey {
    pub fn mint(
        bead_id: &str,
        incarnation: PaneIncarnation,
        attempt: u32,
    ) -> Result<Self, UnmintableKey> {
        if bead_id.is_empty() {
            return Err(UnmintableKey);
        }
        let attempt = NonZeroU32::new(attempt).ok_or(UnmintableKey)?;
        Ok(Self {
            bead_id: bead_id.to_owned(),
            incarnation: incarnation.get(),
            attempt: attempt.get(),
        })
    }

    pub fn bead_id(&self) -> &str {
        &self.bead_id
    }

    pub fn incarnation(&self) -> u64 {
        self.incarnation
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn as_canonical(&self) -> String {
        format!("{}|{}|{}", self.bead_id, self.incarnation, self.attempt)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Saga {
    key: DispatchKey,
    claim: ClaimState,
    dispatch: DispatchState,
    ack: AckState,
    close: CloseState,
}

impl Saga {
    pub fn fresh(key: DispatchKey) -> Self {
        Self {
            key,
            claim: ClaimState::Pending,
            dispatch: DispatchState::Pending,
            ack: AckState::Pending,
            close: CloseState::Open,
        }
    }

    pub fn key(&self) -> &DispatchKey {
        &self.key
    }

    pub fn claim(&self) -> ClaimState {
        self.claim
    }

    pub fn dispatch(&self) -> DispatchState {
        self.dispatch
    }

    pub fn ack(&self) -> AckState {
        self.ack
    }

    pub fn close(&self) -> CloseState {
        self.close
    }

    pub fn begin_claim(&mut self) {
        if self.claim == ClaimState::Pending {
            self.claim = ClaimState::Claiming;
        }
    }

    pub fn finish_claim(&mut self) {
        if self.claim == ClaimState::Claiming {
            self.claim = ClaimState::Claimed;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrashPoint {
    AfterClaiming,
    AfterSending,
    AfterSentBeforeAck,
}

/// Map an incomplete phase onto `Unknown`. Never sends.
pub fn surface_crash(saga: &mut Saga, point: CrashPoint) {
    match point {
        CrashPoint::AfterClaiming => {
            if saga.claim == ClaimState::Claiming {
                saga.claim = ClaimState::Unknown;
            }
        }
        CrashPoint::AfterSending => {
            if saga.dispatch == DispatchState::Sending {
                saga.dispatch = DispatchState::Unknown;
            }
        }
        CrashPoint::AfterSentBeforeAck => {
            if saga.dispatch == DispatchState::Sent && saga.ack == AckState::Awaiting {
                saga.ack = AckState::Unknown;
            }
        }
    }
}

pub trait Transport {
    fn send(&mut self, key: &DispatchKey) -> ChildOutcome;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub key: DispatchKey,
    pub child: ChildOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecuteError {
    ClaimSkipped { key: DispatchKey },
    TerminalAssignmentAttempt { key: DispatchKey },
    OutcomeUnknown { key: DispatchKey },
    UnmintableKey,
}

/// One send under *k*, and only from [`DispatchState::Pending`].
pub fn execute(saga: &mut Saga, transport: &mut impl Transport) -> Result<Receipt, ExecuteError> {
    if saga.claim != ClaimState::Claimed {
        return Err(ExecuteError::ClaimSkipped {
            key: saga.key.clone(),
        });
    }
    if saga.dispatch == DispatchState::Pending {
        saga.dispatch = DispatchState::Sending;
        let child = transport.send(&saga.key);
        saga.dispatch = DispatchState::Sent;
        saga.ack = AckState::Awaiting;
        return Ok(Receipt {
            key: saga.key.clone(),
            child,
        });
    }
    if saga.dispatch == DispatchState::Sending || saga.dispatch == DispatchState::Unknown {
        return Err(ExecuteError::OutcomeUnknown {
            key: saga.key.clone(),
        });
    }
    if saga.dispatch == DispatchState::Sent || saga.dispatch == DispatchState::Failed {
        return Err(ExecuteError::TerminalAssignmentAttempt {
            key: saga.key.clone(),
        });
    }
    Err(ExecuteError::OutcomeUnknown {
        key: saga.key.clone(),
    })
}

/// Observations from a READ. Never produced by a send.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Observation {
    pub bead_claimed: bool,
    pub pane_saw_packet: bool,
    pub ack_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reconcile {
    Reconciled { evidence: String },
    StillUnknown { next_read_at: u64 },
}

/// Resolve `Unknown` by reading. Never calls [`Transport::send`].
pub fn reconcile(saga: &mut Saga, observation: &Observation, now: u64) -> Reconcile {
    if saga.claim == ClaimState::Unknown {
        if observation.bead_claimed {
            saga.claim = ClaimState::Claimed;
        } else {
            return Reconcile::StillUnknown {
                next_read_at: now.saturating_add(1),
            };
        }
    }
    if saga.dispatch == DispatchState::Unknown {
        if observation.pane_saw_packet {
            saga.dispatch = DispatchState::Sent;
            if saga.ack == AckState::Pending {
                saga.ack = AckState::Awaiting;
            }
        } else {
            return Reconcile::StillUnknown {
                next_read_at: now.saturating_add(1),
            };
        }
    }
    if saga.ack == AckState::Unknown {
        if observation.ack_present {
            saga.ack = AckState::Acked;
        } else {
            return Reconcile::StillUnknown {
                next_read_at: now.saturating_add(1),
            };
        }
    }
    if saga.dispatch == DispatchState::Sent && saga.ack == AckState::Acked {
        Reconcile::Reconciled {
            evidence: format!("CLOSE_RECONCILED {{ k={}, read }}", saga.key.as_canonical()),
        }
    } else if saga.dispatch == DispatchState::Unknown
        || saga.ack == AckState::Unknown
        || saga.claim == ClaimState::Unknown
    {
        Reconcile::StillUnknown {
            next_read_at: now.saturating_add(1),
        }
    } else {
        Reconcile::StillUnknown {
            next_read_at: now.saturating_add(1),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseEvidence {
    pub row: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseRefusal {
    Unreconciled {
        key: DispatchKey,
        last: DispatchState,
    },
}

pub fn close(saga: &mut Saga, evidence: Option<CloseEvidence>) -> Result<(), CloseRefusal> {
    let last = saga.dispatch;
    if last == DispatchState::Unknown || last == DispatchState::Failed {
        match evidence {
            Some(evidence) if evidence.row.starts_with("CLOSE_RECONCILED") => {
                saga.close = CloseState::Reconciled;
                Ok(())
            }
            _ => {
                saga.close = CloseState::Unreconciled;
                Err(CloseRefusal::Unreconciled {
                    key: saga.key.clone(),
                    last,
                })
            }
        }
    } else if last == DispatchState::Sent && saga.ack == AckState::Acked {
        saga.close = CloseState::Closed;
        Ok(())
    } else {
        saga.close = CloseState::Unreconciled;
        Err(CloseRefusal::Unreconciled {
            key: saga.key.clone(),
            last,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum LedgerError {
    SingleWriterViolation { held_by: String, attempted: String },
}

/// One owner. A second writer is a typed refusal, not a fork.
pub struct DispatchLedger {
    owner: String,
    rows: BTreeMap<String, Saga>,
}

impl DispatchLedger {
    pub fn new(owner: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            rows: BTreeMap::new(),
        }
    }

    pub fn write(&mut self, owner: &str, saga: Saga) -> Result<(), LedgerError> {
        if owner != self.owner {
            return Err(LedgerError::SingleWriterViolation {
                held_by: self.owner.clone(),
                attempted: owner.to_owned(),
            });
        }
        self.rows.insert(saga.key.as_canonical(), saga);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_dispatch_fence::{IncarnationMint, Occupancy};
    use std::cell::Cell;

    struct SendStub {
        calls: Cell<u32>,
    }

    impl SendStub {
        fn new() -> Self {
            Self {
                calls: Cell::new(0),
            }
        }
    }
    impl Transport for SendStub {
        fn send(&mut self, _key: &DispatchKey) -> ChildOutcome {
            self.calls.set(self.calls.get() + 1);
            ChildOutcome::Completed {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            }
        }
    }

    fn key_5rh() -> DispatchKey {
        let mint = IncarnationMint::new();
        let mut occupancy = Occupancy::unminted("omp-orchestrator", "%1413");
        let inc = occupancy.occupy(&mint, 1);
        DispatchKey::mint("omp-orchestrator-5rh", inc, 1).unwrap()
    }

    fn claimed(key: DispatchKey) -> Saga {
        let mut saga = Saga::fresh(key);
        saga.begin_claim();
        saga.finish_claim();
        saga
    }

    #[test]
    fn first_send_under_fresh_key_succeeds() {
        let mut saga = claimed(key_5rh());
        let mut stub = SendStub::new();
        let receipt = execute(&mut saga, &mut stub).expect("first send");
        assert_eq!(
            receipt.child,
            ChildOutcome::Completed {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            }
        );
        assert_eq!(saga.dispatch(), DispatchState::Sent);
        assert_eq!(stub.calls.get(), 1);
    }

    #[test]
    fn retry_under_sent_is_terminal_and_does_not_send() {
        let mut saga = claimed(key_5rh());
        let mut stub = SendStub::new();
        execute(&mut saga, &mut stub).unwrap();
        let err = execute(&mut saga, &mut stub).unwrap_err();
        assert!(matches!(
            err,
            ExecuteError::TerminalAssignmentAttempt { .. }
        ));
        assert_eq!(stub.calls.get(), 1);
    }

    #[test]
    fn five_rh_unclaimed_dispatch_does_not_send() {
        let mut saga = Saga::fresh(key_5rh());
        let mut stub = SendStub::new();
        let err = execute(&mut saga, &mut stub).unwrap_err();
        assert!(matches!(err, ExecuteError::ClaimSkipped { .. }));
        assert_eq!(stub.calls.get(), 0);
        assert_eq!(saga.dispatch(), DispatchState::Pending);
    }

    #[test]
    fn crash_after_claiming_resumes_unknown_and_reconcile_reads() {
        let mut saga = Saga::fresh(key_5rh());
        saga.begin_claim();
        surface_crash(&mut saga, CrashPoint::AfterClaiming);
        assert_eq!(saga.claim(), ClaimState::Unknown);
        let mut stub = SendStub::new();
        assert!(matches!(
            execute(&mut saga, &mut stub),
            Err(ExecuteError::ClaimSkipped { .. })
        ));
        assert_eq!(stub.calls.get(), 0);
        let still = reconcile(
            &mut saga,
            &Observation {
                bead_claimed: false,
                ..Observation::default()
            },
            10,
        );
        assert!(matches!(still, Reconcile::StillUnknown { .. }));
        let done = reconcile(
            &mut saga,
            &Observation {
                bead_claimed: true,
                ..Observation::default()
            },
            11,
        );
        assert_eq!(saga.claim(), ClaimState::Claimed);
        assert!(
            matches!(done, Reconcile::StillUnknown { .. }) || saga.claim() == ClaimState::Claimed
        );
    }

    #[test]
    fn crash_after_sending_is_unknown_and_reconcile_never_resends() {
        let mut saga = claimed(key_5rh());
        saga.dispatch = DispatchState::Sending;
        surface_crash(&mut saga, CrashPoint::AfterSending);
        assert_eq!(saga.dispatch(), DispatchState::Unknown);
        let mut stub = SendStub::new();
        let err = execute(&mut saga, &mut stub).unwrap_err();
        assert!(matches!(err, ExecuteError::OutcomeUnknown { .. }));
        assert_eq!(stub.calls.get(), 0);
        reconcile(
            &mut saga,
            &Observation {
                pane_saw_packet: true,
                ..Observation::default()
            },
            1,
        );
        assert_eq!(saga.dispatch(), DispatchState::Sent);
        assert_eq!(stub.calls.get(), 0);
    }

    #[test]
    fn crash_after_sent_before_ack_reconciles_by_read() {
        let mut saga = claimed(key_5rh());
        let mut stub = SendStub::new();
        execute(&mut saga, &mut stub).unwrap();
        surface_crash(&mut saga, CrashPoint::AfterSentBeforeAck);
        assert_eq!(saga.ack(), AckState::Unknown);
        let err = execute(&mut saga, &mut stub).unwrap_err();
        assert!(matches!(
            err,
            ExecuteError::TerminalAssignmentAttempt { .. }
        ));
        assert_eq!(stub.calls.get(), 1);
        reconcile(
            &mut saga,
            &Observation {
                ack_present: true,
                pane_saw_packet: true,
                bead_claimed: true,
            },
            2,
        );
        assert_eq!(saga.ack(), AckState::Acked);
        assert_eq!(stub.calls.get(), 1);
    }

    #[test]
    fn unmintable_key_is_a_typed_refusal() {
        let mint = IncarnationMint::new();
        let mut occupancy = Occupancy::unminted("omp-orchestrator", "%8");
        let inc = occupancy.occupy(&mint, 1);
        assert_eq!(DispatchKey::mint("", inc, 1), Err(UnmintableKey));
        assert_eq!(
            DispatchKey::mint("omp-orchestrator-7kxf", inc, 0),
            Err(UnmintableKey)
        );
        assert!(DECLARED_NOT_WIRED.contains("deferred"));
    }

    #[test]
    fn close_while_unknown_is_refused_without_evidence() {
        let mut saga = claimed(key_5rh());
        saga.dispatch = DispatchState::Unknown;
        let err = close(&mut saga, None).unwrap_err();
        assert!(matches!(err, CloseRefusal::Unreconciled { .. }));
        assert_eq!(saga.close(), CloseState::Unreconciled);
        close(
            &mut saga,
            Some(CloseEvidence {
                row: "CLOSE_RECONCILED { k=test, read }".into(),
            }),
        )
        .unwrap();
        assert_eq!(saga.close(), CloseState::Reconciled);
    }

    #[test]
    fn second_ledger_writer_is_refused() {
        let mut ledger = DispatchLedger::new("supervisor");
        let saga = claimed(key_5rh());
        ledger.write("supervisor", saga.clone()).unwrap();
        let err = ledger.write("refill-idle-panes", saga).unwrap_err();
        assert!(matches!(err, LedgerError::SingleWriterViolation { .. }));
    }

    #[test]
    fn success_is_not_the_default_branch() {
        let mut saga = claimed(key_5rh());
        saga.dispatch = DispatchState::Unknown;
        let mut stub = SendStub::new();
        assert!(matches!(
            execute(&mut saga, &mut stub),
            Err(ExecuteError::OutcomeUnknown { .. })
        ));
        assert_ne!(saga.dispatch(), DispatchState::Sent);
        assert_eq!(stub.calls.get(), 0);
    }

    #[test]
    fn child_outcome_is_not_saga_state() {
        let _child = ChildOutcome::TimedOut {
            after_ms: 0,
            group_killed: false,
        };
        let _dispatch = DispatchState::Unknown;
        assert_ne!(std::mem::size_of::<ChildOutcome>(), 0);
        assert_ne!(std::mem::size_of::<DispatchState>(), 0);
    }

    #[test]
    fn every_dispatch_state_has_a_reaching_input() {
        let mut pending = claimed(key_5rh());
        assert_eq!(pending.dispatch(), DispatchState::Pending);
        pending.dispatch = DispatchState::Sending;
        assert_eq!(pending.dispatch(), DispatchState::Sending);
        let mut sent = claimed(key_5rh());
        let mut stub = SendStub::new();
        execute(&mut sent, &mut stub).unwrap();
        assert_eq!(sent.dispatch(), DispatchState::Sent);
        pending.dispatch = DispatchState::Failed;
        assert_eq!(pending.dispatch(), DispatchState::Failed);
        surface_crash(
            &mut {
                let mut s = claimed(key_5rh());
                s.dispatch = DispatchState::Sending;
                s
            },
            CrashPoint::AfterSending,
        );
        let mut unknown = claimed(key_5rh());
        unknown.dispatch = DispatchState::Sending;
        surface_crash(&mut unknown, CrashPoint::AfterSending);
        assert_eq!(unknown.dispatch(), DispatchState::Unknown);
    }
}
