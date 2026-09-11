#![forbid(unsafe_code)]

//! L4-SRC-MAIL legs: `age_ms` derived from `am robot status` `_meta.timestamp`.
//!
//! The single acceptance-named leg `missing_mail_meta_is_silent` also lives in
//! `tests/l4_liveness.rs`, where the bead's acceptance command points. This file
//! carries the full battery — known-good, the two clock-disagreement legs, and
//! the anti-vacuity error leg — in a file held by one worker, so a mutation
//! proof never has to revert a shared file.

use ompo_start::liveness::{classify, LiveVerdict, SourceVerdict};
use ompo_start::mail::{mail_freshness, mail_source, MailFreshness, MAIL_SOURCE_NAME};

/// The measured `am robot status --json` envelope head, confirmed live against the
/// installed `am` on 2026-09-11: RFC3339 with an explicit `+00:00` offset.
const STATUS_WITH_META: &str = r#"{
  "_meta": {
    "command": "robot status",
    "timestamp": "2026-09-11T18:05:39.843414+00:00",
    "format": "json",
    "version": "1.0",
    "project": "omp-orchestrator"
  },
  "_alerts": []
}"#;

/// The same envelope with `_meta` absent — the SILENT case the bead names.
const STATUS_WITHOUT_META: &str = r#"{
  "_alerts": [],
  "sessions": []
}"#;

/// `_meta` present but carrying no `timestamp` — absence one level deeper.
const STATUS_META_WITHOUT_TIMESTAMP: &str = r#"{
  "_meta": { "command": "robot status", "format": "json" },
  "_alerts": []
}"#;

/// The writer clock reading embedded in [`STATUS_WITH_META`], recovered through the
/// parser itself rather than hand-computed, so the fixture and the expectation cannot drift.
fn writer_ms() -> i64 {
    // A far-future observer reading (year 2096 in epoch ms) so the difference is
    // positive for any plausible fixture stamp and the parse result is an Age.
    match mail_freshness(STATUS_WITH_META, 4_000_000_000_000) {
        Ok(MailFreshness::Age { writer_ms, .. }) => writer_ms,
        other => panic!("the measured envelope must parse to an Age: {other:?}"),
    }
}

fn other_source(name: &str) -> SourceVerdict {
    SourceVerdict {
        name: name.to_owned(),
        available: true,
        fresh: true,
        reason_code: "fixture".to_owned(),
        age_ms: Some(1),
        panes: vec!["%7".to_owned()],
    }
}

#[test]
fn mail_meta_timestamp_yields_age_ms() {
    // KNOWN-GOOD: both clocks readable and ordered, so an age exists.
    let observer_ms = writer_ms() + 5_000;
    let freshness = mail_freshness(STATUS_WITH_META, observer_ms).expect("envelope is readable");
    println!("known-good freshness={freshness:?} status={}", freshness.status());
    assert_eq!(freshness.age_ms(), Some(5_000));
    assert_eq!(freshness.status(), "FRESH");
    assert!(!freshness.is_silent());

    let source = mail_source(STATUS_WITH_META, observer_ms, vec!["%7".to_owned()])
        .expect("envelope is readable");
    assert_eq!(source.name, MAIL_SOURCE_NAME);
    assert!(source.fresh);
    assert_eq!(source.age_ms, Some(5_000));
}

#[test]
fn missing_mail_meta_is_silent() {
    // KNOWN-BAD: the absent timestamp must be UNKNOWN, never age 0.
    for (label, envelope) in [
        ("no _meta at all", STATUS_WITHOUT_META),
        ("_meta without timestamp", STATUS_META_WITHOUT_TIMESTAMP),
    ] {
        let freshness = mail_freshness(envelope, writer_ms() + 5_000).expect("envelope is JSON");
        println!("{label}: {} {}", freshness.status(), freshness.reason_code());
        assert_eq!(freshness.status(), "SILENT");
        assert!(freshness.is_silent());
        assert_eq!(freshness.age_ms(), None, "{label}: absent must be UNKNOWN");
        assert_ne!(
            freshness.age_ms(),
            Some(0),
            "{label}: an absent timestamp must NOT read as age 0"
        );
        assert!(freshness.reason_code().contains("L4_MAIL_MISSING_META"));

        let source = mail_source(envelope, writer_ms() + 5_000, vec!["%7".to_owned()])
            .expect("envelope is JSON");
        assert!(!source.fresh);
        assert_eq!(source.age_ms, None);

        let verdict = classify(vec![
            source,
            other_source("ntm"),
            other_source("tick-monitor"),
        ])
        .expect("complete source set");
        println!("{label}: swarm={} {}", verdict.status(), verdict.reason_code());
        assert!(matches!(verdict, LiveVerdict::NotLive { .. }));
        assert!(verdict.reason_code().contains(MAIL_SOURCE_NAME));
    }
}

#[test]
fn mail_writer_clock_ahead_of_observer_is_silent_not_zero() {
    // The two clocks disagree. A clamp to 0 would report the worst disagreement as
    // the freshest possible reading, so the difference is refused instead.
    let freshness =
        mail_freshness(STATUS_WITH_META, writer_ms() - 1).expect("envelope is readable");
    println!("skew: {} {}", freshness.status(), freshness.reason_code());
    assert_eq!(freshness.status(), "SILENT");
    assert_eq!(freshness.age_ms(), None);
    assert_ne!(freshness.age_ms(), Some(0));
    assert!(freshness.reason_code().contains("L4_MAIL_CLOCK_SKEW"));
}

