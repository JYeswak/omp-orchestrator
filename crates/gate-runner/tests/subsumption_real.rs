//! `omp-orchestrator-fsu7` — the MOVED invariant, asserted over the REAL workspace.
//!
//! NAMED TARGET per the mmt4 ruling: cite as
//! `cargo test -j 2 -p gate-runner --test subsumption_real`.
//!
//! # What moved, and from where
//!
//! `no-shell-gate/tests/config_parses.rs` carried
//! `state_wildcard_lint_has_its_own_job_with_one_runs_on_and_one_steps`, which asserted a TWELVE-JOB
//! structure: that `\n  state-wildcard-lint:\n` appears at job indent, and that the file declares
//! at least ten jobs. Those assertions were the restore-proof from the 2026-09-06 incident, when a
//! MISSING job key collapsed two jobs into one and silently halved coverage.
//!
//! That incident's real requirement was never "this YAML has a job named X". It was **"X's gate is
//! reached"**. Under a twelve-job workflow those were the same sentence; under one entry point they
//! are not, and only the second survives translation.
//!
//! So the invariant moves here: **every crate the twelve jobs invoked must still be reachable
//! through the entry point** — its tests via the derived roster, and its binary via a
//! `[package.metadata.gate]` stanza in its OWN manifest.
//!
//! # Why this file exists BEFORE the old leg is deleted
//!
//! `%6`'s condition 1: the invariant must be PROVEN to have moved, not asserted. **A deleted leg
//! whose replacement is untested is a coverage hole with a commit message.** That is item 10's rule
//! — nothing deleted without showing what subsumes it — applied to a test instead of a job.
//!
//! # And `m0c`'s headline was already false
//!
//! Its title claims `gate.yml` is invalid YAML with duplicate keys at `:46/:52` so nine jobs are
//! unreachable. Refuted and recorded in `AGENTS.md`: the file parses clean under a strict loader,
//! and `:52` is a well-formed `kernel-bypass-gate:` job. The false report came from
//! `yaml.safe_load`, which **silently accepts duplicate keys and takes the last** — so a bare
//! `safe_load` cannot disprove a duplicate-key claim. The legs are worth keeping as restore-proofs;
//! the bead's stated cause is not.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The crates the twelve `gate.yml` jobs invoked, and whether each job ran a BINARY as well as
/// `cargo test`.
///
/// A historical list on purpose: these are the jobs that existed at the moment of the reduction, so
/// the set is closed. It is not a roster — the roster is derived — it is the **evidence that the
/// reduction lost nothing**, which is inherently a statement about a past shape.
const FORMER_JOB_CRATES: &[(&str, bool)] = &[
    ("no-shell-gate", true),
    ("asupersync-conformance", true),
    ("convergence-stamp", true),
    ("path-literal-guard", false),
    ("grader-attribution-gate", true),
    ("undrained-pipe-lint", true),
    ("kernel-bypass-gate", true),
    ("state-wildcard-lint", true),
    ("installer", true),
    ("omp-inventory-map", false),
    ("pre-delete-citation-check", true),
    ("commit-build-fence", true),
    ("porting-gate", true),
    ("preregistration-gate", true),
    ("plan-assemble", true),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

fn manifest(root: &Path, crate_name: &str) -> String {
    let path = root.join("crates").join(crate_name).join("Cargo.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "GATE_SUBSUMPTION_UNREADABLE crate={crate_name} path={} detail={error} — a former \
             gate.yml job names a crate whose manifest cannot be read; that is an ERROR, never a \
             pass",
            path.display()
        )
    })
}

/// ⛔ THE PARSER, NOT A SUBSTRING. `omp-orchestrator-fence-ci-non-verdict-16l`, ruling 2.
///
/// This used to be `manifest(root, crate_name).contains("[package.metadata.gate]")` — a substring
/// match on the stanza HEADER. Two readers of "does this crate declare a check" then disagreed BY
/// CONSTRUCTION, and THE WEAKER ONE WAS THE ONE IN THE GATE: the header is present for
/// `commit-build-fence`, whose phases are `init` (setup) then `check` ON THE SAME BIN, so
/// `plan_check_phase` skips the setup and then skips the check as setup-dependent. Nothing of that
/// crate executes on a check pass — INCLUDING ON A GENUINELY FENCED TREE — and the substring could
/// not see it.
///
/// DECLARED IS NOT EXECUTABLE. This asks the same question the runner asks, through the same
/// parser and the same plan, so the gate and the runner cannot drift.
fn declares_executable_check(crate_name: &str) -> bool {
    let metadata = workspace_metadata();
    let checks = gate_runner::derive_checks(metadata).expect("cargo metadata parses");
    checks
        .iter()
        .filter(|c| c.crate_name == crate_name)
        .any(|c| {
            (0..c.phases.len()).any(|index| {
                gate_runner::plan_check_phase(&c.phases, index) == gate_runner::CheckRunPlan::Execute
            })
        })
}

/// `cargo metadata` for THIS workspace, read once. Not a build: `--no-deps --offline`.
static WORKSPACE_METADATA: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = std::process::Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
        .current_dir(repo_root())
        .output()
        .expect("cargo metadata must be spawnable");
    assert!(
        output.status.success(),
        "ANTI-VACUITY: cargo metadata failed, so an empty check set would read as 'nothing \
         declares a check' — a refusal, never a pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("metadata is utf8")
});

fn workspace_metadata() -> &'static str {
    &WORKSPACE_METADATA
}

