//! Pins the ABSENCE of the HEAD-fabrication arm deleted by `94bb35f`.
//!
//! That arm sat in `build_manifest` and, for `revision == "HEAD"` on a checkout that cannot
//! resolve it, returned `InputManifest::new(revision, PROVENANCE_PATHS, PROVENANCE_PATHS, [], [])`
//! — a SYNTHESISED all-present manifest. It fired on every depth-zero checkout, so a genuinely
//! absent path still reported present, and it fabricated toward GREEN. The fix carries no test,
//! so a re-add would not go red. This is that test.
//!
//! # TWO PINS, AND EACH COVERS WHAT THE OTHER MISSES
//!
//! [`no_error_arm_in_build_manifest_may_produce_a_manifest`] is STRUCTURAL: it reads
//! `build_manifest`'s source and refuses any `Err` arm that yields a manifest. It is cheap and it
//! catches the literal re-add.
//!
//! [`worktree_mode_refuses_rather_than_fabricating_an_unresolvable_revision`] is BEHAVIOURAL: it
//! runs the CLI in `--mode worktree` on a depth-zero checkout and requires the typed refusal.
//! It catches what the structural pin cannot — a re-add whose `InputManifest::new(` call has been
//! moved OUT of the arm, e.g.
//! `Err(error) if revision == "HEAD" && git_unavailable(&error) => return Ok(fabricate(revision)),`
//! Measured 2026-09-11 on contabo-4 with that helper form in place: `2 passed; 1 failed` — the
//! structural pin GREEN, this test RED — while the defect is fully live (the CLI exits 0 and lists
//! `.git/s1_cov.py` in both `tree` and `index`). The run itself exits 101 because this test fails;
//! the structural pin's own green is the point. Neither pin is sufficient alone; keep both.
//!
//! # ⛔ THE IMPOSSIBILITY CLAIM IS TRUE ONLY OF `--mode tree`, WHICH IS THE DEFAULT
//!
//! `ed2208b1`'s commit message and the text that stood here said the deleted arm *"changes an
//! INTERMEDIATE value no CLI surface can emit."* **That is false as written and true only of
//! `--mode tree`.** Both halves were measured on the rch worker, same box, opposite answers:
//!
//! ```text
//! --mode tree      arm ABSENT / PRESENT   byte-identical: exit=4, empty stdout, CHECKOUT_UNUSABLE
//! --mode worktree  arm ABSENT             exit=4  S1_COVERAGE_CHECKOUT_UNUSABLE, no report
//! --mode worktree  arm PRESENT            exit=0  a full report; input_manifest.tree AND .index
//!                                                 each list ".git/s1_cov.py"; manifest_verdict
//!                                                 DENOMINATOR_CONSISTENT
//! ```
//!
//! The mechanism is the difference in git-dependence, not in the arm. On the TREE path every step
//! (`report_for` → `load_tree` → `read_tree_file` → `git show <rev>:<path>`) needs the same
//! unresolvable revision, so the fabricated manifest is unreachable behind an earlier refusal of
//! the same shape. On the WORKTREE path `load_worktree` reads all six contracts, `S1.toml` and the
//! bead export through `read_worktree_file`, so **`build_manifest` is the only git-dependent step,
//! and the fabricated manifest is emitted verbatim into `input_manifest`.**
//!
//! # Why the structural assertion keys on the MECHANISM, not on a literal
//!
//! `.git/s1_cov.py` is the *tell* that exposed the fabrication, but it is a mitigation: anyone can
//! apply it to an emitted manifest with no run, and the next rename defeats it. Nor does that test
//! grep for `revision == "HEAD"`, which a reworded re-add (`rev == "HEAD"`, a constant, a helper)
//! would slip past. The mechanism is: **an error arm must never produce a manifest.** A manifest
//! is a claim about a revision; an arm reached *because the revision could not be read* has
//! nothing to base that claim on. That is the invariant the deleted arm violated, and it holds for
//! any wording, any path list, and any future revision special-case — but only while the
//! construction stays lexically inside the arm, which is exactly the behavioural pin's job.

