//! The staged close-reason policy: ONE rule per closed row, scoped to this commit's closes.
//!
//! Supersedes the two-authority arrangement (`check_close_reason_policy_scoped`), where a
//! prefix rule and a worker rule disagreed about the same row.

use pre_delete_citation_check::{check_staged_close_reason_policy, ClosedBead};

fn bead(id: &str, close_reason: &str) -> ClosedBead {
    ClosedBead {
        id: id.to_owned(),
        close_reason: close_reason.to_owned(),
        comments: Vec::new(),
    }
}

fn row(id: &str, status: &str, close_reason: &str) -> String {
    format!("{{\"id\":\"{id}\",\"status\":\"{status}\",\"close_reason\":\"{close_reason}\"}}\n")
}

#[test]
fn historical_unrecoverable_rows_are_visible_but_do_not_block() {
    let head = row("legacy", "closed", "DONE: cargo test passed without worker authority");
    let staged = [
        bead("legacy", "DONE: cargo test passed without worker authority"),
        bead("new", "DONE: worker=contabo-1 cargo test passed"),
    ];
    let report =
        check_staged_close_reason_policy(Some(&head), &staged).expect("both inputs are readable");

    assert_eq!(report.closed_beads, 2);
    assert_eq!(report.historical_closed, 1);
    assert_eq!(report.newly_closed, 1);
    assert_eq!(report.verified, 1);
    assert!(report.violations.is_empty(), "{report:?}");
    assert_eq!(report.legacy_unrecoverable, vec!["legacy".to_owned()]);
}

#[test]
fn a_mirror_change_that_closes_nothing_passes_with_legacy_rows() {
    let head = format!(
        "{}{}",
        row("legacy", "closed", "DONE: cargo test passed"),
        row("open", "open", "")
    );
    let staged = [bead("legacy", "DONE: cargo test passed")];
    let report = check_staged_close_reason_policy(Some(&head), &staged)
        .expect("the unchanged mirror is readable");

    assert_eq!(report.newly_closed, 0);
    assert_eq!(report.verified, 0);
    assert!(report.violations.is_empty(), "{report:?}");
    assert_eq!(report.legacy_unrecoverable, vec!["legacy".to_owned()]);
}

/// The LOCAL worker conjunct, isolated from ack-spine's own rule.
///
/// The old fixture said "DONE: cargo test passed", which trips ack-spine's CargoWorkerMissing
/// branch (it fires only when the reason carries a cargo figure), so the leg passed under
/// EITHER rule and proved neither -- GradeCloseReason showed `has_worker_attribution` could be
/// mutated to always-true with every suite still green. A reason with NO cargo figure can only
/// be refused by the local conjunct, so this fixture measures exactly one thing.
#[test]
fn a_new_close_without_worker_authority_is_a_named_violation() {
    let head = row("open", "open", "");
    let staged = [bead("bad", "DONE: landed the fix")];
    let report =
        check_staged_close_reason_policy(Some(&head), &staged).expect("both inputs are readable");

    assert_eq!(report.newly_closed, 1);
    assert_eq!(report.verified, 0);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].bead_id, "bad");
    assert!(
        report.violations[0].reason.contains("CLOSE_REASON_WORKER_MISSING"),
        "{report:?}"
    );

    // The same reason WITH worker authority is the positive control: if the conjunct were
    // mutated to always-true the leg above goes green, and if it were always-false this one does.
    let good = [bead("good", "DONE: landed the fix worker=contabo-1")];
    let control =
        check_staged_close_reason_policy(Some(&head), &good).expect("both inputs are readable");
    assert_eq!(control.verified, 1, "{control:?}");
    assert!(control.violations.is_empty(), "{control:?}");
}

#[test]
fn a_prose_reason_is_one_violation_not_two() {
    let staged = [bead("prose", "fixed it")];
    let report = check_staged_close_reason_policy(None, &staged).expect("an absent HEAD is a baseline");

    assert_eq!(report.violations.len(), 1, "{report:?}");
    assert!(
        report.violations[0].reason.contains("CLOSE_REASON_POLICY_REFUSED leading=fixed"),
        "{report:?}"
    );
}

#[test]
fn an_absent_head_mirror_is_a_baseline_but_an_unparsable_one_is_not() {
    let staged = [bead("new", "DONE: worker=contabo-1 cargo test passed")];
    let absent = check_staged_close_reason_policy(None, &staged)
        .expect("a first commit that ADDS the mirror has an empty baseline");
    assert_eq!(absent.newly_closed, 1);
    assert_eq!(absent.verified, 1);

    let error = check_staged_close_reason_policy(Some("{not json\n"), &staged)
        .expect_err("a HEAD mirror that exists and cannot be parsed must not pass as empty");
    assert!(
        error.contains("CLOSE_REASON_HEAD_MIRROR_UNREADABLE"),
        "{error}"
    );
}
