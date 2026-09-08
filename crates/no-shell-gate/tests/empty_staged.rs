//! PRE-COMMIT EMPTY-INDEX GATE — the hook must distinguish a real clean scan
//! from an empty index that checked nothing.
//!
//! These tests exercise the actual pre-commit binary against throwaway git
//! indexes. The fixture keeps one harmless Rust source under `crates/` so the
//! other workspace-wide gates have a non-empty, clean scan set; only the file
//! named by each test is staged.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

fn fresh_git_tree(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "no-shell-gate-pre-commit-{}-{test}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(dir.join("docs/plan/flow/boxes")).expect("create R1 fixture");
    fs::write(
        dir.join("docs/plan/flow/CONTRACT.md"),
        "R1_POPULATION_BOXES=fixture\n",
    )
    .expect("write R1 contract fixture");
    fs::write(
        dir.join("docs/plan/flow/boxes/fixture.toml"),
        "id = \"fixture\"\nkernel_input = \"input\"\nkernel_output = \"output\"\nevent_row = \"event\"\nvalidator = \"validator\"\nmeasured = \"measured\"\n",
    )
    .expect("write R1 box fixture");
    fs::create_dir_all(dir.join("crates/example/src")).expect("create fixture tree");
    fs::create_dir_all(dir.join("docs/plan")).expect("create fixture plan");
    fs::write(
        dir.join("crates/example/src/lib.rs"),
        "pub fn clean_fixture() {}\n",
    )
    .expect("write clean workspace source");
    fs::write(
        dir.join("docs/plan/HYPOTHESES.jsonl"),
        r#"{"id":"h1","prediction":"fixture","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"0123456789abcdef0123456789abcdef01234567","observed_result":null}"#.to_owned() + "\n",
    )
    .expect("write fixture hypotheses");
    run_git(&dir, &["init", "-q"], "git init");
    run_git(
        &dir,
        &["add", "--", "crates/example/src/lib.rs", "docs/plan/HYPOTHESES.jsonl"],
        "stage clean baseline",
    );
    run_git(
        &dir,
        &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture baseline [test]"],
        "fixture baseline commit",
    );
    let parent = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "HEAD"], "read fixture parent").stdout)
        .trim()
        .to_owned();
    let hypotheses_path = dir.join("docs/plan/HYPOTHESES.jsonl");
    let hypotheses = fs::read_to_string(&hypotheses_path)
        .expect("read fixture hypotheses")
        .replace("0123456789abcdef0123456789abcdef01234567", &parent);
    fs::write(&hypotheses_path, hypotheses).expect("update fixture parent");
    run_git(&dir, &["add", "--", "docs/plan/HYPOTHESES.jsonl"], "stage updated hypotheses");
    run_git(
        &dir,
        &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture hypotheses [test]"],
        "fixture hypotheses commit",
    );
    dir
}

fn run_git(dir: &Path, args: &[&str], what: &str) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "{what} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn stage(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write staged fixture");
    run_git(dir, &["add", "--", name], "git add");
}
fn stage_close_mirror(dir: &Path, content: &str) {
    fs::create_dir_all(dir.join(".beads")).expect("create bead mirror directory");
    stage(dir, ".beads/issues.jsonl", content);
}

fn run_gate(dir: &Path) -> Output {
    let scoped_index = dir.join(".git/calr-gate-index");
    fs::copy(dir.join(".git/index"), &scoped_index).expect("copy scoped index");
    let output = Command::new(env!("CARGO_BIN_EXE_pre-commit-gate"))
        .current_dir(dir)
        .env("GIT_INDEX_FILE", &scoped_index)
        .output()
        .expect("spawn pre-commit-gate");
    fs::remove_file(scoped_index).expect("remove scoped index");
    output
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn top_level_outcome(stderr: &str) -> &'static str {
    let outcomes = ["CLEAN:", "VIOLATION:", "NOTHING_TO_CHECK:", "ANCESTRY_ONLY_MERGE:"];
    let matches: Vec<_> = stderr
        .lines()
        .filter_map(|line| outcomes.iter().find(|outcome| line.starts_with(*outcome)))
        .copied()
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "gate output must contain exactly one top-level outcome marker: {stderr}"
    );
    matches[0]
}
fn assert_no_ambiguous_nested_outcomes(stderr: &str) {
    for gate in ["path-literal-guard", "state-wildcard-lint"] {
        assert!(
            !stderr.lines().any(|line| {
                line.starts_with(&format!("{gate}: NOTHING_TO_CHECK"))
            }),
            "per-gate scope diagnostics must not reuse the top-level NOTHING_TO_CHECK marker: {stderr}"
        );
    }
}

