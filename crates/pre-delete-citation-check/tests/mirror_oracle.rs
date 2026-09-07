//! Legs for `omp-orchestrator-dpa4`: the tracker oracle must carry COMMENTS, and an
//! unreadable oracle must refuse rather than pass.
//!
//! THE DEFECT: `ClosedBead::comments` existed, `check_deletions` scanned it, and every
//! production caller passed an empty vector. `parse_closed_beads` says so in its own body --
//! "The br JSON does not inline comments; the caller fetches them separately" -- and no
//! caller ever did. Measured 2026-09-07: `br list --json --status closed` returns 196 closed
//! rows and ZERO carry a `comments` key, while 14 closed beads in this tracker cite a `bin/`
//! or `.flywheel/` path ONLY in comments.
//!
//! Every assertion keys on the MESSAGE or on a caught conflict, never on a bare `is_err()`.

use pre_delete_citation_check::{
    beads_mirror_path, check_deletions, parse_closed_beads, parse_closed_beads_jsonl_checked,
    read_closed_beads_from_mirror,
};

/// A closed bead whose ONLY citation of the deleted path is in a comment. This is the exact
/// shape the crate's own module header names (`cp-3k9jq`: a 104-char close_reason with zero
/// path citations, three comment citations) and the shape 14 live beads have.
const COMMENT_ONLY_CITATION: &str = concat!(
    r#"{"id":"fx-comment-only","status":"closed","close_reason":"DONE superseded by a Rust crate","#,
    r#""comments":[{"author":"a","text":"replaced bin/fleet-composite.py with the crate"}]}"#,
    "\n",
    r#"{"id":"fx-unrelated","status":"closed","close_reason":"DONE nothing to see","comments":[]}"#,
    "\n",
    r#"{"id":"fx-open","status":"open","close_reason":"","comments":[{"author":"b","text":"bin/fleet-composite.py"}]}"#,
    "\n"
);

/// FIRES-ON-KNOWN-BAD, and it is the whole bead: the OLD oracle misses this citation and the
/// NEW one catches it. Both halves asserted in one test so the delta cannot be misread.
#[test]
fn a_comment_only_citation_is_caught_by_the_mirror_and_missed_by_the_br_oracle() {
    let deletions = vec!["bin/fleet-composite.py".to_owned()];

    // NEW oracle: comments are present, so the conflict is found and attributed.
    let closed = parse_closed_beads_jsonl_checked(COMMENT_ONLY_CITATION).expect("fixture parses");
    let conflicts = check_deletions(&deletions, &closed);
    assert_eq!(
        conflicts.len(),
        1,
        "the mirror oracle must catch a comment-only citation; got {conflicts:?}"
    );
    assert_eq!(conflicts[0].bead_id, "fx-comment-only");
    assert_eq!(
        conflicts[0].field, "comment[0]",
        "the conflict must name WHICH surface cited it, so a repair knows where to look"
    );

    // OLD oracle: the same rows through `br list --json` shape carry no comments at all, so
    // the identical deletion sails through. This is not a hypothetical -- it is what the live
    // commit-path gate did until dpa4.
    let br_shaped = format!(
        r#"{{"issues":[{{"id":"fx-comment-only","status":"closed","close_reason":"DONE superseded by a Rust crate"}}]}}"#
    );
    let br_closed = parse_closed_beads(&br_shaped);
    assert_eq!(br_closed.len(), 1, "the br-shaped row must still parse");
    assert!(
        br_closed[0].comments.is_empty(),
        "premise of the whole bead: the br oracle carries no comments"
    );
    assert!(
        check_deletions(&deletions, &br_closed).is_empty(),
        "the br oracle MISSES the comment-only citation -- if this ever finds it, the premise \
         has changed and dpa4 must be re-measured, not assumed"
    );
}

/// An OPEN bead citing the path must NOT produce a conflict: the gate keys on CLOSED beads
/// whose evidence is a live dependency, not on every mention anywhere in the tracker.
#[test]
fn an_open_bead_citing_the_path_is_not_a_conflict() {
    let closed = parse_closed_beads_jsonl_checked(COMMENT_ONLY_CITATION).expect("parses");
    assert!(
        closed.iter().all(|b| b.id != "fx-open"),
        "an open bead must not enter the closed-bead oracle"
    );
}

/// KNOWN-GOOD, MANDATORY: a healthy mirror with an UNCITED deletion must pass. An over-strict
/// gate here stops every commit in the repository.
#[test]
fn a_healthy_mirror_with_an_uncited_deletion_passes() {
    let closed = parse_closed_beads_jsonl_checked(COMMENT_ONLY_CITATION).expect("parses");
    let conflicts = check_deletions(&["bin/never-mentioned-anywhere".to_owned()], &closed);
    assert!(
        conflicts.is_empty(),
        "an uncited deletion must pass cleanly; got {conflicts:?}"
    );
}

