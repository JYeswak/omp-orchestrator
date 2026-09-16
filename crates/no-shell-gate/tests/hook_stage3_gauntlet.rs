#![forbid(unsafe_code)]

//! STAGE-3 GAUNTLET (bead 7c37.1) for the live `.git/hooks/pre-commit`
//! binary, per `/hook-certification` Stage 3.
//!
//! WHAT EACH LEG PINS (read this before reordering):
//! - `live_hook_fingerprinted` — the live path resolves to a real binary;
//!   prints sha256+mtime for the run record. Absent live hook is a loud SKIP,
//!   never a pass: lanes without the installed artifact cannot certify it.
//! - `artifact_under_test_matches_live_bytes` — the executed bytes ARE the
//!   live bytes when present (else a declared fresh build). Anti-substitution.
//! - `golden_clean_exits_zero` — benign staged set -> exit 0 + exactly one
//!   CLEAN outcome marker (the known-good golden).
//! - `known_bad_shell_fires_violation_exit_1` — staged `check.sh` -> exit 1 +
//!   VIOLATION marker + exemption sentence + the filename (fires-on-known-bad;
//!   the filename pins the refusal is ABOUT our file, not vacuous).
//! - `empty_staged_exits_3` — no staged files -> exit 3 + NOTHING_TO_CHECK.
//! - `malformed_editmsg_refuses_typed` — missing COMMIT_EDITMSG -> exit 1 +
//!   typed `cannot read COMMIT_EDITMSG` refusal.
//! - `msg_mode_matching_source_allows` — COMMIT_EDITMSG matching OMP_MSG_SRC
//!   -> exit 0 (round-trip accept path; source is consumed, fixture-local).
//! - `unknown_argv_never_panics_nor_hangs` — argv fuzz: exit in {0,1,2,3},
//!   never 101 (panic), every case completes (no-hang pin via timeout).
//! - `staged_tree_fuzz_never_panics` — staged-content fuzz: same contract.
//! - `satisfiability_violation_leg_reddens_on_golden_input` /
//!   `satisfiability_golden_leg_reddens_on_violation_input` — cross-input
//!   controls through the REAL binary: each predicate fails on the other's
//!   input (plus a same-input control proving the input was genuine), so
//!   neither side of the suite is vacuous. No stub stands in for the hook.
//! - `kill_drill_100_runs_no_partial_state` — seeded SIGKILL at randomized
//!   points via kill/reap/descendant-sweep/drain: fixture files (INCLUDING
//!   the firing ledger) byte-identical, ledger parses clean via the real
//!   `query_gate` parser, kill/early-exit counts published (an all-early
//!   drill fails for proving nothing), next invocation healthy.
//! - `latency_cold_warm_budget` — GENUINE states: 5 cold samples (fresh
//!   binary path + fresh repo each) vs 5 warm repeats; medians + ranges
//!   printed with counts; every sample under the fixture hang-guard budget.
//!   No cold/warm ordering asserted (not guaranteed; would flake).
//! - `scratch_home_install_self_test` — binary runs from a fake HOME with
//!   identical decisions (HOME-independence, no settings.json involved).
//! - `scratch_home_install_is_backup_first_and_idempotent` — pre-existing
//!   hook preserved byte-identical beside the install; reinstall changes
//!   nothing (bytes, backup, decisions); reinstalled hook still refuses.
//! - `non_repo_cwd_refuses_typed_exit_2` — outside a repo: exit 2, typed,
//!   never a panic.
//!
//! LIVE-HOOK INVARIANCE. Every leg stages through `stage_hook_binary`,
//! which records each live observation and fails the staging leg on any
//! drift from the first (sha+mtime both named). No ordering assumption:
//! every observation is checked whenever it lands. The live file is only
//! ever read (copy + fingerprint); the suite contains no write to it.
//!
//! EXECUTION MODEL. Every leg executes a SCRATCH COPY of the binary, never
//! the live path. Subprocess control is delegated to the
//! `subprocess-contract` kernel (group-leader child, group-targeted
//! TERM-then-KILL, both pipes drained, child reaped) -- no hand-rolled
//! spawn+poll exists here for the undrained-pipe lint to refuse. All
//! fixture repos live in per-test TempDirs with isolated `.git` state, so
//! the gate-section lock, the firing ledger, and the index never touch a
//! real checkout. Fixture `git commit` messages carry a standalone `[test]`
//! level so the template commit-msg hook passes on Mac lanes and is a no-op
//! elsewhere. forbid(unsafe_code) holds for the whole file.