/// KNOWN-BAD: an empty index is not a successful no-op for the gate. It is a
/// typed NOTHING_TO_CHECK refusal with its own exit code, while `git commit`
/// remains responsible for its ordinary "nothing to commit" behavior.
#[test]
fn empty_staged_index_is_nothing_to_check_refusal() {
    let dir = fresh_git_tree("empty-index");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(
        output.status.code(),
        Some(3),
        "empty staged index must have the distinct NOTHING_TO_CHECK outcome: {error}"
    );
    assert_eq!(
        top_level_outcome(&error),
        "NOTHING_TO_CHECK:",
        "empty scan must report the explicit top-level outcome: {error}"
    );
    assert_no_ambiguous_nested_outcomes(&error);
}

/// KNOWN-GOOD: one clean staged file is checked and reports CLEAN.
#[test]
fn one_clean_staged_file_is_clean() {
    let dir = fresh_git_tree("clean-file");
    stage(&dir, "README.md", "clean staged fixture\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(
        output.status.code(),
        Some(0),
        "one clean staged file must pass: {error}"
    );
    assert_eq!(
        top_level_outcome(&error),
        "CLEAN:",
        "clean verdict must be the explicit top-level outcome: {error}"
    );
    assert_no_ambiguous_nested_outcomes(&error);
}
/// KNOWN-GOOD: the extended vocabulary is accepted when the mirror is staged.
#[test]
fn staged_close_reason_policy_accepts_extended_prefixes() {
    let dir = fresh_git_tree("close-prefix-good");
    stage_close_mirror(
        &dir,
        r#"{"id":"good-premise","status":"closed","close_reason":"PREMISE-FALSE: the premise was disproven"}
{"id":"good-fixed","status":"closed","close_reason":"ALREADY-FIXED: landed in 0123456"}
"#,
    );
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(0), "extended prefixes must pass: {error}");
    assert_eq!(top_level_outcome(&error), "CLEAN:", "{error}");
    assert!(
        error.contains("close-reason-policy: state=CLEAN")
            && error.contains("closed_beads=2")
            && error.contains("verified=2")
            && error.contains("conflicts=0")
            && error.contains("detection_only=true"),
        "the clean report must carry denominators and the detection boundary: {error}"
    );
    fs::remove_dir_all(dir).expect("remove close prefix good fixture");
}

/// FIRES-ON-KNOWN-BAD: prose and an empty reason are both named refusals.
#[test]
fn staged_close_reason_policy_refuses_prose_and_empty_reasons() {
    let dir = fresh_git_tree("close-prefix-bad");
    stage_close_mirror(
        &dir,
        r#"{"id":"bad-prose","status":"closed","close_reason":"fixed it"}
{"id":"bad-empty","status":"closed","close_reason":""}
"#,
    );
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "bad close reasons must refuse: {error}");
    assert_eq!(top_level_outcome(&error), "VIOLATION:", "{error}");
    assert!(
        error.contains("close-reason-policy: state=REFUSED")
            && error.contains("closed_beads=2")
            && error.contains("verified=0")
            && error.contains("conflicts=2")
            && error.contains("bad-prose")
            && error.contains("CLOSE_REASON_POLICY_REFUSED leading=fixed")
            && error.contains("bad-empty")
            && error.contains("CLOSE_REASON_EMPTY")
            && error.contains("detection_only=true"),
        "refusal must pin both bead ids, verdicts, denominators, and scope: {error}"
    );
    fs::remove_dir_all(dir).expect("remove close prefix bad fixture");
}

