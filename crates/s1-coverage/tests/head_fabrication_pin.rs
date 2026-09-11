//! Pins the ABSENCE of the HEAD-fabrication arm deleted by `94bb35f`.
//!
//! That arm sat in `build_manifest` and, for `revision == "HEAD"` on a checkout that cannot
//! resolve it, returned `InputManifest::new(revision, PROVENANCE_PATHS, PROVENANCE_PATHS, [], [])`
//! — a SYNTHESISED all-present manifest. It fired on every depth-zero checkout, so a genuinely
//! absent path still reported present, and it fabricated toward GREEN. The fix carries no test,
//! so a re-add would not go red. This is that test.
//!
//! # Why the pin is STRUCTURAL, measured rather than assumed
//!
//! A behavioural pin was attempted first and it CANNOT discriminate. Measured 2026-09-11 with the
//! arm re-added on a depth-zero fixture: the CLI produced byte-identical output either way —
//! `exit=4`, empty stdout, and
//! `S1_COVERAGE_CHECKOUT_UNUSABLE revision=HEAD ... detail=... invalid object name 'HEAD'`.
//! The reason is structural: when the revision is unresolvable the fabricated manifest is never
//! reachable output, because every downstream step (`report_for` → `read_tree_file` →
//! `git show <rev>:<path>`) needs the same unresolvable revision and fails with the same typed
//! refusal. So the arm changes an INTERMEDIATE value that no CLI surface can emit.
//!
//! `build_manifest` is a private `async fn` in the binary, so no integration test can call it.
//! Making the fabrication observable would mean moving `build_manifest` into the library — a real
//! change in this crate's own design, not a test. Until then the arm is only observable in the
//! source, so that is where it is pinned, and [`the_refusal_contract_for_an_unresolvable_revision`]
//! keeps the behavioural half honest about what it does and does not cover.
//!
//! # Why the assertion keys on the MECHANISM, not on a literal
//!
//! `.git/s1_cov.py` is the *tell* that exposed the fabrication, but it is a mitigation: anyone can
//! apply it to an emitted manifest with no run, and the next rename defeats it. Nor does this test
//! grep for `revision == "HEAD"`, which a reworded re-add (`rev == "HEAD"`, a constant, a helper)
//! would slip past. The mechanism is: **an error arm must never produce a manifest.** A manifest
//! is a claim about a revision; an arm reached *because the revision could not be read* has
//! nothing to base that claim on. That is the invariant the deleted arm violated, and it holds for
//! any wording, any path list, and any future revision special-case.

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

/// The behavioural half. This does NOT pin the fabricating arm — measured, it is identical with
/// the arm present and absent — it pins the typed refusal contract a grader reads: exit 4 means
/// the CHECKOUT is at fault, never this crate, and never a silent success.
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

    let out = Command::new(env!("CARGO_BIN_EXE_s1-coverage"))
        .arg("--repo")
        .arg(&repo)
        .arg("--json")
        .output()
        .expect("the s1-coverage binary was built for this test");
    let code = out.status.code().expect("process produced an exit code");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
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
