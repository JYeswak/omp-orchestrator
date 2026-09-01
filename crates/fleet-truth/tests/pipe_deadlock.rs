//! PLANTED-KNOWN-BAD FIXTURES for the spawn_timeout pipe deadlock.
//!
//! MEASURED 2026-08-27, and it was the true cause of the fleet loop's multi-hour dispatch stall.
//!
//! `spawn_timeout` piped stdout AND stderr, then polled `try_wait()` in a loop WITHOUT ever
//! reading them. A child that writes more than the OS pipe buffer (~64 KiB, and each stream has
//! its own) blocks forever inside `write`. So the child never exits, `try_wait` never returns
//! Some, and the call burns its ENTIRE timeout at 0% CPU before killing a child that was ready to
//! finish in under a second.
//!
//! The measurement that named it: `git -C <repo> log --since "24 hours ago" --oneline` completed
//! in 0.6-0.9s from a shell and sat at 0.0% CPU for 104s as a child of fleet-truth. Reproduced
//! deterministically by polling `try_wait` on a piped child without draining it.
//!
//! Effect of the fix, same box, same load: fleet-truth 400s+ (TIMEOUT) -> 14.5s rc=0 with all 7
//! rows; fleet-reconcile 92.6s -> 38.5s.
//!
//! THE FOURTH INSTANCE: six crates shared this exact shape -- admission-reason,
//! dispatcher-deadman, fleet-reconcile, fleet-truth, pane-dispatch-ready, reap-finished-panes.
//! Fixing only the one that fired would have left five live.

use std::process::Command;
use std::time::{Duration, Instant};

use fleet_truth::spawn_timeout;

/// THE PLANTED KNOWN-BAD: a child that writes far more than one pipe buffer.  Under the old
/// implementation this blocked until the timeout expired and returned a KILLED child's partial
/// output.  It must now complete promptly with the full payload intact.
#[test]
fn a_child_that_outwrites_the_pipe_buffer_still_completes() {
    // 400 KiB on stdout -- comfortably past the ~64 KiB buffer that caused the deadlock.
    let mut cmd = Command::new("/bin/sh");
    cmd.args([
        "-c",
        "awk 'BEGIN{for(i=0;i<8000;i++) printf \"%051d\\n\", i}'",
    ]);

    let start = Instant::now();
    let out = spawn_timeout(cmd, Duration::from_secs(60)).expect("child must run");
    let elapsed = start.elapsed();

    assert!(
        out.status.success(),
        "the child must exit normally, not be killed at the bound"
    );
    assert!(
        out.stdout.len() > 400_000,
        "the FULL payload must survive the drain; got {} bytes",
        out.stdout.len()
    );
    assert!(
        elapsed < Duration::from_secs(20),
        "a prompt child must not burn its timeout: took {elapsed:?} -- this is the deadlock"
    );
}

/// BOTH pipes must be drained.  stderr has its own buffer, so a child that is quiet on stdout can
/// still deadlock on stderr -- which is exactly how a `git` invocation stalls while appearing to
/// produce little output.
#[test]
fn a_child_that_outwrites_the_buffer_on_stderr_still_completes() {
    let mut cmd = Command::new("/bin/sh");
    cmd.args([
        "-c",
        "awk 'BEGIN{for(i=0;i<8000;i++) printf \"%051d\\n\", i}' 1>&2",
    ]);

    let start = Instant::now();
    let out = spawn_timeout(cmd, Duration::from_secs(60)).expect("child must run");

    assert!(out.status.success(), "child must exit normally");
    assert!(
        out.stderr.len() > 400_000,
        "stderr must be drained too; got {} bytes",
        out.stderr.len()
    );
    assert!(
        start.elapsed() < Duration::from_secs(20),
        "draining stderr must not burn the timeout either"
    );
}

/// The negative control for the timeout itself.  Without this leg, an implementation that never
/// timed out at all would pass every test above -- and a hung child would wedge the loop forever,
/// which is a worse failure than the one being fixed.
#[test]
fn a_genuinely_hung_child_is_still_killed_at_the_bound() {
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", "sleep 60"]);

    let start = Instant::now();
    let out = spawn_timeout(cmd, Duration::from_millis(500)).expect("must return");
    let elapsed = start.elapsed();

    assert!(
        !out.status.success(),
        "a killed child must not report success"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "the bound must still be enforced: took {elapsed:?}"
    );
}

/// Ordinary small output must be exact -- the drain must not corrupt or truncate the common case.
#[test]
fn small_output_is_returned_verbatim() {
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", "printf 'hello'"]);
    let out = spawn_timeout(cmd, Duration::from_secs(30)).expect("must run");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hello");
    assert!(out.status.success());
}
