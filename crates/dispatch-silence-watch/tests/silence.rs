#![forbid(unsafe_code)]

use dispatch_silence_watch::{
    classify, classify_from_read, clears_pending_dispatch_intent, has_posted_verdict,
    parse_bead_assignee, resolve_br_id, BeadIdMatchKind, BeadIdResolutionError, SilenceVerdict,
    TrackerRead,
};
#[test]
fn short_br_id_is_canonicalized_before_exact_assignee_read() {
    let response =
        r#"[{"id":"omp-orchestrator-omp-coverage-mission-ipg.18","assignee":"WildStone"}]"#;
    let resolution = resolve_br_id(response, "ipg.18").expect("br suffix result");
    assert_eq!(resolution.match_kind, BeadIdMatchKind::Suffix);
    assert_eq!(
        resolution.canonical_id,
        "omp-orchestrator-omp-coverage-mission-ipg.18"
    );
    let error =
        parse_bead_assignee(response, "ipg.18").expect_err("short id must not enter exact reader");
    assert!(error.to_string().starts_with("UNRESOLVED-SHORT-ID"));
    assert_eq!(
        parse_bead_assignee(response, &resolution.canonical_id)
            .expect("canonical assignee read")
            .as_deref(),
        Some("WildStone")
    );
}

#[test]
fn canonical_br_id_is_an_exact_match_on_both_surfaces() {
    let response = r#"[{"id":"omp-orchestrator-pgzx","assignee":"WildStone"}]"#;
    let resolution = resolve_br_id(response, "omp-orchestrator-pgzx").expect("canonical result");
    assert_eq!(resolution.match_kind, BeadIdMatchKind::Exact);
    assert_eq!(resolution.canonical_id, resolution.requested_id);
    assert_eq!(
        parse_bead_assignee(response, &resolution.canonical_id)
            .expect("canonical assignee read")
            .as_deref(),
        Some("WildStone")
    );
}

#[test]
fn unresolved_short_id_is_typed_not_a_missing_bead() {
    let response = r#"{"error":{"code":"ISSUE_NOT_FOUND"}}"#;
    let error = resolve_br_id(response, "not-a-real-suffix").expect_err("unresolved short id");
    assert_eq!(
        error,
        BeadIdResolutionError::UnresolvedShortId {
            requested_id: "not-a-real-suffix".into()
        }
    );
    assert!(error.to_string().starts_with("UNRESOLVED-SHORT-ID"));
}

#[test]
fn ambiguous_suffix_is_an_error_not_first_match() {
    let response = r#"{"error":{"code":"AMBIGUOUS_ID","context":{"matches":["omp-orchestrator-alpha-p","omp-orchestrator-beta-p"]}}}"#;
    let error = resolve_br_id(response, "p").expect_err("ambiguous suffix");
    assert_eq!(
        error,
        BeadIdResolutionError::Ambiguous {
            requested_id: "p".into(),
            matches: vec![
                "omp-orchestrator-alpha-p".into(),
                "omp-orchestrator-beta-p".into()
            ]
        }
    );
    assert!(error.to_string().starts_with("AMBIGUOUS-BEAD-ID"));
}

const NOW: i64 = 1_000_000;
const DISPATCH: i64 = NOW - 7200; // 2 hours ago
const DEADLINE: i64 = 3600; // 1 hour deadline

fn with_comment() -> String {
    "Comments for cp-test:\n[AmberGate] at 2026-08-31 16:56 UTC\nGRADE from AmberGate — CONFIRMED.\n".into()
}

fn without_comment() -> String {
    "Comments for cp-test:\n".into()
}

fn usage_error_on_stderr() -> String {
    // The `br comment` singular trap: usage error on stderr, exit 0.
    // The caller passes only the STDOUT to classify, so this fixture
    // represents what the caller would see if it mistook stderr for stdout:
    // an error line that must NOT be read as a comment.
    "Error: Issue not found: cp-test\nHint: Run 'br list' to see available issues.\n".into()
}

#[test]
fn posted_comment_is_verdict_posted() {
    let v = classify(
        &with_comment(),
        "AmberGate",
        "AmberGate",
        DISPATCH,
        NOW,
        DEADLINE,
    );
    assert_eq!(v, SilenceVerdict::VerdictPosted);
}

#[test]
fn empty_comments_past_deadline_is_silent() {
    let v = classify(
        &without_comment(),
        "AmberGate",
        "AmberGate",
        DISPATCH,
        NOW,
        DEADLINE,
    );
    assert_eq!(v, SilenceVerdict::SilentPastDeadline);
}

