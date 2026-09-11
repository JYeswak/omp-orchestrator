//! End-to-end tests for the `dispatch-claim-fence` operator surface.
//!
//! These run the real binary with real argv over real files, because the defect
//! this bin exists to fix is precisely that a linked caller could reach the
//! fence and a shell could not. A test that called `authorize` directly would
//! re-prove the library and leave the operator lane unmeasured.

use std::io::Write;
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_dispatch-claim-fence");

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("binary is runnable")
}

fn snapshot_file(name: &str, body: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dcf-{}-{}-{name}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos()
    ));
    std::fs::write(&path, body).expect("snapshot file is writable");
    path
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("process exited normally")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn claimed_bead_is_permitted_from_a_file_snapshot() {
    let path = snapshot_file(
        "ok",
        r#"{"id":"omp-1","title":"t","description":"d","status":"in_progress","assignee":"Worker"}"#,
    );
    let output = run(&[
        "authorize",
        "--bead",
        "omp-1",
        "--receiver",
        "Worker",
        "--snapshot",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        "DISPATCH_PERMIT kind=bead bead=omp-1 receiver=Worker"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn open_unassigned_bead_refuses_with_claim_required_code_ten() {
    let path = snapshot_file(
        "open",
        r#"{"id":"omp-2","title":"t","description":"d","status":"open","assignee":null}"#,
    );
    let output = run(&[
        "authorize",
        "--bead",
        "omp-2",
        "--receiver",
        "Worker",
        "--snapshot",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(code(&output), 10, "stderr: {}", stderr(&output));
    let text = stderr(&output);
    assert!(text.contains("code=CLAIM_REQUIRED"), "stderr: {text}");
    assert!(
        text.contains("br update omp-2 --assignee Worker --status in_progress"),
        "refusal must carry the remedy command: {text}"
    );
    let _ = std::fs::remove_file(&path);
}

/// The refusal line ALWAYS names `inspect_dispatch_ledger` inside its embedded
/// command, so a substring match against whole stderr passes no matter which
/// remedy was selected. Measured 2026-09-11: a mutation flipping the default
/// evidence to `Present` left a whole-stderr assertion green. The advice line
/// is the only place the SELECTED remedy appears, so the assertion reads it.
fn advice_line(output: &Output) -> String {
    let text = stderr(output);
    text.lines()
        .find(|line| line.starts_with("CLAIM_REQUIRED_ACTIONABLE"))
        .unwrap_or_else(|| panic!("no advice line in stderr: {text}"))
        .to_owned()
}

#[test]
fn ledger_evidence_selects_a_single_mutually_exclusive_remedy() {
    let body = r#"{"id":"omp-3","title":"t","description":"d","status":"open","assignee":"Other"}"#;
    let path = snapshot_file("ledger", body);
    let file = path.to_str().expect("utf-8 path");

    let present = run(&[
        "authorize", "--bead", "omp-3", "--receiver", "Worker", "--snapshot", file, "--ledger",
        "present",
    ]);
    assert_eq!(code(&present), 10, "stderr: {}", stderr(&present));
    let line = advice_line(&present);
    assert!(line.contains("next_action=complete_claim"), "{line}");
    assert!(line.contains("dispatch_ledger=PRESENT"), "{line}");

    let absent = run(&[
        "authorize", "--bead", "omp-3", "--receiver", "Worker", "--snapshot", file, "--ledger",
        "absent",
    ]);
    let line = advice_line(&absent);
    assert!(line.contains("next_action=release_fully"), "{line}");
    assert!(line.contains("dispatch_ledger=ABSENT"), "{line}");

    let unset = run(&[
        "authorize", "--bead", "omp-3", "--receiver", "Worker", "--snapshot", file,
    ]);
    let line = advice_line(&unset);
    assert!(
        line.contains("next_action=inspect_dispatch_ledger")
            && line.contains("dispatch_ledger=UNAVAILABLE"),
        "unsupplied evidence must not be guessed: {line}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn bead_claimed_in_progress_by_another_agent_exits_eleven() {
    let path = snapshot_file(
        "elsewhere",
        r#"{"id":"omp-4","title":"t","description":"d","status":"in_progress","assignee":"Other"}"#,
    );
    let output = run(&[
        "authorize",
        "--bead",
        "omp-4",
        "--receiver",
        "Worker",
        "--snapshot",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(code(&output), 11, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("code=ASSIGNED_ELSEWHERE"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn tracker_answering_about_a_different_bead_exits_twelve() {
    let path = snapshot_file(
        "mismatch",
        r#"{"id":"omp-OTHER","title":"t","description":"d","status":"in_progress","assignee":"Worker"}"#,
    );
    let output = run(&[
        "authorize",
        "--bead",
        "omp-5",
        "--receiver",
        "Worker",
        "--snapshot",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(code(&output), 12, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("code=SNAPSHOT_ID_MISMATCH"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_bead_dispatch_without_a_snapshot_is_refused_not_permitted() {
    let output = run(&["authorize", "--bead", "omp-6", "--receiver", "Worker"]);
    assert_eq!(code(&output), 14, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("code=MISSING_SNAPSHOT"));
}

#[test]
fn empty_receiver_is_refused_with_its_own_code() {
    let output = run(&["authorize", "--bead", "omp-7", "--receiver", "   "]);
    assert_eq!(code(&output), 15, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("code=MISSING_RECEIVER_AGENT"));
}

#[test]
fn malformed_snapshot_json_is_a_snapshot_failure_not_a_permit() {
    let path = snapshot_file("bad", "{not json");
    let output = run(&[
        "authorize",
        "--bead",
        "omp-8",
        "--receiver",
        "Worker",
        "--snapshot",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(code(&output), 3, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("SNAPSHOT_UNPARSEABLE"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_unreadable_snapshot_path_never_degrades_into_a_permit() {
    let output = run(&[
        "authorize",
        "--bead",
        "omp-9",
        "--receiver",
        "Worker",
        "--snapshot",
        "/nonexistent/dcf-absent.json",
    ]);
    assert_eq!(code(&output), 3, "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("SNAPSHOT_UNREADABLE"));
}

#[test]
fn a_br_show_envelope_and_a_row_array_parse_the_same_as_a_bare_row() {
    for (name, body) in [
        (
            "array",
            r#"[{"id":"omp-10","title":"t","description":"d","status":"in_progress","assignee":"Worker"}]"#,
        ),
        (
            "envelope",
            r#"{"issues":[{"id":"omp-10","title":"t","description":"d","status":"in_progress","assignee":"Worker"}]}"#,
        ),
    ] {
        let path = snapshot_file(name, body);
        let output = run(&[
            "authorize",
            "--bead",
            "omp-10",
            "--receiver",
            "Worker",
            "--snapshot",
            path.to_str().expect("utf-8 path"),
        ]);
        assert_eq!(code(&output), 0, "{name} stderr: {}", stderr(&output));
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn a_snapshot_can_be_piped_on_stdin() {
    let mut child = Command::new(BIN)
        .args([
            "authorize",
            "--bead",
            "omp-11",
            "--receiver",
            "Worker",
            "--snapshot",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary is spawnable");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(
            br#"{"id":"omp-11","title":"t","description":"d","status":"in_progress","assignee":"Worker"}"#,
        )
        .expect("stdin accepts the snapshot");
    let output = child.wait_with_output().expect("child terminates");
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("DISPATCH_PERMIT kind=bead bead=omp-11"));
}

#[test]
fn a_named_broadcast_needs_no_snapshot_but_still_needs_a_receiver() {
    let permitted = run(&[
        "authorize",
        "--operation",
        "tick-sweep",
        "--kind",
        "broadcast",
        "--receiver",
        "all",
    ]);
    assert_eq!(code(&permitted), 0, "stderr: {}", stderr(&permitted));
    assert!(
        stdout(&permitted).contains("kind=broadcast operation=tick-sweep"),
        "stdout: {}",
        stdout(&permitted)
    );

    let unnamed = run(&[
        "authorize",
        "--operation",
        "  ",
        "--kind",
        "correction",
        "--receiver",
        "Worker",
    ]);
    assert_eq!(code(&unnamed), 15, "stderr: {}", stderr(&unnamed));
    assert!(stderr(&unnamed).contains("code=MISSING_OPERATION"));
}

#[test]
fn usage_errors_are_distinct_from_fence_refusals() {
    for args in [
        vec!["authorize"],
        vec!["authorize", "--bead", "b", "--operation", "o"],
        vec!["authorize", "--receiver"],
        vec!["authorize", "--bead", "b", "--receiver", "r", "--nope", "x"],
        vec!["dispatch"],
    ] {
        let output = run(&args);
        assert_eq!(code(&output), 2, "args {args:?} stderr: {}", stderr(&output));
    }
}

#[test]
fn an_unparseable_ledger_value_is_rejected_rather_than_silently_ignored() {
    let path = snapshot_file(
        "ledgerbad",
        r#"{"id":"omp-12","title":"t","description":"d","status":"open","assignee":"Other"}"#,
    );
    let output = run(&[
        "authorize",
        "--bead",
        "omp-12",
        "--receiver",
        "Worker",
        "--snapshot",
        path.to_str().expect("utf-8 path"),
        "--ledger",
        "maybe",
    ]);
    assert_eq!(code(&output), 2, "stderr: {}", stderr(&output));
    let _ = std::fs::remove_file(&path);
}