/// ANTI-VACUITY: an unreadable or record-free mirror cannot pass as a clean scan.
#[test]
fn staged_close_reason_policy_fails_closed_for_missing_or_record_free_mirror() {
    let missing = fresh_git_tree("close-prefix-missing");
    stage_close_mirror(
        &missing,
        r#"{"id":"missing-at-read","status":"closed","close_reason":"DONE: staged first"}
"#,
    );
    fs::remove_file(missing.join(".beads/issues.jsonl")).expect("remove mirror after staging");
    let missing_output = run_gate(&missing);
    let missing_error = stderr(&missing_output);
    assert_eq!(missing_output.status.code(), Some(1), "missing mirror must refuse: {missing_error}");
    assert!(
        missing_error.contains("close-reason-policy: state=ERROR")
            && missing_error.contains("PRE_DELETE_BEADS_UNREADABLE")
            && missing_error.contains("closed_beads=0"),
        "missing mirror refusal must name unreadability and zero scanned rows: {missing_error}"
    );
    fs::remove_dir_all(missing).expect("remove missing mirror fixture");

    let record_free = fresh_git_tree("close-prefix-record-free");
    stage_close_mirror(
        &record_free,
        r#"{"id":"open-only","status":"open","close_reason":""}
"#,
    );
    let record_free_output = run_gate(&record_free);
    let record_free_error = stderr(&record_free_output);
    assert_eq!(
        record_free_output.status.code(),
        Some(1),
        "record-free mirror must refuse: {record_free_error}"
    );
    assert!(
        record_free_error.contains("close-reason-policy: state=ERROR")
            && record_free_error.contains("PRE_DELETE_BEADS_EMPTY")
            && record_free_error.contains("closed_beads=0"),
        "record-free refusal must remain distinct from a valid zero-conflict scan: {record_free_error}"
    );
    fs::remove_dir_all(record_free).expect("remove record-free mirror fixture");
}

