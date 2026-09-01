#![forbid(unsafe_code)]

use dispatch_silence_watch::{classify, has_posted_verdict, SilenceVerdict};

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
    let v = classify(&with_comment(), "AmberGate", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::VerdictPosted);
}

#[test]
fn empty_comments_past_deadline_is_silent() {
    let v = classify(&without_comment(), "AmberGate", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::SilentPastDeadline);
}

#[test]
fn assignee_change_is_reassigned_even_with_comments() {
    let v = classify(&with_comment(), "SilverWolf", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::Reassigned);
}

#[test]
fn unreadable_tracker_is_never_verdict_posted() {
    let v = classify(&usage_error_on_stderr(), "AmberGate", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::TrackerError);
    assert_ne!(v, SilenceVerdict::VerdictPosted);
}

#[test]
fn empty_output_is_tracker_error() {
    let v = classify("", "AmberGate", "AmberGate", DISPATCH, NOW, DEADLINE);
    assert_eq!(v, SilenceVerdict::TrackerError);
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
    let v = classify(&without_comment(), "AmberGate", "AmberGate", recent, NOW, DEADLINE);
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