use no_shell_gate::firing_ledger::{default_ledger_path, query_gate};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Fixture budget per hook invocation (measured 0.6-2.6s on a minimal
/// fixture 2026-09-16): a hang guard, not a product SLO. The hook runs the
/// full staged gate battery, so this is deliberately generous.
const INVOCATION_BUDGET: Duration = Duration::from_secs(30);
/// Fuzz cases must each complete inside this bound (the no-hang pin).
const FUZZ_CASE_BUDGET: Duration = Duration::from_secs(90);
/// Kill-injection drill volume per the Stage-3 contract (>= 100 runs).
const KILL_RUNS: usize = 100;
/// Max randomized kill delay; covers the measured clean runtime several times over.
const KILL_DELAY_MAX_MS: u64 = 1200;
/// Deterministic drill seed: the same seed replays the same kill schedule.
const KILL_SEED: u64 = 0x7c37;

/// Golden receipt: exit 0 with exactly one CLEAN outcome marker.
fn golden_holds(code: Option<i32>, stderr: &str) -> bool {
    code == Some(0)
        && stderr.contains("CLEAN: all staged files passed the multi-gate checks")
        && stderr
            .lines()
            .filter(|line| {
                [
                    "CLEAN:",
                    "VIOLATION:",
                    "NOTHING_TO_CHECK:",
                    "ANCESTRY_ONLY_MERGE:",
                ]
                .iter()
                .any(|marker| line.starts_with(marker))
            })
            .count()
            == 1
}

/// Violation receipt for `name`: exit 1, VIOLATION marker, the load-bearing
/// exemption sentence, and the filename (proves the refusal is ABOUT our
/// file rather than a vacuous refusal from another gate).
fn violation_holds(code: Option<i32>, stderr: &str, name: &str) -> bool {
    code == Some(1)
        && stderr.contains("VIOLATION:")
        && stderr.contains("the exemption list is empty by design")
        && stderr.contains(name)
}

fn sha256_hex_file(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read file for fingerprint");
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}").expect("hex write");
    }
    hex
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live beneath the workspace root")
        .to_path_buf()
}

fn live_hook_path() -> PathBuf {
    workspace_root().join(".git/hooks/pre-commit")
}

/// Fingerprint of the live hook for the run record. `None` when this lane
/// has no installed hook (workers, CI): legs needing live bytes SKIP loudly.
fn live_fingerprint() -> Option<(String, String)> {
    let path = live_hook_path();
    if !path.is_file() {
        return None;
    }
    let metadata = std::fs::metadata(&path).expect("live hook metadata");
    let mtime = metadata
        .modified()
        .expect("live hook mtime")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("mtime after epoch")
        .as_secs()
        .to_string();
    Some((sha256_hex_file(&path), mtime))
}

/// First live-hook fingerprint observed by this suite process, shared by
/// every leg through `stage_hook_binary` (the single funnel: all legs stage
/// through it). Later observations must equal it byte-for-byte, so a live
/// hook that changes mid-suite -- another pane reinstalling it, a heal,
/// clock skew on mtime -- fails at the staging call of the leg that sees
/// the drift, with both fingerprints named. No ordering assumption: every
/// observation is checked against the first, whenever it lands. Lanes
/// without an installed hook never record (fresh-build path) and check
/// nothing -- there is no artifact to drift.
static LIVE_FIRST_SEEN: std::sync::Mutex<Option<(String, String)>> =
    std::sync::Mutex::new(None);

/// Record a live-hook observation, enforcing pairwise stability against the
/// first observation in this process. Panics naming both fingerprints on
/// drift. Structure-only: the mutex is never held across an assertion.
fn record_live_observation(sha: &str, mtime: &str, case: &str) {
    let observed = (sha.to_owned(), mtime.to_owned());
    let mut guard = LIVE_FIRST_SEEN.lock().expect("live record lock");
    match guard.as_ref().cloned() {
        None => {
            *guard = Some(observed);
        }
        Some(first) => assert_eq!(
            first, observed,
            "LIVE HOOK DRIFTED mid-suite during {case}: first sha={} mtime={} now sha={} mtime={}",
            first.0, first.1, sha, mtime
        ),
    }
}

