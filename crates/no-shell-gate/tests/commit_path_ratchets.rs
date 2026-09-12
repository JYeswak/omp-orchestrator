#![forbid(unsafe_code)]

//! Real pre-commit trigger coverage for the four ratchets that used to run only as tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn fresh_repo(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("omp-yqun-{label}-{nonce}"));
    fs::create_dir_all(root.join("docs/plan/flow/boxes")).expect("create plan boxes");
    fs::write(
        root.join("docs/plan/flow/CONTRACT.md"),
        "R1_POPULATION_BOXES=fixture\n",
    )
    .expect("write contract fixture");
    fs::write(
        root.join("docs/plan/flow/boxes/fixture.toml"),
        "id = \"fixture\"\nkernel_input = \"input\"\nkernel_output = \"output\"\nevent_row = \"event\"\nvalidator = \"validator\"\nmeasured = \"measured\"\n",
    )
    .expect("write flow fixture");
    fs::create_dir_all(root.join("crates/example/src")).expect("create example crate");
    fs::write(
        root.join("crates/example/src/lib.rs"),
        "pub fn clean_fixture() {}\n",
    )
    .expect("write example source");
    fs::create_dir_all(root.join(".omp/agents")).expect("create grader directory");
    fs::write(
        root.join(".omp/agents/omp-grader.md"),
        r#"---
tools: [read, grep, glob, bash, yield]
output:
  properties:
    verdict:
    verdict_class:
    worker:
    proof_exit:
    proof_test_result:
    tree_pin:
    reexecuted:
    controls:
    no_claim:
---
"#,
    )
    .expect("write grader fixture");
    fs::create_dir_all(root.join("docs/plan")).expect("create plan directory");
    fs::write(
        root.join("docs/plan/HYPOTHESES.jsonl"),
        r#"{"id":"h1","prediction":"fixture","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"0123456789abcdef0123456789abcdef01234567","observed_result":null}
"#,
    )
    .expect("write hypotheses fixture");
    run_git(&root, &["init", "-q"], "git init");
    run_git(
        &root,
        &["config", "user.name", "yqun-test"],
        "git user name",
    );
    run_git(
        &root,
        &["config", "user.email", "yqun@example.invalid"],
        "git user email",
    );
    fs::write(root.join("README.md"), "baseline\n").expect("write baseline");
    run_git(
        &root,
        &["add", "--", "README.md", ".omp/agents/omp-grader.md"],
        "stage baseline",
    );
    run_git(
        &root,
        &["commit", "-qm", "chore: baseline [test]"],
        "commit baseline",
    );
    let parent = String::from_utf8_lossy(&run_git(
        &root,
        &["rev-parse", "HEAD"],
        "read baseline head",
    )
    .stdout)
    .trim()
    .to_owned();
    let hypotheses = root.join("docs/plan/HYPOTHESES.jsonl");
    let text = fs::read_to_string(&hypotheses)
        .expect("read hypotheses")
        .replace(
            "0123456789abcdef0123456789abcdef01234567",
            &parent,
        );
    fs::write(&hypotheses, text).expect("write parent pointer");
    run_git(&root, &["add", "--", "docs/plan/HYPOTHESES.jsonl"], "stage hypotheses");
    run_git(
        &root,
        &["commit", "-qm", "chore: hypothesis fixture [test]"],
        "commit hypotheses",
    );
    root
}

fn run_git(root: &Path, args: &[&str], what: &str) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
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

fn stage(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create staged parent");
    }
    fs::write(path, contents).expect("write staged content");
    run_git(root, &["add", "--", relative], "stage content");
}

fn install_hook(root: &Path) -> PathBuf {
    let hook = root.join(".git/hooks/pre-commit");
    fs::copy(env!("CARGO_BIN_EXE_pre-commit-gate"), &hook).expect("copy pre-commit gate");
    #[cfg(unix)]
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("make hook executable");
    hook
}

fn commit(root: &Path, message: &str) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["commit", "-qm", message])
        .output()
        .expect("spawn git commit")
}

