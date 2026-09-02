//! Fires-on-known-bad mutation legs. Named RED lines, not just nonzero exit.

use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_admission-reason"))
}

fn run(args: &[&str], env: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(rust_bin());
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn mutation_name_the_failing_gate() {
    let fx =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real-check-sh-ledger.json");
    let off = run(
        &[
            "--ledger",
            fx.to_str().unwrap(),
            "--mutation",
            "--disable-rule",
            "name_the_failing_gate",
        ],
        &[],
    );
    assert!(
        !off.contains("close-evidence"),
        "disabled name_the_failing_gate must not name the RED gate, got {off:?}"
    );
    println!("MUTATION name_the_failing_gate disabled -> RED gate unnamed");

    let on = run(&["--ledger", fx.to_str().unwrap()], &[]);
    assert!(
        on.contains("close-evidence") && on.contains("RED"),
        "rule name_the_failing_gate: real ledger must name close-evidence RED, got {on:?}"
    );
    println!("MUTATION RED name_the_failing_gate: close-evidence RED (real standing verdict)");
}

#[test]
fn mutation_expired_pass_is_not_red() {
    let tmp = std::env::temp_dir().join(format!("ar-mut-exp-{}", std::process::id()));
    std::fs::write(
        &tmp,
        r#"{"schema":"control-plane.check.v1","entries":[{"gate":"docs-staleness","verdict":"PASS","detail":"fine"}],"overall":"PASS","completed_ts":"2020-01-01T00:00:00Z"}"#,
    )
    .unwrap();
    let off = run(
        &[
            "--ledger",
            tmp.to_str().unwrap(),
            "--mutation",
            "--disable-rule",
            "expired_pass_is_not_red",
        ],
        &[("ADMISSION_FRESH_SECONDS", "900")],
    );
    assert!(
        off.contains("did not PASS"),
        "disabled expired_pass_is_not_red must use the RED-gate message, got {off:?}"
    );
    println!("MUTATION expired_pass_is_not_red disabled -> 'did not PASS' (wrong diagnosis)");

    let on = run(
        &["--ledger", tmp.to_str().unwrap()],
        &[("ADMISSION_FRESH_SECONDS", "900")],
    );
    assert!(
        on.contains("EXPIRED") && on.contains("age=") && on.contains("window="),
        "rule expired_pass_is_not_red: expired PASS names age/window, got {on:?}"
    );
    println!("MUTATION RED expired_pass_is_not_red: EXPIRED age= window= (not a RED gate)");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn mutation_repaired_but_unpublished() {
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/planted-repaired-but-unpublished.json");
    let envp = &[
        ("ADMISSION_LIVE_OVERRIDE", "docs-staleness:PASS"),
        ("ADMISSION_FRESH_SECONDS", "900"),
    ];
    let off = run(
        &[
            "--ledger",
            fx.to_str().unwrap(),
            "--publication-check",
            "--mutation",
            "--disable-rule",
            "repaired_but_unpublished",
        ],
        envp,
    );
    assert!(
        !off.contains("REPAIRED_BUT_UNPUBLISHED"),
        "disabled repaired_but_unpublished must not emit the token, got {off:?}"
    );
    println!("MUTATION repaired_but_unpublished disabled -> published RED named, publication failure silent");

    let on = run(
        &["--ledger", fx.to_str().unwrap(), "--publication-check"],
        envp,
    );
    assert!(
        on.contains("REPAIRED_BUT_UNPUBLISHED") && on.contains("docs-staleness"),
        "rule repaired_but_unpublished: planted published-RED + live-PASS must name publication, got {on:?}"
    );
    println!("MUTATION RED repaired_but_unpublished: REPAIRED_BUT_UNPUBLISHED (gate PASSES live; publication is the failure)");
}