/// Stage the binary under test into `dir`: a copy of the LIVE bytes when the
/// installed hook exists here, else a fresh build. Returns the staged path
/// plus a provenance line printed into every leg that matters. Every live
/// observation is recorded for pairwise stability: a hook that changes
/// mid-suite fails here, not silently downstream.
fn stage_hook_binary(dir: &Path) -> (PathBuf, String) {
    let live = live_hook_path();
    let staged = dir.join("hook-under-test");
    if live.is_file() {
        std::fs::copy(&live, &staged).expect("copy live hook bytes");
        let (sha, mtime) = live_fingerprint().expect("live fingerprint of present hook");
        record_live_observation(&sha, &mtime, "stage_hook_binary");
        let provenance = format!("ARTIFACT_UNDER_TEST source=live sha={sha}");
        println!("{provenance}");
        assert_eq!(
            sha256_hex_file(&staged),
            sha,
            "staged bytes must equal live bytes at stage time"
        );
        (staged, provenance)
    } else {
        println!("SKIP live-hook lane: no installed hook; using fresh build (provenance below)");
        std::fs::copy(env!("CARGO_BIN_EXE_pre-commit-gate"), &staged)
            .expect("copy fresh hook build");
        let provenance = format!(
            "ARTIFACT_UNDER_TEST source=fresh-build sha={}",
            sha256_hex_file(&staged)
        );
        println!("{provenance}");
        (staged, provenance)
    }
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed in fixture");
}

/// Fixture repo the real hook accepts: base + hypotheses commits with
/// `[test]` levels (the template commit-msg hook enforces them on Mac
/// lanes), the installed hook copy (freshness would otherwise refuse), and
/// the real grader file copied from this checkout (validity by
/// construction, always current, never transcribed).
fn behavior_repo(scratch: &Path) -> PathBuf {
    let repo = scratch.join("repo");
    std::fs::create_dir_all(&repo).expect("fixture repo dir");
    run_git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("README.md"), b"baseline\n").expect("baseline file");
    std::fs::create_dir_all(repo.join(".omp/agents")).expect("grader dir");
    std::fs::copy(
        workspace_root().join(".omp/agents/omp-grader.md"),
        repo.join(".omp/agents/omp-grader.md"),
    )
    .expect("grader file must exist in this checkout");
    run_git(
        &repo,
        &["add", "--", "README.md", ".omp/agents/omp-grader.md"],
    );
    run_git(
        &repo,
        &[
            "-c",
            "user.email=stage3@local",
            "-c",
            "user.name=stage3",
            "commit",
            "-qm",
            "[test] fixture base",
        ],
    );
    let parent = String::from_utf8_lossy(
        &Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read base head")
            .stdout,
    )
    .trim()
    .to_owned();
    std::fs::create_dir_all(repo.join("docs/plan")).expect("plan dir");
    std::fs::write(
        repo.join("docs/plan/HYPOTHESES.jsonl"),
        format!(
            "{{\"id\":\"h1\",\"prediction\":\"fixture\",\"falsifier\":\"missing section\",\"evidence_scope\":\"docs/plan\",\"recorded_commit\":\"{parent}\",\"observed_result\":null}}\n"
        ),
    )
    .expect("hypotheses fixture");
    run_git(&repo, &["add", "--", "docs/plan/HYPOTHESES.jsonl"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.email=stage3@local",
            "-c",
            "user.name=stage3",
            "commit",
            "-qm",
            "[test] hypothesis fixture",
        ],
    );
    repo
}
/// Assert no process in the tree runs the staged binary path anymore: the
/// kill drill's survivor proof. `pgrep -f` matches the full command line, and
/// the staged path is unique per suite run, so a hit is unambiguously ours.
/// Grandchildren (git) inherit nothing matchable, but they die with the group
/// the kernel signalled -- and a hook that outlives its own kill would hold
/// the gate lock and fail the restore assertions below anyway.
fn assert_no_staged_strays(staged: &Path, case: &str) {
    let output = Command::new("pgrep")
        .args(["-f", &staged.display().to_string()])
        .output()
        .expect("pgrep must exist on Mac and Linux lanes");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.trim().is_empty(),
        "HANG-PIN {case}: staged binary survives after kill: {}",
        text.trim()
    );
}

