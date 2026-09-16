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
//! - `satisfiability_always_pass_stub_reddens_violation_leg` — /usr/bin/true
//!   fails the violation predicate: the leg can return the other answer.
//! - `satisfiability_always_refuse_stub_reddens_golden_leg` — /usr/bin/false
//!   fails the golden predicate. Together the pair proves neither side of the
//!   suite is vacuous (a gate that cannot RED is decoration).
//! - `kill_drill_100_runs_no_partial_state` — seeded SIGKILL at randomized
//!   points: fixture files byte-identical, firing ledger parses clean via the
//!   real `query_gate` parser (a torn line ERRORS), next invocation healthy
//!   (violation exit restored, then golden exit restored).
//! - `latency_cold_warm_budget` — cold + warm timings printed, every run
//!   under the documented fixture budget (hang guard, not a product SLO).
//! - `scratch_home_install_self_test` — binary runs from a fake HOME with
//!   identical decisions (HOME-independence, no settings.json involved).
//! - `non_repo_cwd_refuses_typed_exit_2` — outside a repo: exit 2, typed,
//!   never a panic.
//!
//! EXECUTION MODEL. Every leg executes a SCRATCH COPY of the binary, never
//! the live path: the live file is opened read-only (copy + fingerprint)
//! and its mtime/sha are asserted unchanged only procedurally (before/after
//! the run, pasted into the run record) because mtime is a filesystem fact
//! no in-test assertion can bracket across parallel legs. All fixture repos
//! live in per-test TempDirs with isolated `.git` state, so the gate-section
//! lock, the firing ledger, and the index never touch a real checkout.
//! Fixture `git commit` messages carry a standalone `[test]` level so the
//! template commit-msg hook passes on Mac lanes and is a no-op elsewhere.

use no_shell_gate::firing_ledger::{default_ledger_path, query_gate};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
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

/// Stage the binary under test into `dir`: a copy of the LIVE bytes when the
/// installed hook exists here, else a fresh build. Returns the staged path
/// plus a provenance line printed into every leg that matters.
fn stage_hook_binary(dir: &Path) -> (PathBuf, String) {
    let live = live_hook_path();
    let staged = dir.join("hook-under-test");
    if live.is_file() {
        std::fs::copy(&live, &staged).expect("copy live hook bytes");
        let provenance = format!(
            "ARTIFACT_UNDER_TEST source=live sha={}",
            sha256_hex_file(&staged)
        );
        println!("{provenance}");
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

/// Install the staged binary as this fixture's hook (mirrors the production
/// layout; silences freshness legitimately with a real installation).
fn install_fixture_hook(repo: &Path, staged: &Path) {
    let hook = repo.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().expect("hook parent")).expect("hook dir");
    std::fs::copy(staged, &hook).expect("install fixture hook");
}

fn stage_file(repo: &Path, rel: &str, bytes: &[u8]) {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fixture parent dir");
    }
    std::fs::write(&path, bytes).expect("write staged fixture");
    run_git(repo, &["add", "--", rel]);
}

fn run_hook(bin: &Path, repo: &Path, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(bin);
    command.current_dir(repo).args(args);
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command.output().expect("spawn hook binary")
}

