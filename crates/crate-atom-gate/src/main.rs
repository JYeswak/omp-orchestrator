#![forbid(unsafe_code)]
//! `crate-atom-gate` — measure the workspace, hand the facts to the kernel, print the rows.
//!
//! ```text
//! crate-atom-gate check   [--repo <path>] [--allowances <path>]
//! crate-atom-gate report --json  [--repo <path>] [--allowances <path>]
//! crate-atom-gate --help | --version
//! ```
//!
//! Exit lattice (part 2): `0` pass, `1` refuse, `2` unrun (`SCAN_EMPTY`), `3` instrument
//! error. A timeout reaching `cargo metadata` is `3`, never `1`: an instrument that could
//! not look must not render as a subject that failed.
//!
//! # This binary does the I/O and the library does the deciding
//!
//! Part 1 of the atom this gate enforces forbids I/O in the kernel, so every measurement
//! lives here. Subprocesses go through `subprocess-contract` (bounded, both pipes drained,
//! group kill) rather than a bare `Command::output`, per the atom's cross-cutting rule.

use crate_atom_gate::metric_auth::require_vector_text;
use crate_atom_gate::workspace_hygiene::{
    crate_names_from_git_paths, extra_glob_members, HygieneError,
};
use crate_atom_gate::{
    assess_crate, ceiling_breaches, parse_allowances, verdict, Allowances, Caller, CrateFacts,
    GateVerdict, Part, PartStatus, Row, WIRED_CALLERS,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};



const SCHEMA: &str = "crate-atom-gate.v1";
const METADATA_DEADLINE: Duration = Duration::from_secs(90);

