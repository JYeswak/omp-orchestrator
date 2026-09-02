//! Differential vs the control-plane `bin/admission-reason.sh --ledger` oracle on identical fixtures.
//! Empty comparison set is an ERROR.

use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_admission-reason"))
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn shell() -> PathBuf {
    let oracle = std::env::var_os("OMP_CONTROL_PLANE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join("../control-plane"))
        .join("bin/admission-reason.sh");
    assert!(
        oracle.is_file(),
        "missing shell oracle at {}; set OMP_CONTROL_PLANE_ROOT to the control-plane checkout",
        oracle.display()
    );
    oracle
}

fn run_shell_output(ledger: &std::path::Path) -> std::process::Output {
    let out = Command::new(shell())
        .args(["--ledger", ledger.to_str().unwrap()])
        .env("ADMISSION_REASON_ORACLE", "1")
        .output()
        .expect("shell");
    out
}

fn run_shell(ledger: &std::path::Path) -> String {
    let out = run_shell_output(ledger);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn run_rust_output(ledger: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let out = Command::new(rust_bin())
        .arg("--ledger")
        .arg(ledger)
        .args(extra)
        .output()
        .expect("rust");
    out
}

fn run_rust(ledger: &std::path::Path, extra: &[&str]) -> String {
    let out = run_rust_output(ledger, extra);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn norm(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/planted-repaired-but-unpublished.json");
    let sh = run_shell(&fx);
    let rs = run_rust(
        &fx,
        &[
            "--publication-check",
            "--mutation",
            "--disable-rule",
            "name_the_failing_gate",
        ],
    );
    assert!(
        sh.contains("docs-staleness"),
        "probe setup: shell must name the planted RED gate, got {sh:?}"
    );
    assert!(
        !rs.contains("docs-staleness"),
        "probe setup: rust with name_the_failing_gate disabled must not name it, got {rs:?}"
    );
    assert_ne!(norm(&sh), norm(&rs), "rule comparator_not_vacuous");
    println!("DIFFERENTIAL known-bad probe: shell names docs-staleness; mutant does not");
}

#[test]
fn rust_matches_shell_on_nonempty_case_set() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let cases = [
        dir.join("real-check-sh-ledger.json"),
        dir.join("planted-repaired-but-unpublished.json"),
    ];
    let tmp = std::env::temp_dir().join(format!("ar-diff-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let extra = [
        (
            "red",
            r#"{"schema":"control-plane.check.v1","entries":[{"gate":"domain-closure","verdict":"RED","detail":"{\"violations\":[{\"code\":\"E003\",\"row\":\"zestgraph-capture\",\"detail\":\"drift anchor mismatch: row says aaa, artifact is bbb\"}]}"}]}"#,
        ),
        (
            "expired",
            r#"{"schema":"control-plane.check.v1","entries":[{"gate":"docs-staleness","verdict":"PASS","detail":"fine"}],"overall":"PASS","completed_ts":"2020-01-01T00:00:00Z"}"#,
        ),
        (
            "incomplete",
            r#"{"schema":"control-plane.check.v1","entries":[{"gate":"docs-staleness","verdict":"PASS","detail":"fine"}]}"#,
        ),
        (
            "unrun",
            r#"{"schema":"control-plane.check.v1","entries":[{"gate":"tests","verdict":"UNRUN","detail":"skipped-after-domain-closure"}]}"#,
        ),
        ("missing-path-is-a-case", ""),
    ];
    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for p in &cases {
        compared += 1;
        let sh = run_shell(p);
        let rs = run_rust(p, &[]);
        if norm(&sh) != norm(&rs) {
            disagreements.push(format!("{}: shell={sh:?} rust={rs:?}", p.display()));
        }
    }
    for (name, body) in extra {
        compared += 1;
        let p = tmp.join(format!("{name}.json"));
        if name == "missing-path-is-a-case" {
            let missing = tmp.join("no-such.json");
            let sh = run_shell_output(&missing);
            let rs = run_rust_output(&missing, &[]);
            // Expected hardening divergence: the shell oracle preserves its
            // historical successful exit while naming the missing ledger; Rust
            // must fail closed with a machine-readable ledger_missing error.
            assert!(
                sh.status.success(),
                "shell missing-ledger exit shape changed: {sh:?}"
            );
            assert!(
                String::from_utf8_lossy(&sh.stdout).contains("no check.sh ledger"),
                "shell missing-ledger reason changed: {sh:?}"
            );
            assert!(
                !rs.status.success(),
                "rust missing-ledger unexpectedly succeeded: {rs:?}"
            );
            assert!(
                String::from_utf8_lossy(&rs.stderr).contains("ledger_missing"),
                "rust missing-ledger error changed: {rs:?}"
            );
            continue;
        }
        std::fs::write(&p, body).unwrap();
        let sh = run_shell(&p);
        let rs = run_rust(&p, &[]);
        if norm(&sh) != norm(&rs) {
            disagreements.push(format!("{name}: shell={sh:?} rust={rs:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(compared > 0, "rule anti_vacuity: ZERO cases is an ERROR");
    assert!(
        disagreements.is_empty(),
        "rule differential_vs_oracle: {compared} cases, disagreements:\n{}",
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL admission-reason: {compared} cases compared, 0 disagreements");
}