/// Bounded run: the no-hang pin, delegated to the `subprocess-contract`
/// kernel (group-leader child, group-targeted TERM-then-KILL, both pipes
/// drained, child reaped -- never detached). A timeout is a leg FAILURE
/// with the case named, never a silent skip; the kernel owns the kill, so
/// there is no hand-rolled spawn+poll here for the undrained-pipe lint to
/// (correctly) refuse.
fn run_hook_bounded(
    bin: &Path,
    repo: &Path,
    args: &[&str],
    extra_env: &[(&str, String)],
    budget: Duration,
    case: &str,
) -> (Option<i32>, String, String) {
    let mut command = Command::new(bin);
    command.current_dir(repo).args(args);
    for (key, value) in extra_env {
        command.env(key, value);
    }
    match subprocess_contract::bounded_output(&mut command, budget) {
        subprocess_contract::BoundedOutcome::Completed(output) => split_output(&output),
        subprocess_contract::BoundedOutcome::TimedOut => panic!(
            "HANG-PIN {case}: hook did not complete in {}s; kernel signalled the process group",
            budget.as_secs()
        ),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            panic!("HANG-PIN {case}: spawn failed: {error}")
        }
    }
}

fn snapshot_tree_files(repo: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let mut stack = vec![repo.to_owned()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).expect("read fixture dir");
        for entry in entries {
            let entry = entry.expect("fixture dir entry");
            let path = entry.path();
            if path.is_dir() {
                // The firing ledger is load-bearing kill-drill state: a torn
                // ledger line is exactly the partial write this snapshot must
                // catch. Everything else under .git (objects, index, hooks)
                // is hook-execution residue, not fixture content.
                if path.file_name().is_some_and(|name| name == ".git") {
                    let ledger = path.join("omp-gate-firings.jsonl");
                    if ledger.is_file() {
                        files.insert(
                            ".git/omp-gate-firings.jsonl".to_owned(),
                            sha256_hex_file(&ledger),
                        );
                    }
                    continue;
                }
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(repo)
                .expect("fixture-relative path")
                .display()
                .to_string();
            files.insert(rel, sha256_hex_file(&path));
        }
    }
    files
}

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn live_hook_fingerprinted_for_the_run_record() {
    match live_fingerprint() {
        Some((sha, mtime)) => {
            let size = std::fs::metadata(live_hook_path())
                .expect("live hook metadata")
                .len();
            assert!(
                size > 100_000,
                "live hook must be a real binary, not a placeholder: {size} bytes"
            );
            println!("LIVE_HOOK sha256={sha} mtime_epoch={mtime} size={size}");
        }
        None => println!("SKIP live-hook lane: no installed hook at least one leg needs live bytes; suite uses fresh builds with provenance per leg"),
    }
}

#[test]
fn artifact_under_test_matches_live_bytes_when_present() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (staged, provenance) = stage_hook_binary(scratch.path());
    assert!(staged.is_file(), "staged binary must exist: {provenance}");
    if let Some((live_sha, _)) = live_fingerprint() {
        assert_eq!(
            sha256_hex_file(&staged),
            live_sha,
            "staged bytes must equal live bytes: {provenance}"
        );
    }
}

#[test]
fn golden_clean_exits_zero_with_single_outcome() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "docs/note.md", b"hello\n");
    let output = run_hook(&bin, &repo, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        golden_holds(code, &stderr),
        "golden fixture must exit 0 with one CLEAN marker [{provenance}]: code={code:?} stderr={stderr}"
    );
    println!("READBACK golden exit=0 single-CLEAN [{provenance}]");
}

#[test]
fn known_bad_shell_fires_violation_exit_1() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "check.sh", b"#!/bin/sh\necho hi\n");
    let output = run_hook(&bin, &repo, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        violation_holds(code, &stderr, "check.sh"),
        "staged shell must refuse exit 1 naming the file [{provenance}]: code={code:?} stderr={stderr}"
    );
    println!("READBACK violation exit=1 names check.sh [{provenance}]");
}

#[test]
fn empty_staged_exits_3_nothing_to_check() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    let output = run_hook(&bin, &repo, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert_eq!(
        code,
        Some(3),
        "empty staged set must exit 3 [{provenance}]: {stderr}"
    );
    assert!(
        stderr.contains("NOTHING_TO_CHECK"),
        "empty run must name the outcome [{provenance}]: {stderr}"
    );
}