fn build_id() -> String {
    std::env::var("OMP_BUILD_ID").unwrap_or_else(|_| "unknown".to_owned())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1).cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "crate-atom-gate check|report [--repo <path>] [--allowances <path>] [--json]\n\
             exit: 0 pass  1 refuse  2 unrun(SCAN_EMPTY)  3 instrument-error"
        );
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version") {
        println!(
            "crate-atom-gate {} build={}",
            env!("CARGO_PKG_VERSION"),
            build_id()
        );
        return ExitCode::SUCCESS;
    }
    let repo = flag(&args, "--repo").map_or_else(default_repo, PathBuf::from);
    let allowance_path = flag(&args, "--allowances")
        .map_or_else(|| repo.join("registries/allowances.toml"), PathBuf::from);
    let want_json = args.iter().any(|a| a == "--json");

    let allowances = match load_allowances(&allowance_path) {
        Ok(allowances) => allowances,
        Err(reason) => return emit(&GateVerdict::InstrumentError { reason }, &[], want_json),
    };
    let packages = match read_packages(&repo) {
        Ok(packages) => packages,
        Err(reason) => return emit(&GateVerdict::InstrumentError { reason }, &[], want_json),
    };
    let machine = hostname();
    let hook_bytes = std::fs::read(repo.join(".git/hooks/pre-commit")).unwrap_or_default();
    let units = scheduled_units();
    let mut rows: Vec<Row> = Vec::new();
    for package in &packages {
        let facts = measure(package, &packages, &repo, &hook_bytes, &units, &machine);
        rows.extend(assess_crate(&facts, &allowances));
    }
    let mut outcome = verdict(&rows, packages.len(), &allowances);
    let vector_path = repo.join("docs/plan/METRIC-VECTOR.toml");
    outcome = match std::fs::read_to_string(&vector_path) {
        Err(error) => GateVerdict::InstrumentError {
            reason: format!("METRIC-VECTOR.toml unreadable: {error}"),
        },
        Ok(text) => match require_vector_text(&text) {
            Ok(()) => outcome,
            Err(error) => match outcome {
                GateVerdict::Refused { mut reasons } => {
                    reasons.push(error.to_string());
                    GateVerdict::Refused { reasons }
                }
                GateVerdict::Pass
                | GateVerdict::Unrun { .. }
                | GateVerdict::InstrumentError { .. } => GateVerdict::Refused {
                    reasons: vec![error.to_string()],
                },
            },
        },
    };
    outcome = fold_untracked(outcome, measure_untracked_members(&repo));

    // COMMIT-PATH ATTRIBUTION (omp-orchestrator-nu8lc), OPT-IN and only here.
    //
    // A census invocation -- CI, `--repo .`, a human audit -- must keep every refusal: its
    // subject is the whole repo. A COMMIT's subject is the commit, and on a tree carrying
    // 75 dirty files this gate would otherwise refuse your commit for a peer's in-flight
    // crate. A false red against the committer is worse than a silent gate, because it
    // trains every reader to discount the verdict.
    //
    // FOREIGN IS PRINTED, NEVER SILENCED, and it is printed BEFORE the exit code is
    // decided, so a reader of a green run still sees what the census found.
    if args.iter().any(|a| a == "--attribute-staged") {
        if let GateVerdict::Refused { reasons } = &outcome {
            match git_name_only(&repo, &["diff", "--cached", "--name-only"]) {
                // AN UNREADABLE STAGED SET IS NOT AN EMPTY ONE. Without it every reason
                // would look foreign and the gate would soften itself into silence, so the
                // refusal stands untouched and says why.
                Err(reason) => {
                    eprintln!(
                        "crate-atom-gate: ATTRIBUTION_UNAVAILABLE detail={reason} -- the \
                         staged set could not be read, so every refusal stands"
                    );
                }
                Ok(staged) => {
                    let split = crate_atom_gate::partition_by_staged(reasons, &staged);
                    for foreign in &split.foreign {
                        eprintln!(
                            "crate-atom-gate: FOREIGN_NOT_ATTRIBUTABLE staged_paths={} \
                             reason={foreign} -- a census finding about a crate this commit \
                             does not touch. REPORTED, not refused; fix it in its own commit.",
                            staged.len()
                        );
                    }
                    outcome = if split.attributable.is_empty() {
                        eprintln!(
                            "crate-atom-gate: ATTRIBUTED_CLEAN staged_paths={} foreign={} \
                             attributable=0 -- every census finding belongs to a crate this \
                             commit does not touch",
                            staged.len(),
                            split.foreign.len()
                        );
                        GateVerdict::Pass
                    } else {
                        GateVerdict::Refused {
                            reasons: split.attributable,
                        }
                    };
                }
            }
        }
    }
    emit(&outcome, &rows, want_json)
}

fn emit(outcome: &GateVerdict, rows: &[Row], want_json: bool) -> ExitCode {
    if want_json {
        // The report `plan-materialize` (n92x) consumes: one object per (crate, part) with
        // the labels it will file the bead under.
        let report: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "crate": row.crate_name,
                    "part": row.part.number(),
                    "part_name": row.part.name(),
                    "status": row.status.word(),
                    "detail": row.render(),
                    "labels": [format!("crate:{}", row.crate_name), format!("part:{}", row.part.number())],
                })
            })
            .collect();
        let envelope = json!({
            "schema": SCHEMA,
            "status": outcome.status(),
            "reason_code": reason_code(outcome),
            "build": build_id(),
            "rows": report,
            "reasons": reasons(outcome),
        });
        println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
    } else {
        for row in rows {
            println!("{}", row.render());
        }
        for reason in reasons(outcome) {
            eprintln!("crate-atom-gate: {reason}");
        }
        eprintln!(
            "crate-atom-gate: {} rows={} reason_code={}",
            outcome.status(),
            rows.len(),
            reason_code(outcome)
        );
    }
    ExitCode::from(outcome.exit_code())
}

