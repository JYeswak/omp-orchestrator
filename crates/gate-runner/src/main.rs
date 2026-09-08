#![forbid(unsafe_code)]

//! `gate-runner` — the single entry point `omp-orchestrator-fsu7` asks for.
//!
//! Replaces a 12-job `gate.yml` fan-out. See `lib.rs` for why the roster is derived rather than
//! listed, and for the measured census trap that makes the `--lib` leg mandatory.
//!
//! # Modes
//!
//! ```text
//! gate-runner --plan            derive the roster and print it; runs NOTHING
//! gate-runner --run             derive, run every invocation, report per-crate + total
//! gate-runner --run --only X    same, restricted to crate X (for bisecting a red gate)
//! ```
//!
//! `--plan` exists so the roster itself is auditable without a multi-hour test run, and so the
//! anti-vacuity and ledger-drift legs can be exercised cheaply. It is not a substitute for `--run`
//! and says so in its own output.
//!
//! # No network, no push
//!
//! Every subprocess is `cargo` or a filesystem read. Nothing here contacts a network or a remote,
//! which is the requirement that lets this run as a local pre-push gate rather than only in CI.
//!
//! # Bounded spawns are not optional
//!
//! Child cargo processes go through `subprocess_contract::bounded_output`, which owns the deadline,
//! drains both pipes, and kills the process GROUP. Handrolling `Command::output()` here would
//! reintroduce the measured deadlock: undrained stdout+stderr past ~64 KiB with a `try_wait()`
//! poll hangs at 0% CPU, and a `cargo test` over 88 crates produces far more than 64 KiB.

