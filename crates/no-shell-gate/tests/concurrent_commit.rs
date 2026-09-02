#![forbid(unsafe_code)]
//! END-TO-END concurrency legs for the pre-commit hook (`omp-orchestrator-nh5`).
//!
//! The module's own tests exercise the lock. **These run the REAL BINARY**, twice at
//! once, against a real fixture git repository — because a lock proven only in-process
//! is not proven on the commit path. `no-shell-gate`'s own history is the argument:
//! its gate was invoked solely from a workflow whose runner does not exist, and read
//! as protection for hours.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn gate_bin() -> &'static str {
    env!("CARGO_BIN_EXE_pre-commit-gate")
}

fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdin(Stdio::null())
        .output()
        .expect("git must run")
}

/// A repository with one staged, gate-clean file.
fn fixture(tag: &str) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("nh5-e2e-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&repo).ok();
    std::fs::create_dir_all(repo.join("crates/fixture-crate/src")).expect("tree");
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["config", "user.email", "nh5@example.invalid"]);
    git(&repo, &["config", "user.name", "nh5"]);
    // AN INITIAL COMMIT IS REQUIRED, and finding out why was the useful part: without
    // one, `preregistration-gate` runs `git rev-parse HEAD`, gets exit 128 "ambiguous
    // argument 'HEAD'", and refuses. So the first version of this fixture failed for a
    // reason that had nothing to do with concurrency -- which would have made every
    // leg below unattributable.
    std::fs::write(repo.join("README"), "nh5 fixture\n").expect("write readme");
    // AND the preregistration registry must exist IN HEAD, or that gate refuses with
    // `path 'docs/plan/HYPOTHESES.jsonl' does not exist in 'HEAD'` -- a peer's gate
    // that landed the same day. Two fixture defects, both discovered by running the
    // real binary rather than reasoning about it, and both unrelated to concurrency:
    // exactly the noise that would have made a race verdict unattributable.
    // Copied from THIS repo's real registry rather than hand-written: an empty file
    // yields `PREREGISTRATION_ERROR empty hypothesis registry`, and a hand-written row
    // would be a fixture drifted from production the moment the schema moves -- `fh`
    // C38, whose green is indistinguishable from a working check.
    std::fs::create_dir_all(repo.join("docs/plan")).expect("plan dir");
    let real_registry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/plan/HYPOTHESES.jsonl")
        .canonicalize()
        .expect("this repo's own hypothesis registry must exist");
    let registry_text = std::fs::read_to_string(&real_registry).expect("read registry");
    assert!(
        !registry_text.trim().is_empty(),
        "ANTI-VACUITY: the source registry is empty, so the fixture would prove nothing"
    );
    std::fs::write(repo.join("docs/plan/HYPOTHESES.jsonl"), &registry_text).expect("write registry");
    git(&repo, &["add", "README", "docs/plan/HYPOTHESES.jsonl"]);
    git(&repo, &["commit", "--quiet", "--no-verify", "-m", "base"]);
    // A .rs file with no forbidden extension, no path literal, no wildcard arm: the
    // gates must have nothing to say about it, so any refusal is attributable to the
    // concurrency logic rather than to a real violation.
    std::fs::write(
        repo.join("crates/fixture-crate/src/lib.rs"),
        "pub fn ok() -> u8 { 7 }\n",
    )
    .expect("write");
    git(&repo, &["add", "crates/fixture-crate/src/lib.rs"]);
    repo
}

