#![forbid(unsafe_code)]

//! 8nuh: the supervisor and this binary must agree on the ARGUMENT LIST.
//!
//! The defect this target exists to catch was latent for three days and cost
//! 447 recorded refusals: `finished_pane_reaper_args` built a list, the reaper
//! rejected one of its flags, and the supervisor aborted at
//! `SUPERVISOR_REFUSED reap-finished-panes exited=2` BEFORE `decide()` was ever
//! reached. Both halves were well-formed in isolation. Nothing ran the pair.
//!
//! So these legs SPAWN the real binary with the exact bytes the supervisor
//! sends, obtained from the supervisor's own public seam rather than retyped
//! here. A leg that constructs the args and never executes anything cannot
//! catch this class, which is why the pre-existing arg tests did not.
//!
//! # The discriminator, and why exit codes are useless here
//!
//! MEASURED 2026-09-11 against the installed reaper:
//!   `--repo <path> --session missing`  -> exit 2, "empty pane set - refusing a
//!                                        vacuous reap sweep"   (SEMANTIC)
//!   `--definitely-not-a-flag`          -> exit 2, "usage error: unknown
//!                                        argument ..."          (USAGE)
//! BOTH EXIT 2. An exit-code check cannot tell "my arguments were rejected"
//! from "the callee ran and refused for a real reason" -- and reading `exited=2`
//! as the former is precisely how 8nuh's premise was formed. The stderr CLASS
//! is the only discriminator, so that is what these legs assert.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The usage-error prefix the reaper emits for an argument it does not accept.
const USAGE_REJECTION: &str = "usage error: unknown argument";

fn reaper_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_reap-finished-panes"))
}

/// ANTI-VACUITY (acceptance item 6): a leg that passes because the binary was
/// never found is not a pass. Proven by EXECUTING it, not by a path check --
/// a path can exist and be unexecutable.
fn run(args: &[String]) -> (Option<i32>, String) {
    let binary = reaper_binary();
    assert!(
        binary.is_file(),
        "ANTI-VACUITY: reaper binary absent at {}; this target proves nothing without it",
        binary.display()
    );
    let output = Command::new(&binary)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("ANTI-VACUITY: reaper did not execute: {error}"));
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    (output.status.code(), text)
}

/// The supervisor's list is ACCEPTED by the binary it is sent to.
///
/// Asserts the stderr CLASS, never the exit code: a scoped session with no
/// finished panes legitimately refuses, and that refusal is a healthy callee,
/// not an argument mismatch.
#[test]
fn supervisor_argument_list_is_accepted_by_the_reaper() {
    let repo = tempdir();
    let args = omp_orchestrator::resident::reaper_args_for(
        repo.as_path(),
        "caller-arg-parity-absent-session",
    );
    // Pin the shape too: a caller that silently stopped sending --session would
    // widen the sweep to every session, which is the capture ma3b closed.
    assert!(
        args.contains(&"--repo".to_owned()) && args.contains(&"--session".to_owned()),
        "supervisor must scope the sweep by repo AND session: {args:?}"
    );

    let (code, text) = run(&args);
    assert!(
        !text.contains(USAGE_REJECTION),
        "the reaper REJECTED the supervisor's own argument list: code={code:?} output={text}"
    );
    assert!(
        code.is_some(),
        "reaper was killed by a signal rather than exiting: {text}"
    );
}

/// FIRES-ON-KNOWN-BAD (acceptance item 4): the same list plus one flag the
/// reaper does not accept MUST be rejected as a usage error. Without this, the
/// leg above would pass against a binary that accepts everything.
#[test]
fn an_unknown_argument_is_still_a_usage_rejection() {
    let repo = tempdir();
    let mut args = omp_orchestrator::resident::reaper_args_for(
        repo.as_path(),
        "caller-arg-parity-absent-session",
    );
    args.push("--definitely-not-a-flag".to_owned());

    let (code, text) = run(&args);
    assert!(
        text.contains(USAGE_REJECTION),
        "an unaccepted flag must refuse as a usage error, else the acceptance leg is vacuous: \
         code={code:?} output={text}"
    );
}

/// A unique temp directory without pulling in a dev-dependency for it.
fn tempdir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "reap-caller-arg-parity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&path).expect("temp fixture dir");
    path
}