mod ci_citation;
use gate_runner::{
    build_report_scoped, check_allowance, derive_checks, derive_roster, parse_ledger, verdict_for,
    Invocation, Observed, UnmeasurablePrecondition, EXIT_METADATA_UNREADABLE,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

/// One crate's test run. Generous, because a slow suite is not a broken one; the bound exists to
/// stop a wedged child holding the whole gate, not to enforce a performance budget.
const PER_CRATE_DEADLINE: Duration = Duration::from_secs(600);
/// `cargo metadata` is a manifest read. If it takes this long, something is wrong with the tree.
const METADATA_DEADLINE: Duration = Duration::from_secs(120);

/// The committed crate list, relative to the repository root.
const LEDGER_PATH: &str = "docs/gate-roster.txt";

fn usage() -> ExitCode {
    eprintln!(
        r#"usage: gate-runner --plan | --run [--only <crate>]

omp-orchestrator-fsu7. Derives every workspace gate from cargo metadata and runs it
through ONE entry point. An empty roster is an ERROR, never a pass.
gate-runner --ci-citation <run-id|latest> emits a citable CI aggregate with InputManifest."#
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--ci-citation" {
        return ci_citation::run(&args[1]);
    }
    let mut plan = false;
    let mut run = false;
    let mut only: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plan" => plan = true,
            "--run" => run = true,
            "--only" => {
                index += 1;
                match args.get(index) {
                    Some(value) => only = Some(value.clone()),
                    None => return usage(),
                }
            }
            "-h" | "--help" => return usage(),
            _ => return usage(),
        }
        index += 1;
    }
    if plan == run {
        // Both or neither. A default mode here would let an operator believe they had run the
        // gates when they had only planned them.
        return usage();
    }

    let repo = match repo_root() {
        Some(root) => root,
        None => {
            eprintln!(
                "GATE_RUNNER_NO_REPO_ROOT no .git or .beads marker at or above the current \
                 directory — refusing to guess a root, because a wrong root gates the wrong tree"
            );
            return ExitCode::from(EXIT_METADATA_UNREADABLE);
        }
    };

    let metadata = match cargo_metadata(&repo) {
        Ok(json) => json,
        Err(detail) => {
            eprintln!("GATE_RUNNER_METADATA_UNREADABLE detail={detail}");
            return ExitCode::from(EXIT_METADATA_UNREADABLE);
        }
    };

    let lib_tests = scan_lib_tests(&repo, &metadata);
    let roster = match derive_roster(&metadata, &lib_tests) {
        Ok(roster) => roster,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_METADATA_UNREADABLE);
        }
    };
    if let Err(error) = check_allowance(&roster) {
        eprintln!("{error}");
        return ExitCode::from(EXIT_METADATA_UNREADABLE);
    }

    // The DECLARED checks, parsed from each crate's own manifest. A parse failure is fatal:
    // silently treating a broken declaration as "no check" is how a gate stops running while
    // everything reads green, which is `etyur`'s whole subject.
    let checks = match derive_checks(&metadata) {
        Ok(checks) => checks,
        Err(error) => {
            eprintln!("GATE_RUNNER_CHECKS_UNREADABLE {error}");
            return ExitCode::from(EXIT_METADATA_UNREADABLE);
        }
    };
    // `{scratch}` is load-bearing for three declared invocations — `installer --bin-dir`,
    // `gate-reachability --out`, `head-compiles-gate --receipt` — so it must be a real directory
    // that OUTLIVES the command. Per AGENTS.md that is ZS_SCRATCH, never `mktemp`: a bare
    // `mktemp` has no session owner and cannot be safely reaped.
    let scratch = scratch_dir(&repo);

    // LEDGER DRIFT IS A WORKSPACE FACT, NOT A SCOPE FACT.
    //
    // Measured by this crate's own anti-vacuity leg: comparing the 88-row ledger against a
    // `--only`-FILTERED roster reported 87 spurious `in_ledger_absent_from_workspace` rows, and
    // because `exit_code()` ranks drift ABOVE gate failures, a scoped run would have returned
    // EXIT_LEDGER_DRIFT regardless of the actual verdict -- making `--only` useless for exactly
    // the decision it exists to support.
    //
    // `--only` narrows what is RUN. It does not narrow the workspace, so the ledger is compared
    // against the FULL derived roster and the scoped subset is what gets executed.
    let full_roster = roster.clone();
    let roster: Vec<_> = match &only {
        None => roster,
        Some(name) => roster
            .into_iter()
            .filter(|entry| entry.crate_name == *name)
            .collect(),
    };

    let ledger = std::fs::read_to_string(repo.join(LEDGER_PATH))
        .map(|text| parse_ledger(&text))
        .unwrap_or_default();

    if plan {
        let mut total = 0usize;
        println!("GATE_RUNNER_PLAN crates={} (NOTHING WAS RUN)", roster.len());
        for entry in &roster {
            total += entry.expected();
            let rendered: Vec<String> = entry.invocations.iter().map(ToString::to_string).collect();
            println!(
                "PLAN crate={} invocations={} [{}]",
                entry.crate_name,
                entry.expected(),
                rendered.join(" | ")
            );
        }
        println!("GATE_RUNNER_PLAN_TOTAL invocations={total}");
        // The plan is not a verdict. Report drift, because that is knowable without running.
        let report = build_report_scoped(&roster, &full_roster, &BTreeMap::new(), &ledger);
        for name in &report.ledger_only {
            println!(
                "GATE_RUNNER_LEDGER_DRIFT crate={name} reason=in_ledger_absent_from_workspace"
            );
        }
        for name in &report.workspace_only {
            println!(
                "GATE_RUNNER_LEDGER_DRIFT crate={name} reason=in_workspace_absent_from_ledger"
            );
        }
        if roster.is_empty() {
            eprintln!(
                "GATE_RUNNER_EMPTY_ROSTER derived zero crates. An empty gate set is an ERROR, \
                 never a pass."
            );
            return ExitCode::from(gate_runner::EXIT_EMPTY_ROSTER);
        }
        return ExitCode::SUCCESS;
    }

    // STREAM EACH VERDICT AS IT LANDS, AND MAKE IT DURABLE BEFORE MOVING ON.
    //
    // Measured 2026-09-07: the first full `--run` took 1289s and emitted 2471 bytes with ZERO
    // verdict rows until it finished, because the report was rendered once after the loop. That
    // is not "slow" — an interruption at crate 80 of 88 destroyed the record that 79 passed, so
    // every interrupted run cost its entire cost. `9gta3` item 1.
    let bank = bank_path(&repo);
    let mut observations = BTreeMap::new();
    for entry in &roster {
        let observed = run_crate(&repo, &entry.crate_name);
        let verdict = verdict_for(entry, Some(&observed));
        let row = verdict.render_row(&entry.crate_name);
        // stdout first so an operator sees it even if the bank is unwritable, then the durable
        // copy. A bank failure must be LOUD and must not be mistaken for a crate verdict.
        print!("{row}");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        if let Err(error) = bank_append(&bank, &row) {
            eprintln!(
                "GATE_RUNNER_BANK_UNWRITABLE path={} detail={error} -- verdicts are still \
                 streaming to stdout, but an interrupted run will NOT be resumable",
                bank.display()
            );
        }
        observations.insert(entry.crate_name.clone(), observed);
    }

    // ── THE RUN HALF: EXECUTE THE DECLARED CHECKS ─────────────────────────────────────────
    //
    // `omp-orchestrator-etyur` / R10(b). Until this loop existed, `derive_checks`, `expand` and
    // `subsumption` were exercised ONLY by `tests/roster.rs` — measured with
    // `ripwire --uses`: 5 uses, 2 uses, 1 use, every one a test, ZERO production consumers. So
    // 13 crates were graded `Covered { checks: true }` by a chain nothing called, and the field
    // named `checks` carried the value `declared`.
    //
    // `declared` and `executed` are different facts. This loop produces the second one.
    let mut check_rows = 0usize;
    let mut check_failures = 0usize;
    for invocation in &checks {
        if !roster.iter().any(|e| e.crate_name == invocation.crate_name) {
            continue;
        }
        for (index, phase) in invocation.phases.iter().enumerate() {
            let outcome = run_declared_check(&repo, &scratch, &invocation.crate_name, phase);
            let row = outcome.render_row(&invocation.crate_name, index);
            print!("{row}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            if let Err(error) = bank_append(&bank, &row) {
                eprintln!(
                    "GATE_RUNNER_BANK_UNWRITABLE path={} detail={error}",
                    bank.display()
                );
            }
            check_rows += 1;
            if outcome.is_blocking() {
                check_failures += 1;
            }
        }
    }
    // ANTI-VACUITY, SCOPED — because an over-strict gate gets routed around, which is a slower
    // death than no gate (rule 2).
    //
    // A FULL-roster run that executes zero declared checks is the dead-wiring state `etyur` was
    // filed for, and it read as green for as long as it existed: that is an ERROR.
    //
    // A SCOPED run is different. 75 of the 88 crates declare no check at all, so
    // `--only subprocess-contract` legitimately has nothing to execute — erroring there would
    // make `--only` unusable for most of the workspace and teach everyone to stop using it.
    // It still must not be SILENT: absence gets its own row.
    if check_rows == 0 && !roster.is_empty() {
        if only.is_some() {
            let row = format!(
                "CHECK_NONE_DECLARED scope={} reason=this_crate_declares_no_check\n",
                roster
                    .iter()
                    .map(|e| e.crate_name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            print!("{row}");
            let _ = bank_append(&bank, &row);
        } else {
            eprintln!(
                "GATE_RUNNER_NO_CHECKS_EXECUTED roster={} declared={} -- a FULL roster that \
                 executes ZERO declared checks is an ERROR, never a pass: it is \
                 indistinguishable from the parse-only state omp-orchestrator-etyur was filed \
                 for, where 13 crates were graded Covered{{checks:true}} by a chain nothing \
                 called",
                roster.len(),
                checks.len()
            );
            return ExitCode::from(gate_runner::EXIT_EMPTY_ROSTER);
        }
    }
    println!(
        "GATE_RUNNER_CHECKS executed={check_rows} failed={check_failures} declared_crates={}",
        checks.len()
    );

    let report = build_report_scoped(&roster, &full_roster, &observations, &ledger);
    print!("{}", report.render());
    // A FAILING DECLARED CHECK MUST FAIL THE RUN. Reporting it and exiting 0 is the
    // ledger-instead-of-a-gate defect this repo has already paid for twice.
    let code = report.exit_code();
    if code == gate_runner::EXIT_OK && check_failures > 0 {
        return ExitCode::from(gate_runner::EXIT_GATE_FAILED);
    }
    ExitCode::from(code)
}

/// What executing ONE declared check phase produced.
///
/// `Skipped` is a first-class outcome, not an absence: a SETUP phase mutates, so running it
/// during a gate pass would make the gate unable to distinguish *"already conformant"* from
/// *"I made it conformant"*. Declining is the correct behaviour and it must be VISIBLE.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckOutcome {
    Passed,
    Failed {
        code: String,
    },
    /// The environment could not host the check. NOT a failure of the check.
    Unmeasurable {
        reason: String,
    },
    /// A declared SETUP phase, deliberately not executed on a check pass.
    SkippedSetup,
}

