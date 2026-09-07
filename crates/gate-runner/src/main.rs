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

use gate_runner::{
    build_report, check_allowance, derive_roster, parse_ledger, Invocation, Observed,
    EXIT_METADATA_UNREADABLE,
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
        "usage: gate-runner --plan | --run [--only <crate>]\n\
         \n\
         omp-orchestrator-fsu7. Derives every workspace gate from `cargo metadata` and runs it\n\
         through ONE entry point. An empty roster is an ERROR, never a pass."
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        let report = build_report(&roster, &BTreeMap::new(), &ledger);
        for name in &report.ledger_only {
            println!("GATE_RUNNER_LEDGER_DRIFT crate={name} reason=in_ledger_absent_from_workspace");
        }
        for name in &report.workspace_only {
            println!("GATE_RUNNER_LEDGER_DRIFT crate={name} reason=in_workspace_absent_from_ledger");
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

    let mut observations = BTreeMap::new();
    for entry in &roster {
        let observed = run_crate(&repo, &entry.crate_name);
        observations.insert(entry.crate_name.clone(), observed);
    }

    let report = build_report(&roster, &observations, &ledger);
    print!("{}", report.render());
    ExitCode::from(report.exit_code())
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
        .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
        .current_dir(repo);
    match bounded_output(&mut command, METADATA_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Err(format!(
                    "cargo metadata exited {:?}",
                    output.status.code()
                ))
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
    let mut command = Command::new("cargo");
    command
        .args([
            "test",
            "-p",
            crate_name,
            "--no-fail-fast",
            "-j",
            "2",
        ])
        .current_dir(repo);
    match bounded_output(&mut command, PER_CRATE_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            parse_cargo_output(&text)
        }
        BoundedOutcome::TimedOut => Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(format!(
                "exceeded_{}s_deadline",
                PER_CRATE_DEADLINE.as_secs()
            )),
        },
        BoundedOutcome::Unspawned(error) => Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some(format!("cargo_unspawnable:{error}")),
        },
    }
}
/// Parse `Running`/`Doc-tests` headers paired with `test result:` lines.
///
/// A workspace-loading failure is UNMEASURABLE rather than a gate failure — it produces exit 101
/// with no `test result:` line at all, and calling that a failed gate would misattribute an
/// unrelated breakage to whichever crate happened to be next.
fn parse_cargo_output(text: &str) -> Observed {
    if text.contains("failed to load manifest") || text.contains("failed to parse manifest") {
        return Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some("workspace_manifest_unloadable".to_owned()),
        };
    }
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    let mut current = String::from("unknown");
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Running ") {
            current = target_label(rest);
        } else if trimmed.starts_with("Doc-tests ") {
            current = "doc".to_owned();
        } else if let Some(rest) = trimmed.strip_prefix("test result:") {
            if rest.trim_start().starts_with("ok") {
                passed.push(current.clone());
            } else {
                failed.push(current.clone());
            }
        }
    }
    if passed.is_empty() && failed.is_empty() {
        return Observed {
            passed,
            failed,
            unmeasurable: Some("no_test_result_line_in_output".to_owned()),
        };
    }
    Observed {
        passed,
        failed,
        unmeasurable: None,
    }
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
