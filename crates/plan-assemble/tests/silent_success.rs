#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);

struct TempRepo(PathBuf);

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> TempRepo {
    let id = NEXT_REPO.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "plan-assemble-silent-success-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("docs/plan")).expect("create fixture directories");

    write(
        &root,
        "docs/plan/00-brief.md",
        "# Brief\n| Q1 | question |\n",
    );
    write(&root, "docs/plan/01-section.md", "# Section\ncontent\n");
    write(
        &root,
        "docs/plan/SURFACE-MAP.jsonl",
        "{\"disposition\":\"CONSUMED\"}\n",
    );
    write(&root, "docs/plan/FINDINGS.jsonl", "{\"round\":15}\n");
    write(
        &root,
        "docs/plan/CONVERGENCE.jsonl",
        "{\"round\":15,\"new_findings\":1}\n",
    );

    let placeholder = "0".repeat(40);
    let hypothesis = format!(
        r#"{{"id":"fixture-h1","prediction":"plan remains assembled","falsifier":"a missing section","evidence_scope":"docs/plan","recorded_commit":"{placeholder}","observed_result":null}}"#
    );
    write(&root, "docs/plan/HYPOTHESES.jsonl", &(hypothesis + "\n"));
    for round in 15..=21 {
        write(
            &root,
            &format!("docs/plan/round{round}-fixture.jsonl"),
            &format!("{{\"round\":{round}}}\n"),
        );
    }
    for index in 0..4 {
        write(
            &root,
            &format!("docs/plan/round23-fixture-{index}.jsonl"),
            "{\"round\":23}\n",
        );
    }

    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    git(&root, &["config", "user.name", "plan-assemble fixture"]);
    git(&root, &["add", "docs/plan"]);
    git(&root, &["commit", "-qm", "fixture source [test]"]);
    let recorded_commit = git(&root, &["rev-parse", "HEAD"]).trim().to_owned();
    let committed_hypothesis = format!(
        r#"{{"id":"fixture-h1","prediction":"plan remains assembled","falsifier":"a missing section","evidence_scope":"docs/plan","recorded_commit":"{recorded_commit}","observed_result":null}}"#
    );
    write(
        &root,
        "docs/plan/HYPOTHESES.jsonl",
        &(committed_hypothesis + "\n"),
    );
    git(&root, &["add", "docs/plan/HYPOTHESES.jsonl"]);
    git(&root, &["commit", "-qm", "fixture preregistration [test]"]);

    TempRepo(root)
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git command failed: args={args:?} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git fixture output is UTF-8")
}

fn write(root: &Path, relative: &str, contents: &str) {
    fs::write(root.join(relative), contents).expect("write fixture file");
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plan-assemble"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("run plan-assemble")
}

#[test]
fn known_good_assembly_accepts_missing_prior_output() {
    let repo = fixture();
    let target = repo.0.join("docs/PLAN.md");
    assert!(!target.exists(), "fixture must start without prior output");

    let assembled = run(&repo.0, &[]);
    assert!(assembled.status.success(), "assembly failed: {assembled:?}");
    assert!(target.is_file(), "assembly did not create PLAN.md");

    let before_check = fs::read(&target).expect("read assembled output");
    let checked = run(&repo.0, &["--check"]);
    assert!(checked.status.success(), "check failed: {checked:?}");
    let after_check = fs::read(&target).expect("read checked output");
    assert_eq!(before_check, after_check, "--check must remain read-only");
}

#[test]
fn unreadable_required_input_refuses_without_mutating_output() {
    let repo = fixture();
    let target = repo.0.join("docs/PLAN.md");
    fs::write(&target, b"sentinel prior output\n").expect("write sentinel output");

    fs::remove_file(repo.0.join("docs/plan/SURFACE-MAP.jsonl")).expect("remove surface map");
    fs::create_dir(repo.0.join("docs/plan/SURFACE-MAP.jsonl")).expect("plant unreadable input");

    let assembled = run(&repo.0, &[]);
    assert!(
        !assembled.status.success(),
        "assembly accepted unreadable input"
    );
    assert!(String::from_utf8_lossy(&assembled.stderr).contains("cannot read required input"));
    assert_eq!(
        fs::read(&target).expect("read sentinel output"),
        b"sentinel prior output\n"
    );

    let checked = run(&repo.0, &["--check"]);
    assert!(
        !checked.status.success(),
        "--check accepted unreadable input"
    );
    assert!(String::from_utf8_lossy(&checked.stderr).contains("cannot read required input"));
    assert_eq!(
        fs::read(&target).expect("read sentinel output"),
        b"sentinel prior output\n"
    );
}

#[test]
fn mutating_required_section_refuses_then_byte_identical_restore_passes() {
    let repo = fixture();
    let assembled = run(&repo.0, &[]);
    assert!(
        assembled.status.success(),
        "initial assembly failed: {assembled:?}"
    );

    let section = repo.0.join("docs/plan/01-section.md");
    let original = fs::read(&section).expect("read original section");
    fs::write(&section, b"# Section\nmutated\n").expect("mutate section");

    let mutated = run(&repo.0, &["--check"]);
    assert!(!mutated.status.success(), "--check accepted mutated input");
    assert!(
        String::from_utf8_lossy(&mutated.stderr).contains("source fingerprint or manifest differs")
    );

    fs::write(&section, &original).expect("restore section byte-identically");
    let restored = run(&repo.0, &["--check"]);
    assert!(
        restored.status.success(),
        "byte-identical restore did not recover check: {restored:?}"
    );
}

#[test]
fn changed_evidence_without_prior_hypothesis_refuses_before_write() {
        let repo = fixture();
        let target = repo.0.join("docs/PLAN.md");
        fs::write(&target, b"sentinel prior output\n").expect("write sentinel output");
        fs::write(
            repo.0.join("docs/plan/FINDINGS.jsonl"),
            b"{\"round\":15,\"finding\":\"new evidence\"}\n",
        )
        .expect("mutate evidence without citation");

        let assembled = run(&repo.0, &[]);
        assert!(
            !assembled.status.success(),
            "assembly accepted evidence without a prior hypothesis"
        );
        assert!(
            String::from_utf8_lossy(&assembled.stderr).contains("lacks hypothesis_id"),
            "unexpected refusal: {}",
            String::from_utf8_lossy(&assembled.stderr)
        );
        assert_eq!(
            fs::read(&target).expect("read sentinel output"),
            b"sentinel prior output\n"
        );
    }