impl CheckOutcome {
    fn is_blocking(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    fn render_row(&self, crate_name: &str, phase: usize) -> String {
        match self {
            Self::Passed => format!("CHECK_PASS crate={crate_name} phase={phase}\n"),
            Self::Failed { code } => {
                format!("CHECK_FAIL crate={crate_name} phase={phase} {code}\n")
            }
            Self::Unmeasurable { reason } => {
                format!("CHECK_UNMEASURABLE crate={crate_name} phase={phase} reason={reason}\n")
            }
            Self::SkippedSetup => format!(
                "CHECK_SKIPPED_SETUP crate={crate_name} phase={phase} \
                 reason=declared_setup_mutates_and_is_not_a_check\n"
            ),
        }
    }
}

/// Execute one declared check phase and classify it.
///
/// # Why the exit code alone is not the verdict
///
/// `AGENTS.md` rule 7: `101` is `cargo`'s generic failure and cannot discriminate a real RED from
/// a workspace that failed to load — this repo has hit that twice. So the row carries the exit
/// code AND the classification, and a spawn failure is `Unmeasurable` rather than `Failed`:
/// "the environment could not host this" and "the check refused" are different facts with
/// different remedies.
fn run_declared_check(
    repo: &Path,
    scratch: &Path,
    crate_name: &str,
    phase: &gate_runner::CheckPhase,
) -> CheckOutcome {
    if phase.setup {
        return CheckOutcome::SkippedSetup;
    }
    let bin = phase.bin.clone().unwrap_or_else(|| crate_name.to_owned());
    let args = gate_runner::expand(
        &phase.args,
        &repo.to_string_lossy(),
        &scratch.to_string_lossy(),
    );
    let mut command = Command::new("cargo");
    command
        .args(["run", "--quiet", "-p", crate_name, "--bin", &bin, "--"])
        .args(&args)
        .current_dir(repo)
        .env("CARGO_TERM_COLOR", "never");
    match bounded_output(&mut command, PER_CRATE_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            if output.status.success() {
                CheckOutcome::Passed
            } else {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                // A refused REMOTE build is the environment, not the check. Measured this
                // session: `[RCH] remote required; refusing local fallback` arrives with a
                // nonzero code that says nothing about the subject.
                if text.contains("remote required; refusing local fallback")
                    || text.contains("no admissible workers")
                {
                    return CheckOutcome::Unmeasurable {
                        reason: "rch_refused_the_build".to_owned(),
                    };
                }
                if text.contains("LOCAL BUILD REFUSED") {
                    return CheckOutcome::Unmeasurable {
                        reason: "local_build_refused_by_gate".to_owned(),
                    };
                }
                CheckOutcome::Failed {
                    code: format!(
                        "bin={bin} exit={} argv={}",
                        output
                            .status
                            .code()
                            .map_or_else(|| "signal".to_owned(), |c| c.to_string()),
                        args.join(" ")
                    ),
                }
            }
        }
        BoundedOutcome::TimedOut => CheckOutcome::Unmeasurable {
            reason: format!("exceeded_{}s_deadline", PER_CRATE_DEADLINE.as_secs()),
        },
        BoundedOutcome::Unspawned(error) => CheckOutcome::Unmeasurable {
            reason: format!("cargo_unspawnable:{error}"),
        },
    }
}