fn run_installed_hook(root: &Path) -> Output {
    Command::new(root.join(".git/hooks/pre-commit"))
        .current_dir(root)
        .output()
        .expect("run installed pre-commit hook")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn sha256(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum runs on the remote test host");
    assert!(output.status.success(), "sha256sum failed: {output:?}");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("sha256sum output")
        .to_owned()
}

fn refusal_line<'a>(stderr: &'a str, marker: &str) -> &'a str {
    stderr
        .lines()
        .find(|line| line.contains(marker))
        .expect("typed refusal line")
}
#[test]
fn real_hook_refuses_plan_citation_and_restored_commit_passes() {
    let root = fresh_repo("plan-citation");
    let hook = install_hook(&root);
    let path = root.join("docs/plan/00-brief.md");
    let clean = b"safe plan text\n";
    stage(&root, "docs/plan/00-brief.md", clean);
    run_git(&root, &["commit", "-qm", "chore: plan fixture [test]"], "commit plan baseline");
    let clean_hash = sha256(&path);
    let bad = b"safe plan text\ncrates/example/src/lib.rs:42\n";
    stage(&root, "docs/plan/00-brief.md", bad);
    let refused = commit(&root, "feat: planted plan citation [test]");
    let error = stderr(&refused);
    assert_eq!(refused.status.code(), Some(1), "plan citation must refuse: {error}");
    assert!(
        error.contains("plan_citations: REFUSED")
            && error.contains("file=docs/plan/00-brief.md:2")
            && error.contains("kind=citation"),
        "refusal must name ratchet, file, line, and kind: {error}"
    );
    println!(
        "YQUN_PLAN_REFUSAL exit={:?} refusal={}",
        refused.status.code(),
        refusal_line(&error, "plan_citations: REFUSED")
    );
    fs::write(&path, clean).expect("restore plan bytes");
    run_git(
        &root,
        &["reset", "-q", "HEAD", "--", "docs/plan/00-brief.md"],
        "restore plan index",
    );
    stage(&root, "README.md", b"plan ratchet restored\n");
    let restored = commit(&root, "chore: restore plan fixture [test]");
    assert!(
        restored.status.success(),
        "byte-identical plan restore must pass: {}",
        stderr(&restored)
    );
    let restored_hash = sha256(&path);
    println!(
        "YQUN_PLAN_RESTORE sha_before={clean_hash} sha_after={restored_hash} equal={}",
        clean_hash == restored_hash
    );
    assert_eq!(fs::read(&path).expect("read restored plan"), clean);
    assert!(hook.is_file(), "real installed hook must remain present");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn real_hook_refuses_new_crate_without_census_row() {
    let root = fresh_repo("census-membership");
    install_hook(&root);
    let manifest = "[package]\nname = \"unowned-gate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    let manifest_path = root.join("crates/unowned-gate/Cargo.toml");
    stage(&root, "crates/unowned-gate/Cargo.toml", manifest.as_bytes());
    let manifest_hash = sha256(&manifest_path);
    let refused = commit(&root, "feat: planted census member [test]");
    let error = stderr(&refused);
    assert_eq!(
        refused.status.code(),
        Some(1),
        "unowned crate must refuse: {error}"
    );
    assert!(
        error.contains("census_membership: REFUSED")
            && error.contains("crate=unowned-gate")
            && error.contains("reason=NO_CENSUS_ROW"),
        "refusal must name census membership and its missing row: {error}"
    );
    println!(
        "YQUN_CENSUS_REFUSAL exit={:?} refusal={} manifest_sha={manifest_hash}",
        refused.status.code(),
        refusal_line(&error, "census_membership: REFUSED")
    );
    run_git(
        &root,
        &["reset", "-q", "HEAD", "--", "crates/unowned-gate/Cargo.toml"],
        "unstage census mutation",
    );
    fs::remove_dir_all(root.join("crates/unowned-gate")).expect("remove census mutation");
    assert!(!manifest_path.exists(), "census mutation must restore to absent");
    println!("YQUN_CENSUS_RESTORE manifest_sha={manifest_hash} restored_absent=true");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

/// RE-BASELINED to the HEAL contract (`omp-orchestrator-ic5xr`, after
/// `omp-orchestrator-7h8kr`'s supersession landed at 0dca236).
///
/// The OLD leg asserted `exit=1` + `hook_freshness: REFUSED reason=STALE_HOOK`.
/// That refusal no longer exists: a stale hook now HEALS and never refuses, so
/// the old assertion pinned a contract the product had stopped performing and
/// this leg was RED in CI (gate.yml 34652659103: "stale hook must refuse:
/// left: Some(0) right: Some(1)").
///
/// What survives, and is what this leg now proves, is the DETECTION: a touched
/// covered source must be SEEN and NAMED with a heal state. Asserting `exit=0`
/// alone would pass on a gate that noticed nothing at all -- which is the hole
/// the old leg existed to close -- so the STATE is asserted, not the exit code
/// alone, and `CLEAN`/`REFUSED`/`GATE_NOT_APPLICABLE` are all excluded.
///
/// The heal lock is pre-created so `ensure_heal` reports `already_running` and
/// no background cross-build is spawned from a synthetic fixture repo.
#[test]
fn real_hook_heals_touched_hook_source_and_never_refuses() {
    let root = fresh_repo("hook-freshness");
    let source = "fn pre_commit_fixture() {}\n";
    stage(
        &root,
        "crates/no-shell-gate/src/bin/pre-commit-gate.rs",
        source.as_bytes(),
    );
    run_git(
        &root,
        &["commit", "-qm", "chore: hook source fixture [test]"],
        "commit hook source baseline",
    );
    let hook = install_hook(&root);
    fs::create_dir_all(root.join(".git/hook-heal.lock")).expect("pre-hold the heal lock");
    std::thread::sleep(Duration::from_millis(20));
    let before = fs::read(&root.join("crates/no-shell-gate/src/bin/pre-commit-gate.rs"))
        .expect("read hook source");
    let before_hash = sha256(&root.join("crates/no-shell-gate/src/bin/pre-commit-gate.rs"));
    fs::write(
        root.join("crates/no-shell-gate/src/bin/pre-commit-gate.rs"),
        &before,
    )
    .expect("touch hook source without changing bytes");
    stage(&root, "README.md", b"hook freshness probe\n");
    let healed = commit(&root, "feat: stale hook fixture [test]");
    let error = stderr(&healed);
    assert_eq!(
        healed.status.code(),
        Some(0),
        "a stale hook heals and never refuses an unrelated commit: {error}"
    );
    // ANTI-VACUITY: the row must EXIST and must not have opted out. A fixture
    // whose covered-source scan set is empty prints GATE_NOT_APPLICABLE, and a
    // leg that accepted that would be measuring nothing.
    assert!(
        error.contains("hook_freshness:") && !error.contains("hook_freshness: GATE_NOT_APPLICABLE"),
        "the freshness row must be present and applicable in this fixture: {error}"
    );
    assert!(
        error.contains("hook_freshness: STALE_HEALING")
            // The heal state is asserted, not just the exit code: a leg that
            // only checked exit=0 would pass on a gate that noticed NOTHING.
            // In this fixture the lock is held (nothing spawns) and no ledger
            // exists, so the honest state is `unknown_no_log` -- the ledger
            // override, not the request outcome.
            && error.contains("heal=unknown_no_log")
            && error.contains("changed=crates/no-shell-gate/src/bin/pre-commit-gate.rs")
            && error.contains("pre-commit"),
        "a touched hook source must be detected, named, and carry a heal state: {error}"
    );
    assert!(
        !error.contains("hook_freshness: CLEAN") && !error.contains("hook_freshness: REFUSED"),
        "a touched hook source may be neither clean nor refused: {error}"
    );
    println!(
        "YQUN_HOOK_HEALING exit={:?} observation={}",
        healed.status.code(),
        refusal_line(&error, "hook_freshness: STALE_HEALING")
    );
    fs::copy(env!("CARGO_BIN_EXE_pre-commit-gate"), &hook).expect("reinstall fresh hook");
    #[cfg(unix)]
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("restore hook mode");
    // The first commit CONSUMED the staged README (it landed rather than being
    // refused), so the post-reinstall commit needs its own staged content:
    // an empty index is a different gate's refusal and not what this leg measures.
    stage(&root, "docs/plan/00-brief.md", b"safe plan text\n");
    let restored = commit(&root, "chore: reinstall fresh hook [test]");
    assert!(
        restored.status.success(),
        "reinstalled hook must pass: {}",
        stderr(&restored)
    );
    assert_eq!(
        fs::read(root.join("crates/no-shell-gate/src/bin/pre-commit-gate.rs"))
            .expect("read restored source"),
        before,
        "source bytes must restore identically"
    );
    let restored_hash = sha256(&root.join("crates/no-shell-gate/src/bin/pre-commit-gate.rs"));
    println!(
        "YQUN_HOOK_RESTORE sha_before={before_hash} sha_after={restored_hash} equal={}",
        before_hash == restored_hash
    );
    fs::remove_dir_all(root).expect("fixture cleanup");
}

/// THE INVARIANT `ic5xr` OWNS, PROVEN TO SURVIVE THE SUPERSESSION: a touched
/// covered source is NEVER SILENTLY CLEAN.
///
/// Distinct from the leg above in the arm it exercises: there the staged set is
/// UNRELATED (README only), here the staged set IS the covered hook source --
/// the gate author's own commit, the exact arm the deleted
/// `staged_touches_covered_source` discriminator used to separate. Both arms
/// must report STALE_HEALING and neither may report CLEAN, which is what makes
/// the discriminator's removal a scoping change rather than a blind spot.
#[test]
fn a_touched_hook_source_is_never_silently_clean() {
    let root = fresh_repo("hook-never-clean");
    stage(
        &root,
        "crates/no-shell-gate/src/bin/pre-commit-gate.rs",
        b"fn pre_commit_fixture() {}\n",
    );
    run_git(
        &root,
        &["commit", "-qm", "chore: hook source baseline [test]"],
        "commit hook source baseline",
    );
    install_hook(&root);
    fs::create_dir_all(root.join(".git/hook-heal.lock")).expect("pre-hold the heal lock");
    // The gate's OWN source is what this commit changes.
    stage(
        &root,
        "crates/no-shell-gate/src/bin/pre-commit-gate.rs",
        b"fn pre_commit_fixture() { /* edited by the gate author */ }\n",
    );
    let landed = commit(&root, "feat: author edits the gate itself [test]");
    let error = stderr(&landed);
    assert!(
        error.contains("hook_freshness:") && !error.contains("hook_freshness: GATE_NOT_APPLICABLE"),
        "the freshness row must be present and applicable in this fixture: {error}"
    );
    assert!(
        !error.contains("hook_freshness: CLEAN"),
        "a commit that edits a covered hook source must never read CLEAN: {error}"
    );
    assert!(
        error.contains("hook_freshness: STALE_HEALING") && error.contains("heal="),
        "the touched source must be reported stale with a heal state: {error}"
    );
    assert_eq!(
        landed.status.code(),
        Some(0),
        "detection is an observation, not a refusal: {error}"
    );
    println!(
        "YQUN_HOOK_NEVER_CLEAN exit={:?} observation={}",
        landed.status.code(),
        refusal_line(&error, "hook_freshness: STALE_HEALING")
    );
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn installed_hook_has_clean_staged_and_empty_index_bands() {
    let clean_root = fresh_repo("clean-staged");
    install_hook(&clean_root);
    stage(&clean_root, "README.md", b"clean staged scratch\n");
    let clean = commit(&clean_root, "chore: clean staged scratch [test]");
    let clean_error = stderr(&clean);
    assert!(clean.status.success(), "clean staged commit must pass: {clean_error}");
    assert!(
        clean_error.contains("empty_staged: CLEAN") && clean_error.contains("CLEAN:"),
        "clean commit must identify both ratchet and top-level state: {clean_error}"
    );
    for marker in [
        "plan_citations: GATE_NOT_APPLICABLE",
        "census_membership: GATE_NOT_APPLICABLE",
        "hook_freshness: GATE_NOT_APPLICABLE",
    ] {
        assert!(clean_error.contains(marker), "clean commit missed {marker}: {clean_error}");
    }
    println!(
        "YQUN_CLEAN exit={:?} clean={}",
        clean.status.code(),
        refusal_line(&clean_error, "empty_staged: CLEAN")
    );
    fs::remove_dir_all(clean_root).expect("clean fixture cleanup");

    let empty_root = fresh_repo("empty-staged");
    install_hook(&empty_root);
    let empty = run_installed_hook(&empty_root);
    let empty_error = stderr(&empty);
    assert_eq!(
        empty.status.code(),
        Some(3),
        "empty staged scratch must use the NOTHING_TO_CHECK band: {empty_error}"
    );
    assert!(
        empty_error.contains("empty_staged: NOTHING_TO_CHECK")
            && empty_error.contains("NOTHING_TO_CHECK: no staged files to check"),
        "empty staged output must name its state: {empty_error}"
    );
    println!(
        "YQUN_EMPTY exit={:?} refusal={}",
        empty.status.code(),
        refusal_line(&empty_error, "empty_staged: NOTHING_TO_CHECK")
    );
    fs::remove_dir_all(empty_root).expect("empty fixture cleanup");
}