#[test]
fn assignee_change_is_reassigned_even_with_comments() {
    let v = classify(
        &with_comment(),
        "SilverWolf",
        "AmberGate",
        DISPATCH,
        NOW,
        DEADLINE,
    );
    assert_eq!(v, SilenceVerdict::Reassigned);
}

#[test]
fn genuine_br_failure_read_is_tracker_error() {
    let v = classify_from_read(
        TrackerRead::TrackerError("br exited nonzero"),
        "AmberGate",
        "AmberGate",
        DISPATCH,
        NOW,
        DEADLINE,
    );
    assert_eq!(v, SilenceVerdict::TrackerError);
    assert_eq!(v.detector(), "TRACKER_ERROR");
    assert!(clears_pending_dispatch_intent(v));
}

#[test]
fn quoting_error_colon_on_successful_read_is_not_tracker_error() {
    // Live case: dp21 comments contain `Error:` twice; `br comments list` exits 0.
    let payload = "Comments for omp-orchestrator-retry-producer-unfiltered-dp21:\n\
         [WildStone] at 2026-09-06 13:00 UTC\n\
         Evidence: `Error: cannot claim blocked issue` pasted verbatim.\n";
    assert!(payload.contains("Error:"));
    let v = classify_from_read(
        TrackerRead::Read(payload.to_owned()),
        "AmberGate",
        "AmberGate",
        DISPATCH,
        NOW,
        DEADLINE,
    );
    assert_ne!(v, SilenceVerdict::TrackerError);
    assert_eq!(v, SilenceVerdict::VerdictPosted);
    assert!(!clears_pending_dispatch_intent(
        SilenceVerdict::SilentPastDeadline
    ));
}

#[test]
fn stderr_shaped_error_payload_is_not_tracker_error() {
    // Channel, not content: this string is what `br` prints on failure.
    // When it arrives as a successful stdout Read, it is payload, not a
    // tracker terminal. The known-good TrackerError path is classify_from_read.
    let v = classify(
        &usage_error_on_stderr(),
        "AmberGate",
        "AmberGate",
        DISPATCH,
        NOW,
        DEADLINE,
    );
    assert_ne!(v, SilenceVerdict::TrackerError);
    assert_eq!(v, SilenceVerdict::SilentPastDeadline);
}

#[test]
fn empty_output_is_tracker_error() {
    let v = classify("", "AmberGate", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::TrackerError);
    assert!(clears_pending_dispatch_intent(v));
}

#[test]
fn br_comment_singular_trap_does_not_produce_verdict_posted() {
    // The usage-error fixture has no `[Author] at date` block, so
    // has_posted_verdict must return false even though the command exited 0.
    assert!(!has_posted_verdict(&usage_error_on_stderr()));
}

#[test]
fn within_deadline_no_comment_is_not_verdict_posted() {
    let recent = NOW - 60; // 1 minute ago, deadline 3600s
    let v = classify(
        &without_comment(),
        "AmberGate",
        "AmberGate",
        recent,
        NOW,
        DEADLINE,
    );
    assert_ne!(v, SilenceVerdict::VerdictPosted, "no comment = not posted");
}

#[test]
fn json_array_in_comments_does_not_match_attribution() {
    // A JSON array in the output must not be mistaken for a comment block.
    let json_like = "[\"key\", \"value\"] at something";
    assert!(!has_posted_verdict(json_like));
}

#[test]
fn multiple_comments_still_verdict_posted() {
    let multi = "Comments for cp-test:\n[AmberGate] at 2026-08-31 16:56 UTC\nFirst.\n\n[SilverWolf] at 2026-08-31 17:00 UTC\nSecond.\n";
    assert!(has_posted_verdict(multi));
    let v = classify(multi, "AmberGate", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::VerdictPosted);
}
// Wiring proof: dispatch-silence-watch must appear in the live crontab.
// The conductor cron (controller-tick) runs at :18,:38,:58 — silence-watch
// fires at :01,:21,:41, 3 minutes after each tick, so it sees the post-tick
// board state. This test reads the LIVE crontab and asserts the entry exists.
#[test]
fn dispatch_silence_watch_is_in_crontab() {
    let output = std::process::Command::new("crontab")
        .arg("-l")
        .output()
        .expect("crontab -l must be runnable");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("dispatch-silence-watch"),
        "crontab must contain a dispatch-silence-watch entry — it is not wired to the conductor cadence"
    );
}