#[test]
fn malformed_editmsg_refuses_typed_exit_1() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    let missing = scratch.path().join("COMMIT_EDITMSG");
    let output = run_hook(&bin, &repo, &[&missing.display().to_string()], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert_eq!(
        code,
        Some(1),
        "missing COMMIT_EDITMSG must refuse exit 1 [{provenance}]: {stderr}"
    );
    assert!(
        stderr.contains("cannot read COMMIT_EDITMSG"),
        "refusal must name the unreadable path [{provenance}]: {stderr}"
    );
}

#[test]
fn msg_mode_matching_source_allows_exit_0() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    let editmsg = scratch.path().join("COMMIT_EDITMSG");
    let source = scratch.path().join("msg-src.txt");
    std::fs::write(&editmsg, b"fix: probe [test]\n").expect("editmsg fixture");
    std::fs::write(&source, b"fix: probe [test]\n").expect("message source");
    let source_arg = source.display().to_string();
    let output = run_hook(
        &bin,
        &repo,
        &[&editmsg.display().to_string()],
        &[("OMP_MSG_SRC", source_arg.as_str())],
    );
    let (code, _stdout, stderr) = split_output(&output);
    assert_eq!(
        code,
        Some(0),
        "matching message source must allow [{provenance}]: {stderr}"
    );
    assert!(
        !source.exists(),
        "accepted source is consumed by the round trip [{provenance}]"
    );
}

#[test]
fn unknown_argv_never_panics_nor_hangs() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "docs/note.md", b"hello\n");
    let long = "x".repeat(8000);
    let cases: [&str; 9] = [
        "",
        "--help",
        "-h",
        "--frobnicate",
        "COMMIT_EDITMSG",
        "/nonexistent/COMMIT_EDITMSG",
        long.as_str(),
        "commit_editmsg",
        "--",
    ];
    for (index, argv) in cases.into_iter().enumerate() {
        let case = format!("argv[{index}]={argv:.40}");
        let (code, _stdout, stderr) =
            run_hook_bounded(&bin, &repo, &[argv], &[], FUZZ_CASE_BUDGET, &case);
        assert!(
            matches!(code, Some(0) | Some(1) | Some(2) | Some(3)),
            "HANG-PIN {case}: exit must be a typed outcome, never a panic or hang [{provenance}]: code={code:?} stderr={stderr}"
        );
        if code != Some(0) {
            assert!(
                !stderr.is_empty(),
                "HANG-PIN {case}: a refusal must explain itself [{provenance}]"
            );
        }
    }
    println!("READBACK 9 argv cases typed-or-clean [{provenance}]");
}

#[test]
fn staged_tree_fuzz_never_panics() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let big = "z".repeat(2 * 1024 * 1024);
    let cases: [(&str, Vec<u8>); 6] = [
        ("docs/empty.md", Vec::new()),
        ("docs/big.md", big.into_bytes()),
        ("docs/no-newline.md", b"no trailing newline".to_vec()),
        ("docs/ünïcode.md", "unicode content\n".as_bytes().to_vec()),
        ("docs/a/b/c/d/e/f.md", b"deep\n".to_vec()),
        ("my check.sh", b"#!/bin/sh\necho spaced\n".to_vec()),
    ];
    for (index, (rel, bytes)) in cases.iter().enumerate() {
        let rel: &str = rel;
        let case = format!("staged[{index}]={rel}");
        let repo = behavior_repo(&scratch.path().join(format!("fuzz-{index}")));
        install_fixture_hook(&repo, &bin);
        stage_file(&repo, rel, bytes);
        let (code, _stdout, stderr) =
            run_hook_bounded(&bin, &repo, &[], &[], FUZZ_CASE_BUDGET, &case);
        assert!(
            matches!(code, Some(0) | Some(1) | Some(3)),
            "HANG-PIN {case}: staged content must decide typed, never panic [{provenance}]: code={code:?} stderr={stderr}"
        );
        if rel.ends_with(".sh") {
            assert!(
                code == Some(1) && stderr.contains(rel),
                "HANG-PIN {case}: staged shell must refuse naming itself [{provenance}]: code={code:?}"
            );
        }
    }
    println!("READBACK 6 staged-tree cases typed-or-clean [{provenance}]");
}

