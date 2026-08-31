use dispatch_claim_fence::{
    authorize, parse_br_show_json, BeadSnapshot, ClaimFenceError, DispatchIntent, DispatchPermit,
};

fn bead(id: &str, status: &str, assignee: Option<&str>) -> BeadSnapshot {
    BeadSnapshot::new(id, "A dispatch bead", "Claim before send", status, assignee)
}

#[test]
fn open_unassigned_bead_is_refused_with_actual_status_and_claim_command() {
    let error = authorize(
        &DispatchIntent::bead("5rh", "BlueLantern"),
        Some(&bead("5rh", "open", None)),
    )
    .expect_err("an open bead must not be dispatched");

    assert!(matches!(
        error,
        ClaimFenceError::ClaimRequired {
            ref bead_id,
            ref actual_status,
            ref command,
            ..
        } if bead_id == "5rh"
            && actual_status == "open"
            && command == "br update 5rh --assignee BlueLantern --status in_progress"
    ));
    assert!(error.to_string().contains("status=open"));
    assert!(error
        .to_string()
        .contains("br update 5rh --assignee BlueLantern --status in_progress"));
}

#[test]
fn bead_assigned_to_different_agent_is_refused() {
    let error = authorize(
        &DispatchIntent::bead("5rh", "BlueLantern"),
        Some(&bead("5rh", "in_progress", Some("AmberGate"))),
    )
    .expect_err("a bead owned by another agent must not be dispatched");

    assert!(matches!(
        error,
        ClaimFenceError::AssignedElsewhere {
            ref bead_id,
            ref expected_agent,
            ref actual_assignee,
            ..
        } if bead_id == "5rh"
            && expected_agent == "BlueLantern"
            && actual_assignee == "AmberGate"
    ));
}

#[test]
fn correctly_claimed_bead_is_permitted() {
    let permit = authorize(
        &DispatchIntent::bead("5rh", "BlueLantern"),
        Some(&bead("5rh", "in_progress", Some("BlueLantern"))),
    )
    .expect("the receiving agent's claimed bead must pass");

    assert!(
        matches!(permit, DispatchPermit::Bead { bead_id, receiver_agent }
        if bead_id == "5rh" && receiver_agent == "BlueLantern")
    );
}

#[test]
fn missing_bead_id_is_an_error() {
    let error = authorize(
        &DispatchIntent::bead("  ", "BlueLantern"),
        Some(&bead("5rh", "in_progress", Some("BlueLantern"))),
    )
    .expect_err("a packet without a bead id is not a broadcast");

    assert!(matches!(error, ClaimFenceError::MissingBeadId));
}

#[test]
fn broadcasts_and_corrections_are_distinct_named_operations() {
    let broadcast = authorize(
        &DispatchIntent::broadcast("fleet-status", "BlueLantern"),
        None,
    )
    .expect("broadcasts are not bead dispatches");
    let correction = authorize(
        &DispatchIntent::correction("repair-receipt", "BlueLantern"),
        None,
    )
    .expect("corrections are not bead dispatches");

    assert!(
        matches!(broadcast, DispatchPermit::Broadcast { operation, .. }
        if operation == "fleet-status")
    );
    assert!(
        matches!(correction, DispatchPermit::Correction { operation, .. }
        if operation == "repair-receipt")
    );
}

#[test]
fn br_show_json_is_parsed_into_typed_snapshot() {
    let snapshot = parse_br_show_json(
        br#"[{"id":"5rh","title":"Claim before send","description":"Claim it","status":"in_progress","assignee":"BlueLantern"}]"#,
    )
    .expect("valid br show output");

    assert_eq!(snapshot.id(), "5rh");
    assert_eq!(snapshot.status_label(), "in_progress");
    assert_eq!(snapshot.assignee(), Some("BlueLantern"));
}
