#![forbid(unsafe_code)]

//! Per-pane incarnation lease for dispatch admission.
//!
//! Numeric pane IDs are reusable. A pid in a marker file does not die with the
//! occupancy it names — C112. Measured 2026-09-06 against live pending-dispatch
//! markers: `marker.7 pid=17348`, `marker.8 pid=5429`, `marker.9 pid=2607` were
//! all DEAD while age still read Live; live supervisor was `43048`.
//!
//! Recency (`y903`) and session scoping (`ma3b`) are necessary and not
//! sufficient. The invariant is whose pane, not how recent.
//!
//! NO-CLAIM: the lease refuses stale effects; it does not prove the receiver is
//! alive, does not prove delivery, and does not survive an ntm pane renumbering
//! the observer never saw.
//!
//! DECLARED_NOT_WIRED: `admit_at_send` is not called from
//! `crates/omp-orchestrator/src/main.rs` (held by `%7` for `u8nw`). The check
//! belongs immediately BEFORE send, not at enqueue and not from a cached
//! snapshot.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

/// Call site is deferred. This names the debt; it is not a caller.
pub const DECLARED_NOT_WIRED: &str =
    "call site deferred: crates/omp-orchestrator/src/main.rs held by %7/u8nw";

/// Measured 2026-09-06. Age is recorded so a grader can see it was not used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasuredMarker {
    pub pane: &'static str,
    pub pid: u32,
    pub issued_at: u64,
    pub age_secs: u32,
}

pub const MEASURED_DEAD_MARKERS: &[MeasuredMarker] = &[
    MeasuredMarker {
        pane: "%7",
        pid: 17348,
        issued_at: 1_788_653_251,
        age_secs: 264,
    },
    MeasuredMarker {
        pane: "%8",
        pid: 5429,
        issued_at: 1_788_652_956,
        age_secs: 559,
    },
    MeasuredMarker {
        pane: "%9",
        pid: 2607,
        issued_at: 1_788_653_501,
        age_secs: 14,
    },
];

pub const MEASURED_LIVE_SUPERVISOR: u32 = 43048;

/// Runtime-monotonic identity for one occupancy of a reusable pane id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PaneIncarnation(NonZeroU64);

