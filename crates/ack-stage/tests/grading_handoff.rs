#![forbid(unsafe_code)]

use ack_stage::impl_to_grading_after_ack;
use dispatch_saga::grading::{GradingCandidate, GradingDecision, HAND_MOVED_TWELVE};

#[test]
fn ack_stage_calls_dispatch_saga_on_the_twelve() {
    let rows: Vec<GradingCandidate> = HAND_MOVED_TWELVE
        .iter()
        .map(|id| GradingCandidate {
            id: (*id).to_string(),
            status: "in_progress".into(),
            implementer: "WildStone".into(),
            landed_commit: true,
        })
        .collect();
    let graders = vec!["Orchestrator".into()];
    let decisions = impl_to_grading_after_ack(&rows, &graders).expect("non-empty");
    assert_eq!(decisions.len(), 12);
    assert!(decisions.iter().all(GradingDecision::is_transition));
}