fn has_any_test(root: &Path, crate_name: &str) -> bool {
    let dir = root.join("crates").join(crate_name);
    if dir.join("tests").is_dir()
        && std::fs::read_dir(dir.join("tests"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "rs"))
    {
        return true;
    }
    // Unit tests inside the lib are the half `cargo metadata` target kinds cannot see. Measured:
    // 12 of 87 packages have ZERO integration targets and ALL TWELVE have unit tests.
    fn scan(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    scan(&path)
                } else {
                    path.extension().is_some_and(|x| x == "rs")
                        && std::fs::read_to_string(&path)
                            .is_ok_and(|text| text.contains("#[test]"))
                }
            })
    }
    scan(&dir.join("src"))
}

/// THE MOVED INVARIANT. Every former job's crate is still reached through the entry point.
///
/// This is the assertion that replaces `state_wildcard_lint_has_its_own_job_...`. It is strictly
/// stronger in one direction — it covers all fifteen invoked crates rather than one — and strictly
/// weaker in another: it says nothing about YAML, which is deliberate, because YAML validity stays
/// `m0c`'s and a second validator is what item 9 forbids.
#[test]
fn every_former_gate_job_crate_is_still_reached_through_the_entry_point() {
    let root = repo_root();
    let mut unreached = Vec::new();
    for (crate_name, ran_binary) in FORMER_JOB_CRATES {
        let tests = has_any_test(&root, crate_name);
        let checks = declares_executable_check(crate_name);
        if !tests {
            unreached.push(format!("{crate_name}: no test invocation in the derived roster"));
        }
        // ⛔ TWO CAUSES, TWO SENTENCES. "No stanza at all" and "a stanza whose every phase is
        // setup or setup-dependent" are DIFFERENT FACTS WITH DIFFERENT REMEDIES — declare a
        // check, versus make the declared check executable. Reporting both as "declares no
        // stanza" is the residual-label defect, and it would have been actively FALSE here:
        // commit-build-fence's stanza is present and correct, and still nothing runs.
        if *ran_binary && !checks {
            let declared = manifest(&root, crate_name).contains("[package.metadata.gate]");
            unreached.push(if declared {
                format!(
                    "{crate_name}: ran its BINARY in gate.yml and its [package.metadata.gate] \
                     stanza declares NO EXECUTABLE phase — every phase is setup or \
                     setup-dependent, so the check is SKIPPED on a check pass, INCLUDING ON A \
                     GENUINELY FAILING TREE. Its run half is UNREACHED. The remedy is to make the \
                     check executable, NOT to declare one."
                )
            } else {
                format!(
                    "{crate_name}: ran its BINARY in gate.yml and declares no \
                     [package.metadata.gate] stanza — its run half is UNREACHED"
                )
            });
        }
    }
    assert!(
        unreached.is_empty(),
        "the reduction would LOSE these gates; each must be reachable before gate.yml drops its \
         job:\n  {}",
        unreached.join("\n  ")
    );
}

/// The specific crate whose job went missing in the 2026-09-06 incident, called out BY NAME.
///
/// The old leg named it because it was the one that broke. Keeping it named here means a grep for
/// `state-wildcard-lint` still lands on a live assertion after the job is gone — otherwise the
/// incident's own restore-proof becomes unfindable.
#[test]
fn state_wildcard_lint_is_reached_by_declaration_not_by_a_yaml_job() {
    let root = repo_root();
    assert!(
        has_any_test(&root, "state-wildcard-lint"),
        "state-wildcard-lint must contribute tests to the derived roster"
    );
    assert!(
        declares_executable_check("state-wildcard-lint"),
        "state-wildcard-lint's BINARY check must be declared in its own \
         crates/state-wildcard-lint/Cargo.toml under [package.metadata.gate]. This is the 2026-09-06 \
         invariant after translation: the incident's requirement was that its gate is REACHED, not \
         that a YAML key exists."
    );
}

/// ANTI-VACUITY: the former-job list must be non-empty and every name must resolve to a real crate.
///
/// A list that silently shrank, or that named a deleted crate, would make the two legs above pass
/// while checking nothing — the never-fires class reappearing inside the proof that nothing
/// never-fires.
#[test]
fn the_former_job_list_is_non_empty_and_every_crate_exists() {
    let root = repo_root();
    assert!(
        FORMER_JOB_CRATES.len() >= 15,
        "the measured fan-out was 15 crate invocations across 12 jobs; a shorter list means \
         coverage was dropped from the proof itself, found {}",
        FORMER_JOB_CRATES.len()
    );
    let names: BTreeSet<&str> = FORMER_JOB_CRATES.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names.len(),
        FORMER_JOB_CRATES.len(),
        "duplicate entries would inflate the denominator without adding coverage"
    );
    for (crate_name, _) in FORMER_JOB_CRATES {
        assert!(
            root.join("crates").join(crate_name).join("Cargo.toml").is_file(),
            "former job crate {crate_name} has no manifest — the list is stale and the legs above \
             are checking a crate that no longer exists"
        );
    }
}

/// POSITIVE CONTROL for the detector itself: a crate that declares no stanza must be seen as
/// declaring none.
///
/// Without this, a `declares_executable_check` that always returned `true` would make every leg
/// above pass.
/// A scan that cannot report absence cannot report presence either. `omp-types` is a normal
/// library crate with no gate metadata; `gate-runner` is intentionally metadata-declared so its
/// own lane is reachable.
#[test]
fn the_stanza_detector_can_report_absence() {
    let root = repo_root();
    assert!(
        !declares_executable_check("omp-types"),
        "omp-types declares no gate stanza, so the detector must report absence here; if this fails, \
         the detector is stuck on true and every other leg in this file is vacuous"
    );
    assert!(
        declares_executable_check("state-wildcard-lint"),
        "and it must report presence where a stanza exists"
    );
}