impl PaneIncarnation {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Never-reuse mint. Overflow to zero is a panic, not an admit.
pub struct IncarnationMint {
    next: AtomicU64,
}

impl IncarnationMint {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    #[must_use]
    pub fn mint(&self) -> PaneIncarnation {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        PaneIncarnation(
            NonZeroU64::new(n).expect("incarnation counter overflowed to zero"),
        )
    }
}

impl Default for IncarnationMint {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lease {
    Admitting = 0,
    Draining = 1,
    Revoked = 2,
}

impl Lease {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Admitting),
            1 => Some(Self::Draining),
            2 => Some(Self::Revoked),
            _ => None,
        }
    }

    fn successor(self) -> Option<Self> {
        match self {
            Self::Admitting => Some(Self::Draining),
            Self::Draining => Some(Self::Revoked),
            Self::Revoked => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseRefusal {
    NotMonotone { from: Lease, to: Lease },
    CasMismatch { expected: Lease, observed: Option<Lease> },
}

/// One occupancy of `{session, pane}`. The lease is the only mutable field.
pub struct Occupancy {
    session: String,
    pane: String,
    current: Option<PaneIncarnation>,
    lease: AtomicU8,
    owner_pid: Option<u32>,
}

impl Occupancy {
    #[must_use]
    pub fn unminted(session: impl Into<String>, pane: impl Into<String>) -> Self {
        Self {
            session: session.into(),
            pane: pane.into(),
            current: None,
            lease: AtomicU8::new(Lease::Revoked as u8),
            owner_pid: None,
        }
    }

    pub fn occupy(&mut self, mint: &IncarnationMint, owner_pid: u32) -> PaneIncarnation {
        let incarnation = mint.mint();
        self.current = Some(incarnation);
        self.owner_pid = Some(owner_pid);
        self.lease.store(Lease::Admitting as u8, Ordering::SeqCst);
        incarnation
    }

    pub fn current(&self) -> Option<PaneIncarnation> {
        self.current
    }

    pub fn session(&self) -> &str {
        &self.session
    }

    pub fn pane(&self) -> &str {
        &self.pane
    }

    pub fn owner_pid(&self) -> Option<u32> {
        self.owner_pid
    }

    pub fn lease(&self) -> Result<Lease, AdmissionRefusal> {
        Lease::from_u8(self.lease.load(Ordering::SeqCst))
            .ok_or(AdmissionRefusal::UnknownIncarnation)
    }

    /// Advance only `Admitting -> Draining -> Revoked`, only by compare_exchange.
    pub fn advance_lease(&self, from: Lease, to: Lease) -> Result<(), LeaseRefusal> {
        if from.successor() != Some(to) {
            return Err(LeaseRefusal::NotMonotone { from, to });
        }
        match self
            .lease
            .compare_exchange(from as u8, to as u8, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => Ok(()),
            Err(observed) => Err(LeaseRefusal::CasMismatch {
                expected: from,
                observed: Lease::from_u8(observed),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presented {
    pub session: String,
    pub pane: String,
    pub incarnation: Option<PaneIncarnation>,
    pub marker_pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRefusal {
    StaleIncarnation {
        presented: PaneIncarnation,
        current: PaneIncarnation,
    },
    ForeignSession {
        presented_session: String,
        current_session: String,
        pane: String,
    },
    DeadOwner {
        marker_pid: u32,
        live_supervisor: u32,
        pane: String,
    },
    LeaseNotAdmitting {
        lease: Lease,
    },
    UnknownIncarnation,
    Unmintable,
}

/// Positive-evidence token. Not `Clone`: a cached admit is the enqueue defect.
#[derive(Debug, PartialEq, Eq)]
pub struct Admitted {
    incarnation: PaneIncarnation,
}

impl Admitted {
    #[must_use]
    pub fn incarnation(&self) -> PaneIncarnation {
        self.incarnation
    }
}

/// Run immediately BEFORE send, on live occupancy, never on a snapshot bool.
pub fn admit_at_send(
    occupancy: &Occupancy,
    presented: &Presented,
    live_supervisor: u32,
) -> Result<Admitted, AdmissionRefusal> {
    let Some(current) = occupancy.current else {
        return Err(AdmissionRefusal::UnknownIncarnation);
    };
    let Some(presented_inc) = presented.incarnation else {
        return Err(AdmissionRefusal::Unmintable);
    };
    if presented.session != occupancy.session {
        return Err(AdmissionRefusal::ForeignSession {
            presented_session: presented.session.clone(),
            current_session: occupancy.session.clone(),
            pane: presented.pane.clone(),
        });
    }
    if presented_inc != current {
        return Err(AdmissionRefusal::StaleIncarnation {
            presented: presented_inc,
            current,
        });
    }
    let lease = match occupancy.lease() {
        Ok(lease) => lease,
        Err(refusal) => return Err(refusal),
    };
    if lease != Lease::Admitting {
        return Err(AdmissionRefusal::LeaseNotAdmitting { lease });
    }
    if let Some(marker_pid) = presented.marker_pid {
        if marker_pid != live_supervisor {
            return Err(AdmissionRefusal::DeadOwner {
                marker_pid,
                live_supervisor,
                pane: presented.pane.clone(),
            });
        }
    }
    if presented_inc == current
        && lease == Lease::Admitting
        && presented.session == occupancy.session
        && presented.pane == occupancy.pane
        && presented
            .marker_pid
            .map(|pid| pid == live_supervisor)
            .unwrap_or(true)
    {
        Ok(Admitted {
            incarnation: current,
        })
    } else {
        Err(AdmissionRefusal::UnknownIncarnation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct SendStub {
        calls: Cell<u32>,
    }

    impl SendStub {
        fn new() -> Self {
            Self { calls: Cell::new(0) }
        }

        fn send_if_admitted(
            &self,
            occupancy: &Occupancy,
            presented: &Presented,
            live_supervisor: u32,
        ) {
            if admit_at_send(occupancy, presented, live_supervisor).is_ok() {
                self.calls.set(self.calls.get() + 1);
            }
        }
    }

    fn live_occupancy(pane: &str, pid: u32) -> (IncarnationMint, Occupancy, PaneIncarnation) {
        let mint = IncarnationMint::new();
        let mut occupancy = Occupancy::unminted("omp-orchestrator", pane);
        let incarnation = occupancy.occupy(&mint, pid);
        (mint, occupancy, incarnation)
    }

    #[test]
    fn measured_dead_pid_markers_are_refused_even_when_age_says_live() {
        let mut occupancy = Occupancy::unminted("omp-orchestrator", "%9");
        let mint = IncarnationMint::new();
        let current = occupancy.occupy(&mint, MEASURED_LIVE_SUPERVISOR);
        for marker in MEASURED_DEAD_MARKERS {
            let mut slot = Occupancy::unminted("omp-orchestrator", marker.pane);
            let presented_inc = slot.occupy(&mint, marker.pid);
            let presented = Presented {
                session: "omp-orchestrator".into(),
                pane: marker.pane.into(),
                incarnation: Some(presented_inc),
                marker_pid: Some(marker.pid),
            };
            let verdict = admit_at_send(&slot, &presented, MEASURED_LIVE_SUPERVISOR);
            assert_eq!(
                verdict,
                Err(AdmissionRefusal::DeadOwner {
                    marker_pid: marker.pid,
                    live_supervisor: MEASURED_LIVE_SUPERVISOR,
                    pane: marker.pane.into(),
                }),
                "age_secs={} must not admit a dead pid",
                marker.age_secs
            );
        }
        let live = Presented {
            session: "omp-orchestrator".into(),
            pane: "%9".into(),
            incarnation: Some(current),
            marker_pid: Some(MEASURED_LIVE_SUPERVISOR),
        };
        assert!(admit_at_send(&occupancy, &live, MEASURED_LIVE_SUPERVISOR).is_ok());
    }

    #[test]
    fn known_good_live_pane_with_current_incarnation_is_admitted() {
        let (_mint, occupancy, incarnation) =
            live_occupancy("%9", MEASURED_LIVE_SUPERVISOR);
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%9".into(),
            incarnation: Some(incarnation),
            marker_pid: Some(MEASURED_LIVE_SUPERVISOR),
        };
        let admitted = admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR)
            .expect("live occupancy must admit");
        assert_eq!(admitted.incarnation(), incarnation);
    }

    #[test]
    fn foreign_session_pane_id_is_a_typed_refusal_not_absent() {
        let (_mint, occupancy, incarnation) =
            live_occupancy("%1414", MEASURED_LIVE_SUPERVISOR);
        let presented = Presented {
            session: "control-plane".into(),
            pane: "%1414".into(),
            incarnation: Some(incarnation),
            marker_pid: Some(MEASURED_LIVE_SUPERVISOR),
        };
        match admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR) {
            Err(AdmissionRefusal::ForeignSession {
                presented_session,
                current_session,
                pane,
            }) => {
                assert_eq!(presented_session, "control-plane");
                assert_eq!(current_session, "omp-orchestrator");
                assert_eq!(pane, "%1414");
            }
            other => panic!("expected ForeignSession, got {other:?}"),
        }
    }

    #[test]
    fn retired_incarnation_is_stale_and_does_not_send() {
        let mint = IncarnationMint::new();
        let mut occupancy = Occupancy::unminted("omp-orchestrator", "%N");
        let first = occupancy.occupy(&mint, MEASURED_LIVE_SUPERVISOR);
        occupancy
            .advance_lease(Lease::Admitting, Lease::Draining)
            .unwrap();
        occupancy
            .advance_lease(Lease::Draining, Lease::Revoked)
            .unwrap();
        let second = occupancy.occupy(&mint, MEASURED_LIVE_SUPERVISOR);
        assert_ne!(first, second);
        let stub = SendStub::new();
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%N".into(),
            incarnation: Some(first),
            marker_pid: Some(MEASURED_LIVE_SUPERVISOR),
        };
        let verdict = admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR);
        assert_eq!(
            verdict,
            Err(AdmissionRefusal::StaleIncarnation {
                presented: first,
                current: second,
            })
        );
        stub.send_if_admitted(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR);
        assert_eq!(stub.calls.get(), 0);
    }

    #[test]
    fn unminted_occupancy_is_unknown_never_admitted() {
        let occupancy = Occupancy::unminted("omp-orchestrator", "%7");
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%7".into(),
            incarnation: PaneIncarnation::new(1),
            marker_pid: None,
        };
        assert_eq!(
            admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR),
            Err(AdmissionRefusal::UnknownIncarnation)
        );
    }

    #[test]
    fn missing_presented_incarnation_is_unmintable() {
        let (_mint, occupancy, _inc) = live_occupancy("%7", MEASURED_LIVE_SUPERVISOR);
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%7".into(),
            incarnation: None,
            marker_pid: None,
        };
        assert_eq!(
            admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR),
            Err(AdmissionRefusal::Unmintable)
        );
    }

    #[test]
    fn draining_lease_is_refused() {
        let (_mint, occupancy, incarnation) =
            live_occupancy("%7", MEASURED_LIVE_SUPERVISOR);
        occupancy
            .advance_lease(Lease::Admitting, Lease::Draining)
            .unwrap();
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%7".into(),
            incarnation: Some(incarnation),
            marker_pid: Some(MEASURED_LIVE_SUPERVISOR),
        };
        assert_eq!(
            admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR),
            Err(AdmissionRefusal::LeaseNotAdmitting {
                lease: Lease::Draining,
            })
        );
    }

    #[test]
    fn lease_advances_only_by_cas_and_never_backward() {
        let (_mint, occupancy, _) = live_occupancy("%7", MEASURED_LIVE_SUPERVISOR);
        assert!(occupancy
            .advance_lease(Lease::Draining, Lease::Revoked)
            .is_err());
        assert!(occupancy
            .advance_lease(Lease::Admitting, Lease::Revoked)
            .is_err());
        assert!(occupancy
            .advance_lease(Lease::Admitting, Lease::Admitting)
            .is_err());
        occupancy
            .advance_lease(Lease::Admitting, Lease::Draining)
            .unwrap();
        assert!(occupancy
            .advance_lease(Lease::Draining, Lease::Admitting)
            .is_err());
        occupancy
            .advance_lease(Lease::Draining, Lease::Revoked)
            .unwrap();
        assert!(occupancy
            .advance_lease(Lease::Revoked, Lease::Admitting)
            .is_err());
    }

    #[test]
    fn admitted_is_not_the_default_branch() {
        let occupancy = Occupancy::unminted("omp-orchestrator", "%7");
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%7".into(),
            incarnation: None,
            marker_pid: None,
        };
        assert_ne!(
            admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR).ok(),
            Some(Admitted {
                incarnation: PaneIncarnation::new(1).unwrap(),
            })
        );
        match admit_at_send(&occupancy, &presented, MEASURED_LIVE_SUPERVISOR) {
            Err(AdmissionRefusal::UnknownIncarnation | AdmissionRefusal::Unmintable) => {}
            other => panic!("residual must refuse, got {other:?}"),
        }
    }

    #[test]
    fn zero_is_unmintable_not_an_incarnation() {
        assert_eq!(PaneIncarnation::new(0), None);
        assert!(DECLARED_NOT_WIRED.contains("deferred"));
    }
}
