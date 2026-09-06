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