/// KNOWN-BAD: one staged forbidden-extension file reports VIOLATION, not a
/// generic empty-index refusal or a successful partial scan.
#[test]
fn one_violating_staged_file_is_violation() {
    let dir = fresh_git_tree("violating-file");
    stage(&dir, "evil.sh", "#!/bin/sh\necho known-bad\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(
        output.status.code(),
        Some(1),
        "one violating staged file must be a violation refusal: {error}"
    );
    assert_eq!(
        top_level_outcome(&error),
        "VIOLATION:",
        "violation verdict must be the explicit top-level outcome: {error}"
    );
    assert!(
        error.contains("evil.sh"),
        "violation must name the staged file: {error}"
    );
    assert_no_ambiguous_nested_outcomes(&error);
}
#[test]
fn collapsing_whole_commit_marker_into_gate_note_is_red() {
    let dir = fresh_git_tree("scope-boundary-mutation");
    stage(&dir, "README.md", "clean staged fixture\n");
    let actual = stderr(&run_gate(&dir));
    assert_eq!(top_level_outcome(&actual), "CLEAN:");

    let collapsed = actual
        .lines()
        .map(|line| {
            line.strip_prefix("CLEAN:").map_or_else(
                || line.to_owned(),
                |rest| format!("path-literal-guard: CLEAN:{rest}"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let red = std::panic::catch_unwind(|| top_level_outcome(&collapsed));
    assert!(
        red.is_err(),
        "a whole-commit marker collapsed into a per-gate note must be rejected: {collapsed}"
    );
}
/// The same run proves the two empty-index states are not collapsed: no merge is
/// NOTHING_TO_CHECK, while a real ancestry-only merge is an explicit successful decision.
#[test]
fn ancestry_only_merge_runs_gate_and_preserves_empty_index_refusal() {
    let dir = fresh_git_tree("ancestry-only-merge");
    run_git(&dir, &["branch", "-M", "main"], "rename fixture branch");

    let empty = run_gate(&dir);
    let empty_error = stderr(&empty);
    assert_eq!(empty.status.code(), Some(3), "a clean index without merge state must refuse: {empty_error}");
    assert!(
        empty_error.contains("NOTHING_TO_CHECK: no staged files to check"),
        "empty non-merge gate stderr: {empty_error}"
    );

    let marker = dir.join("docs/plan/merge-marker.txt");
    fs::write(&marker, "same patch\n").expect("write main patch");
    run_git(&dir, &["add", "--", "docs/plan/merge-marker.txt"], "stage main patch");
    run_git(&dir, &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "main equivalent patch [test]"], "commit main patch");
    let parent = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "HEAD^"], "read merge base").stdout).trim().to_owned();

    run_git(&dir, &["switch", "-c", "ci/fence-16l-13ca3f5", &parent], "create duplicate patch branch");
    fs::write(&marker, "same patch\n").expect("write duplicate patch");
    run_git(&dir, &["add", "--", "docs/plan/merge-marker.txt"], "stage duplicate patch");
    run_git(&dir, &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "duplicate patch branch [test]"], "commit duplicate patch");
    run_git(&dir, &["switch", "main"], "return to main");

    let cherry = run_git(&dir, &["cherry", "-v", "main", "ci/fence-16l-13ca3f5"], "verify patch-id duplicate");
    assert!(String::from_utf8_lossy(&cherry.stdout).lines().any(|line| line.trim_start().starts_with('-')), "branch commit must be already represented by patch-id: {:?}", String::from_utf8_lossy(&cherry.stdout));
    let branch_tip = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "ci/fence-16l-13ca3f5"], "read duplicate branch tip").stdout).trim().to_owned();
    run_git(&dir, &["update-ref", "refs/branch-rationalization-backup/ci-fence-16l-13ca3f5", &branch_tip], "record branch backup ref");
    run_git(&dir, &["merge", "--no-commit", "--no-ff", "ci/fence-16l-13ca3f5"], "create ancestry-only merge");

    assert!(dir.join(".git/MERGE_HEAD").is_file(), "the live merge state must exist");
    for filter in ["ACMR", "D"] {
        let diff = run_git(&dir, &["diff", "--cached", "--name-only", &format!("--diff-filter={filter}")], "verify empty staged merge set");
        assert!(diff.stdout.is_empty(), "merge index must be empty for {filter}: {:?}", String::from_utf8_lossy(&diff.stdout));
    }
    let merge_tree = String::from_utf8_lossy(&run_git(&dir, &["write-tree"], "read merge index tree").stdout).trim().to_owned();
    let head_tree = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "HEAD^{tree}"], "read HEAD tree").stdout).trim().to_owned();
    assert_eq!(merge_tree, head_tree, "the merge must be ancestry-only");

    let output = run_gate(&dir);
    let error = stderr(&output);
    assert_eq!(output.status.code(), Some(0), "ancestry-only merge must reach the gate: {error}");
    assert_eq!(top_level_outcome(&error), "ANCESTRY_ONLY_MERGE:", "the merge decision must be explicit: {error}");
    assert!(!error.contains("NOTHING_TO_CHECK"), "merge state must not collapse into empty-index refusal: {error}");

    let bundle = dir.join("ci-fence-16l-13ca3f5.bundle");
    let bundle_path = bundle.to_str().expect("bundle path is UTF-8");
    run_git(&dir, &["bundle", "create", bundle_path, "main", "ci/fence-16l-13ca3f5"], "create object bundle");
    run_git(&dir, &["commit", "--no-edit", "-q"], "commit ancestry-only merge");
    run_git(&dir, &["branch", "-d", "ci/fence-16l-13ca3f5"], "safe-delete merged branch");
    let backup = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "refs/branch-rationalization-backup/ci-fence-16l-13ca3f5"], "resolve backup ref").stdout).trim().to_owned();
    assert_eq!(backup, branch_tip, "backup ref must preserve the deleted branch tip");
    run_git(&dir, &["bundle", "verify", bundle_path], "verify object bundle");
    run_git(&dir, &["cat-file", "-e", &format!("{branch_tip}^{{commit}}")], "resolve branch object");
    fs::remove_dir_all(dir).expect("remove merge fixture");
}
/// A deletion-only staged set is real work and must reach GATE 5 rather than the
/// top-level empty-index refusal. The exact historical 243-row deletion is a
/// separate input size; this regression asserts the same decision boundary.
#[test]
fn deletion_only_staged_set_reaches_pre_delete_gate() {
    let dir = fresh_git_tree("deletion-only");
    run_git(&dir, &["rm", "-q", "--", "docs/plan/HYPOTHESES.jsonl"], "stage deletion-only fixture");
    let output = run_gate(&dir);
    let error = stderr(&output);
    assert_ne!(output.status.code(), Some(3), "deletion-only work must not be NOTHING_TO_CHECK: {error}");
    assert!(!error.contains("NOTHING_TO_CHECK: no staged files to check"), "deletion-only work must reach the gates: {error}");
    assert_eq!(top_level_outcome(&error), "CLEAN:", "deletion-only work must run the normal gate path: {error}");
    fs::remove_dir_all(dir).expect("remove deletion fixture");
}
#[cfg(unix)]
fn stage_with_mode(dir: &Path, name: &str, mode: u32, content: &str) {
    if let Some(parent) = Path::new(name).parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(dir.join(parent)).expect("create staged fixture parent");
    }
    fs::write(dir.join(name), content).expect("write mode fixture");
    let mut permissions = fs::metadata(dir.join(name)).expect("mode fixture metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(dir.join(name), permissions).expect("set mode fixture permissions");
    run_git(dir, &["add", "--", name], "stage mode fixture");
}

