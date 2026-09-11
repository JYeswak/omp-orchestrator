#![forbid(unsafe_code)]

//! Specimen-based legs for the undrained-pipe lint (bead -w4j acceptance 1-3).

use undrained_pipe_lint::{find_detailed_violations_in_source, find_violations_in_source};

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

/// KNOWN-BAD: a wait_with_output call nested inside the try_wait exit arm is
/// too late to drain a child blocked while writing; it must not mask the lint.
#[test]
fn nested_wait_with_output_after_poll_is_still_flagged() {
    let source = r#"
use std::process::{Command, Stdio};
fn run() {
    let mut cmd = Command::new("git");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let _ = child.wait_with_output();
                return;
            }
            Ok(None) => {}
            Err(_) => return,
        }
    }
}
"#;
    assert_eq!(
        find_violations_in_source(source),
        vec![(5, 8)],
        "a post-poll wait_with_output cannot justify undrained pipes"
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
    let _out = child.stdout.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let _err = child.stderr.take().map(|mut r| {
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
        "both pipe readers must pass"
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
/// KNOWN-BAD REAL SHAPE: oracle-compare splits the pipe construction and
/// try_wait poll across spawn_timeout and wait_deadline. The local lint must
/// still follow that named helper call so the real defect is not missed.
#[test]
fn oracle_compare_split_helper_is_flagged_with_actionable_lines() {
    let mut lines = vec![String::new(); 243];
    lines.extend(
        [
            "fn wait_deadline(mut child: Child) {",
            "    loop {",
            "        let _status = child.id();",
            "        match child.try_wait() {",
            "            Ok(Some(_)) => return,",
            "            Ok(None) => return,",
            "            Err(_) => return,",
            "        }",
            "    }",
            "}",
            "",
            "",
            "",
            "",
            "",
            "",
            "fn spawn_timeout(mut cmd: Command) {",
            "    let mut child = cmd",
            "        .stdout(Stdio::piped())",
            "        .stderr(Stdio::piped());",
            "    wait_deadline(child);",
            "}",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    let source = lines.join("\n");
    assert_eq!(
        find_detailed_violations_in_source(&source),
        vec![(262, 263, 247)],
        "split oracle defect must name both pipes and the poll"
    );
}

/// KNOWN-GOOD in-tree contract: subprocess-contract uses its concurrent asupersync drain.
#[test]
fn subprocess_contract_is_not_flagged() {
    let source = include_str!("../../subprocess-contract/src/lib.rs");
    assert!(
        find_violations_in_source(source).is_empty(),
        "the shared subprocess contract must remain a known-good leg"
    );
}
/// MUTATION: removing the second pipe repairs this specimen. The original
/// source remains unchanged in the test, so the RED-to-GREEN attribution is
/// against the actual scanner predicate rather than a test-only flag.
#[test]
fn mutation_removing_stderr_pipe_retires_violation() {
    let source = r#"fn run() {
    let mut cmd = Command::new("git");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(_) => return,
        }
    }
}"#;
    assert_eq!(find_violations_in_source(source).len(), 1);
    let repaired = source.replace(".stderr(Stdio::piped())", ".stderr(Stdio::null())");
    assert!(
        find_violations_in_source(&repaired).is_empty(),
        "mutation removing stderr piping must make this site safe"
    );
}
// THE `wired_into_ci_workflow` TEST WAS DELETED HERE — omp-orchestrator-poumg.6.
//
// It grepped `.github/workflows/gate.yml` for the literal
// `cargo run --quiet -p undrained-pipe-lint -- .` and failed CI on its absence. The line was
// deleted deliberately by `omp-orchestrator-fsu7`, which replaced a twelve-job YAML fan-out with
// ONE `gate-runner` entry point: each gate now declares its own invocation in its OWN
// `Cargo.toml` under `[package.metadata.gate]`, so `gate.yml` no longer names this crate at all
// and by design never will again.
//
// So the test pinned an implementation detail of a superseded mechanism while the invariant it
// defended HELD — and the same CI run proved it: run 34549975939 (head cb9d3941) emitted
// `CHECK_PASS crate=undrained-pipe-lint phase=0` in the very run where this test asserted CI
// does not invoke the lint.
//
// NOT RE-POINTED AT THE gate-runner LINE, and that is the repo rule rather than a preference:
// re-pinning a wording probe at the new wording manufactures the next stale test. It would also
// be wrong on the facts, since the string to pin no longer exists in that file.
//
// DO NOT "RESTORE THE WIRING" BY ADDING A JOB TO gate.yml. That is the fan-out fsu7 removed, and
// believing this crate unwired is precisely the false conclusion the deleted test produced. The
// wiring lives in this crate's own `[package.metadata.gate]`, is executed by `gate-runner --run`,
// and is observable as a `CHECK_PASS crate=undrained-pipe-lint` row in any completed gate run.
//
// AND NO REPLACEMENT PROBE WAS ADDED HERE, DELIBERATELY: the invariant ALREADY HAS AN OWNER.
// `crates/gate-runner/tests/subsumption_real.rs:52` lists this crate with `ran_binary = true`,
// and `gate_runner::subsumption` (lib.rs:990) refuses any such crate that declares no
// `[package.metadata.gate]` checks — "its test half is covered, its run half is NOT". Measured:
// deleting the stanza from this crate's Cargo.toml turns
// `every_former_gate_job_crate_is_still_reached_through_the_entry_point` RED (exit 101), and
// restoring it byte-identically returns 4 passed / 0 failed.
//
// So a probe here would be a SECOND COPY of a gate owned elsewhere, which is how two gates drift
// into disagreeing about one fact — the boundary rule `workflow_shape.rs` states for itself.

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
    std::fs::create_dir_all(dir.join("crates/empty/src")).expect("create empty fixture workspace");
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

/// The CLI must expose the library's anti-vacuity result as its typed exit
/// status, not silently treat a workspace with no Rust files as clean.
#[test]
fn cli_empty_scan_set_exits_three() {
    let dir = std::env::temp_dir().join(format!(
        "undrained-pipe-cli-empty-{}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(dir.join("crates/empty/src")).expect("create empty fixture workspace");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_undrained-pipe-lint"))
        .arg(&dir)
        .output()
        .expect("run undrained-pipe-lint binary");
    assert_eq!(
        output.status.code(),
        Some(3),
        "empty scan set must exit 3, stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("UNDRAINED-PIPE-LINT ERROR: empty scan set"),
        "exit 3 must carry the typed empty-scan error"
    );

    std::fs::remove_dir_all(&dir).ok();
}
use std::sync::atomic::{AtomicU32, Ordering};
static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);