/// Satisfiability through the ACTUAL suite decision path: the violation leg
/// fed golden input (nothing staged to refuse) must NOT satisfy the
/// violation predicate. A leg that stays satisfied with nothing to refuse
/// about cannot RED on any input -- vacuous. The hook binary really runs;
/// no stub stands in for it.
#[test]
fn satisfiability_violation_leg_reddens_on_golden_input() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "docs/note.md", b"hello\n");
    let output = run_hook(&bin, &repo, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        !violation_holds(code, &stderr, "check.sh"),
        "golden input through the real binary must NOT satisfy the violation predicate (else the leg is vacuous) [{provenance}]: code={code:?}"
    );
    assert!(
        golden_holds(code, &stderr),
        "control: the same run satisfies the golden predicate, so the input was genuinely golden [{provenance}]"
    );
    println!("READBACK violation leg reddens on golden input via real binary [{provenance}]");
}

/// Mirror image: the golden leg fed violation input (a staged shell) must
/// NOT satisfy the golden predicate, through the same real binary.
#[test]
fn satisfiability_golden_leg_reddens_on_violation_input() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "check.sh", b"#!/bin/sh\necho hi\n");
    let output = run_hook(&bin, &repo, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        !golden_holds(code, &stderr),
        "violation input through the real binary must NOT satisfy the golden predicate (else the leg is vacuous) [{provenance}]: code={code:?}"
    );
    assert!(
        violation_holds(code, &stderr, "check.sh"),
        "control: the same run satisfies the violation predicate, so the input was genuinely violating [{provenance}]"
    );
    println!("READBACK golden leg reddens on violation input via real binary [{provenance}]");
}