/// The mode gate owns only staged Rust source modes. A clean Rust file is a real
/// known-good verdict, and its output must state the write-time residual.
#[cfg(unix)]
#[test]
fn staged_rust_mode_100644_is_clean_and_names_write_time_residual() {
    let dir = fresh_git_tree("rust-mode-clean");
    stage_with_mode(&dir, "mode_probe.rs", 0o100644, "fn mode_probe() {}\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(0), "100644 Rust source must pass: {error}");
    assert_eq!(top_level_outcome(&error), "CLEAN:", "clean mode verdict: {error}");
    assert!(
        error.contains("mode-gate: COMMIT_TIME_ONLY") && error.contains("git diff --numstat -- <path>"),
        "the gate must state its write-time residual: {error}"
    );
    assert!(!error.contains("mode-gate: REFUSED"), "100644 must not be refused: {error}");
    fs::remove_dir_all(dir).expect("remove mode clean fixture");
}

/// KNOWN-BAD: an executable staged Rust source is refused with both path and
/// index mode, not folded into a generic cargo or empty-index result.
#[cfg(unix)]
#[test]
fn staged_rust_mode_100755_is_refused_with_path_and_mode() {
    let dir = fresh_git_tree("rust-mode-refused");
    stage_with_mode(&dir, "mode_probe.rs", 0o100755, "fn mode_probe() {}\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "100755 Rust source must refuse: {error}");
    assert_eq!(top_level_outcome(&error), "VIOLATION:", "mode refusal verdict: {error}");
    assert!(
        error.contains("mode-gate: REFUSED path=mode_probe.rs mode=100755"),
        "refusal must name exact path and mode: {error}"
    );
    fs::remove_dir_all(dir).expect("remove mode refusal fixture");
}

/// KNOWN-GOOD: executable non-Rust paths are outside this gate's scope. The
/// no-shell gate and the mode gate must not invent a refusal for a real binary
/// or hook path merely because its index mode is executable.
#[cfg(unix)]
#[test]
fn executable_non_rust_staged_path_is_not_flagged_by_mode_gate() {
    let dir = fresh_git_tree("non-rust-executable");
    stage_with_mode(&dir, "bin/probe", 0o100755, "binary-or-hook-placeholder\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(0), "non-Rust executable must pass: {error}");
    assert_eq!(top_level_outcome(&error), "CLEAN:", "non-Rust mode verdict: {error}");
    assert!(!error.contains("mode-gate: REFUSED"), "mode gate over-scoped non-Rust path: {error}");
    fs::remove_dir_all(dir).expect("remove non-Rust executable fixture");
}