/// ANTI-VACUITY: every way of having no oracle is an ERROR with a NAMED reason, never an
/// empty success. A deliverable never checked reports identically to one that passed.
#[test]
fn every_empty_or_unreadable_oracle_is_a_named_refusal() {
    let empty = parse_closed_beads_jsonl_checked("")
        .expect_err("an empty mirror must refuse, never return an empty pass");
    assert!(
        empty.contains("PRE_DELETE_BEADS_EMPTY reason=zero_bead_records_readable"),
        "got {empty:?}"
    );

    let no_closed = parse_closed_beads_jsonl_checked("{\"id\":\"a\",\"status\":\"open\"}\n")
        .expect_err("a mirror with no CLOSED rows must refuse");
    assert!(
        no_closed.contains("PRE_DELETE_BEADS_EMPTY reason=no_closed_records_readable"),
        "got {no_closed:?}"
    );

    let malformed = parse_closed_beads_jsonl_checked("{not json\n")
        .expect_err("malformed JSONL must refuse");
    assert!(
        malformed.contains("PRE_DELETE_BEADS_UNREADABLE reason=malformed_jsonl line=0"),
        "the refusal must name the offending line; got {malformed:?}"
    );

    let absent = read_closed_beads_from_mirror(std::path::Path::new(
        "/nonexistent/dpa4-probe-repo-root",
    ))
    .expect_err("an ABSENT mirror must refuse, not read as zero citations");
    assert!(
        absent.contains("PRE_DELETE_BEADS_UNREADABLE reason=mirror_unreadable"),
        "got {absent:?}"
    );
    assert!(
        absent.contains(".beads/issues.jsonl"),
        "the refusal must name the path it could not read; got {absent:?}"
    );
}

/// The REAL mirror in this repository must be readable and must actually carry comments.
/// A fixture-only suite cannot prove the production oracle is non-vacuous.
///
/// ENVIRONMENT DISCRIMINATION, and it is the point of this comment. Joshua's contabo-lane rule
/// makes the Linux workers authoritative, and `rch` syncs SOURCE without `.git` or `.beads/`.
/// This test failed there on its first lane run -- correctly, as written, and for the wrong
/// reason: the mirror was ABSENT, which is UNMEASURED, not "the oracle is vacuous". Those are
/// two verdicts with two remedies and a Rust test has only pass/fail to say them in.
///
/// The discriminator is POSITIVE, never "the input is missing so assume fine": a tree with no
/// `.git` is a synced worker copy and cannot answer this question at all. A tree that IS a
/// checkout and has no mirror is the real defect and still FAILS. That keeps the vacuous-green
/// shape out: absence alone never satisfies this test, only absence PLUS proof that the
/// environment is not a repository.
///
/// CAVEAT MEASURED ON THE LANE, and it limits the claim: libtest CAPTURES stdout for a
/// PASSING test, so the `UNMEASURED` line below is invisible in a green run unless
/// `-- --nocapture` is passed. Verified on contabo-1 -- the declaration is emitted and the
/// default lane run does not show it. So a green lane result for this suite cannot, by
/// itself, distinguish "this leg ran and passed" from "this leg declined". Cite the
/// `--nocapture` run when the distinction matters; a per-run artifact would remove the
/// caveat and is not built.
#[test]
fn the_real_repository_mirror_carries_comments() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let path = beads_mirror_path(&repo_root);

    if !repo_root.join(".git").exists() {
        // Loud, named, and it prints the reason so a reader of a green run can see that this
        // leg did NOT run rather than inferring it passed.
        println!(
            "UNMEASURED reason=not_a_repo_checkout root={} mirror_present={} -- rch syncs \
             source without .git or .beads, so the production oracle is unobservable here. \
             This is not a pass for the subject.",
            repo_root.display(),
            path.exists()
        );
        return;
    }

    assert!(
        path.exists(),
        "this IS a repo checkout ({}) and the mirror is absent at {} -- the live gate has no \
         oracle, which is the dpa4 defect in its most direct form",
        repo_root.display(),
        path.display()
    );

    let closed = read_closed_beads_from_mirror(&repo_root).expect("the real mirror must parse");
    let with_comments = closed.iter().filter(|b| !b.comments.is_empty()).count();
    assert!(
        closed.len() >= 50,
        "closed-bead denominator collapsed to {} -- a shrinking oracle is how this gate goes \
         vacuously green",
        closed.len()
    );
    assert!(
        with_comments > 0,
        "{} closed beads and NONE carry comments: the oracle is vacuous again, which is \
         exactly the dpa4 defect",
        closed.len()
    );
}