/// The per-run `{scratch}` directory, session-owned per `AGENTS.md`.
///
/// `ZS_SCRATCH` when set; otherwise a session-scoped path under the documented root. NOT
/// `mktemp`: three declared invocations write artifacts that outlive their command
/// (`installer --bin-dir`, `gate-reachability --out`, `head-compiles-gate --receipt`), and a
/// `mktemp` directory has no session owner and cannot be safely reaped.
fn scratch_dir(repo: &Path) -> PathBuf {
    let base = std::env::var("ZS_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
            Path::new(&home)
                .join(".local/state/zeststream/scratch")
                .join("omp-orchestrator")
                .join("gate-runner")
        });
    let dir = base.join(format!("checks-{}", std::process::id()));
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "GATE_RUNNER_SCRATCH_UNWRITABLE path={} detail={error} -- declared checks that write \
             to {{scratch}} will report UNMEASURABLE rather than failing",
            dir.display()
        );
    }
    let _ = repo;
    dir
}

/// Where streamed verdicts are banked so an interrupted run keeps what it earned.
///
/// `GATE_RUNNER_BANK` overrides it — the tests need a path they own, and a test writing into the
/// real bank would corrupt a running gate's resume state.
///
/// The default lives under `target/`, which is already ignored, so the bank cannot become the
/// untracked-file-at-the-repo-root class this repo has a janitor skill for.
fn bank_path(repo: &Path) -> PathBuf {
    if let Ok(explicit) = std::env::var("GATE_RUNNER_BANK") {
        if !explicit.trim().is_empty() {
            return PathBuf::from(explicit);
        }
    }
    repo.join("target").join("gate-runner-bank.rows")
}

/// Append one already-rendered row, then `sync_all` before returning.
///
/// # What makes the KILL survivable, corrected by a mutation that FAILED TO BITE
///
/// This comment first claimed `sync_all` was what defended against a kill, *"the exact case where
/// an unflushed buffer is lost with the process."* **That is false.** `std::fs::File` is not
/// buffered in user space, so `write_all` is a `write(2)` and the kernel holds the bytes the
/// moment it returns; a `SIGKILL` after that loses nothing. Measured 2026-09-07 by replacing
/// `file.sync_all()` with `Ok(())` and re-running the suite on the lane:
///
/// ```text
/// sync_all REMOVED -> exit=0, 4 passed / 0 failed   <- the mutation did NOT bite
/// the whole bank write REMOVED -> exit=101, 3 of 4 RED  <- that one did
/// ```
///
/// **So `write_all` provides the tested property and `sync_all` provides a DIFFERENT, untested
/// one:** durability across a machine crash or power loss, where the page cache is also lost. It
/// is kept because 88 fsyncs cost nothing against 88 `cargo test` invocations, and because a
/// resume bank that survives only a tidy shutdown is the case that needed no bank.
///
/// **NO-CLAIM: no leg in this suite covers the crash case.** Removing `sync_all` leaves the suite
/// green, so its presence here is a judgement about a failure mode this repo cannot cheaply test,
/// not a property under gate. Said out loud because an fsync that nothing exercises is exactly
/// the kind of line a later reader deletes as noise.
fn bank_append(path: &Path, row: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(row.as_bytes())?;
    file.sync_all()
}

/// Walk up for a repository marker. Never a constant: a wrong root compiles fine and then gates
/// the wrong tree, which is `omp-orchestrator-npq`'s mechanism.
fn repo_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join(".git").exists() || current.join(".beads").exists() {
            return Some(current);
        }
        current = current.parent()?.to_path_buf();
    }
}

fn cargo_metadata(repo: &Path) -> Result<String, String> {
    let mut command = Command::new("cargo");
    command
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(repo);
    match bounded_output(&mut command, METADATA_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Err(format!("cargo metadata exited {:?}", output.status.code()))
            }
        }
        BoundedOutcome::TimedOut => Err(format!(
            "cargo metadata exceeded {}s",
            METADATA_DEADLINE.as_secs()
        )),
        BoundedOutcome::Unspawned(error) => Err(format!("cargo not spawnable: {error}")),
    }
}

/// Does each crate's library carry `#[test]` functions?
///
/// `cargo metadata` cannot answer this — it reports integration targets only — and a roster that
/// ignores it silently skips 12 crates in this workspace. See the module docs in `lib.rs`.
fn scan_lib_tests(repo: &Path, metadata: &str) -> BTreeMap<String, bool> {
    let mut map = BTreeMap::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata) else {
        return map;
    };
    let Some(packages) = value.get("packages").and_then(serde_json::Value::as_array) else {
        return map;
    };
    for package in packages {
        let Some(name) = package.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(manifest) = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let src = Path::new(manifest)
            .parent()
            .map(|dir| dir.join("src"))
            .unwrap_or_else(|| repo.join("src"));
        map.insert(name.to_owned(), dir_has_test_attr(&src));
    }
    map
}

fn dir_has_test_attr(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if dir_has_test_attr(&path) {
                return true;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if text.contains("#[test]") {
                    return true;
                }
            }
        }
    }
    false
}

