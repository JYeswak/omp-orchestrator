use pre_delete_citation_check::check_close_reason_policy_scoped;

fn row(id: &str, status: &str, close_reason: &str, assignee: Option<&str>) -> String {
    let assignee = assignee.map_or_else(|| "null".to_owned(), |value| format!("\"{value}\""));
    format!(
        "{{\"id\":\"{id}\",\"status\":\"{status}\",\"close_reason\":\"{close_reason}\",\"assignee\":{assignee}}}"
    )
}

#[test]
fn historical_unrecoverable_rows_are_visible_but_do_not_block() {
    let head = row(
        "legacy",
        "closed",
        "DONE: cargo test passed without worker authority",
        None,
    );
    let staged = format!(
        "{head}\n{}\n",
        row(
            "new",
            "closed",
            "DONE: worker=contabo-1 cargo test passed",
            Some("pane=%7;agent=WildStone"),
        )
    );
    let report = check_close_reason_policy_scoped(
        format!("{head}\n").as_bytes(),
        staged.as_bytes(),
    )
    .expect("both mirror inputs are readable");

    assert_eq!(report.historical_closed, 1);
    assert_eq!(report.newly_closed, 1);
    assert_eq!(report.verified_new_closes, 1);
    assert!(report.violations.is_empty(), "{report:?}");
    assert_eq!(report.legacy_unrecoverable, vec!["legacy".to_owned()]);
}

#[test]
fn a_mirror_change_that_closes_nothing_passes_with_legacy_rows() {
    let head = format!(
        "{}\n{}\n",
        row("legacy", "closed", "DONE: cargo test passed", None),
        row("open", "open", "", None),
    );
    let report = check_close_reason_policy_scoped(head.as_bytes(), head.as_bytes())
        .expect("the unchanged mirror is readable");

    assert_eq!(report.newly_closed, 0);
    assert_eq!(report.verified_new_closes, 0);
    assert!(report.violations.is_empty(), "{report:?}");
    assert_eq!(report.legacy_unrecoverable, vec!["legacy".to_owned()]);
}

#[test]
fn a_new_close_without_worker_authority_is_a_named_violation() {
    let head = row("open", "open", "", None);
    let staged = format!(
        "{head}\n{}\n",
        row("bad", "closed", "DONE: cargo test passed", None)
    );
    let report = check_close_reason_policy_scoped(
        format!("{head}\n").as_bytes(),
        staged.as_bytes(),
    )
    .expect("both mirror inputs are readable");

    assert_eq!(report.newly_closed, 1);
    assert_eq!(report.verified_new_closes, 0);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].bead_id, "bad");
    assert!(
        report.violations[0]
            .reason
            .contains("CLOSE_REASON_WORKER_MISSING")
    );
}

#[test]
fn an_unreadable_head_mirror_is_typed_and_distinct() {
    let staged = row(
        "new",
        "closed",
        "DONE: worker=contabo-1 cargo test passed",
        None,
    );
    let error = check_close_reason_policy_scoped(b"", staged.as_bytes())
        .expect_err("an absent HEAD mirror cannot be treated as history");
    assert_eq!(
        error,
        "CLOSE_REASON_HEAD_MIRROR_EMPTY reason=zero_bead_records_readable"
    );
}
