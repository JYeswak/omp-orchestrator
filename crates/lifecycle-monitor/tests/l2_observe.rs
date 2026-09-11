#![forbid(unsafe_code)]

//! Named L2 observe target (0pc9): the monitor answers for S1.L2 with an AGE,
//! or refuses in a typed way -- never a bare boolean.
//!
//! A monitor that answers true/false without an age cannot distinguish "fresh
//! and clear" from "stale and unread". This repo has already paid for that
//! collapse (six red CI runs unread in one evening; a supervisor's typed
//! refusal read 29 times by nobody), so every leg below asserts the age
//! alongside the state.
//!
//! The layer selector is NOT invented here: `observe --layer <L>` already
//! exists (crates/lifecycle-monitor/src/main.rs:57) and METRICS.toml already
//! carries the L2 row (`MET-L2-INIT-READBACK-FAILURE-RATE`). The threshold is
//! READ from that file rather than restated, so a threshold edit reddens these
//! legs instead of silently diverging from them.

use lifecycle_monitor::load_metrics;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const METRICS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../METRICS.toml");

fn observe(journal: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lifecycle-monitor"))
        .args([
            "observe",
            "--journal",
            journal.to_str().expect("journal path utf8"),
            "--layer",
            "L2",
            "--metrics",
            Path::new(METRICS_PATH)
                .to_str()
                .expect("metrics path utf8"),
        ])
        .output()
        .expect("lifecycle-monitor observe must launch")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs()
}

/// The row the ompo init write+reprobe chokepoint actually emits, with a
/// caller-chosen timestamp. Shape copied from the live emit
/// (`emit_init_event`: Layer::L2, S1.L1 -> S1.L2, actor ompo-init,
/// reason INIT_REPROBE_OK) so a divergence in the writer shows up here.
fn l2_row(ts: u64) -> String {
    format!(
        r#"{{"schema":"omp.lifecycle_event.v1","layer":"L2","stage_from":"S1.L1","stage_to":"S1.L2","actor":"ompo-init","outcome":"emitted","reason_code":"INIT_REPROBE_OK","ts_unix":{ts}}}"#
    )
}

fn l2_stall_after_ms() -> u64 {
    load_metrics(Path::new(METRICS_PATH))
        .expect("METRICS.toml")
        .iter()
        .find(|spec| spec.layer.as_str() == "L2")
        .expect("METRICS.toml must carry an L2 row")
        .stall_after_ms
}

fn age_ms(stdout: &str) -> u64 {
    stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("age_ms="))
        .expect("observe output must carry age_ms")
        .parse()
        .expect("age_ms must be numeric")
}

/// KNOWN-GOOD: a fresh L2 row observes as progressing, and carries a measured
/// age inside the declared threshold. Present so the stale leg below cannot
/// pass by the monitor refusing everything.
#[test]
fn l2_observe_reports_progressing_with_measured_fresh_age() {
    let directory = tempfile::tempdir().expect("fresh journal fixture");
    let journal = directory.path().join("fresh.jsonl");
    std::fs::write(&journal, l2_row(now_secs())).expect("write fresh row");

    let output = observe(&journal);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    assert!(stdout.contains("layer=L2 state=progressing"), "{stdout}");
    assert!(stdout.contains("fresh=true"), "{stdout}");
    assert!(stdout.contains("reason=INIT_REPROBE_OK"), "{stdout}");
    assert!(age_ms(&stdout) <= l2_stall_after_ms(), "{stdout}");
}