#[test]
fn kill_drill_100_runs_no_partial_state_next_run_healthy() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "check.sh", b"#!/bin/sh\necho hi\n");
    let before = snapshot_tree_files(&repo);
    // Each run executes through the kernel deadline path: the delay doubles
    // as the kill schedule, so every SIGKILL is delivered by
    // `bounded_output`'s group-targeted TERM-then-KILL, never by a handrolled
    // kill in this file. Completed vs TimedOut are counted separately so a
    // degenerate all-early drill cannot masquerade as kill coverage.
    let mut kills_landed = 0usize;
    let mut early_exits = 0usize;
    for run in 0..KILL_RUNS {
        let delay_ms = xorshift(&mut seed) % KILL_DELAY_MAX_MS;
        let mut command = Command::new(&bin);
        command.current_dir(&repo);
        match subprocess_contract::bounded_output(
            &mut command,
            Duration::from_millis(delay_ms),
        ) {
            subprocess_contract::BoundedOutcome::Completed(_) => {
                early_exits += 1;
            }
            subprocess_contract::BoundedOutcome::TimedOut => {
                kills_landed += 1;
            }
            subprocess_contract::BoundedOutcome::Unspawned(error) => {
                panic!("drill run {run}: spawn failed: {error}");
            }
        }
    }
    assert!(
        kills_landed > 0,
        "KILL-DRILL: zero kills landed in {KILL_RUNS} runs (all early exits); the drill proved nothing [{provenance}]"
    );
    println!("READBACK kill drill {kills_landed} kernel kills landed, {early_exits} early exits [{provenance}]");
    // No survivor from any of the 100 runs may still walk the tree: the
    // staged path is unique per suite run, so any hit is unambiguously ours.
    assert_no_staged_strays(&bin, "kill-drill-sweep");
    // No partial fixture state: every non-git file byte-identical.
    let after = snapshot_tree_files(&repo);
    assert_eq!(
        before, after,
        "KILL-DRILL: 100 SIGKILLs must leave fixture content byte-identical [{provenance}]"
    );
    // The firing ledger parses clean through the REAL parser: a torn
    // trailing line ERRORS here, which is the partial-write detector.
    let ledger = default_ledger_path(&repo);
    if ledger.is_file() {
        query_gate(&ledger, "no-shell-gate").expect("KILL-DRILL: ledger must parse clean");
    }
    // Next invocation healthy with retry discipline: a stale gate-section
    // lock from our own kills may need one beat to age out, exactly as the
    // operator retry_refusal prescribes. Eventual expected outcome or RED.
    let mut healthy = false;
    for _attempt in 0..10 {
        let output = run_hook(&bin, &repo, &[], &[]);
        let (code, _stdout, stderr) = split_output(&output);
        if violation_holds(code, &stderr, "check.sh") {
            healthy = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(
        healthy,
        "KILL-DRILL: violation behavior must restore after the drill [{provenance}]"
    );
    let golden_repo = behavior_repo(&scratch.path().join("post-drill"));
    install_fixture_hook(&golden_repo, &bin);
    stage_file(&golden_repo, "docs/note.md", b"hello\n");
    let output = run_hook(&bin, &golden_repo, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        golden_holds(code, &stderr),
        "KILL-DRILL: golden behavior must hold on a fresh repo after the drill [{provenance}]"
    );
    println!("READBACK kill drill 100 runs clean, behavior restored [{provenance}]");
}

/// Cold-vs-warm latency with GENUINE states, not labels on one population:
/// every cold sample executes a FRESHLY COPIED binary at a fresh path
/// (cold dentry/inode/page for that path) against a fresh repo, while warm
/// samples repeat the SAME binary and repo. Reported statistic is the median
/// of each group (5 cold + 5 warm); the budget assertion applies per sample
/// as a hang guard. NO-CLAIM inside the test's honesty bounds: without
/// cache-drop privileges the OS page cache warms across samples, so "cold"
/// here means cold-path, not cold-machine -- and the leg asserts budget
/// compliance per sample, never a cold/warm ordering (ordering is not
/// guaranteed and asserting it would be a flaky gate).
#[test]
fn latency_cold_warm_budget() {
    const COLD_SAMPLES: usize = 5;
    const WARM_SAMPLES: usize = 5;
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let mut cold: Vec<Duration> = Vec::with_capacity(COLD_SAMPLES);
    for sample in 0..COLD_SAMPLES {
        // Fresh path AND fresh repo per cold sample: nothing about this
        // invocation has executed before on this machine state.
        let fresh_bin = scratch
            .path()
            .join(format!("hook-cold-{sample}"));
        std::fs::copy(&bin, &fresh_bin).expect("fresh cold binary copy");
        let repo = behavior_repo(&scratch.path().join(format!("cold-repo-{sample}")));
        install_fixture_hook(&repo, &fresh_bin);
        stage_file(&repo, "docs/note.md", b"hello\n");
        let start = Instant::now();
        let output = run_hook(&fresh_bin, &repo, &[], &[]);
        let elapsed = start.elapsed();
        let (code, _stdout, _stderr) = split_output(&output);
        assert_eq!(
            code,
            Some(0),
            "cold sample {sample} must exit clean [{provenance}]"
        );
        cold.push(elapsed);
    }
    let repo = behavior_repo(&scratch.path().join("warm-repo"));
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "docs/note.md", b"hello\n");
    let mut warm: Vec<Duration> = Vec::with_capacity(WARM_SAMPLES);
    for sample in 0..WARM_SAMPLES {
        let start = Instant::now();
        let output = run_hook(&bin, &repo, &[], &[]);
        let elapsed = start.elapsed();
        let (code, _stdout, _stderr) = split_output(&output);
        assert_eq!(
            code,
            Some(0),
            "warm sample {sample} must exit clean [{provenance}]"
        );
        warm.push(elapsed);
    }
    cold.sort();
    warm.sort();
    let cold_median = cold[COLD_SAMPLES / 2];
    let warm_median = warm[WARM_SAMPLES / 2];
    println!("LATENCY cold_n={COLD_SAMPLES} cold_median={cold_median:?} cold_min={:?} cold_max={:?} warm_n={WARM_SAMPLES} warm_median={warm_median:?} warm_min={:?} warm_max={:?} [{provenance}]", cold[0], cold[COLD_SAMPLES - 1], warm[0], warm[WARM_SAMPLES - 1]);
    for (group, samples) in [("cold", &cold), ("warm", &warm)] {
        for (index, sample) in samples.iter().enumerate() {
            assert!(
                *sample < INVOCATION_BUDGET,
                "latency {group}[{index}] exceeds fixture budget {INVOCATION_BUDGET:?}: {sample:?} [{provenance}]"
            );
        }
    }
}

