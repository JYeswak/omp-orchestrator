#![forbid(unsafe_code)]

//! Specimen-based legs for the undrained-pipe lint (bead -w4j acceptance 1-3).

use undrained_pipe_lint::find_violations_in_source;

/// KNOWN-BAD: both pipes piped + try_wait poll + no drain -> RED.
#[test]
fn known_bad_both_pipes_try_wait_poll_is_flagged() {
    let source = "\
use std::process::{Command, Stdio};
fn run() -> Option<String> {
    let mut cmd = Command::new(\"git\");
    cmd.arg(\"log\")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Some(\"done\".to_owned()),
            Ok(None) if started.elapsed().as_secs() < 30 => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}
";
    let hits = find_violations_in_source(source);
    assert_eq!(hits.len(), 1, "known-bad must be flagged: {hits:?}");
}

/// KNOWN-GOOD: stdout-only piping is not the defect (one pipe cannot deadlock).
#[test]
fn known_good_stdout_only_passes() {
    let source = "\
use std::process::{Command, Stdio};
fn run() -> Option<String> {
    let mut cmd = Command::new(\"git\");
    cmd.arg(\"log\")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Some(\"done\".to_owned()),
            Ok(None) if started.elapsed().as_secs() < 30 => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}
";
    assert!(
        find_violations_in_source(source).is_empty(),
        "stdout-only must pass"
    );
}

/// KNOWN-GOOD: wait_with_output without try_wait passes (std drains both concurrently).
#[test]
fn known_good_wait_with_output_passes() {
    let source = "\
use std::process::{Command, Stdio};
fn run() -> Option<String> {
    let out = Command::new(\"git\")
        .arg(\"log\")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}
";
    assert!(
        find_violations_in_source(source).is_empty(),
        "wait_with_output must pass"
    );
}

/// KNOWN-GOOD: concurrent thread drain passes (the known-good specimen pattern).
#[test]
fn known_good_thread_drain_passes() {
    let source = "\
use std::process::{Command, Stdio};
use std::io::Read;
fn run() -> Option<String> {
    let mut cmd = Command::new(\"git\");
    cmd.arg(\"log\")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let out = child.stdout.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Some(\"done\".to_owned()),
            Ok(None) if started.elapsed().as_secs() < 30 => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}
";
    assert!(
        find_violations_in_source(source).is_empty(),
        "thread drain must pass"
    );
}

/// COMMENT-STRIPPING: the hazard documentation comment must not trigger the lint.
#[test]
fn hazard_documentation_comment_does_not_trigger() {
    let source = "\
use std::process::{Command, Stdio};
// DRAIN THE PIPES ON DEDICATED THREADS.  try_wait in a poll loop CANNOT be paired with
// undrained pipes: a child that writes more than the OS pipe buffer (~64 KiB) blocks in
// write forever, so it never exits, so try_wait never returns Some.
fn run() -> Option<String> {
    let out = Command::new(\"git\")
        .arg(\"log\")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}
";
    assert!(
        find_violations_in_source(source).is_empty(),
        "hazard documentation comment must not trigger"
    );
}

/// WIRED, not merely built (bead -w4j clause 7): the lint must be invoked by a
/// production surface. This leg asserts the CI workflow references the crate —
/// a positive control: if the step is removed, this test goes RED.
#[test]
fn wired_into_ci_workflow() {
    let workflow = match std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/gate.yml")) {
        Ok(text) => text,
        Err(_) => panic!("gate.yml not found — the wiring probe cannot run without the CI surface"),
    };
    assert!(
        workflow.contains("undrained-pipe-lint"),
        "clause 7: the CI workflow must invoke the lint — BUILT != WIRED"
    );
}

/// ANTI-VACUITY positive control (clause 6): an empty scan set must be a TYPED
/// error (exit 3), never a phantom violation (exit 1).
#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    // The bin's exit-3 leg is exercised via the CLI test; here we assert the
    // library contract: lint_workspace on a dir with no crates/ yields an
    // empty scan set, which the caller must treat as an error.
    let dir = std::env::temp_dir().join(format!(
        "undrained-pipe-empty-{}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("create empty fixture dir");
    let report = undrained_pipe_lint::lint_workspace(&dir);
    assert!(
        report.scanned.is_empty(),
        "empty scan set must be empty, got: {:?}",
        report.scanned
    );
    assert!(
        report.violations.is_empty(),
        "empty scan set must produce zero violations, got: {:?}",
        report.violations
    );
    std::fs::remove_dir_all(&dir).ok();
}

use std::sync::atomic::{AtomicU32, Ordering};
static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);
