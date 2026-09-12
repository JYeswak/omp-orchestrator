//! Deadline legs for `omp-orchestrator-62lz`.
//!
//! THE DEFECT: both spawns on this commit-path gate used a raw `.output()` with no
//! deadline, so a wedged `br` blocked a `git commit` forever with no typed outcome.
//!
//! WHICH BUG: a DEADLINE hole, not the undrained-pipe deadlock. `.output()` already
//! drains both pipes. Every assertion below therefore keys on the OUTCOME VARIANT,
//! never on an exit code -- AGENTS.md gate rule 7 records `cargo` exiting 101 for two
//! unrelated causes, so an exit-code-only leg goes green on any unrelated breakage.

use pre_delete_citation_check::{run_bounded, ChildOutcome, GIT_DIFF_DEADLINE};
use std::process::Command;
use std::time::{Duration, Instant};

/// The deadline under test. Short enough to keep the suite fast, long enough that a
/// loaded machine cannot make a healthy child miss it.
const TEST_DEADLINE: Duration = Duration::from_millis(600);

/// A child that outlives any plausible test deadline by two orders of magnitude, so
/// the leg cannot pass by the child simply finishing first.
const HUNG_CHILD_SECS: &str = "300";

/// FIRES-ON-KNOWN-BAD: a child that never returns within the deadline must produce
/// `TimedOut`, never `Completed`. Before the fix this call was `.output()` and this
/// test could not be written at all -- the call had no deadline to miss.
///
/// MUTATION (`62lz` acceptance item 4): widen `TEST_DEADLINE` past
/// `HUNG_CHILD_SECS`, or drop the bound, and this assertion goes RED.
#[test]
fn a_child_that_never_returns_is_timed_out_not_completed() {
    let mut command = Command::new("/bin/sleep");
    command.arg(HUNG_CHILD_SECS);

    let started = Instant::now();
    let outcome = run_bounded(&mut command, TEST_DEADLINE);
    let elapsed = started.elapsed();

    match outcome {
        ChildOutcome::TimedOut {
            after_ms,
            group_killed,
        } => {
            assert!(
                group_killed,
                "the deadline arm must report the process GROUP was signalled, not just the pid: \
                 an unsignalled group leaves grandchildren at ppid=1, which is the orphan class \
                 AGENTS.md's asupersync contract exists to prevent"
            );
            assert_eq!(
                after_ms,
                u64::try_from(TEST_DEADLINE.as_millis()).unwrap(),
                "the typed outcome must carry the deadline it enforced so a reader can tell \
                 which bound fired"
            );
        }
        // A `Completed` here is the pre-fix behaviour: the call waited the child out.
        other => panic!(
            "expected ChildOutcome::TimedOut for a {HUNG_CHILD_SECS}s child under a {}ms \
             deadline; got {} after {}ms -- the deadline did not fire, so the spawn is unbounded",
            TEST_DEADLINE.as_millis(),
            other.kind(),
            elapsed.as_millis()
        ),
    }

    assert!(
        elapsed < Duration::from_secs(30),
        "the call returned only after {}ms; a bound that does not bound is not a bound",
        elapsed.as_millis()
    );
}

/// KNOWN-GOOD, MANDATORY: a fast child must still complete and return its stdout
/// UNCHANGED. An attack-only suite ships an over-strict gate, and an over-strict gate
/// on the COMMIT path stops every commit in the repository.
#[test]
fn a_fast_child_still_completes_with_its_stdout_intact() {
    let mut command = Command::new("/bin/echo");
    command.arg("citation-check-known-good");

    match run_bounded(&mut command, TEST_DEADLINE) {
        ChildOutcome::Completed { code, stdout, .. } => {
            assert_eq!(code, Some(0), "a healthy /bin/echo must exit 0");
            assert_eq!(
                stdout.trim(),
                "citation-check-known-good",
                "the bounded path must not alter captured stdout -- this gate PARSES it"
            );
        }
        other => panic!(
            "expected ChildOutcome::Completed for /bin/echo; got {} -- the deadline is \
             over-strict and would refuse healthy commits",
            other.kind()
        ),
    }
}

/// A real `git` invocation on the actual gate argv must complete. This is the
/// known-good leg for the production call site rather than a synthetic child.
#[test]
fn the_real_git_diff_argv_completes_within_its_deadline() {
    let mut command = Command::new("git");
    command.args(["diff", "--cached", "--diff-filter=D", "--name-only"]);

    match run_bounded(&mut command, GIT_DIFF_DEADLINE) {
        ChildOutcome::Completed { .. } => {}
        other => panic!(
            "the production git argv did not complete within {}s; got {}",
            GIT_DIFF_DEADLINE.as_secs(),
            other.kind()
        ),
    }
}

/// A missing binary is `SpawnFailed`, NOT `TimedOut`. The two must stay distinct
/// because the remedies differ: a PATH/env problem versus a wedged subject. Collapsing
/// them re-creates the conflation that made a healthy pool read as saturated for six
/// hours.
#[test]
fn an_unspawnable_command_is_spawn_failed_not_timed_out() {
    let mut command = Command::new("/nonexistent/pre-delete-citation-check-probe");

    match run_bounded(&mut command, TEST_DEADLINE) {
        ChildOutcome::SpawnFailed { message } => {
            assert!(
                !message.trim().is_empty(),
                "a spawn failure must name its cause; an empty message is indistinguishable \
                 from a silent success"
            );
        }
        other => panic!(
            "expected ChildOutcome::SpawnFailed for an absent binary; got {}",
            other.kind()
        ),
    }
}

// REMOVED WITH ITS SUBJECT: `the_br_deadline_sits_above_the_measured_contention_band`
// asserted a band for `BR_LIST_DEADLINE`. Nothing on the commit path spawns `br` any more
// -- the closed-bead oracle is the STAGED `.beads/issues.jsonl` blob -- so the constant and
// its leg are both gone. A green assertion about a constant no caller reads is exactly the
// frozen-snapshot-as-measurement shape this repository keeps deleting.
