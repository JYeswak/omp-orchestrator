//! Hermetic slice-c tests: the ack detector's known-bad, known-good, and mutation legs.
//!
//! These tests feed repository-local read-back fixtures into the production
//! classifier. They never mutate the live tracker or depend on a bead existing.

use ack_spine::ack::{classify_ack_readback, AckVerdict, SingularTrapResult};

const FIXTURE_BEAD: &str = "fixture-bead";
const TRAP_MARKER: &str = "ACK-TRAP-FIXTURE";
const GOOD_MARKER: &str = "ACK-GOOD-FIXTURE";

fn singular_trap_fixture() -> SingularTrapResult {
    SingularTrapResult {
        // Measured singular br comment trap shape: exit 0 with an error on stderr.
        exit_code: Some(0),
        stderr: "error: unexpected argument".to_owned(),
        comment_landed: false,
    }
}

fn empty_comment_list() -> &'static str {
    "Comments for fixture-bead:\n"
}

/// KNOWN-BAD: the singular verb reports exit 0 but does not post. A subsequent
/// successful read-back with no marker is Missing, never Confirmed.
#[test]
fn singular_verb_trap_produces_missing_ack() {
    let trap = singular_trap_fixture();
    assert_eq!(trap.exit_code, Some(0));
    assert!(!trap.comment_landed);
    assert!(trap.stderr.contains("unexpected argument"));

    let verdict =
        classify_ack_readback(FIXTURE_BEAD, TRAP_MARKER, Some(0), empty_comment_list(), "");
    assert!(
        matches!(verdict, AckVerdict::Missing { .. }),
        "the singular-verb trap must produce Missing, got {verdict:?}"
    );
}

/// KNOWN-GOOD: a repository-local read-back fixture containing the marker is
/// Confirmed. No live br comments add call is needed.
#[test]
fn genuinely_posted_comment_is_confirmed() {
    let read_back = "Comments for fixture-bead:\n[ack-test] ACK-GOOD-FIXTURE\n";
    let verdict = classify_ack_readback(FIXTURE_BEAD, GOOD_MARKER, Some(0), read_back, "");
    assert!(
        matches!(verdict, AckVerdict::Confirmed { .. }),
        "a genuinely posted comment must be Confirmed, got {verdict:?}"
    );
}

/// TRACKER UNREADABLE is an ERROR, never "no ack".
#[test]
fn tracker_unreadable_is_unverifiable_not_missing() {
    let verdict = classify_ack_readback(
        FIXTURE_BEAD,
        GOOD_MARKER,
        Some(3),
        "",
        "Error: Issue not found: fixture-bead",
    );
    match &verdict {
        AckVerdict::Unverifiable { detail, .. } => {
            assert!(
                detail.contains("Issue not found"),
                "the Unverifiable variant must preserve tracker detail: {verdict:?}"
            );
        }
        AckVerdict::Missing { .. } => {
            panic!("an unreadable tracker must not be reported as Missing");
        }
        AckVerdict::Confirmed { .. } => {
            panic!("an unreadable tracker must never produce Confirmed");
        }
    }
}

/// MUTATION: read-back distinguishes an empty fixture from a marker-bearing
/// fixture. Removing that comparison makes both cases produce the same result.
#[test]
fn read_back_distinguishes_trap_from_good() {
    let trap_verdict =
        classify_ack_readback(FIXTURE_BEAD, TRAP_MARKER, Some(0), empty_comment_list(), "");
    assert!(
        matches!(trap_verdict, AckVerdict::Missing { .. }),
        "trap must produce Missing, got {trap_verdict:?}"
    );

    let good_read_back = "Comments for fixture-bead:\n[ack-test] ACK-GOOD-DISTINCT\n";
    let good_verdict = classify_ack_readback(
        FIXTURE_BEAD,
        "ACK-GOOD-DISTINCT",
        Some(0),
        good_read_back,
        "",
    );
    assert!(
        matches!(good_verdict, AckVerdict::Confirmed { .. }),
        "good must produce Confirmed, got {good_verdict:?}"
    );

    assert_ne!(
        trap_verdict, good_verdict,
        "read-back must distinguish the two"
    );
}