/// Run one crate's whole suite and parse per-target results.
///
/// `--no-fail-fast` is load-bearing and is a measured requirement, not a preference: without it
/// `cargo test` STOPS at the first failing target, so the reported figure silently omits every
/// later target. That is how a crate-health claim of "251 passed / 55 failed" came from 52 of 55
/// targets while reading as complete.
fn run_crate(repo: &Path, crate_name: &str) -> Observed {
    if let Some(reason) = environment_precondition(repo, crate_name) {
        return Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(reason),
        };
    }
    let mut command = Command::new("cargo");
    command
        .args([
            "test",
            "-p",
            crate_name,
            "--no-fail-fast",
            "-j",
            "2",
            // REMOVE THE CAUSE, not just the symptom. rch allocates a PTY, so cargo and libtest
            // colorize: the header arrives as `\x1b[1m\x1b[92m     Running\x1b[0m unittests …`,
            // which `strip_prefix("Running ")` can never match. Both halves are needed — the
            // first is cargo's own status stream, the second is libtest's `FAILED` token.
            "--color=never",
        ])
        .args(["--", "--color", "never"])
        .current_dir(repo);
    match bounded_output(&mut command, PER_CRATE_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            classify_completed_cargo_output(output.status.code(), &text)
        }

        BoundedOutcome::TimedOut => Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(UnmeasurablePrecondition::PolicyUnavailable {
                policy: "cargo-deadline".to_owned(),
                detail: format!("exceeded_{}s_deadline", PER_CRATE_DEADLINE.as_secs()),
            }),
        },
        BoundedOutcome::Unspawned(error) => Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(UnmeasurablePrecondition::MissingExecutable {
                executable: "cargo".to_owned(),
                detail: format!("cargo could not be spawned: {error}"),
            }),
        },
    }
}

/// Classify only preconditions that make a lane's result untruthful, without hiding an observed
/// test failure. These probes are lane-specific because a generic environment refusal would turn
/// ordinary test failures into silent passes.
fn environment_precondition(repo: &Path, crate_name: &str) -> Option<UnmeasurablePrecondition> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    environment_precondition_with_path(repo, crate_name, &path)
}

fn environment_precondition_with_path(
    repo: &Path,
    crate_name: &str,
    path: &std::ffi::OsStr,
) -> Option<UnmeasurablePrecondition> {
    let missing_executable = |executable: &str| {
        (!executable_available(executable, path)).then(|| {
            UnmeasurablePrecondition::MissingExecutable {
                executable: executable.to_owned(),
                detail: format!("{executable} is unavailable on PATH"),
            }
        })
    };

    if matches!(
        crate_name,
        "fleet-composite" | "omp-idle-dispatch" | "wired-but-inert-guard"
    ) {
        let temp_root = std::env::temp_dir();
        if let Some(marker) = marker_in_ancestors(&temp_root) {
            return Some(UnmeasurablePrecondition::FixtureScopeUnavailable {
                root: temp_root.display().to_string(),
                conflicting_marker: marker.display().to_string(),
                detail: "the marker-free fixture is nested below a repository marker".to_owned(),
            });
        }
    }
    match crate_name {
        "installer" if !repo.join(".git").exists() => Some(UnmeasurablePrecondition::MissingPath {
            path: ".git".to_owned(),
            detail: "the installer lane requires repository metadata".to_owned(),
        }),
        "ack-spine" if !repo.join(".beads/issues.jsonl").is_file() => {
            Some(UnmeasurablePrecondition::MissingPath {
                path: ".beads/issues.jsonl".to_owned(),
                detail: "the ack-spine lane requires the tracker ledger".to_owned(),
            })
        }
        "finding" => missing_executable("br"),
        "loop-queue-filter" => missing_executable("bv"),
        "dispatch-silence-watch" if !executable_available("crontab", path) => {
            Some(UnmeasurablePrecondition::MissingScheduler {
                scheduler: "crontab".to_owned(),
                detail: "the dispatch-silence-watch lane requires the scheduler CLI".to_owned(),
            })
        }
        "admission-reason" => {
            if !executable_available("sh", path) {
                Some(UnmeasurablePrecondition::MissingExecutable {
                    executable: "sh".to_owned(),
                    detail: "the differential oracle requires a shell interpreter".to_owned(),
                })
            } else {
                let oracle = repo.join("../control-plane/bin/admission-reason.sh");
                (!oracle.is_file()).then(|| UnmeasurablePrecondition::PolicyUnavailable {
                    policy: "admission-reason-differential".to_owned(),
                    detail: format!("oracle missing at {}", oracle.display()),
                })
            }
        }
        "loop-driver" if !Path::new("/usr/bin/lockf").is_file() => {
            Some(UnmeasurablePrecondition::PolicyUnavailable {
                policy: "lockf-shell-oracle".to_owned(),
                detail: "the loop-driver differential oracle requires /usr/bin/lockf".to_owned(),
            })
        }
        _ => None,
    }
}

