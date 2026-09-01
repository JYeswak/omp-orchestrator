#![forbid(unsafe_code)]

//! Typed transitions for one selected bead.

use crate::{Approved, TypedAction, Wave};
use loop_coverage::LoopLayer;
use std::collections::BTreeSet;

mod model;
pub use model::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeadLifecycle {
    bead: BeadId,
    target: DispatchTarget,
    objective: String,
    phase: LoopLayer,
    action: TypedAction,
    state: LifecycleStatus,
    receiver: Option<ReceiverEvidence>,
    grade: Option<GradeReceipt>,
    blocker: Option<BlockerEvidence>,
    fix_name: Option<String>,
    seen_events: BTreeSet<EventId>,
    history: Vec<LifecycleEvent>,
}
impl BeadLifecycle {
    pub fn select(
        bead: BeadId,
        target: DispatchTarget,
        objective: impl Into<String>,
        approval: Approved,
        event_id: EventId,
    ) -> Result<Self, LifecycleError> {
        let objective = objective.into();
        if objective.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "objective" });
        }
        let wave = approval.wave();
        let mut seen_events = BTreeSet::new();
        seen_events.insert(event_id.clone());
        Ok(Self {
            bead,
            target,
            objective,
            phase: wave.phase,
            action: wave.action,
            state: LifecycleStatus::Selected,
            receiver: None,
            grade: None,
            blocker: None,
            fix_name: None,
            seen_events,
            history: vec![LifecycleEvent {
                id: event_id,
                kind: LifecycleEventKind::Selected,
                status: LifecycleStatus::Selected,
                blocker: None,
            }],
        })
    }
    pub fn select_wave(
        bead: BeadId,
        target: DispatchTarget,
        objective: impl Into<String>,
        wave: Wave,
        event_id: EventId,
    ) -> Result<Self, LifecycleError> {
        let approval = Approved::authorize(wave).map_err(LifecycleError::NotApproved)?;
        Self::select(bead, target, objective, approval, event_id)
    }
    pub fn status(&self) -> LifecycleStatus {
        self.state
    }
    pub fn phase(&self) -> LoopLayer {
        self.phase
    }
    pub fn action(&self) -> TypedAction {
        self.action
    }
    pub fn bead(&self) -> &BeadId {
        &self.bead
    }
    pub fn target(&self) -> &DispatchTarget {
        &self.target
    }
    pub fn receiver(&self) -> Option<&ReceiverEvidence> {
        self.receiver.as_ref()
    }
    pub fn grade_receipt(&self) -> Option<&GradeReceipt> {
        self.grade.as_ref()
    }
    pub fn history(&self) -> &[LifecycleEvent] {
        &self.history
    }
    pub fn blocker(&self) -> Option<&BlockerEvidence> {
        self.blocker.as_ref()
    }

    fn transition(
        &mut self,
        id: EventId,
        kind: LifecycleEventKind,
        next: LifecycleStatus,
    ) -> Result<(), LifecycleError> {
        self.transition_with_blocker(id, kind, next, None)
    }
    fn transition_with_blocker(
        &mut self,
        id: EventId,
        kind: LifecycleEventKind,
        next: LifecycleStatus,
        blocker: Option<BlockerEvidence>,
    ) -> Result<(), LifecycleError> {
        if !self.seen_events.insert(id.clone()) {
            return Err(LifecycleError::DuplicateEvent(id));
        }
        self.state = next;
        self.history.push(LifecycleEvent {
            id,
            kind,
            status: next,
            blocker,
        });
        Ok(())
    }
    fn require_state(
        &self,
        event_id: &EventId,
        event: LifecycleEventKind,
        allowed: &[LifecycleStatus],
    ) -> Result<(), LifecycleError> {
        if self.seen_events.contains(event_id) {
            return Err(LifecycleError::DuplicateEvent(event_id.clone()));
        }
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(LifecycleError::InvalidTransition {
                from: self.state,
                event,
            })
        }
    }
    fn check_identity(
        &self,
        bead: &BeadId,
        target: &DispatchTarget,
        objective: Option<&str>,
    ) -> Result<(), LifecycleError> {
        if bead != &self.bead {
            return Err(LifecycleError::WrongBead);
        }
        if target != &self.target {
            return Err(LifecycleError::WrongTarget);
        }
        if let Some(objective) = objective {
            if objective != self.objective {
                return Err(LifecycleError::WrongObjective);
            }
        }
        Ok(())
    }
    fn check_fresh(timestamp: u64, policy: EvidencePolicy) -> Result<(), LifecycleError> {
        if timestamp > policy.now_ms {
            return Err(LifecycleError::EvidenceInFuture {
                observed_at_ms: timestamp,
                now_ms: policy.now_ms,
            });
        }
        let age_ms = policy.now_ms - timestamp;
        if age_ms > policy.max_age_ms {
            return Err(LifecycleError::StaleEvidence {
                age_ms,
                max_age_ms: policy.max_age_ms,
            });
        }
        Ok(())
    }
    pub fn dispatch(&mut self, receipt: DispatchReceipt) -> Result<(), LifecycleError> {
        self.require_state(
            &receipt.id,
            LifecycleEventKind::Dispatched,
            &[
                LifecycleStatus::Selected,
                LifecycleStatus::RedispatchRequired,
            ],
        )?;
        self.check_identity(&receipt.bead, &receipt.target, Some(&receipt.objective))?;
        self.transition(
            receipt.id,
            LifecycleEventKind::Dispatched,
            LifecycleStatus::Dispatched,
        )
    }
    pub fn mark_ungraded(&mut self, event_id: EventId) -> Result<(), LifecycleError> {
        self.require_state(
            &event_id,
            LifecycleEventKind::MarkedUngraded,
            &[LifecycleStatus::Dispatched],
        )?;
        self.transition(
            event_id,
            LifecycleEventKind::MarkedUngraded,
            LifecycleStatus::Ungraded,
        )
    }
    pub fn verify_receiver(
        &mut self,
        evidence: ReceiverEvidence,
        policy: EvidencePolicy,
    ) -> Result<(), LifecycleError> {
        self.require_state(
            &evidence.id,
            LifecycleEventKind::ReceiverVerified,
            &[LifecycleStatus::Dispatched, LifecycleStatus::Ungraded],
        )?;
        self.check_identity(&evidence.bead, &evidence.target, Some(&evidence.objective))?;
        Self::check_fresh(evidence.observed_at_ms, policy)?;
        self.receiver = Some(evidence.clone());
        self.transition(
            evidence.id,
            LifecycleEventKind::ReceiverVerified,
            LifecycleStatus::ReceiverVerified,
        )
    }
    pub fn start_grading(&mut self, event_id: EventId) -> Result<(), LifecycleError> {
        self.require_state(
            &event_id,
            LifecycleEventKind::GradingStarted,
            &[LifecycleStatus::ReceiverVerified],
        )?;
        if self.receiver.is_none() {
            return Err(LifecycleError::MissingReceiverEvidence);
        }
        self.transition(
            event_id,
            LifecycleEventKind::GradingStarted,
            LifecycleStatus::Grading,
        )
    }
    pub fn grade(
        &mut self,
        receipt: GradeReceipt,
        policy: EvidencePolicy,
    ) -> Result<(), LifecycleError> {
        self.require_state(
            &receipt.id,
            LifecycleEventKind::GradedPass,
            &[LifecycleStatus::Grading],
        )?;
        self.check_identity(&receipt.bead, &receipt.target, None)?;
        Self::check_fresh(receipt.graded_at_ms, policy)?;
        let Some(receiver) = self.receiver.as_ref() else {
            return Err(LifecycleError::MissingReceiverEvidence);
        };
        if receipt.receiver_event_id != receiver.id {
            return Err(LifecycleError::WrongReceiverEvent);
        }
        let (kind, next, fix_name) = match &receipt.result {
            GradeResult::Pass => (
                LifecycleEventKind::GradedPass,
                LifecycleStatus::GradedPass,
                None,
            ),
            GradeResult::Fix { name } => (
                LifecycleEventKind::GradedFix,
                LifecycleStatus::GradedFix,
                Some(name.clone()),
            ),
        };
        self.fix_name = fix_name;
        self.grade = Some(receipt.clone());
        self.transition(receipt.id, kind, next)
    }
    pub fn close(&mut self, event_id: EventId) -> Result<(), LifecycleError> {
        self.require_state(
            &event_id,
            LifecycleEventKind::Closed,
            &[LifecycleStatus::GradedPass],
        )?;
        if !matches!(
            self.grade.as_ref().map(|g| &g.result),
            Some(GradeResult::Pass)
        ) {
            return Err(LifecycleError::GradeMustBePass);
        }
        self.transition(
            event_id,
            LifecycleEventKind::Closed,
            LifecycleStatus::Closed,
        )
    }
    pub fn require_redispatch(&mut self, plan: RedispatchPlan) -> Result<(), LifecycleError> {
        self.require_state(
            &plan.id,
            LifecycleEventKind::RedispatchRequired,
            &[LifecycleStatus::GradedFix],
        )?;
        self.check_identity(&plan.bead, &plan.target, None)?;
        if self.fix_name.as_deref() != Some(plan.fix_name.as_str()) {
            return Err(LifecycleError::WrongObjective);
        }
        self.transition(
            plan.id,
            LifecycleEventKind::RedispatchRequired,
            LifecycleStatus::RedispatchRequired,
        )
    }
    pub fn block(
        &mut self,
        event_id: EventId,
        blocker: BlockerEvidence,
        policy: EvidencePolicy,
    ) -> Result<(), LifecycleError> {
        self.require_state(
            &event_id,
            LifecycleEventKind::Blocked,
            &[
                LifecycleStatus::Selected,
                LifecycleStatus::Dispatched,
                LifecycleStatus::ReceiverVerified,
                LifecycleStatus::Ungraded,
                LifecycleStatus::Grading,
                LifecycleStatus::RedispatchRequired,
            ],
        )?;
        if !blocker.kind().is_external() {
            return Err(LifecycleError::LocalBlockerNotAllowed);
        }
        Self::check_fresh(blocker.observed_at_ms(), policy)?;
        self.transition_with_blocker(
            event_id,
            LifecycleEventKind::Blocked,
            LifecycleStatus::Blocked,
            Some(blocker.clone()),
        )?;
        self.blocker = Some(blocker);
        Ok(())
    }
}
