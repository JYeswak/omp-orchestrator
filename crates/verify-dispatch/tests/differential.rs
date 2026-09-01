//! Differential vs `bin/verify-dispatch.py`. Same input on both sides.
//!
//! A comparator whose two sides receive different inputs is not comparing
//! anything (bin/close-evidence-differential.sh header). ANTI-VACUITY (fh C86):
//! comparing ZERO cases is an ERROR, not a pass. The first leg proves the
//! comparator can SEE a disagreement it manufactured, before the N-case run.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under repo/crates/verify-dispatch")
        .to_path_buf()
}

fn python_oracle() -> PathBuf {
    repo_root().join("bin/verify-dispatch.py")
}

#[derive(Debug)]
enum OracleStatus {
    Ready(PathBuf),
    MissingScript(PathBuf),
    MissingInterpreter,
}

fn oracle_status() -> OracleStatus {
    let script = python_oracle();
    if !script.is_file() {
        return OracleStatus::MissingScript(script);
    }
    match Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => OracleStatus::Ready(script),
        _ => OracleStatus::MissingInterpreter,
    }
}

fn announce_skip(test: &str, status: &OracleStatus) {
    let (reason, detail) = match status {
        OracleStatus::MissingScript(path) => ("missing_script", path.display().to_string()),
        OracleStatus::MissingInterpreter => ("missing_interpreter", "python3".to_owned()),
        OracleStatus::Ready(_) => ("ready", String::new()),
    };
    println!(
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail={detail}\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         src/lib.rs unit tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_verify-dispatch"))
}

fn fake_br_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("verify-dispatch-fakebr-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("fake br dir");
    let br = dir.join("br");
    // Maps bead id prefix onto status. closed-* → closed, anything else → open.
    fs::write(
        &br,
        r#"#!/bin/sh
bead="$2"
case "$bead" in
  closed-*) printf '[{"id":"%s","status":"closed"}]\n' "$bead" ;;
  missing-*) exit 1 ;;
  *) printf '[{"id":"%s","status":"open"}]\n' "$bead" ;;
esac
"#,
    )
    .expect("write fake br");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&br).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&br, p).unwrap();
    }
    dir
}

fn path_with_fake_br(fake: &Path) -> String {
    let home_local_bin = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| format!("{}/.local/bin", std::path::PathBuf::from(&home).display()))
        .unwrap_or_default();
    format!("{}:/opt/homebrew/bin:{home_local_bin}:/usr/bin:/bin", fake.display())
}

fn stamp_now() -> String {
    verify_dispatch::local_wall_z_stamp()
}

fn stamp_old() -> String {
    "1999-01-01T00:00:00Z".to_string()
}

struct Case {
    name: &'static str,
    ledger: String,
}

fn cases() -> Vec<Case> {
    let now = stamp_now();
    let old = stamp_old();
    vec![
        Case {
            name: "missing-ledger",
            ledger: String::new(), // special: file will not exist
        },
        Case {
            name: "empty-ledger",
            ledger: String::new(),
        },
        Case {
            name: "old-window",
            ledger: format!(
                "{{\"ts\":\"{old}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"closed-a\"]}}\n"
            ),
        },
        Case {
            name: "closed-bead",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"pane\":\"1\",\"count\":1,\"beads\":[\"closed-a\"],\"invoker\":\"TEST\"}}\n"
            ),
        },
        Case {
            name: "open-bead",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"open-a\"],\"invoker\":\"TEST\"}}\n"
            ),
        },
        Case {
            name: "mixed",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"closed-a\",\"open-b\"],\"invoker\":\"TEST\"}}\n"
            ),
        },
        Case {
            name: "legacy-idless",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"pane\":\"1\",\"count\":2,\"invoker\":\"TEST\"}}\n"
            ),
        },
        Case {
            name: "non-dispatched-event",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"tick\",\"repo\":\"control-plane\",\"beads\":[\"closed-a\"]}}\n"
            ),
        },
        Case {
            name: "malformed-then-closed",
            ledger: format!(
                "not json\n{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"closed-a\"]}}\n"
            ),
        },
        Case {
            name: "missing-repo",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"definitely-not-a-developer-repo-zzz\",\"beads\":[\"closed-a\"]}}\n"
            ),
        },
        Case {
            name: "two-dispatches-same-repo",
            ledger: format!(
                "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"closed-a\"]}}\n{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"open-b\"]}}\n"
            ),
        },
        Case {
            name: "blank-and-comment-lines",
            ledger: format!(
                "\n\n{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"closed-a\"]}}\n"
            ),
        },
    ]
}

