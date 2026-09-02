#![forbid(unsafe_code)]

use bead_availability::{
    evaluate_graph, parse_bv_unblocks, Availability, BlockerEdge, IssueRecord, IssueStatus,
    Unblocks,
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
