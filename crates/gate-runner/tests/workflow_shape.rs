//! `omp-orchestrator-fsu7` — the workflow's SHAPE, asserted locally in Rust.
//!
//! NAMED TARGET per the mmt4 ruling: cite as
//! `cargo test -j 2 -p gate-runner --test workflow_shape`.
//!
//! # Why this leg exists, and why it cannot be a CI check
//!
//! GitHub Actions is STRICT on duplicate keys: measured n=60 runs, **49 duplicate-key runs started
//! ZERO jobs**. A workflow that fails to parse starts nothing and reports nothing — and *from
//! outside CI those are indistinguishable from a gate that passed.* Asking CI whether CI is
//! healthy cannot work.
//!
//! So the invariant is asserted HERE, in a test that runs on a laptop and on the lane: a duplicate
//! key or a re-fan-out becomes a **loud local RED** instead of a silent zero-job run. This repo has
//! the receipt for the alternative — six consecutive red CI runs went unread while a local
//! `PRE_PUSH_GATE_OK` receipt green-lit every commit.
//!
//! # BOUNDARY, per acceptance item 9
//!
//! This asserts only what `fsu7` owns: that `gate.yml` declares exactly ONE job and that the job
//! invokes `gate-runner`. **General YAML validity remains `m0c`'s**, and its four-leg strict parser
//! is not duplicated here. A second copy of someone else's gate is how two gates drift into
//! disagreeing about the same file.

use std::path::PathBuf;

fn workflow_text() -> String {
    let path = workflow_path();
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "GATE_WORKFLOW_UNREADABLE path={} detail={error} — an unreadable workflow is an \
             ERROR, never a pass: it is the state in which nothing runs and nothing reports",
            path.display()
        )
    })
}

fn workflow_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".github/workflows/gate.yml")
}

/// Job keys, counted AFTER the `jobs:` anchor.
///
/// The anchor matters and is not defensive coding: a bare two-space indentation match returns
/// **17** on this file, because `push`, `pull_request`, `workflow_dispatch` and the two
/// `concurrency` sub-keys sit at the same depth. Those are TRIGGERS, not jobs.
fn job_keys(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let anchor = lines
        .iter()
        .position(|line| line.trim_end() == "jobs:")
        .expect("GATE_WORKFLOW_NO_JOBS_ANCHOR — no `jobs:` line; nothing can run");
    lines[anchor + 1..]
        .iter()
        .filter_map(|line| indented_key(line))
        .collect()
}

/// Every two-space key in the file, anchor ignored — the WRONG method, kept so the known-bad leg
/// can prove the two differ on the same input.
fn all_two_space_keys(text: &str) -> Vec<String> {
    text.lines().filter_map(indented_key).collect()
}

fn indented_key(line: &str) -> Option<String> {
    let rest = line.strip_prefix("  ")?;
    if rest.starts_with(' ') || rest.starts_with('#') {
        return None;
    }
    let name = rest.strip_suffix(':').or_else(|| rest.split(':').next())?;
    let name = name.trim();
    if name.is_empty() || !rest.contains(':') {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return None;
    }
    Some(name.to_owned())
}

/// THE INVARIANT: exactly one job.
///
/// A subset attach is the failure R9 named — it fires and *reads as coverage*. One job with a
/// derived roster has a denominator; twelve jobs naming eleven crates did not.
#[test]
fn the_workflow_declares_exactly_one_job() {
    let text = workflow_text();
    let jobs = job_keys(&text);
    assert_eq!(
        jobs.len(),
        1,
        "gate.yml must declare exactly ONE job (the single entry point); found {}: {:?}",
        jobs.len(),
        jobs
    );
}

/// And that job must invoke `gate-runner`, or the single job is a single job that gates nothing.
///
/// `--run` specifically: `--plan` derives the roster and deliberately runs NOTHING, so a workflow
/// that only planned would report green having executed no gate at all. That is the never-fires
/// class, one level up.
#[test]
fn the_single_job_invokes_the_entry_point_in_run_mode() {
    let text = workflow_text();
    assert!(
        text.contains("cargo run --quiet -p gate-runner -- --run"),
        "the single job must invoke `gate-runner -- --run`; a workflow that only plans reports \
         green having executed no gate"
    );
    assert!(
        text.contains("cargo run --quiet -p gate-runner -- --plan"),
        "and it must plan first, so an empty or unreadable roster is named before the gates run"
    );
}

/// KNOWN-BAD FOR THE COUNTING METHOD, per the acceptance amendment.
///
/// The two methods must be shown to DISAGREE on the same file. A runner that cannot distinguish
/// them has not established which one it is using — and the bare form silently counted triggers as
/// jobs, returning 17 where the truth was 12.
#[test]
fn the_bare_indentation_method_disagrees_and_is_the_wrong_one() {
    let text = workflow_text();
    let anchored = job_keys(&text);
    let bare = all_two_space_keys(&text);
    assert!(
        bare.len() > anchored.len(),
        "the bare method must over-count on this file, or this leg proves nothing: bare={} \
         anchored={}",
        bare.len(),
        anchored.len()
    );
    for trigger in ["push", "pull_request", "workflow_dispatch"] {
        assert!(
            bare.contains(&trigger.to_owned()),
            "the bare method must pick up `{trigger}` — that is WHY it is wrong"
        );
        assert!(
            !anchored.contains(&trigger.to_owned()),
            "the anchored method must NOT count `{trigger}` as a job"
        );
    }
}

/// ANTI-VACUITY: a workflow with no `jobs:` anchor, or an empty job set, is an ERROR.
///
/// R9 exists because seven gates read `status=open` while nothing invoked them. A leg that treats
/// "no jobs found" as "no problems found" recreates that condition inside a green test.
#[test]
fn a_workflow_with_no_jobs_is_an_error_not_a_pass() {
    let empty = "name: gate\non:\n  push:\n";
    assert!(
        std::panic::catch_unwind(|| job_keys(empty)).is_err(),
        "a file with no `jobs:` anchor must PANIC rather than return an empty list that reads as \
         a clean scan"
    );

    let text = workflow_text();
    assert!(
        !job_keys(&text).is_empty(),
        "positive control: the real workflow must yield a non-empty job set, or this leg is \
         vacuous on every input"
    );
}

/// The former twelve job keys must not reappear.
///
/// If someone re-fans-out, this fails BY NAME rather than by a count, so the message says which
/// gate went back to YAML instead of declaring a stanza.
#[test]
fn no_former_per_gate_job_key_has_reappeared() {
    let text = workflow_text();
    let jobs = job_keys(&text);
    for former in [
        "no-shell-gate",
        "head-compiles-as-committed",
        "path-literal-guard",
        "grader-attribution-gate",
        "undrained-pipe-lint",
        "kernel-bypass-gate",
        "state-wildcard-lint",
        "installer",
        "omp-inventory-map",
        "pre-delete-citation-check",
        "commit-build-fence",
        "porting-gate",
    ] {
        assert!(
            !jobs.contains(&former.to_owned()),
            "job `{former}` is back in gate.yml — its check belongs in its own crate's \
             [package.metadata.gate] stanza, where the declaration travels with the crate and \
             cannot contend for this shared file"
        );
    }
}