fn run_side(bin: &Path, is_python: bool, ledger: Option<&Path>, out: &Path, path: &str, extra: &[&str]) -> (i32, String) {
    let mut cmd = if is_python {
        let mut c = Command::new("python3");
        c.arg(bin);
        c
    } else {
        Command::new(bin)
    };
    cmd.env("PATH", path)
        .env("VERIFY_OUT", out)
        .env(
            "HOME",
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .unwrap_or_default(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match ledger {
        Some(p) => {
            cmd.env("VERIFY_LEDGER", p);
        }
        None => {
            cmd.env(
                "VERIFY_LEDGER",
                std::env::temp_dir().join(format!("verify-dispatch-no-such-ledger-{}", std::process::id())),
            );
        }
    }
    for a in extra {
        cmd.arg(a);
    }
    let output = cmd.output().expect("spawn side");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    (output.status.code().unwrap_or(99), stdout)
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn comparator_sees_a_manufactured_disagreement_before_trusting_agreement() {
    let status = oracle_status();
    let OracleStatus::Ready(script) = &status else {
        announce_skip(
            "comparator_sees_a_manufactured_disagreement_before_trusting_agreement",
            &status,
        );
        return;
    };
    let fake = fake_br_dir();
    let path = path_with_fake_br(&fake);
    let dir = std::env::temp_dir().join(format!(
        "verify-dispatch-probe-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let ledger = dir.join("ledger.jsonl");
    let py_out = dir.join("py.jsonl");
    let rs_out = dir.join("rs.jsonl");
    let now = stamp_now();
    fs::write(
        &ledger,
        format!(
            "{{\"ts\":\"{now}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"open-a\"]}}\n"
        ),
    )
    .unwrap();

    let (_py_rc, py) = run_side(script, true, Some(&ledger), &py_out, &path, &[]);
    let (_rs_rc, rs) = run_side(
        &rust_bin(),
        false,
        Some(&ledger),
        &rs_out,
        &path,
        &["--mutation", "--disable-rule", "named_beads_all_closed"],
    );
    assert!(
        normalize(&py).contains("NO EVIDENCE"),
        "probe setup: python oracle must say NO EVIDENCE on an open bead, got {py:?}"
    );
    assert!(
        normalize(&rs).contains("VERIFIED"),
        "probe setup: rust with named_beads_all_closed disabled must false-PASS, got {rs:?}"
    );
    assert_ne!(
        normalize(&py),
        normalize(&rs),
        "rule comparator_not_vacuous: a manufactured disagreement must be visible; if this assertion is deleted the N-case agreement proves nothing"
    );
}

#[test]
fn rust_matches_python_oracle_on_nonempty_case_set() {
    let status = oracle_status();
    let OracleStatus::Ready(script) = &status else {
        announce_skip("rust_matches_python_oracle_on_nonempty_case_set", &status);
        return;
    };
    let fake = fake_br_dir();
    let path = path_with_fake_br(&fake);
    let dir = std::env::temp_dir().join(format!(
        "verify-dispatch-diff-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();

    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for (i, case) in cases().iter().enumerate() {
        let ledger_path = dir.join(format!("{i}-{}.jsonl", case.name));
        let py_out = dir.join(format!("{i}-py.jsonl"));
        let rs_out = dir.join(format!("{i}-rs.jsonl"));
        let ledger_arg = if case.name == "missing-ledger" {
            None
        } else {
            let mut f = fs::File::create(&ledger_path).unwrap();
            f.write_all(case.ledger.as_bytes()).unwrap();
            Some(ledger_path.as_path())
        };
        let (py_rc, py) = run_side(script, true, ledger_arg, &py_out, &path, &[]);
        let (rs_rc, rs) = run_side(&rust_bin(), false, ledger_arg, &rs_out, &path, &[]);
        compared += 1;
        if py_rc != rs_rc || normalize(&py) != normalize(&rs) {
            disagreements.push(format!(
                "case {} rc py={} rs={}\n-- python --\n{}\n-- rust --\n{}",
                case.name, py_rc, rs_rc, py, rs
            ));
        }
        let _ = (py_out, rs_out);
    }
    assert!(
        compared > 0,
        "rule anti_vacuity: a differential that compares ZERO cases is an ERROR, not a pass"
    );
    assert!(
        disagreements.is_empty(),
        "{} disagreement(s) of {compared} cases:\n{}",
        disagreements.len(),
        disagreements.join("\n\n")
    );
    // Also emit a column-0 verdict so `cargo test` itself is greppable as a gate.
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}