fn reason_code(outcome: &GateVerdict) -> &'static str {
    match outcome {
        GateVerdict::Pass => "ATOM_COMPLETE",
        GateVerdict::Refused { .. } => "ATOM_INCOMPLETE",
        GateVerdict::Unrun { .. } => "SCAN_EMPTY",
        GateVerdict::InstrumentError { .. } => "INSTRUMENT_ERROR",
    }
}

fn reasons(outcome: &GateVerdict) -> Vec<String> {
    match outcome {
        GateVerdict::Pass => Vec::new(),
        GateVerdict::Refused { reasons } => reasons.clone(),
        GateVerdict::Unrun { reason } | GateVerdict::InstrumentError { reason } => {
            vec![reason.clone()]
        }
    }
}

fn default_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// An ABSENT registry is an instrument error, not an empty one.
///
/// Treating a missing file as "no allowances" would make every systemic gap unexcused and
/// refuse the whole repo — and, worse, the same code path would hide a typo'd `--allowances`
/// behind a plausible refusal.
fn load_allowances(path: &Path) -> Result<Allowances, String> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "ALLOWANCES_UNREADABLE path={} detail={error} \
             next_action=create-the-registry-or-name-the-path",
            path.display()
        )
    })?;
    parse_allowances(&text)
}

/// One package as `cargo metadata` describes it. Topology from cargo, never from a listing.
struct Package {
    name: String,
    dir: PathBuf,
    has_lib: bool,
    has_bin: bool,
    bin_names: Vec<String>,
    path_deps: Vec<String>,
}

