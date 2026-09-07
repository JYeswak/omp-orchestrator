#![forbid(unsafe_code)]

use dispatch_saga::grading::{decide_all, GradingDecision};
use dispatch_saga::{run_bounded, BR_UPDATE_DEADLINE};
use omp_types::ChildOutcome;
use std::io::{self, Read};
use std::process::{Command, ExitCode};

/// Exit code for a DEADLINE on the `br` write. Kept DISTINCT from 1 (bad input) and
/// from 2 (usage) because a timeout is not a verdict: after a group kill the bead's
/// stage is UNKNOWN, not "unchanged", and the caller must reconcile by READING the
/// bead rather than retrying blind.
const EXIT_TRANSITION_DEADLINE: u8 = 4;

/// Exit code for a `br` that RAN and REFUSED, or that could not be spawned. The
/// transition did not happen and the reason is in the captured output. Distinct from
/// 4 because the remedy differs: fix the refusal, versus reconcile an unknown.
const EXIT_TRANSITION_NOT_APPLIED: u8 = 5;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("grading-transition") {
        eprintln!("usage: dispatch-saga grading-transition [--apply] [--grader NAME]...");
        return ExitCode::from(2);
    }
    let apply = args.iter().any(|a| a == "--apply");
    let mut graders = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--grader" && i + 1 < args.len() {
            graders.push(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    if graders.is_empty() {
        graders.push("Orchestrator".into());
    }
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        eprintln!("dispatch-saga: stdin unread");
        return ExitCode::from(1);
    }
    let candidates: Vec<dispatch_saga::grading::GradingCandidate> = match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("dispatch-saga: candidate json: {e}");
            return ExitCode::from(1);
        }
    };
    let decisions = match decide_all(&candidates, &graders) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("dispatch-saga: {e:?}");
            return ExitCode::from(1);
        }
    };
    // A timeout is not a verdict, and neither is a refusal: this loop used to print
    // `-> {status:?}` and return 0 no matter what happened, so a REFUSED `br update`
    // was indistinguishable from an applied one. Each class now gets its own code.
    let mut deadline_hit = false;
    let mut not_applied = false;
    for d in &decisions {
        println!("{d:?}");
        if apply {
            if let GradingDecision::Transition { br_update, .. } = d {
                if br_update.first().map(String::as_str) == Some("br") {
                    let mut command = Command::new(&br_update[0]);
                    command.args(&br_update[1..]);
                    match run_bounded(&mut command, BR_UPDATE_DEADLINE) {
                        ChildOutcome::Completed {
                            code: Some(0),
                            stdout,
                            ..
                        } => {
                            eprintln!(
                                "dispatch-saga: APPLIED {br_update:?} -- {}",
                                stdout.trim()
                            );
                        }
                        ChildOutcome::Completed {
                            code,
                            stdout,
                            stderr,
                        } => {
                            not_applied = true;
                            eprintln!(
                                "dispatch-saga: TRANSITION_REFUSED {br_update:?} code={code:?} \
                                 stdout={} stderr={}",
                                stdout.trim(),
                                stderr.trim()
                            );
                        }
                        ChildOutcome::TimedOut {
                            after_ms,
                            group_killed,
                        } => {
                            deadline_hit = true;
                            eprintln!(
                                "dispatch-saga: TRANSITION_DEADLINE {br_update:?} after_ms={after_ms} \
                                 group_killed={group_killed} -- stage is UNKNOWN, reconcile by \
                                 reading the bead; do NOT re-apply"
                            );
                        }
                        ChildOutcome::SpawnFailed { message } => {
                            not_applied = true;
                            eprintln!(
                                "dispatch-saga: TRANSITION_UNSPAWNED {br_update:?} -- {message}"
                            );
                        }
                    }
                }
            }
        }
    }
    if deadline_hit {
        return ExitCode::from(EXIT_TRANSITION_DEADLINE);
    }
    if not_applied {
        return ExitCode::from(EXIT_TRANSITION_NOT_APPLIED);
    }
    ExitCode::from(0)
}