use s1_coverage::CONTRACT_PATHS;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `build_manifest`'s source, as text. Read from the crate under test rather than from a fixture
/// so the pin cannot drift away from the code it defends.
const MAIN_SOURCE: &str = include_str!("../src/main.rs");

/// Extracts the body of a top-level `async fn NAME(` by brace balance.
fn function_body(source: &str, signature: &str) -> String {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature:?} not found — the pin's subject moved or was renamed"));
    let after = &source[start..];
    let open = after
        .find('{')
        .unwrap_or_else(|| panic!("{signature:?} has no body"));
    let mut depth = 0usize;
    for (index, byte) in after[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return after[open..=open + index].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("{signature:?} body never closes");
}

#[test]
fn no_error_arm_in_build_manifest_may_produce_a_manifest() {
    let body = function_body(&MAIN_SOURCE, "async fn build_manifest(");

    // ANTI-VACUITY 1: the subject must actually contain error arms. If a refactor removed them,
    // "no error arm produces a manifest" would be trivially true and this test would guard
    // nothing — that is an ERROR for this pin, not a pass.
    let error_arms: Vec<&str> = body
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("Err(") && trimmed.contains("=>")
        })
        .collect();
    assert!(
        error_arms.len() >= 2,
        "expected at least two Err arms in build_manifest (the typed checkout refusal and the \
         passthrough); found {}: an empty or restructured match makes this pin vacuous, so \
         re-derive it against the new shape rather than letting it pass",
        error_arms.len()
    );

    // ANTI-VACUITY 2: the fabrication's constructor must be a name that exists here at all,
    // otherwise the scan below searches for something unreachable and always finds nothing.
    assert!(
        body.contains("InputManifest::new("),
        "build_manifest no longer constructs an InputManifest — the pin's mechanism is gone and \
         must be re-derived, not silently satisfied"
    );

    // THE MECHANISM. Walk each Err arm and refuse any that yields a manifest. An arm is entered
    // only because the revision could not be read, so it has nothing to base a manifest on.
    let lines: Vec<&str> = body.lines().collect();
    let mut offenders: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("Err(") && trimmed.contains("=>")) {
            continue;
        }
        // The arm is either inline on this line, or a block that follows. Scan from the arm to
        // the end of its block: brace balance from the arm's own line.
        let mut depth = 0i32;
        let mut started = false;
        for probe in lines.iter().skip(index) {
            depth += probe.matches('{').count() as i32;
            depth -= probe.matches('}').count() as i32;
            if probe.contains('{') {
                started = true;
            }
            if probe.contains("InputManifest::new(") {
                offenders.push(format!("arm at {trimmed:?} yields a manifest via {probe:?}"));
                break;
            }
            // Inline arm (no block) ends at its own line; a block arm ends when balance returns.
            if !started || depth <= 0 {
                break;
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "AN ERROR ARM IN build_manifest PRODUCES A MANIFEST. This is the 94bb35f fabrication \
         re-added: a revision that could not be read cannot support a claim about its contents, \
         and a synthesised manifest fabricates toward GREEN because an absent path still reports \
         present. Offenders: {offenders:#?}"
    );
}