#[test]
fn scratch_home_install_self_test() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let home = scratch.path().join("fake-home");
    let bin_dir = home.join(".local/bin");
    std::fs::create_dir_all(&bin_dir).expect("fake home bin dir");
    let installed = bin_dir.join("hook-under-test");
    std::fs::copy(&bin, &installed).expect("install into fake home");
    let home_arg = home.display().to_string();
    let golden_repo = behavior_repo(&scratch.path().join("home-golden"));
    install_fixture_hook(&golden_repo, &installed);
    stage_file(&golden_repo, "docs/note.md", b"hello\n");
    let output = run_hook(
        &installed,
        &golden_repo,
        &[],
        &[("HOME", home_arg.as_str())],
    );
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        golden_holds(code, &stderr),
        "fake-HOME install must decide golden identically [{provenance}]"
    );
    let bad_repo = behavior_repo(&scratch.path().join("home-bad"));
    install_fixture_hook(&bad_repo, &installed);
    stage_file(&bad_repo, "check.sh", b"#!/bin/sh\necho hi\n");
    let output = run_hook(&installed, &bad_repo, &[], &[("HOME", home_arg.as_str())]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        violation_holds(code, &stderr, "check.sh"),
        "fake-HOME install must decide violation identically [{provenance}]"
    );
    println!("READBACK fake-HOME install decides identically [{provenance}]");
}

/// Backup-first wiring merge plus idempotent reinstall, still under a fake
/// HOME: a pre-existing hook is preserved byte-identical beside the install
/// (never overwritten in place), and running the install twice changes
/// nothing the second time -- same bytes, same backup, same decisions.
/// Fixture machinery for the install discipline (mirrors production
/// install-then-verify); the decisions it installs are asserted, not the
/// helper itself.
#[test]
fn scratch_home_install_is_backup_first_and_idempotent() {
    fn install_backup_first(source: &Path, dest: &Path) -> PathBuf {
        if dest.is_file() {
            let backup = dest.with_extension("pre-stage3.bak");
            if !backup.is_file() {
                std::fs::copy(dest, &backup).expect("back up pre-existing hook");
            }
            let before = sha256_hex_file(dest);
            assert_eq!(
                sha256_hex_file(&backup),
                before,
                "backup must preserve the pre-existing hook byte-identical"
            );
        }
        std::fs::copy(source, dest).expect("install hook bytes");
        dest.to_owned()
    }
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let home = scratch.path().join("fake-home-backup");
    let bin_dir = home.join(".local/bin");
    std::fs::create_dir_all(&bin_dir).expect("fake home bin dir");
    let installed = bin_dir.join("hook-under-test");
    // A foreign pre-existing hook the install must not destroy.
    std::fs::write(&installed, b"#!/bin/sh\necho foreign hook\n").expect("foreign hook");
    install_backup_first(&bin, &installed);
    let backup = bin_dir.join("hook-under-test.pre-stage3.bak");
    assert_eq!(
        std::fs::read(&backup).expect("backup bytes"),
        b"#!/bin/sh\necho foreign hook\n",
        "backup-first must preserve the foreign hook [{provenance}]"
    );
    assert_eq!(
        sha256_hex_file(&installed),
        sha256_hex_file(&bin),
        "installed bytes must equal staged bytes [{provenance}]"
    );
    // Second install: idempotent -- bytes, backup, and decisions unchanged.
    install_backup_first(&bin, &installed);
    assert_eq!(
        sha256_hex_file(&installed),
        sha256_hex_file(&bin),
        "reinstall must leave identical bytes [{provenance}]"
    );
    assert_eq!(
        std::fs::read(&backup).expect("backup bytes after reinstall"),
        b"#!/bin/sh\necho foreign hook\n",
        "reinstall must not touch the backup [{provenance}]"
    );
    let home_arg = home.display().to_string();
    let repo = behavior_repo(&scratch.path().join("home-reinstall"));
    install_fixture_hook(&repo, &installed);
    stage_file(&repo, "check.sh", b"#!/bin/sh\necho hi\n");
    let output = run_hook(&installed, &repo, &[], &[("HOME", home_arg.as_str())]);
    let (code, _stdout, stderr) = split_output(&output);
    assert!(
        violation_holds(code, &stderr, "check.sh"),
        "reinstalled hook must still refuse identically [{provenance}]"
    );
    println!("READBACK backup-first install + idempotent reinstall [{provenance}]");
}

#[test]
fn non_repo_cwd_refuses_typed_exit_2() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let bare = scratch.path().join("not-a-repo");
    std::fs::create_dir_all(&bare).expect("bare dir");
    let output = run_hook(&bin, &bare, &[], &[]);
    let (code, _stdout, stderr) = split_output(&output);
    assert_eq!(
        code,
        Some(2),
        "outside a repo the hook must fail closed typed [{provenance}]: {stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "the operational refusal must explain itself [{provenance}]"
    );
    assert!(
        !stderr.contains("panicked"),
        "refusal must be typed, never a panic [{provenance}]: {stderr}"
    );
}