fn marker_in_ancestors(start: &Path) -> Option<PathBuf> {
    for directory in start.ancestors() {
        for marker in [".git", ".beads"] {
            let candidate = directory.join(marker);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}
fn executable_available(executable: &str, path: &std::ffi::OsStr) -> bool {
    let candidate = Path::new(executable);
    if candidate.components().count() > 1 {
        return candidate.is_file();
    }
    std::env::split_paths(path).any(|directory| directory.join(executable).is_file())
}
/// Parse `test result:` lines, attributing each to a target.
///
/// A workspace-loading failure is UNMEASURABLE rather than a gate failure — it produces exit 101
/// with no `test result:` line at all, and calling that a failed gate would misattribute an
/// unrelated breakage to whichever crate happened to be next.
///
/// # RETRACTED 2026-09-07, by a negative control on my own instrument
///
/// This comment first read: *"MEASURED LANE FACT: the `Running` headers do not arrive… across
/// every rch lane log, `Running`, `Finished`, and `Compiling` counts were 0."* **That is false and
/// it was an instrument artifact.** The counting grep was `grep -c 'Running unittests'` — an
/// **adjacent literal** — while the real line is
/// `\x1b[1m\x1b[92m     Running\x1b[0m unittests src/lib.rs (…)`, with an ANSI reset sitting
/// between the two words. The probe returned **0 for present and 0 for absent**, which is the
/// definition of an instrument that cannot answer the question. Re-measured with a bare word
/// count, the very log used as evidence carries **3** `Running` lines. Cargo's stderr arrives
/// fine: `warning: profiles for the non root package…` and `error: no test target named…` were
/// both delivered.
///
/// # THE TWO REAL CAUSES of `failing_targets=unknown`
///
/// 1. **ANSI.** rch allocates a PTY, so cargo and libtest colorize, and
///    `strip_prefix("Running ")` is dead code against a line beginning with escape bytes.
///    Addressed at the spawn with `--color=never` (cargo) and `-- --color never` (libtest), and
///    defended here by stripping CSI sequences in case a caller's env forces colour anyway.
/// 2. **STREAM ORDER, which is structural.** `bounded_output` returns stdout and stderr as
///    SEPARATE buffers, and this crate concatenates stdout-then-stderr. `Running` is stderr and
///    `test result:` is stdout, so *every* result line precedes *every* header regardless of
///    colour, lane, or terminal. Interleaving cannot be recovered from two finished buffers, so
///    header attribution is impossible here even after fixing the colour.
///
/// Hence the primary mechanism is **stdout alone**, which carries `test <name> … FAILED` and
/// `test result:` in true relative order. When even that is absent the label is
/// `unattributed_target#N` — an explicit statement that the target is unnamed, never a target
/// that appears to be called `unknown`.
///
/// NOT FIXED BY THIS: a per-crate run can COUNT result lines but cannot NAME which of a roster's
/// declared invocations produced them, so `observed < expected` still catches a shortfall while
/// "the declared targets are the ones that ran" stays UNMEASURED. Only a per-invocation run
/// attributes by construction — which is the real argument for the 286-invocation denominator.
fn classify_completed_cargo_output(status_code: Option<i32>, text: &str) -> Observed {
    if status_code == Some(4) {
        return Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(UnmeasurablePrecondition::PolicyUnavailable {
                policy: "child-exit-4".to_owned(),
                detail: "child returned exit=4; gate result is unmeasurable, not failed".to_owned(),
            }),
        };
    }
    parse_cargo_output(text)
}

fn parse_cargo_output(text: &str) -> Observed {
    if text.contains("failed to load manifest") || text.contains("failed to parse manifest") {
        return Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(UnmeasurablePrecondition::PolicyUnavailable {
                policy: "cargo-workspace".to_owned(),
                detail: "cargo could not load or parse the workspace manifest".to_owned(),
            }),
        };
    }
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    let mut current: Option<String> = None;
    let mut pending: Vec<String> = Vec::new();
    let mut ordinal = 0usize;
    let mut summaries = Vec::new();
    for line in text.lines() {
        let plain = strip_ansi(line);
        let trimmed = plain.trim();
        if let Some(rest) = trimmed.strip_prefix("Running ") {
            current = Some(target_label(rest));
        } else if trimmed.starts_with("Doc-tests ") {
            current = Some("doc".to_owned());
        } else if let Some(rest) = trimmed.strip_prefix("test result:") {
            ordinal += 1;
            if let Some(summary) = parse_summary_counts(rest) {
                summaries.push(summary);
            }
            let label = match &current {
                Some(name) => name.clone(),
                None if !pending.is_empty() => {
                    format!("unattributed_target(failing_tests:{})", pending.join(","))
                }
                None => format!("unattributed_target#{ordinal}"),
            };
            if rest.trim_start().starts_with("ok") {
                passed.push(label);
            } else {
                failed.push(label);
            }
            current = None;
            pending.clear();
        } else if let Some(name) = failing_test_name(trimmed) {
            pending.push(name);
        }
    }
    if summaries
        .iter()
        .all(|(passed, failed, _)| *passed + *failed == 0)
        && !summaries.is_empty()
    {
        let skipped = summaries.iter().map(|(_, _, skipped)| *skipped).sum();
        return Observed {
            passed,
            failed,
            unmeasurable: Some(UnmeasurablePrecondition::AllTestsSkipped {
                expected: summaries.len(),
                skipped,
                detail: "every cargo test result reported zero executed tests".to_owned(),
            }),
        };
    }
    if passed.is_empty() && failed.is_empty() {
        return Observed {
            passed,
            failed,
            unmeasurable: Some(UnmeasurablePrecondition::AllTestsSkipped {
                expected: 0,
                skipped: 0,
                detail: "no cargo test result line was observed".to_owned(),
            }),
        };
    }
    Observed {
        passed,
        failed,
        unmeasurable: None,
    }
}

fn parse_summary_counts(rest: &str) -> Option<(usize, usize, usize)> {
    let trimmed = rest.trim_start();
    let details = trimmed
        .strip_prefix("ok.")
        .or_else(|| trimmed.strip_prefix("FAILED."))?
        .trim();
    Some((
        summary_count(details, "passed")?,
        summary_count(details, "failed")?,
        summary_count(details, "ignored")?,
    ))
}

