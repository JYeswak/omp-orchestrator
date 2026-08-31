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