/// The TREE-mode refusal contract. This does NOT pin the fabricating arm: on the default
/// `--mode tree` path the CLI is byte-identical with the arm present and absent, because every
/// step downstream needs the same unresolvable revision and refuses first. What it pins is the
/// typed refusal a grader reads — exit 4 means the CHECKOUT is at fault, never this crate, and
/// never a silent success. The arm itself is pinned structurally above and behaviourally by
/// [`worktree_mode_refuses_rather_than_fabricating_an_unresolvable_revision`].
#[test]
fn the_refusal_contract_for_an_unresolvable_revision() {
    let repo = depth_zero_fixture();

    // ANTI-VACUITY: the fixture really is depth-zero. With a resolvable HEAD this test would be
    // about a different condition entirely.
    let all_commits = git_stdout(&repo, &["rev-list", "--count", "--all"]);
    assert_eq!(
        all_commits, "0",
        "fixture must have ZERO commits; got {all_commits:?}"
    );

    let (code, stdout, stderr) = run_cli(&repo, &["--json"]);
    println!("REFUSAL_EXIT={code}");
    println!("REFUSAL_STDOUT={stdout}");
    println!("REFUSAL_STDERR={stderr}");

    // The binary ran: 101 is a panic, 127 is not-found, and either makes the rest vacuous.
    assert!(
        code != 101 && code != 127,
        "binary did not run ({code}): {stderr}"
    );
    assert_ne!(
        code, 0,
        "an unresolvable revision produced a SUCCESSFUL run: stdout={stdout}"
    );
    assert_eq!(code, 4, "checkout-unusable must exit 4, not {code}: {stderr}");
    assert!(
        stderr.contains("S1_COVERAGE_CHECKOUT_UNUSABLE"),
        "refusal must attribute the failure to the checkout: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "a refused run must emit no report: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

/// The BEHAVIOURAL pin, and the leg the structural one cannot cover.
///
/// `--mode worktree` loads all six contracts, `S1.toml` and the bead export through
/// `read_worktree_file`, so `build_manifest` is the ONLY git-dependent step on that path. On a
/// depth-zero checkout the manifest is the single thing standing between the run and a report —
/// exactly what the fabricating arm supplied. Re-add that fabrication in ANY form, lexically
/// inside the `Err` arm or hidden behind a one-line helper the source scan cannot see, and this
/// test reddens: the run exits 0 and prints an `input_manifest` listing `.git/s1_cov.py` as
/// tracked, which no real tree can contain.
#[test]
fn worktree_mode_refuses_rather_than_fabricating_an_unresolvable_revision() {
    // ANTI-VACUITY, POSITIVE CONTROL. The same fixture with a RESOLVABLE revision must SUCCEED in
    // worktree mode. Without this leg an unreadable contract or an unparseable bead export would
    // refuse for its own reason, the assertions below would still pass, and the pin would prove
    // nothing about the manifest.
    let resolvable = depth_zero_fixture();
    commit_fixture(&resolvable);
    let control_commits = git_stdout(&resolvable, &["rev-list", "--count", "--all"]);
    assert_eq!(
        control_commits, "1",
        "positive control must carry exactly one commit; got {control_commits:?}"
    );
    let (control_code, control_stdout, control_stderr) =
        run_cli(&resolvable, &["--mode", "worktree", "--json"]);
    println!("CONTROL_EXIT={control_code}");
    println!("CONTROL_STDERR={control_stderr}");
    assert_eq!(
        control_code, 0,
        "POSITIVE CONTROL FAILED: worktree mode must succeed when the revision resolves. Until it \
         does, a refusal below is not attributable to the revision. stderr={control_stderr}"
    );
    assert!(
        control_stdout.contains("\"rev_mode\": \"worktree\""),
        "positive control emitted no worktree report: {control_stdout}"
    );
    let _ = std::fs::remove_dir_all(&resolvable);

    // THE PIN. Byte-identical inputs on disk, no resolvable revision.
    let repo = depth_zero_fixture();
    let all_commits = git_stdout(&repo, &["rev-list", "--count", "--all"]);
    assert_eq!(
        all_commits, "0",
        "fixture must have ZERO commits; got {all_commits:?}"
    );

    let (code, stdout, stderr) = run_cli(&repo, &["--mode", "worktree", "--json"]);
    println!("WORKTREE_EXIT={code}");
    println!("WORKTREE_STDOUT={stdout}");
    println!("WORKTREE_STDERR={stderr}");

    // The binary ran: 101 is a panic, 127 is not-found, and either makes the rest vacuous.
    assert!(
        code != 101 && code != 127,
        "binary did not run ({code}): {stderr}"
    );
    assert_ne!(
        code, 0,
        "WORKTREE MODE EMITTED A REPORT FOR A REVISION IT COULD NOT READ. `build_manifest` is the \
         only git-dependent step on this path, so a successful run means the manifest was \
         SYNTHESISED — the 94bb35f HEAD fabrication is back, in the arm or behind a helper. \
         stdout={stdout}"
    );
    assert_eq!(code, 4, "checkout-unusable must exit 4, not {code}: {stderr}");
    assert!(
        stderr.contains("S1_COVERAGE_CHECKOUT_UNUSABLE"),
        "refusal must attribute the failure to the checkout: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "a refused run must emit no report: {stdout}"
    );
    assert!(
        !stdout.contains(".git/s1_cov.py"),
        "the fabrication tell reached an emitted manifest: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

/// A checkout with an initialised `.git`, a working tree, and NO commits and NO objects — the
/// shape measured on the rch workers on 2026-09-10 (`is-shallow=false`, `.git/shallow` absent,
/// `rev-list --count --all` = 0). This is the state in which the deleted arm fired.
fn depth_zero_fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "s1cov-pin-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("docs/plan/flow/maturity0")).expect("fixture dirs");
    std::fs::create_dir_all(root.join("crates/s1-coverage/src")).expect("fixture dirs");
    std::fs::write(root.join("docs/plan/flow/S1-COVERAGE.md"), "# fixture\n").expect("write");
    std::fs::write(root.join("crates/s1-coverage/Cargo.toml"), "[package]\n").expect("write");
    std::fs::write(root.join("crates/s1-coverage/src/main.rs"), "fn main() {}\n").expect("write");

    // The inputs `--mode worktree` reads straight off disk. With all of them present,
    // `build_manifest` is the ONLY step on that path that needs git — which is what makes the
    // fabricating arm observable there while `--mode tree` refuses identically either way.
    std::fs::create_dir_all(root.join("docs/contracts")).expect("fixture dirs");
    std::fs::create_dir_all(root.join("docs/plan/flow/boxes")).expect("fixture dirs");
    std::fs::create_dir_all(root.join(".beads")).expect("fixture dirs");
    for (index, path) in CONTRACT_PATHS.into_iter().enumerate() {
        std::fs::write(
            root.join(path),
            format!("# fixture contract {index}\n\nRequirement `L{index}-FIXTURE` is declared.\n"),
        )
        .expect("write");
    }
    std::fs::write(
        root.join("docs/plan/flow/boxes/S1.toml"),
        "[[box.layer]]\nid = \"L0\"\nexists = \"crates/installer\"\n",
    )
    .expect("write");
    std::fs::write(
        root.join(".beads/issues.jsonl"),
        "{\"id\":\"omp-orchestrator-fixture\",\"title\":\"fixture bead for L0-FIXTURE\"}\n",
    )
    .expect("write");

    let status = Command::new("git")
        .args(["init", "-q"])
        .arg(&root)
        .status()
        .expect("git is on PATH");
    assert!(status.success(), "git init failed for {}", root.display());
    root
}

fn git_stdout(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git is on PATH");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Runs the CLI under test against `repo`. Returns exit code, stdout, stderr — never a partial
/// view, because every assertion in this file needs all three to say why it failed.
fn run_cli(repo: &Path, extra: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_s1-coverage"))
        .arg("--repo")
        .arg(repo)
        .args(extra)
        .output()
        .expect("the s1-coverage binary was built for this test");
    (
        out.status.code().expect("process produced an exit code"),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Turns a depth-zero fixture into one with a resolvable `HEAD`, for the positive control.
/// Identity and signing are supplied inline so the run does not depend on the caller's git config.
fn commit_fixture(repo: &Path) {
    for args in [
        vec!["add", "-A"],
        vec![
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "user.name=fixture",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--no-verify",
            "-m",
            "fixture",
        ],
    ] {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(&args)
            .output()
            .expect("git is on PATH");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