fn summary_count(details: &str, label: &str) -> Option<usize> {
    details.split(';').find_map(|field| {
        field
            .trim()
            .strip_suffix(label)
            .and_then(|count| count.trim().parse().ok())
    })
}
#[test]
fn environment_preconditions_have_typed_bad_and_good_legs() {
    let empty_path = std::ffi::OsString::new();
    assert!(matches!(
        environment_precondition_with_path(Path::new("/missing-repo"), "finding", &empty_path),
        Some(UnmeasurablePrecondition::MissingExecutable { ref executable, .. })
            if executable == "br"
    ));
    assert!(matches!(
        environment_precondition_with_path(Path::new("/missing-repo"), "dispatch-silence-watch", &empty_path),
        Some(UnmeasurablePrecondition::MissingScheduler { ref scheduler, .. })
            if scheduler == "crontab"
    ));

    let bin_dir =
        std::env::temp_dir().join(format!("gate-runner-precondition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bin_dir);
    std::fs::create_dir_all(&bin_dir).expect("fixture directory");
    std::fs::write(bin_dir.join("br"), b"fixture").expect("fixture executable");
    let path = std::env::join_paths([bin_dir.as_path()]).expect("fixture PATH");
    assert_eq!(
        environment_precondition_with_path(Path::new("/missing-repo"), "finding", &path),
        None,
        "a present positive-control executable must not be classified as missing"
    );
    let _ = std::fs::remove_dir_all(bin_dir);
}
/// Remove ANSI CSI sequences so a colorized line matches the same predicates as a plain one.
///
/// # Why a parser needs this at all
///
/// `--color=never` at the spawn is the real fix; this is the residual. `CARGO_TERM_COLOR=always`
/// in a caller's environment overrides the flag, and a parser that silently stops matching when
/// colour appears is the defect this crate already shipped once: the header branch was dead code
/// for every run, and the failure was invisible because it degraded to a label rather than an
/// error.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // ESC [ … <final byte in @..~>. Anything else after ESC is dropped along with the ESC.
        if chars.next() != Some('[') {
            continue;
        }
        for p in chars.by_ref() {
            if ('\u{40}'..='\u{7e}').contains(&p) {
                break;
            }
        }
    }
    out
}

/// `test real_atomic_install_publishes_complete_binary ... FAILED` -> the test's name.
///
/// Deliberately anchored on both ends: `test result: FAILED. 25 passed; 1 failed` shares the
/// prefix and must never be read as a test name.
fn failing_test_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("test ")?;
    let name = rest.strip_suffix(" ... FAILED")?;
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    Some(name.to_owned())
}