fn read_packages(repo: &Path) -> Result<Vec<Package>, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command
        .current_dir(repo)
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1");
    let raw = match bounded_output(&mut command, METADATA_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => output.stdout,
        BoundedOutcome::Completed(output) => {
            return Err(format!(
                "CARGO_METADATA_FAILED status={:?} stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ")
            ))
        }
        // A deadline is NOT a verdict about the workspace.
        BoundedOutcome::TimedOut { .. } => {
            return Err(
                "CARGO_METADATA_TIMEOUT: the instrument could not look, which is \
                        not a finding about any crate"
                    .to_owned(),
            )
        }
        BoundedOutcome::Unspawned(error) => return Err(format!("CARGO_UNSPAWNED detail={error}")),
    };
    let value: Value = serde_json::from_slice(&raw)
        .map_err(|error| format!("CARGO_METADATA_UNPARSEABLE detail={error}"))?;
    let list = value
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "CARGO_METADATA_NO_PACKAGES_KEY".to_owned())?;
    let mut out = Vec::with_capacity(list.len());
    for package in list {
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let manifest = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let dir = Path::new(manifest)
            .parent()
            .map_or_else(|| repo.to_path_buf(), Path::to_path_buf);
        let targets = package
            .get("targets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let kinds = |kind: &str| {
            targets.iter().any(|target| {
                target
                    .get("kind")
                    .and_then(Value::as_array)
                    .is_some_and(|list| list.iter().any(|k| k.as_str() == Some(kind)))
            })
        };
        let bin_names = targets
            .iter()
            .filter(|target| {
                target
                    .get("kind")
                    .and_then(Value::as_array)
                    .is_some_and(|list| list.iter().any(|k| k.as_str() == Some("bin")))
            })
            .filter_map(|target| target.get("name").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        let path_deps = package
            .get("dependencies")
            .and_then(Value::as_array)
            .map(|deps| {
                deps.iter()
                    .filter(|dep| dep.get("path").is_some())
                    .filter_map(|dep| dep.get("name").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        out.push(Package {
            name,
            dir,
            has_lib: kinds("lib"),
            has_bin: kinds("bin"),
            bin_names,
            path_deps,
        });
    }
    Ok(out)
}

fn measure(
    package: &Package,
    all: &[Package],
    repo: &Path,
    hook_bytes: &[u8],
    units: &[(String, String)],
    machine: &str,
) -> CrateFacts {
    let src = read_tree(&package.dir.join("src"));
    let tests = read_tree(&package.dir.join("tests"));

    let verdict_markers = ["Unrun", "InstrumentError", "Verdictlike", "Outcome"]
        .into_iter()
        .filter(|marker| src.contains(marker))
        .map(str::to_owned)
        .collect::<BTreeSet<String>>();

    let test_fn_names = tests
        .lines()
        .filter_map(|line| line.trim().strip_prefix("fn "))
        .filter_map(|rest| rest.split(['(', '<']).next())
        .map(str::to_owned)
        .collect::<BTreeSet<String>>();

    let in_crate_fuzz_targets = list_dir(&package.dir.join("fuzz/fuzz_targets"));

    // A crate has something to fuzz when it parses, classifies, or maps a lattice.
    let has_fuzzable_kernel = [
        "fn parse",
        "fn classify",
        "fn assess",
        "fn decide",
        "fn scan",
    ]
    .iter()
    .any(|needle| src.contains(needle));
    // A crate is on a tick path when the supervisor or a tick lane names it.
    let on_tick_path = ["tick", "dispatch", "pane", "ack", "heartbeat"]
        .iter()
        .any(|needle| package.name.contains(needle));

    let mut callers: Vec<Caller> = all
        .iter()
        .filter(|other| other.path_deps.iter().any(|dep| *dep == package.name))
        .map(|other| Caller::ManifestDependency {
            dependent: other.name.clone(),
        })
        .collect();
    // Part 9 is REACHABILITY: the installed hook's BYTES, on this machine.
    for bin in &package.bin_names {
        if !hook_bytes.is_empty() && contains(hook_bytes, bin.as_bytes()) {
            callers.push(Caller::InstalledHook {
                hook: ".git/hooks/pre-commit".to_owned(),
                machine: machine.to_owned(),
            });
            break;
        }
        for (unit, unit_text) in units {
            if unit_text.contains(bin.as_str()) {
                callers.push(Caller::ScheduledUnit {
                    unit: unit.clone(),
                    machine: machine.to_owned(),
                });
                break;
            }
        }
    }
    let _ = repo;

    CrateFacts {
        name: package.name.clone(),
        has_lib: package.has_lib,
        has_bin: package.has_bin,
        verdict_markers,
        test_fn_names,
        in_crate_fuzz_targets,
        claim_ids: BTreeSet::new(),
        slo_rows: BTreeSet::new(),
        oracle: None,
        on_tick_path,
        has_fuzzable_kernel,
        callers,
    }
}

fn read_tree(dir: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    out.push_str(&text);
                    out.push('\n');
                }
            }
        }
    }
    out
}

fn list_dir(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// launchd plists and the crontab, so a bin invoked only by a schedule is REACHABLE.
///
/// The bead's own caller scan is in-repo Rust plus hook source, and its NO-CLAIM says a
/// launchd/cron/hand invocation is invisible to it. This is the part that fixes that.
fn scheduled_units() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let agents = PathBuf::from(&home).join("Library/LaunchAgents");
        if let Ok(entries) = std::fs::read_dir(&agents) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(text) = std::fs::read_to_string(&path) {
                    out.push((
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("plist")
                            .to_owned(),
                        text,
                    ));
                }
            }
        }
    }
    let mut crontab = Command::new("crontab");
    crontab.arg("-l");
    if let BoundedOutcome::Completed(output) = bounded_output(&mut crontab, Duration::from_secs(10))
    {
        if output.status.success() {
            out.push((
                "crontab".to_owned(),
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ));
        }
    }
    out
}