/// The reported reason MOVES WITH THE ROW, so `reason=` is a round-trip and not
/// a literal.
///
/// Self-audit finding on the leg above: it asserts `reason=INIT_REPROBE_OK`
/// against a fixture that already carries that value, so a `last_reason`
/// hardcoded anywhere in the observe path would satisfy it with 4 passed,
/// 0 filtered and both proof lines — a green that proves nothing. Driving a
/// DISTINCT reason through the same command and asserting the observed value
/// changes with it is the only thing that separates a live field from a
/// constant. Two sides, one assertion each way.
#[test]
fn the_observed_reason_moves_with_the_row_and_is_not_a_constant() {
    let directory = tempfile::tempdir().expect("reason fixture");
    let journal = directory.path().join("reason.jsonl");

    std::fs::write(&journal, l2_row(now_secs())).expect("write canonical reason");
    let canonical = String::from_utf8_lossy(&observe(&journal).stdout).into_owned();
    assert!(canonical.contains("reason=INIT_REPROBE_OK"), "{canonical}");

    // Same layer, same freshness, DIFFERENT reason. Only the reason may move.
    let other = l2_row(now_secs()).replace("INIT_REPROBE_OK", "INIT_REPROBE_OTHER");
    assert!(
        other.contains("INIT_REPROBE_OTHER"),
        "fixture substitution must actually apply: {other}"
    );
    std::fs::write(&journal, other).expect("write distinct reason");
    let distinct = String::from_utf8_lossy(&observe(&journal).stdout).into_owned();

    assert!(distinct.contains("reason=INIT_REPROBE_OTHER"), "{distinct}");
    assert!(
        !distinct.contains("reason=INIT_REPROBE_OK"),
        "a hardcoded reason would still report the canonical value: {distinct}"
    );
    assert!(
        distinct.contains("layer=L2 state=progressing"),
        "only the reason may move, not the state: {distinct}"
    );
}

/// KNOWN-BAD (0pc9, mandatory): push the observation past its threshold and the
/// monitor reports SILENT **with an age**, not a bare false.
///
/// Both halves are pinned. `state=silent` alone would survive an age field
/// collapsing to zero, and a large `age_ms` alone would survive the state
/// staying `progressing` -- and a stale layer reported as progressing is the
/// silent pass this layer exists to prevent.
#[test]
fn l2_observe_reports_silent_with_an_age_never_a_bare_false() {
    let directory = tempfile::tempdir().expect("stale journal fixture");
    let journal = directory.path().join("stale.jsonl");
    std::fs::write(&journal, l2_row(1)).expect("write stale row");

    let threshold = l2_stall_after_ms();
    let output = observe(&journal);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    assert!(stdout.contains("layer=L2 state=silent"), "{stdout}");
    assert!(stdout.contains("fresh=false"), "{stdout}");
    let observed = age_ms(&stdout);
    assert!(observed > threshold, "age {observed} <= {threshold}: {stdout}");
    assert!(observed > 0, "silent must carry a real age, not zero: {stdout}");
}

/// ANTI-VACUITY: an empty observation set is a TYPED error with its own reason
/// code and its own exit, never a clear and never a silence.
///
/// Absence and staleness are different failures: "nobody wrote an L2 row" needs
/// a writer, "the L2 row is old" needs a run. Collapsing them into one verdict
/// is what makes an unread refusal possible, so the leg asserts the empty-scan
/// exit AND that the word `silent` never appears.
#[test]
fn empty_l2_observation_set_is_a_typed_error_not_a_clear() {
    let directory = tempfile::tempdir().expect("empty journal fixture");
    let journal = directory.path().join("empty.jsonl");
    std::fs::write(&journal, "").expect("write empty journal");

    let output = observe(&journal);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("LIFECYCLE_MONITOR_EMPTY_SCAN"), "{stderr}");
    assert!(!stderr.contains("state=silent"), "{stderr}");
    assert!(
        !stderr.contains("fresh=true"),
        "an empty scan must never read as fresh: {stderr}"
    );
}

/// A journal carrying OTHER layers but no L2 row is still an L2 absence, with
/// the same typed refusal. Without this the empty-file leg above would pass a
/// monitor that answered for whichever layer happened to be present.
#[test]
fn foreign_layer_rows_do_not_satisfy_the_l2_observation() {
    let directory = tempfile::tempdir().expect("foreign journal fixture");
    let journal = directory.path().join("foreign.jsonl");
    let foreign = l2_row(now_secs())
        .replace(r#""layer":"L2""#, r#""layer":"L5""#)
        .replace(r#""stage_to":"S1.L2""#, r#""stage_to":"S1.L5""#);
    std::fs::write(&journal, foreign).expect("write foreign row");

    let output = observe(&journal);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("LIFECYCLE_MONITOR_EMPTY_SCAN"), "{stderr}");
}