fn run_gate(repo: &Path) -> (Option<i32>, String) {
    let out = Command::new(gate_bin())
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .expect("gate must run");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// KNOWN-GOOD, acceptance 4, **scoped to what this change governs**.
///
/// The first version asserted exit 0 and `CLEAN`. That was wrong, and the three
/// failures it produced are worth recording because each was a real precondition of a
/// DIFFERENT gate, not of concurrency:
///
/// 1. no initial commit -> `preregistration-gate: git rev-parse HEAD exited 128`
/// 2. no registry in HEAD -> `path 'docs/plan/HYPOTHESES.jsonl' does not exist in 'HEAD'`
/// 3. a copied registry   -> `recorded_commit=4893b36… is not the committed parent`
///
/// The third cannot be fixed without reproducing this repository's commit ancestry in
/// a temp directory, because that gate binds each hypothesis to a real ancestor. So
/// **requiring CLEAN would import every other gate's preconditions into a concurrency
/// test** — the same over-scoping I fixed in `staged-build-gate`, where a divergence
/// check refused on files `cargo build` never reads.
///
/// The property that actually belongs here: the serialization must be INVISIBLE to a
/// sequential run. Same verdict every time, and never the race refusal.
#[test]
fn serialization_is_invisible_to_sequential_runs() {
    let repo = fixture("sequential");
    let (first_code, first_text) = run_gate(&repo);
    for attempt in 1..4 {
        let (code, stderr) = run_gate(&repo);
        assert_eq!(
            code, first_code,
            "attempt {attempt} changed the verdict; a lock that leaks into sequential \
             runs is worse than no lock:\nfirst:\n{first_text}\nnow:\n{stderr}"
        );
        assert!(
            !stderr.contains("RETRY_CONCURRENT_COMMIT"),
            "a sequential run must never see the race refusal:\n{stderr}"
        );
    }
    // ANTI-VACUITY: the run must actually have reached the gate body. A verdict that
    // never entered the section would be stable for the wrong reason.
    assert!(
        first_text.contains("orchestration-tick-gate")
            || first_text.contains("CLEAN: all staged files passed"),
        "the gates must have RUN, or this leg proves only that the binary exits:\n{first_text}"
    );
    // And the lock must be released: a leaked lock would make run 2 a refusal, which
    // the loop above already caught -- assert the file is gone so the reason is named.
    assert!(
        !repo.join(".git/omp-pre-commit-gate.lock").exists(),
        "the guard must release on drop"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// THE REPRODUCTION, acceptance 1 and 2. Two REAL gate processes at once against one
/// git dir.
///
/// **Keyed on gate-section ENTRY, not on gate outcome.** The lock is taken before any
/// gate runs, so the loser's refusal is independent of whether the winner's staged set
/// was clean — and keying on exit 0 would have made this leg hostage to every other
/// gate's preconditions, which is what the three fixture failures above proved.
///
/// Before the fix, BOTH processes entered the section: each read `.git/index` while
/// the other could change it, and both verdicts were honest about sets that no longer
/// existed. Now at most one may enter.
#[test]
fn two_concurrent_gate_processes_cannot_both_enter_the_section() {
    let repo = fixture("concurrent");
    let spawn = || {
        Command::new(gate_bin())
            .current_dir(&repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn")
    };
    let (a, b) = (spawn(), spawn());
    let a = a.wait_with_output().expect("wait a");
    let b = b.wait_with_output().expect("wait b");
    let texts = [
        String::from_utf8_lossy(&a.stderr).into_owned(),
        String::from_utf8_lossy(&b.stderr).into_owned(),
    ];

    // "Entered the section" is observable: a process that got the lock ran the gates
    // and said so. A process refused at the door emitted RETRY_CONCURRENT_COMMIT and
    // nothing else.
    let entered = texts
        .iter()
        .filter(|t| t.contains("orchestration-tick-gate") || t.contains("CLEAN: all staged"))
        .count();
    let refused_at_the_door = texts
        .iter()
        .filter(|t| t.contains("RETRY_CONCURRENT_COMMIT"))
        .count();

    // The two may not overlap on a fast machine, in which case both entered
    // SEQUENTIALLY and that is not the defect. What must never happen is two entries
    // WITH a refusal, or an overlap with no refusal.
    assert_eq!(
        entered + refused_at_the_door,
        2,
        "every process must either enter or be refused by name -- silence is the defect:\n{texts:?}"
    );
    if refused_at_the_door == 1 {
        assert_eq!(entered, 1, "an overlap admits exactly one:\n{texts:?}");
        let refusal = texts
            .iter()
            .find(|t| t.contains("RETRY_CONCURRENT_COMMIT"))
            .expect("refusal text");
        assert!(refusal.contains("next_action=re-run-git-commit"), "{refusal}");
        // The refusal must name WHICH race, or an author cannot tell a held section
        // from a moved index.
        assert!(
            refusal.contains("reason=GATE_SECTION_HELD")
                || refusal.contains("reason=INDEX_DRIFTED_MID_GATE"),
            "{refusal}"
        );
        assert!(
            !refusal.contains("CLEAN: all staged"),
            "a refused process must not also claim a clean verdict:\n{refusal}"
        );
    } else {
        assert_eq!(
            refused_at_the_door, 0,
            "either exactly one refusal or none; two refusals means nobody can commit:\n{texts:?}"
        );
    }
    std::fs::remove_dir_all(&repo).ok();
}

/// The gate must still BITE after the fix — acceptance 4's second half. A serialised
/// gate that stopped refusing real violations would be the worst outcome of this bead.
#[test]
fn the_gate_still_refuses_a_real_violation_after_serialization() {
    let repo = fixture("violation");
    // A tracked `.sh` is this repo's founding violation, and the exemption list is
    // empty by design.
    std::fs::write(repo.join("install.sh"), "echo hi\n").expect("write");
    git(&repo, &["add", "install.sh"]);
    let (code, stderr) = run_gate(&repo);
    assert_eq!(code, Some(1), "a staged .sh must refuse; stderr:\n{stderr}");
    assert!(
        stderr.contains("install.sh"),
        "the refusal must name the file:\n{stderr}"
    );
    assert!(
        !stderr.contains("RETRY_CONCURRENT_COMMIT"),
        "a real violation must not be reported as a race:\n{stderr}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// ANTI-VACUITY. If the fixture staged nothing, every leg above would pass over an
/// empty set and prove nothing about concurrency.
#[test]
fn the_fixture_actually_stages_something() {
    let repo = fixture("antivacuity");
    let out = git(&repo, &["diff", "--cached", "--name-only"]);
    let staged: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        staged,
        vec!["crates/fixture-crate/src/lib.rs".to_owned()],
        "the fixture must stage exactly the file the legs assume"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// THE LEG THAT ACTUALLY BITES, and I only have it because the mutation exposed the
/// one above as toothless.
///
/// With serialization removed at the source, the two-process leg above stayed
/// **GREEN 4/4** while the module's own race legs went RED. The reason is structural:
/// that leg accepts "both entered, zero refusals" as a legitimate non-overlap, and an
/// UNSERIALIZED gate produces exactly that shape every time. **It cannot distinguish
/// "they did not overlap" from "there is no lock."**
///
/// Forcing an overlap with a sleep would need test-only code in the production hook.
/// Pre-planting a FRESH lock is deterministic, needs no timing, and no production
/// change: a correctly serialized gate MUST refuse at the door, and an unserialized
/// one MUST walk straight past it.
#[test]
fn a_planted_live_lock_refuses_at_the_door_and_runs_no_gate() {
    let repo = fixture("planted");
    let git_dir = repo.join(".git");
    let lock = git_dir.join("omp-pre-commit-gate.lock");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    // A foreign pid, stamped NOW: unambiguously a live holder.
    std::fs::write(&lock, format!("424242 {now}")).expect("plant lock");

    let (code, stderr) = run_gate(&repo);
    assert_eq!(code, Some(1), "a held section must refuse; stderr:\n{stderr}");
    assert!(
        stderr.contains("RETRY_CONCURRENT_COMMIT"),
        "the refusal must be typed:\n{stderr}"
    );
    assert!(
        stderr.contains("reason=GATE_SECTION_HELD"),
        "and it must name WHICH race:\n{stderr}"
    );
    assert!(
        stderr.contains("holder_pid=424242"),
        "and it must name the holder, or an author cannot check whether it is alive:\n{stderr}"
    );
    // NO GATE MAY HAVE RUN. This is the half that makes the leg bite: an
    // unserialized hook reaches the gates and says so, so their absence is the
    // observable difference.
    assert!(
        !stderr.contains("orchestration-tick-gate"),
        "no gate may run behind a held section:\n{stderr}"
    );
    assert!(
        !stderr.contains("CLEAN: all staged"),
        "a refused run must never also claim a clean verdict:\n{stderr}"
    );

    // The refused process must NOT have stolen or removed the holder's lock.
    let body = std::fs::read_to_string(&lock).expect("the holder's lock must survive");
    assert!(body.starts_with("424242 "), "lock body was overwritten: {body}");
    std::fs::remove_dir_all(&repo).ok();
}