#[test]
fn identical_clocks_are_age_zero_only_when_both_were_read() {
    // Age 0 is a LEGITIMATE reading — but only when both clocks were actually read.
    // This leg is what keeps the absent-is-not-zero rule from being a blanket ban on 0.
    let freshness = mail_freshness(STATUS_WITH_META, writer_ms()).expect("envelope is readable");
    assert_eq!(freshness.age_ms(), Some(0));
    assert_eq!(freshness.status(), "FRESH");
}

#[test]
fn unparsable_mail_timestamp_is_silent_not_zero() {
    let envelope = r#"{"_meta":{"timestamp":"yesterday afternoon"}}"#;
    let freshness = mail_freshness(envelope, writer_ms()).expect("envelope is JSON");
    println!("unparsable: {} {}", freshness.status(), freshness.reason_code());
    assert_eq!(freshness.status(), "SILENT");
    assert_eq!(freshness.age_ms(), None);
    assert!(freshness
        .reason_code()
        .contains("L4_MAIL_UNPARSABLE_TIMESTAMP"));
}

#[test]
fn naive_mail_timestamp_without_offset_is_silent() {
    // No offset means no knowable instant; assuming UTC would invent a clock.
    let envelope = r#"{"_meta":{"timestamp":"2026-09-11T18:05:39.843414"}}"#;
    let freshness = mail_freshness(envelope, writer_ms()).expect("envelope is JSON");
    assert_eq!(freshness.age_ms(), None);
    assert!(freshness
        .reason_code()
        .contains("L4_MAIL_UNPARSABLE_TIMESTAMP"));
}

#[test]
fn unreadable_mail_envelope_is_an_error_not_a_silent_source() {
    // ANTI-VACUITY: an instrument that could not be read is a hard error. If this
    // degraded to SILENT the predicate would report "not live" for a broken parser.
    let error = mail_freshness("this is not json", 0).expect_err("garbage must refuse");
    assert!(error.contains("L4_MAIL_UNREADABLE_ENVELOPE"), "{error}");
    let error = mail_freshness("[]", 0).expect_err("a JSON array is not an envelope");
    assert!(error.contains("L4_MAIL_UNREADABLE_ENVELOPE"), "{error}");
    let error = mail_source("this is not json", 0, Vec::new()).expect_err("mail_source refuses too");
    assert!(error.contains("L4_MAIL_UNREADABLE_ENVELOPE"), "{error}");
}

#[test]
fn offset_forms_agree_on_the_same_instant() {
    // Z, +00:00 and a shifted offset naming the same instant must yield one age.
    let zulu = r#"{"_meta":{"timestamp":"2026-09-11T18:05:39.843Z"}}"#;
    let explicit = r#"{"_meta":{"timestamp":"2026-09-11T18:05:39.843+00:00"}}"#;
    let shifted = r#"{"_meta":{"timestamp":"2026-09-11T20:05:39.843+02:00"}}"#;
    let observer = writer_ms() + 1_000;
    let ages: Vec<Option<u64>> = [zulu, explicit, shifted]
        .iter()
        .map(|envelope| {
            mail_freshness(envelope, observer)
                .expect("envelope is readable")
                .age_ms()
        })
        .collect();
    println!("offset forms ages={ages:?}");
    assert_eq!(ages[0], ages[1]);
    assert_eq!(ages[1], ages[2]);
    assert_eq!(ages[0], Some(1_000));
}

/// The process-level consumer: `ompo-start-mail-freshness` must carry the same
/// three readings the library derives. BUILT IS NOT WIRED — these two legs are
/// what make the derivation something a caller actually observes.
fn run_probe(label: &str, envelope: &str, now_ms: i64) -> (Option<i32>, String) {
    use std::io::Write;
    let mut path = std::env::temp_dir();
    // `label` keeps concurrent legs off one another's fixture: two legs sharing a
    // now_ms would otherwise share a filename and read each other's envelope.
    path.push(format!("ompo_start_mail_probe_{label}_{now_ms}.json"));
    let mut file = std::fs::File::create(&path).expect("fixture path is writable");
    file.write_all(envelope.as_bytes()).expect("fixture written");
    drop(file);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ompo-start-mail-freshness"))
        .arg(&path)
        .arg("--now-ms")
        .arg(now_ms.to_string())
        .output()
        .expect("the consumer binary must be spawnable");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.code(), text)
}

#[test]
fn consumer_binary_reports_age_ms_for_a_stamped_envelope() {
    let (code, text) = run_probe("fresh", STATUS_WITH_META, writer_ms() + 5_000);
    println!("probe good: code={code:?} out={}", text.trim());
    assert_eq!(code, Some(0));
    assert!(text.contains("status=FRESH"), "{text}");
    assert!(text.contains("age_ms=5000"), "{text}");
}

#[test]
fn consumer_binary_refuses_an_absent_timestamp_without_emitting_zero() {
    let (code, text) = run_probe("absent", STATUS_WITHOUT_META, writer_ms() + 5_000);
    println!("probe silent: code={code:?} out={}", text.trim());
    assert_eq!(code, Some(2), "SILENT is exit 2, not success");
    assert!(text.contains("status=SILENT"), "{text}");
    assert!(text.contains("age_ms=UNKNOWN"), "{text}");
    assert!(!text.contains("age_ms=0"), "absent must never print 0: {text}");
    assert!(text.contains("L4_MAIL_MISSING_META"), "{text}");
}