/// `Running unittests src/lib.rs (target/debug/deps/foo-abc123)` -> `--lib`;
/// `Running tests/roster.rs (…)` -> `roster`.
fn target_label(rest: &str) -> String {
    let head = rest.split(" (").next().unwrap_or(rest).trim();
    if head.starts_with("unittests") {
        return Invocation::Lib.to_string();
    }
    Path::new(head)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| head.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KNOWN-BAD, taken verbatim in shape from a real `installer` lane log 2026-09-07.
    ///
    /// The `Running` headers are ABSENT because cargo writes them to stderr and this lane does
    /// not deliver them. The first version of this parser labelled the RED `unknown`, producing
    /// `FAIL crate=installer failing_targets=unknown` — a verdict naming nothing.
    #[test]
    fn a_failure_is_attributed_from_stdout_when_the_running_header_never_arrives() {
        let lane = "\
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test real_atomic_install_publishes_complete_binary ... FAILED
test result: FAILED. 25 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.70s
";
        let observed = parse_cargo_output(lane);
        assert_eq!(observed.unmeasurable, None, "result lines are present");
        assert_eq!(observed.passed.len(), 1);
        assert_eq!(
            observed.failed,
            vec![
                "unattributed_target(failing_tests:real_atomic_install_publishes_complete_binary)"
            ],
            "the failing test names its own cause; `unknown` names nothing"
        );
        assert!(
            !observed.failed.iter().any(|f| f == "unknown"),
            "the retired label must not come back"
        );
    }

    /// The prefix trap: `test result: FAILED.` shares `test ` with a test name and must never be
    /// harvested as one. Without this the parser would invent a test called `result:`.
    #[test]
    fn the_summary_line_is_never_read_as_a_test_name() {
        assert_eq!(
            failing_test_name("test result: FAILED. 25 passed; 1 failed; 0 ignored"),
            None
        );
        // POSITIVE CONTROL: the real shape still parses, so the guard above is not vacuous.
        assert_eq!(
            failing_test_name("test some_gate_leg ... FAILED").as_deref(),
            Some("some_gate_leg")
        );
        // And an `ok` line is not a failure.
        assert_eq!(failing_test_name("test some_gate_leg ... ok"), None);
    }

    /// KNOWN-GOOD: when the headers DO arrive the label is the real target, so the stdout path is
    /// a fallback and not a replacement. An over-strict parser that always said `unattributed`
    /// would pass the leg above and be wrong everywhere else.
    #[test]
    fn a_present_running_header_still_names_the_target() {
        let text = "\
     Running unittests src/lib.rs (target/debug/deps/installer-abc)
test result: ok. 11 passed; 0 failed; 0 ignored
     Running tests/l0_install.rs (target/debug/deps/l0_install-def)
test bad ... FAILED
test result: FAILED. 25 passed; 1 failed; 0 ignored
";
        let observed = parse_cargo_output(text);
        assert_eq!(observed.passed, vec!["--lib"]);
        assert_eq!(
            observed.failed,
            vec!["l0_install"],
            "a real header wins over the stdout fallback"
        );
    }

    /// ANTI-VACUITY: no `test result:` line at all is UNMEASURABLE, never a pass and never a
    /// failure — the workspace-loading case that would otherwise misattribute to the next crate.
    #[test]
    fn output_with_no_result_line_is_unmeasurable() {
        let observed = parse_cargo_output("error: could not compile `installer`\n");
        assert!(
            matches!(
                &observed.unmeasurable,
                Some(UnmeasurablePrecondition::AllTestsSkipped {
                    expected: 0,
                    skipped: 0,
                    ..
                })
            ),
            "missing result evidence must be typed as unmeasurable: {:?}",
            observed.unmeasurable
        );
        assert!(observed.passed.is_empty() && observed.failed.is_empty());
    }
    #[test]
    fn zero_executed_results_are_all_skipped_not_passed() {
        let text = "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        let observed = parse_cargo_output(text);
        assert!(matches!(
            observed.unmeasurable,
            Some(UnmeasurablePrecondition::AllTestsSkipped {
                expected: 1,
                skipped: 1,
                ..
            })
        ));
    }

    /// KNOWN-BAD, byte-exact from a measured rch lane log 2026-09-07.
    ///
    /// This is the line that made `strip_prefix("Running ")` dead code. It is written with real
    /// escape bytes rather than a prose description, because the whole defect was that a
    /// human-readable rendering of this line looks identical to the plain one.
    #[test]
    fn a_colorized_running_header_still_names_its_target() {
        let colorized = concat!(
            "\u{1b}[1m\u{1b}[92m     Running\u{1b}[0m unittests src/lib.rs (target/debug/deps/x-1)\n",
            "test result: ok. 11 passed; 0 failed; 0 ignored\n"
        );
        let observed = parse_cargo_output(colorized);
        assert_eq!(
            observed.passed,
            vec!["--lib"],
            "an ANSI reset between `Running` and `unittests` must not blind the parser"
        );

        // The same bytes, and the retired instrument: an adjacent-literal search finds nothing.
        // This is the negative control on the PROBE, kept in-tree so the lesson cannot be lost.
        assert!(
            !colorized.contains("Running unittests"),
            "the adjacent literal is absent from a line that plainly contains both words -- \
             which is why `grep -c 'Running unittests'` returned 0 for a present subject"
        );
        assert!(
            strip_ansi(colorized.lines().next().expect("a line")).contains("Running unittests"),
            "and it is present once the escapes are removed"
        );
    }

    /// STRUCTURAL: stdout and stderr arrive as separate buffers, so a header can never precede
    /// the result line it describes. Attribution must come from stdout alone.
    ///
    /// This reproduces `run_crate`'s own `format!("{stdout}{stderr}")` ordering. Even with colour
    /// fixed, header attribution is unreachable — so the stdout path is the mechanism, not a
    /// fallback of last resort.
    #[test]
    fn concatenated_streams_cannot_attribute_by_header_and_still_name_the_failure() {
        let stdout = "\
test result: ok. 11 passed; 0 failed; 0 ignored
test real_atomic_install_publishes_complete_binary ... FAILED
test result: FAILED. 25 passed; 1 failed; 0 ignored
";
        let stderr = "\
     Running unittests src/lib.rs (target/debug/deps/installer-abc)
     Running tests/l0_install.rs (target/debug/deps/l0_install-def)
";
        let observed = parse_cargo_output(&format!("{stdout}{stderr}"));
        assert_eq!(
            observed.failed,
            vec![
                "unattributed_target(failing_tests:real_atomic_install_publishes_complete_binary)"
            ],
            "the failing test is named even though both headers are present but out of order"
        );
        assert!(
            !observed.failed.iter().any(|f| f == "unknown"),
            "and never the retired label"
        );
    }

    /// A bare ESC with no `[`, and a truncated CSI, must not eat the rest of the line silently
    /// past their own extent -- an over-eager stripper would delete real content.
    #[test]
    fn strip_ansi_does_not_swallow_ordinary_text() {
        assert_eq!(strip_ansi("plain text"), "plain text");
        assert_eq!(strip_ansi("a\u{1b}[0mb"), "ab");
        assert_eq!(
            strip_ansi("a\u{1b}Xb"),
            "ab",
            "a non-CSI escape drops only itself"
        );
        assert_eq!(
            strip_ansi("a\u{1b}[31"),
            "a",
            "a truncated CSI consumes to end of line"
        );
    }
    #[test]
    fn child_exit_four_is_unmeasurable_not_a_failure() {
        let observed = classify_completed_cargo_output(
            Some(4),
            "test result: FAILED. 0 passed; 1 failed; 0 ignored",
        );
        assert!(observed.failed.is_empty());
        assert!(matches!(
            observed.unmeasurable,
            Some(UnmeasurablePrecondition::PolicyUnavailable { ref policy, .. })
                if policy == "child-exit-4"
        ));
    }

    #[test]
    fn ordinary_child_failure_still_parses_as_failure() {
        let observed = classify_completed_cargo_output(
            Some(101),
            "test bad ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored",
        );
        assert!(observed.unmeasurable.is_none());
        assert_eq!(observed.failed.len(), 1);
    }
}
