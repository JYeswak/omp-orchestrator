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

/// 7jazc: the worker demand is TREE PROVENANCE FOR A NUMBER, so it fires on a
/// CARGO FIGURE and on nothing else.
///
/// This leg previously asserted that "DONE: landed the fix" — real evidence, NO
/// cargo figure — was a CLOSE_REASON_WORKER_MISSING violation. That pinned a
/// broadening the owning crate never stated, and it made `.beads/issues.jsonl`
/// unlandable: 47 of 89 newly-closed rows made no execution claim, and any one
/// refused the whole commit. The leg now measures the rule ack-spine actually
/// owns, in both directions.
#[test]
fn a_new_close_without_worker_authority_is_a_named_violation() {
    let head = row("open", "open", "");

    // KNOWN-BAD, THE ONE THAT MUST SURVIVE: a cargo figure with no execution
    // authority still refuses, naming both the label and the bead id.
    let staged = [bead("bad", "DONE: cargo test -p inbox-monitor 29 passed")];
    let report =
        check_staged_close_reason_policy(Some(&head), &staged).expect("both inputs are readable");
    assert_eq!(report.newly_closed, 1);
    assert_eq!(report.verified, 0);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].bead_id, "bad");
    assert!(
        report.violations[0]
            .reason
            .contains("CLOSE_REASON_WORKER_MISSING"),
        "{report:?}"
    );
    assert!(report.violations[0].reason.contains("bad"), "{report:?}");

    // KNOWN-GOOD: the same figure WITH authority, in both accepted forms, so the
    // fix cannot have regressed either of ack-spine's two spellings.
    for reason in [
        "DONE: cargo test worker=contabo-3 29 passed",
        "DONE: local cargo test 29 passed",
    ] {
        let good = [bead("good", reason)];
        let control = check_staged_close_reason_policy(Some(&head), &good)
            .expect("both inputs are readable");
        assert_eq!(control.verified, 1, "{reason}: {control:?}");
        assert!(control.violations.is_empty(), "{reason}: {control:?}");
    }

    // KNOWN-GOOD, the row the broadened rule wrongly refused: real evidence, no
    // cargo figure, nothing to attribute to a tree.
    let no_figure = [bead(
        "zrq-shape",
        "DONE: df -h /Volumes/BuildShared and du of cargo-targets show the reclaim landed",
    )];
    let passing = check_staged_close_reason_policy(Some(&head), &no_figure)
        .expect("both inputs are readable");
    assert_eq!(passing.verified, 1, "{passing:?}");
    assert!(passing.violations.is_empty(), "{passing:?}");
}

/// ANTI-VACUITY: a run over an EMPTY newly-closed set is not a pass.
#[test]
fn an_empty_newly_closed_set_is_not_a_verified_run() {
    let head = row("only", "closed", "DONE: local cargo test 1 passed");
    let staged = [bead("only", "DONE: local cargo test 1 passed")];
    let report =
        check_staged_close_reason_policy(Some(&head), &staged).expect("both inputs are readable");
    assert_eq!(report.newly_closed, 0, "nothing new was closed");
    assert_eq!(
        report.verified, 0,
        "a scan that verified nothing must not report a verified row"
    );
    // And the caller can tell "nothing to check" from "everything checked out":
    // verified==0 with newly_closed==0 is the empty scan, not a clean bill.
    assert!(report.violations.is_empty());
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

fn bead_with_comments(id: &str, close_reason: &str, comments: &[&str]) -> ClosedBead {
    ClosedBead {
        id: id.to_owned(),
        close_reason: close_reason.to_owned(),
        comments: comments.iter().map(|text| (*text).to_owned()).collect(),
    }
}

/// 5erif KNOWN-GOOD: the authority may live in a COMMENT.
///
/// The reason field is unamendable without `br reopen` + `br close`, which flaps a
/// closed bead OPEN — and zero-open-S1 is the predicate the S1 done-bar is defined
/// over. So the evidence lives where it can be added without a state change. The
/// specimen shape is S1L3Obs's xar6 / 0ne8 provenance comments: worker name plus
/// the pool path the figure was observed in.
#[test]
fn worker_authority_in_a_comment_verifies_the_row() {
    let head = row("open", "open", "");
    let staged = [bead_with_comments(
        "xar6-shape",
        "DONE: cargo test -p ompo-doctor --test start_observability 5 passed",
        &[
            "PROVENANCE S1L4Src: known-good run observed on worker=contabo-4, pool path \
             .rch-target-contabo-4-pool-9fce9d6739375f7cae8bcfc4b2e0b25b; mutation run on \
             worker=contabo-2.",
        ],
    )];
    let report =
        check_staged_close_reason_policy(Some(&head), &staged).expect("both inputs are readable");
    assert_eq!(report.newly_closed, 1);
    assert_eq!(report.verified, 1, "{report:?}");
    assert!(report.violations.is_empty(), "{report:?}");
}

/// 5erif KNOWN-BAD, THE INVARIANT THAT MUST SURVIVE: a cargo figure with NO
/// authority in the reason AND NONE IN ANY COMMENT still refuses. Both the message
/// substring AND the counts are asserted, because this repo has measured each pin
/// failing alone in opposite directions.
#[test]
fn a_cargo_figure_with_no_authority_anywhere_still_refuses() {
    let head = row("open", "open", "");
    for comments in [
        vec![],
        vec!["PROVENANCE: ran it, looked fine"],
        vec!["the pool path was .rch-target-contabo-4-pool-9fce9d67 but I did not record a worker"],
    ] {
        let staged = [bead_with_comments(
            "bad",
            "DONE: cargo test -p inbox-monitor 29 passed",
            &comments,
        )];
        let report = check_staged_close_reason_policy(Some(&head), &staged)
            .expect("both inputs are readable");
        assert_eq!(report.verified, 0, "{comments:?}: {report:?}");
        assert_eq!(report.violations.len(), 1, "{comments:?}: {report:?}");
        assert_eq!(report.violations[0].bead_id, "bad");
        assert!(
            report.violations[0]
                .reason
                .contains("CLOSE_REASON_WORKER_MISSING"),
            "{comments:?}: {report:?}"
        );
    }
}

/// A comment cannot rescue a row that states no VERDICT CLASS: the widened surface
/// is for the execution authority only, never for the prefix policy.
#[test]
fn a_comment_does_not_admit_an_unsanctioned_prefix() {
    let head = row("open", "open", "");
    let staged = [bead_with_comments(
        "prose",
        "ran the tests, cargo test 4 passed",
        &["PROVENANCE: worker=contabo-1"],
    )];
    let report =
        check_staged_close_reason_policy(Some(&head), &staged).expect("both inputs are readable");
    assert_eq!(report.verified, 0, "{report:?}");
    assert!(
        report.violations[0]
            .reason
            .contains("CLOSE_REASON_POLICY_REFUSED"),
        "{report:?}"
    );
}