fn hostname() -> String {
    let mut command = Command::new("hostname");
    match bounded_output(&mut command, Duration::from_secs(5)) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        // A machine we could not name is stated as such: part 9's answer is per-machine,
        // so an unnamed machine makes the answer unciteable rather than wrong. Every arm
        // is enumerated because `BoundedOutcome` is a state enum and a wildcard here would
        // fold a TIMEOUT into the same string as a clean failure.
        BoundedOutcome::Completed(_) => "machine-unnamed:hostname-nonzero".to_owned(),
        BoundedOutcome::TimedOut { .. } => "machine-unnamed:hostname-timeout".to_owned(),
        BoundedOutcome::Unspawned(_) => "machine-unnamed:hostname-unspawned".to_owned(),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Keep the wiring roster reachable from the binary so a reader can print it.
#[allow(dead_code)]
fn wired_callers() -> &'static [(&'static str, &'static str)] {
    WIRED_CALLERS
}

#[allow(dead_code)]
fn part_names() -> Vec<&'static str> {
    Part::ALL.iter().map(|part| part.name()).collect()
}

#[allow(dead_code)]
fn is_missing(status: &PartStatus) -> bool {
    status.refuses()
}

#[allow(dead_code)]
fn breaches(rows: &[Row], allowances: &Allowances) -> Vec<String> {
    ceiling_breaches(rows, allowances)
}

const GIT_DEADLINE: Duration = Duration::from_secs(30);

fn git_name_only(repo: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match bounded_output(&mut command, GIT_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => Ok(String::from_utf8_lossy(
            &output.stdout,
        )
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()),
        BoundedOutcome::Completed(output) => Err(format!(
            "GIT_FAILED args={args:?} status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )),
        BoundedOutcome::TimedOut { .. } => Err("GIT_TIMEOUT: instrument could not look".to_owned()),
        BoundedOutcome::Unspawned(error) => Err(format!("GIT_UNSPAWNED detail={error}")),
    }
}

fn disk_crate_names(repo: &Path) -> Result<BTreeSet<String>, String> {
    let crates_dir = repo.join("crates");
    let entries = std::fs::read_dir(&crates_dir).map_err(|error| {
        format!(
            "CRATES_DIR_UNREADABLE path={} detail={error}",
            crates_dir.display()
        )
    })?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("CRATES_DIR_ENTRY detail={error}"))?;
        let path = entry.path();
        if path.is_dir() && path.join("Cargo.toml").is_file() {
            if let Some(name) = entry.file_name().to_str() {
                names.insert(name.to_owned());
            }
        }
    }
    Ok(names)
}

fn measure_untracked_members(repo: &Path) -> Result<Vec<String>, String> {
    let disk = disk_crate_names(repo)?;
    let committed = crate_names_from_git_paths(git_name_only(
        repo,
        &["ls-tree", "-r", "--name-only", "HEAD"],
    )?);
    let staged = crate_names_from_git_paths(git_name_only(
        repo,
        &["diff", "--cached", "--name-only"],
    )?);
    extra_glob_members(&disk, &committed, &staged).map_err(|error| error.to_string())
}

fn fold_untracked(outcome: GateVerdict, extras: Result<Vec<String>, String>) -> GateVerdict {
    match extras {
        Err(reason) if reason.contains("WORKSPACE_HYGIENE_UNRUN") => GateVerdict::Unrun { reason },
        Err(reason) => GateVerdict::InstrumentError { reason },
        Ok(names) if names.is_empty() => outcome,
        Ok(names) => {
            let reason = format!(
                "UNTRACKED_GLOB_MEMBER crates=[{}] next_action=add-or-delete — cargo crates/* loads disk members git ls-tree HEAD does not see",
                names.join(",")
            );
            match outcome {
                GateVerdict::Refused { mut reasons } => {
                    reasons.push(reason);
                    GateVerdict::Refused { reasons }
                }
                GateVerdict::Pass
                | GateVerdict::Unrun { .. }
                | GateVerdict::InstrumentError { .. } => GateVerdict::Refused {
                    reasons: vec![reason],
                },
            }
        }
    }
}

#[allow(dead_code)]
fn hygiene_empty_is_unrun(error: HygieneError) -> bool {
    matches!(error, HygieneError::EmptyDiskScan)
}
