#![forbid(unsafe_code)]

//! 812ax: `hd0009_status` must derive from the ledger, and the operator flag
//! must SAY when it is overriding.
//!
//! These legs are the ledger-side contract. The binary-level readback lives in
//! crates/ompo-doctor/tests/start_observability.rs.

use std::fs;
use std::path::{Path, PathBuf};

use ompo_start::hd0009::{self, Authority};

fn fixture() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "l3-hd0009-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    fs::create_dir_all(path.join("docs")).expect("fixture docs dir");
    path
}

fn write_ledger(repo: &Path, lines: &[&str]) {
    fs::write(
        hd0009::ledger_path(repo),
        format!("{}\n", lines.join("\n")),
    )
    .expect("fixture ledger");
}

const ASKED: &str = r#"{"id":"HD-0009","question":"which substrate?","decision":""}"#;
const ANSWERED: &str = r#"{"answers":"HD-0009","decider":"Joshua","decision":"frankentui rust yes"}"#;

/// THE BEAD (item 3). Rows PRESENT and NO flag must NOT read undecided. This
/// leg fails against the pre-812ax resolver by construction, because that
/// resolver never read the ledger at all.
#[test]
fn an_answered_ledger_row_decides_without_any_flag() {
    let repo = fixture();
    write_ledger(&repo, &[ASKED, ANSWERED]);
    let resolved = hd0009::resolve(&repo, false);
    assert!(resolved.decided, "an answered row decides: {resolved:?}");
    assert_eq!(resolved.authority, Authority::LedgerDecided);
    assert_eq!(resolved.authority.token(), "ledger_decided");
}

/// ⛔ THE TRAP: a merely-ASKED question must NOT count as decided. The request
/// row carries `id = HD-0009` with an EMPTY decision, so `grep -c HD-0009`
/// reads 2 on a ledger that has decided NOTHING. A presence predicate passes
/// this input and is wrong.
#[test]
fn an_asked_but_unanswered_question_is_not_decided() {
    let repo = fixture();
    write_ledger(&repo, &[ASKED]);
    let resolved = hd0009::resolve(&repo, false);
    assert!(!resolved.decided, "an empty decision is not a decision: {resolved:?}");
    assert_eq!(resolved.authority, Authority::LedgerUndecided);
}

/// ANTI-VACUITY (item 6). An absent ledger is a REFUSAL with its own token and
/// its own detail, never the representation a recorded non-decision uses.
/// These two inputs agree on `decided` and MUST NOT agree on authority.
#[test]
fn an_absent_ledger_is_a_refusal_and_not_a_recorded_non_decision() {
    let absent = fixture();
    let recorded = fixture();
    write_ledger(&recorded, &[ASKED]);

    let absent_resolved = hd0009::resolve(&absent, false);
    let recorded_resolved = hd0009::resolve(&recorded, false);

    assert!(!absent_resolved.decided && !recorded_resolved.decided);
    assert!(
        absent_resolved.authority.is_refusal(),
        "a missing ledger is a refusal: {absent_resolved:?}"
    );
    assert!(
        !recorded_resolved.authority.is_refusal(),
        "a readable ledger that decided nothing is a READING: {recorded_resolved:?}"
    );
    assert_ne!(
        absent_resolved.authority.token(),
        recorded_resolved.authority.token(),
        "a missing ledger and a recorded non-decision must not share a representation"
    );
    assert!(absent_resolved
        .authority
        .detail()
        .is_some_and(|d| d.contains("HD0009_LEDGER_ABSENT")));
}

/// An unreadable ledger is its OWN refusal, distinct from absence: "nobody
/// recorded anything" and "the record cannot be read" are different facts.
#[test]
fn an_unparseable_ledger_is_a_distinct_refusal() {
    let repo = fixture();
    write_ledger(&repo, &["{this is not json"]);
    let resolved = hd0009::resolve(&repo, false);
    assert!(!resolved.decided);
    assert!(resolved.authority.is_refusal());
    assert_eq!(resolved.authority.token(), "ledger_unreadable");
    assert!(resolved
        .authority
        .detail()
        .is_some_and(|d| d.contains("HD0009_LEDGER_UNREADABLE")));
}

/// PRECEDENCE (item 1), stated as a leg rather than only in prose: the flag may
/// decide, and when it does it reports ITSELF, never the ledger's authority.
#[test]
fn the_flag_decides_only_by_naming_itself_an_override() {
    let repo = fixture();
    write_ledger(&repo, &[ASKED]);
    let resolved = hd0009::resolve(&repo, true);
    assert!(resolved.decided, "the override still decides: {resolved:?}");
    assert_eq!(resolved.authority, Authority::FlagOverride);
    assert_eq!(resolved.authority.token(), "flag_override");
    assert!(
        !resolved.authority.is_refusal(),
        "an override is an assertion, not a refusal"
    );
}

/// PROVENANCE BEATS AN AGREEING ASSERTION: a ledger that already decided is not
/// relabelled by a flag that happens to agree. Otherwise every recorded
/// decision would be reported as an operator override the moment anyone passed
/// the flag, and the field would lose the provenance it exists to carry.
#[test]
fn a_decided_ledger_is_not_relabelled_by_an_agreeing_flag() {
    let repo = fixture();
    write_ledger(&repo, &[ASKED, ANSWERED]);
    let resolved = hd0009::resolve(&repo, true);
    assert!(resolved.decided);
    assert_eq!(
        resolved.authority,
        Authority::LedgerDecided,
        "the ledger is the authority; the flag adds nothing here: {resolved:?}"
    );
}

/// The override must NOT invent provenance on a refusal either: with the ledger
/// absent, the flag can decide, but nothing may claim the ledger said so.
#[test]
fn an_override_over_an_absent_ledger_still_names_itself() {
    let repo = fixture();
    let resolved = hd0009::resolve(&repo, true);
    assert!(resolved.decided);
    assert_eq!(resolved.authority, Authority::FlagOverride);
    assert_ne!(resolved.authority.token(), "ledger_decided");
}

/// THE PIN (the j4ert lesson applied before it bites): `detail()` is Some
/// EXACTLY when `is_refusal()` is true, across every authority this module can
/// produce. Two encodings of one fact drift unless something holds them
/// together, and this is that leg.
#[test]
fn refusal_detail_is_present_exactly_when_the_authority_is_a_refusal() {
    let absent = fixture();
    let unreadable = fixture();
    write_ledger(&unreadable, &["{"]);
    let asked = fixture();
    write_ledger(&asked, &[ASKED]);
    let answered = fixture();
    write_ledger(&answered, &[ASKED, ANSWERED]);

    let cases = [
        hd0009::resolve(&absent, false).authority,
        hd0009::resolve(&unreadable, false).authority,
        hd0009::resolve(&asked, false).authority,
        hd0009::resolve(&answered, false).authority,
        hd0009::resolve(&asked, true).authority,
    ];
    // Positive control: the matrix really does contain both polarities, so the
    // equivalence below is not vacuously true over a single-polarity set.
    assert!(cases.iter().any(|a| a.is_refusal()));
    assert!(cases.iter().any(|a| !a.is_refusal()));
    for authority in cases {
        assert_eq!(
            authority.detail().is_some(),
            authority.is_refusal(),
            "detail and is_refusal must not disagree: {authority:?}"
        );
    }
}
