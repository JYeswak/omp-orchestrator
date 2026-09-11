//! Floor 1 before S2. metric=0 must refuse.

use s2_gate::{admit, readback_ok, S2Refuse, FLOOR};
use std::fs;
use std::path::PathBuf;

#[test]
fn metric_zero_refuses() {
    match admit(0) {
        Err(S2Refuse::MetricZero) => {
            println!("S2_REFUSE metric=0 floor={FLOOR}");
        }
        other => panic!("metric=0 must refuse, got {other:?}"),
    }
}

#[test]
fn metric_one_admits() {
    admit(1).expect("floor 1 admits");
}

#[test]
fn absent_inception_is_metric_zero_and_refuses() {
    let missing = PathBuf::from("/no/such/.omp-orchestrator/inception.json");
    let metric = readback_ok(&missing);
    assert_eq!(metric, 0, "absent file is metric 0");
    assert!(matches!(admit(metric), Err(S2Refuse::MetricZero)));
}

#[test]
fn incomplete_inception_is_metric_zero() {
    let dir = std::env::temp_dir().join(format!("s2-gate-{}-{}", std::process::id(), "incomplete"));
    fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("inception.json");
    fs::write(&path, r#"{"schema_version":"1"}"#).expect("write");
    assert_eq!(readback_ok(&path), 0);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// omp-orchestrator-s1-l5-metric-readback-fqgi
//
// ACCEPTANCE: "run the S2 gate; expect refuse when metric=0".
//
// Every leg above this line calls the LIBRARY. None of them runs the gate. That
// distinction is the whole bead: a caller -- a CI step, a human, a downstream
// admission check -- observes the BINARY's exit code and its stderr token, and
// until this block existed neither was pinned by anything. Measured before this
// block landed, the acceptance-shaped command
//
//     cargo test -p s2-gate --test s2_gate gate_binary_refuses_when_metric_is_zero
//
// returned `0 passed; 4 filtered out` at exit=0 -- a silent green over a leg that
// did not exist. A refusal nobody executes is a comment.
//
// FLOOR DIRECTION: these legs only ADD constraints. `the_floor_never_admits_below_one`
// is a ratchet guard -- it reddens if anyone widens `admit` to accept anything but 1.
// ---------------------------------------------------------------------------

use std::process::Command;

/// Every key `readback_ok` requires, so a fixture cannot drift from the contract
/// by hand-copying six of seven.
fn complete_inception() -> String {
    let body = s2_gate::REQUIRED_KEYS
        .iter()
        .map(|key| format!("\"{key}\":\"present\""))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

/// A private root per leg: these tests run in parallel and share one pid.
fn root(leg: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("s2-gate-{}-{leg}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join(".omp-orchestrator")).expect("fixture root");
    dir
}

fn run_gate(root: &PathBuf) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_s2-gate"))
        .arg(root)
        .output()
        .expect("the gate binary must be runnable");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// KNOWN-BAD: metric=0 (no inception.json at all). The gate must REFUSE, and the
/// refusal must be legible -- nonzero exit AND the token, not one or the other.
/// An exit code with no token is unattributable; a token with exit 0 is a lie.
#[test]
fn gate_binary_refuses_when_metric_is_zero() {
    let dir = root("metric-zero");
    assert_eq!(
        readback_ok(&dir.join(".omp-orchestrator/inception.json")),
        0,
        "fixture must actually be metric 0"
    );
    let (code, stdout, stderr) = run_gate(&dir);
    assert_eq!(code, Some(1), "refusal must be nonzero; stderr={stderr}");
    assert!(
        stderr.contains("S2_REFUSE L5-METRIC-READBACK-OK=0"),
        "refusal must name the metric it refused on: {stderr}"
    );
    assert!(
        stderr.contains(&format!("floor={FLOOR}")),
        "refusal must name the floor: {stderr}"
    );
    assert!(
        !stdout.contains("S2_OK"),
        "a refusing gate must not also emit admission: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// KNOWN-BAD: present-but-partial is NOT present. Six of seven keys is metric 0.
/// Missing must never collapse into healthy.
#[test]
fn gate_binary_refuses_an_incomplete_inception() {
    let dir = root("metric-partial");
    let partial = s2_gate::REQUIRED_KEYS
        .iter()
        .take(s2_gate::REQUIRED_KEYS.len() - 1)
        .map(|key| format!("\"{key}\":\"present\""))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        dir.join(".omp-orchestrator/inception.json"),
        format!("{{{partial}}}"),
    )
    .expect("write partial");
    let (code, _stdout, stderr) = run_gate(&dir);
    assert_eq!(code, Some(1), "6 of 7 keys must refuse; stderr={stderr}");
    assert!(stderr.contains("S2_REFUSE L5-METRIC-READBACK-OK=0"), "{stderr}");
    let _ = fs::remove_dir_all(&dir);
}

/// KNOWN-GOOD: the gate is not merely always-refusing. metric=1 admits, quietly,
/// exit 0. Without this leg a gate hard-wired to refuse would pass every leg above.
#[test]
fn gate_binary_admits_when_metric_is_one() {
    let dir = root("metric-one");
    fs::write(
        dir.join(".omp-orchestrator/inception.json"),
        complete_inception(),
    )
    .expect("write complete");
    assert_eq!(
        readback_ok(&dir.join(".omp-orchestrator/inception.json")),
        1,
        "fixture must actually be metric 1"
    );
    let (code, stdout, stderr) = run_gate(&dir);
    assert_eq!(code, Some(0), "floor 1 admits; stderr={stderr}");
    assert!(stdout.contains("S2_OK L5-METRIC-READBACK-OK=1"), "{stdout}");
    assert!(!stderr.contains("S2_REFUSE"), "{stderr}");
    let _ = fs::remove_dir_all(&dir);
}

/// RATCHET GUARD: the floor is 1 and `admit` accepts exactly one value. Exhaustive
/// over the whole domain, so widening the predicate in any direction reddens here.
#[test]
fn the_floor_never_admits_below_one() {
    assert_eq!(FLOOR, 1, "the floor may be raised, never lowered");
    let admitted: Vec<u8> = (u8::MIN..=u8::MAX).filter(|m| admit(*m).is_ok()).collect();
    assert_eq!(
        admitted,
        vec![1],
        "exactly one metric value may admit S2, and it must be the floor"
    );
    assert!(matches!(admit(0), Err(S2Refuse::MetricZero)));
}
