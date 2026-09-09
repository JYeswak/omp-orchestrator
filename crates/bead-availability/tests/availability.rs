#![forbid(unsafe_code)]

use bead_availability::{
    evaluate_graph, parse_bv_unblocks, parse_r7_parent_child_jsonl, reconcile_readiness,
    Availability, BeadAvailability, BlockerEdge, GraphReport, IssueRecord, IssueStatus,
    QueueIssue, R7_S0_EPIC, Unblocks,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;

fn issue(id: &str, status: IssueStatus) -> IssueRecord {
    IssueRecord {
        id: id.to_owned(),
        status,
    }
}

fn edge(child_id: &str, blocker_id: &str) -> BlockerEdge {
    BlockerEdge {
        child_id: child_id.to_owned(),
        blocker_id: blocker_id.to_owned(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").expect("String writes cannot fail");
    }
    encoded
}

#[test]
fn closed_blocker_is_released_and_open_child_is_available() {
    let graph = evaluate_graph(
        &[
            issue("child", IssueStatus::Open),
            issue("blocker", IssueStatus::Closed),
        ],
        &[edge("child", "blocker")],
    )
    .expect("graph");
    let child = graph.issue("child").expect("child report");
    assert_eq!(child.availability, Availability::Available);
    assert_eq!(
        child.blockers[0].state,
        bead_availability::BlockerState::Released
    );
    assert_eq!(
        graph.issue("blocker").unwrap().unblocks,
        Unblocks::Known { count: 1 }
    );
    let text = graph.render_text();
    println!("{text}");
    assert!(text.contains("RELEASED"));
    assert!(text.contains("child"));
}

#[test]
fn open_blocker_keeps_open_child_blocked() {
    let graph = evaluate_graph(
        &[
            issue("child", IssueStatus::Open),
            issue("blocker", IssueStatus::Open),
        ],
        &[edge("child", "blocker")],
    )
    .expect("graph");
    let child = graph.issue("child").expect("child report");
    assert_eq!(child.availability, Availability::Blocked);
    assert_eq!(
        child.blockers[0].state,
        bead_availability::BlockerState::Blocking
    );
    assert_eq!(
        graph.issue("blocker").unwrap().unblocks,
        Unblocks::Known { count: 1 }
    );
    let text = graph.render_text();
    println!("{text}");
    assert!(text.contains("BLOCKING"));
    assert!(text.contains("child"));
}

#[test]
fn absent_bv_unblocks_is_unknown_not_zero() {
    let value = json!({"id":"bead-without-field"});
    let unblocks = parse_bv_unblocks(&value);
    assert!(matches!(unblocks, Unblocks::Unknown { .. }));
    println!("unblocks={unblocks:?}");
    assert_ne!(unblocks, Unblocks::Known { count: 0 });
}

#[test]
fn stale_dependency_edge_is_reported_as_data() {
    let graph = evaluate_graph(
        &[issue("child", IssueStatus::Open)],
        &[edge("child", "missing-blocker")],
    )
    .expect("stale edge is data, not parser failure");
    assert_eq!(
        graph.issue("child").unwrap().availability,
        Availability::Unknown
    );
    assert_eq!(graph.stale_edges.len(), 1);
    assert_eq!(graph.stale_edges[0].blocker_id, "missing-blocker");
    println!("{}", graph.render_text());
}

#[test]
fn empty_issue_graph_is_error_not_everything_available() {
    let error = evaluate_graph(&[], &[]).expect_err("empty graph must be unknown/error");
    assert!(error.to_string().contains("EMPTY_ISSUE_SET"));
    println!("graph_error={error}");
}

#[test]
fn closed_to_blocking_mutation_is_red_and_restores_byte_identically() {
    let before = serde_json::to_vec(&[
        issue("child", IssueStatus::Open),
        issue("blocker", IssueStatus::Closed),
    ])
    .expect("serialize baseline");
    let mutated = serde_json::to_vec(&[
        issue("child", IssueStatus::Open),
        issue("blocker", IssueStatus::Open),
    ])
    .expect("serialize mutation");
    let before_sha = sha256_hex(&before);
    let mutated_sha = sha256_hex(&mutated);
    assert_ne!(before_sha, mutated_sha);

    let mutated_graph = evaluate_graph(
        &[
            issue("child", IssueStatus::Open),
            issue("blocker", IssueStatus::Open),
        ],
        &[edge("child", "blocker")],
    )
    .expect("mutated graph");
    assert_eq!(
        mutated_graph.issue("child").unwrap().availability,
        Availability::Blocked
    );
    println!("MUTATION RED closed-as-blocking child=child");

    let path = std::env::temp_dir().join(format!("bead-availability-{}", std::process::id()));
    fs::write(&path, &mutated).expect("write mutation");
    fs::write(&path, &before).expect("restore baseline");
    let after = fs::read(&path).expect("read restored baseline");
    let after_sha = sha256_hex(&after);
    assert_eq!(before, after);
    assert_eq!(before_sha, after_sha);
    let restored_graph = evaluate_graph(
        &[
            issue("child", IssueStatus::Open),
            issue("blocker", IssueStatus::Closed),
        ],
        &[edge("child", "blocker")],
    )
    .expect("restored graph");
    assert_eq!(
        restored_graph.issue("child").unwrap().availability,
        Availability::Available
    );
    println!(
        "MUTATION RESTORED before={before_sha} mutated={mutated_sha} after={after_sha} byte_identical=true"
    );
    fs::remove_file(path).expect("remove fixture");
}
fn queue_issue(id: &str, issue_type: &str) -> QueueIssue {
    QueueIssue {
        id: id.to_owned(),
        status: IssueStatus::Open,
        priority: 0,
        issue_type: issue_type.to_owned(),
        assignee: String::new(),
    }
}

fn queue_graph(ids: &[&str], availability: Availability) -> GraphReport {
    GraphReport {
        schema: "bead-availability/v1",
        issues: ids
            .iter()
            .map(|id| BeadAvailability {
                bead_id: (*id).to_owned(),
                status: IssueStatus::Open,
                availability,
                blockers: Vec::new(),
                unblocks: Unblocks::Known { count: 0 },
            })
            .collect(),
        stale_edges: Vec::new(),
    }
}

#[test]
fn available_hidden_child_is_reoffered_instead_of_silently_dropped() {
    let issues = vec![queue_issue("child", "task"), queue_issue("control", "task")];
    let report = reconcile_readiness(
        &issues,
        &["control".to_owned()],
        &[],
        &queue_graph(&["child", "control"], Availability::Available),
    )
    .expect("non-empty ready surface and graph");
    assert_eq!(report.recovered_ids(), vec!["child".to_owned()]);
    assert_eq!(
        report
            .admitted
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["child", "control"]
    );
    assert_eq!(report.open_count, 2);
    assert_eq!(report.ready_count, 1);
    assert_eq!(report.blocked_count, 0);
    assert_eq!(report.residual_count, 1);
    assert!(report.render_text().contains("READINESS open=2 ready=1 blocked=0 residual=1"));
}
#[test]
fn visible_ready_row_is_not_duplicated_by_reconciliation() {
    let issues = vec![queue_issue("child", "task")];
    let report = reconcile_readiness(
        &issues,
        &["child".to_owned()],
        &[],
        &queue_graph(&["child"], Availability::Available),
    )
    .expect("visible ready row");
    assert!(report.recovered.is_empty());
    assert_eq!(report.admitted.len(), 1);
}

#[test]
fn hidden_graph_blocker_is_named_not_reoffered() {
    let issues = vec![queue_issue("child", "task"), queue_issue("control", "task")];
    let report = reconcile_readiness(
        &issues,
        &["control".to_owned()],
        &[],
        &queue_graph(&["child", "control"], Availability::Blocked),
    )
    .expect("non-empty ready surface and graph");
    assert!(report.recovered.is_empty());
    assert_eq!(report.refused.len(), 1);
    assert_eq!(report.refused[0].issue.id, "child");
    assert!(report.refused[0].reason.contains("non-terminal blocker"));
}

#[test]
fn empty_ready_surface_is_an_error_not_total_visibility() {
    let error = reconcile_readiness(
        &[queue_issue("child", "task")],
        &[],
        &[],
        &queue_graph(&["child"], Availability::Available),
    )
    .expect_err("empty ready input is anti-vacuous error");
    assert!(error.to_string().contains("EMPTY_READY_SURFACE"));
}
#[test]
fn queue_parser_preserves_surface_identity_fields() {
    let value = json!({
        "issues": [{
            "id": "child",
            "status": "open",
            "priority": 0,
            "issue_type": "task",
            "assignee": ""
        }]
    });
    let rows = bead_availability::parse_queue_issues(&value).expect("queue rows");
    assert_eq!(rows, vec![queue_issue("child", "task")]);
}

#[test]
fn queue_parser_refuses_missing_priority_instead_of_guessing() {
    let value = json!({
        "issues": [{"id": "child", "status": "open", "issue_type": "task"}]
    });
    let error = bead_availability::parse_queue_issues(&value).expect_err("missing priority");
    assert!(error.to_string().contains("MISSING_QUEUE_FIELD"));
}
#[test]
fn queue_id_parser_accepts_ready_and_blocked_envelopes() {
    let value = json!({"issues": [{"id": "child"}]});
    assert_eq!(
        bead_availability::parse_queue_ids(&value, "br blocked").expect("issues envelope"),
        vec!["child".to_owned()]
    );
}

#[test]
fn r7_parent_child_jsonl_known_good_has_real_membership() {
    let text = [
        json!({"id": R7_S0_EPIC, "status": "closed", "dependencies": []}).to_string(),
        json!({
            "id": "child",
            "status": "closed",
            "dependencies": [{
                "issue_id": "child",
                "depends_on_id": R7_S0_EPIC,
                "type": "parent-child"
            }]
        })
        .to_string(),
    ]
    .join("\n");
    let report = parse_r7_parent_child_jsonl(&text, R7_S0_EPIC).expect("real parent-child edge");
    assert_eq!(report.source, ".beads/issues.jsonl");
    assert_eq!(report.child_ids, vec!["child".to_owned()]);
    assert!(report.non_terminal_ids.is_empty());
    assert_eq!(report.residual_count(), 0);
}

#[test]
fn r7_parent_child_jsonl_old_surface_is_typed_bad() {
    let text = [
        json!({"id": R7_S0_EPIC, "status": "closed"}).to_string(),
        json!({
            "id": "child",
            "status": "closed",
            "dependencies": [{"id": R7_S0_EPIC, "dependency_type": "parent-child"}]
        })
        .to_string(),
    ]
    .join("\n");
    let error = parse_r7_parent_child_jsonl(&text, R7_S0_EPIC).expect_err("old shape must refuse");
    assert_eq!(
        error.to_string(),
        "MALFORMED_DEPENDENCY: line=2 missing=depends_on_id"
    );
}

#[test]
fn r7_parent_child_jsonl_empty_dependency_input_is_error() {
    let error = parse_r7_parent_child_jsonl(
        &json!({"id": R7_S0_EPIC, "status": "closed"}).to_string(),
        R7_S0_EPIC,
    )
    .expect_err("empty dependency input must not pass by prefix");
    assert_eq!(
        error.to_string(),
        "EMPTY_DEPENDENCY_SOURCE: dependency source contained no edges"
    );
}

#[test]
fn r7_tombstone_child_is_terminal() {
    let text = [
        json!({"id": R7_S0_EPIC, "status": "closed"}).to_string(),
        json!({
            "id": "tombstone-child",
            "status": "tombstone",
            "dependencies": [{
                "issue_id": "tombstone-child",
                "depends_on_id": R7_S0_EPIC,
                "type": "parent-child"
            }]
        })
        .to_string(),
    ]
    .join("\n");
    let report = parse_r7_parent_child_jsonl(&text, R7_S0_EPIC).expect("tombstone child");
    assert!(IssueStatus::Tombstone.is_terminal());
    assert!(report.non_terminal_ids.is_empty());
}

#[test]
fn r7_live_jsonl_source_has_parent_child_membership() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.beads/issues.jsonl");
    let text = fs::read_to_string(path).expect("live dependency source");
    let report = parse_r7_parent_child_jsonl(&text, R7_S0_EPIC).expect("live R7 graph");
    assert!(!report.child_ids.is_empty(), "R7 graph membership must be nonempty");
    println!(
        "R7 source={} epic={} children={} non_terminal={}",
        report.source,
        report.epic_id,
        report.child_ids.len(),
        report.non_terminal_ids.len()
    );
}