fn split_output(output: &Output) -> (Option<i32>, String, String) {
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Bounded run: the no-hang pin. A timeout is a leg FAILURE with the case
/// named, never a silent skip; the stray child is documented, not hidden.
fn run_hook_bounded(
    bin: &Path,
    repo: &Path,
    args: &[&str],
    extra_env: &[(&str, String)],
    budget: Duration,
    case: &str,
) -> (Option<i32>, String, String) {
    let owned_bin = bin.to_owned();
    let owned_repo = repo.to_owned();
    let owned_args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let owned_env: Vec<(String, String)> = extra_env
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut command = Command::new(&owned_bin);
        command.current_dir(&owned_repo);
        command.args(&owned_args);
        for (key, value) in &owned_env {
            command.env(key, value);
        }
        let _ = sender.send(command.output());
    });
    match receiver.recv_timeout(budget) {
        Ok(Ok(output)) => split_output(&output),
        Ok(Err(error)) => panic!("HANG-PIN {case}: spawn failed: {error}"),
        Err(_) => panic!(
            "HANG-PIN {case}: hook did not complete in {}s (stray child abandoned and documented)",
            budget.as_secs()
        ),
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
                if path.file_name().is_some_and(|name| name == ".git") {
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

#[test]
fn satisfiability_always_pass_stub_reddens_violation_leg() {
    for candidate in ["/usr/bin/true", "/bin/true"] {
        if Path::new(candidate).is_file() {
            let scratch = tempfile::tempdir().expect("scratch dir");
            let repo = behavior_repo(scratch.path());
            stage_file(&repo, "check.sh", b"#!/bin/sh\necho hi\n");
            let output = run_hook(Path::new(candidate), &repo, &[], &[]);
            let (code, _stdout, stderr) = split_output(&output);
            assert!(
                !violation_holds(code, &stderr, "check.sh"),
                "an always-pass binary must NOT satisfy the violation predicate (else the leg is vacuous)"
            );
            println!("READBACK always-pass stub fails violation predicate via {candidate}");
            return;
        }
    }
    panic!("no true(1) binary found for the satisfiability control");
}

#[test]
fn satisfiability_always_refuse_stub_reddens_golden_leg() {
    for candidate in ["/usr/bin/false", "/bin/false"] {
        if Path::new(candidate).is_file() {
            let scratch = tempfile::tempdir().expect("scratch dir");
            let repo = behavior_repo(scratch.path());
            stage_file(&repo, "docs/note.md", b"hello\n");
            let output = run_hook(Path::new(candidate), &repo, &[], &[]);
            let (code, _stdout, stderr) = split_output(&output);
            assert!(
                !golden_holds(code, &stderr),
                "an always-refuse binary must NOT satisfy the golden predicate (else the leg is vacuous)"
            );
            println!("READBACK always-refuse stub fails golden predicate via {candidate}");
            return;
        }
    }
    panic!("no false(1) binary found for the satisfiability control");
}

#[test]
fn kill_drill_100_runs_no_partial_state_next_run_healthy() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "check.sh", b"#!/bin/sh\necho hi\n");
    let before = snapshot_tree_files(&repo);
    let mut seed = KILL_SEED;
    for run in 0..KILL_RUNS {
        let delay_ms = xorshift(&mut seed) % KILL_DELAY_MAX_MS;
        let mut child: Child = Command::new(&bin)
            .current_dir(&repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("drill run {run}: spawn failed: {error}"));
        std::thread::sleep(Duration::from_millis(delay_ms));
        child.kill().unwrap_or_else(|error| {
            if error.kind() == std::io::ErrorKind::InvalidInput {
                // Already exited before the kill landed: a valid sample of
                // the early-exit path, not a drill failure.
            } else {
                panic!("drill run {run}: kill failed: {error}");
            }
        });
        let _ = child.wait();
    }
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

#[test]
fn latency_cold_warm_budget() {
    let scratch = tempfile::tempdir().expect("scratch dir");
    let (bin, provenance) = stage_hook_binary(scratch.path());
    let repo = behavior_repo(scratch.path());
    install_fixture_hook(&repo, &bin);
    stage_file(&repo, "docs/note.md", b"hello\n");
    let mut samples = Vec::new();
    for run in 0..6 {
        let start = Instant::now();
        let output = run_hook(&bin, &repo, &[], &[]);
        let elapsed = start.elapsed();
        let (code, _stdout, _stderr) = split_output(&output);
        assert_eq!(
            code,
            Some(0),
            "latency sample {run} must exit clean [{provenance}]"
        );
        samples.push(elapsed);
    }
    samples.sort();
    let cold = samples[5];
    let warm_median = samples[2];
    println!("LATENCY cold={cold:?} warm_median={warm_median:?} all={samples:?}");
    for (index, sample) in samples.iter().enumerate() {
        assert!(
            *sample < INVOCATION_BUDGET,
            "latency sample {index} exceeds fixture budget {INVOCATION_BUDGET:?}: {sample:?} [{provenance}]"
        );
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
