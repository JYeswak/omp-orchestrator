//! Differential vs `bin/fleet-reconcile.sh`. Same fixture dir on both sides.
//! Empty comparison set is an ERROR (fh C86).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate under repo/crates/fleet-reconcile")
        .to_path_buf()
}

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fleet-reconcile"))
}

fn shell() -> PathBuf {
    let root = std::env::var_os("CONTROL_PLANE_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root());
    root.join("bin/fleet-reconcile.sh")
}

fn write_fix(dir: &std::path::Path, tmux: &str, list: &str, snap: &str, ft: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("tmux-sessions.txt"), tmux).unwrap();
    fs::write(dir.join("ntm-list.txt"), list).unwrap();
    fs::write(dir.join("snapshot.json"), snap).unwrap();
    fs::write(dir.join("ft-state.json"), ft).unwrap();
}

fn run_side(bin: &std::path::Path, args: &[&str], fixture: &std::path::Path) -> (i32, String) {
    let out = Command::new(bin)
        .args(args)
        .env("FLEET_RECONCILE_FIXTURE_DIR", fixture)
        .env(
            "PATH",
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| format!(
                    "/opt/homebrew/bin:{}/.local/bin:/usr/bin:/bin",
                    std::path::PathBuf::from(&home).display()
                ))
                .unwrap_or_else(|| "/opt/homebrew/bin:/usr/bin:/bin".to_owned()),
        )
        .output()
        .expect("spawn");
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn core(json: &str) -> (String, String, i64, i64) {
    let v: serde_json::Value = serde_json::from_str(json.trim()).unwrap_or(serde_json::Value::Null);
    (
        v.get("detector")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        v.get("verdict")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        v.get("tmux_count").and_then(|x| x.as_i64()).unwrap_or(-1),
        v.get("ntm_count").and_then(|x| x.as_i64()).unwrap_or(-1),
    )
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    if !shell().is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=comparator_sees_manufactured_disagreement reason=missing_external_oracle detail={}", shell().display());
        return;
    }
    let dir = std::env::temp_dir().join(format!("fr-probe-{}", std::process::id()));
    write_fix(
        &dir,
        "alpha\nbeta\n",
        "  alpha: 1 pane\n",
        r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha"}]}"#,
        "",
    );
    let (py_rc, py_out) = run_side(&shell(), &["--json"], &dir);
    let (rs_rc, rs_out) = run_side(
        &rust_bin(),
        &[
            "--json",
            "--mutation",
            "--disable-rule",
            "name_sets_must_agree",
        ],
        &dir,
    );
    let (_d, py_ver, _, _) = core(&py_out);
    let (_d2, rs_ver, _, _) = core(&rs_out);
    assert_eq!(
        py_ver, "FAIL",
        "probe setup: shell must FAIL a name-set disagree"
    );
    assert_eq!(
        rs_ver, "PASS",
        "probe setup: rust with name_sets_must_agree disabled must PASS"
    );
    assert_ne!(py_rc, rs_rc);
    assert_ne!(
        py_ver, rs_ver,
        "rule comparator_not_vacuous: a manufactured disagreement must be visible"
    );
}

#[test]
fn rust_matches_shell_on_nonempty_fixture_set() {
    if !shell().is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=rust_matches_shell_on_nonempty_fixture_set reason=missing_external_oracle detail={}", shell().display());
        return;
    }
    let root = std::env::temp_dir().join(format!("fr-diff-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let mut compared = 0usize;
    let mut disagreements = Vec::new();

    let cases: &[(&str, &str, &str, &str, &str)] = &[
        (
            "agree",
            "alpha\nbeta\n",
            "  alpha: 2 panes\n  beta: 1 pane\n",
            r#"{"success":true,"summary":{"total_sessions":2},"sessions":[{"name":"alpha"},{"name":"beta"}]}"#,
            "",
        ),
        (
            "empty-success",
            "control-plane\nalpsinsurance\n",
            "  control-plane: 3 panes\n",
            r#"{"success":true,"summary":{"total_sessions":0},"sessions":[]}"#,
            "",
        ),
        (
            "empty-text",
            "control-plane\n",
            "No tmux sessions running\n",
            r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"control-plane"}]}"#,
            "",
        ),
        (
            "disagree",
            "alpha\nbeta\n",
            "  alpha: 1 pane\n",
            r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha"}]}"#,
            "",
        ),
        ("unparseable", "alpha\n", "alpha:\n", "not json", ""),
        (
            "classifier-error-ignored",
            "alpha\n",
            "  alpha: 1 pane\n",
            r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha","agents":[{"state":"error","pane":"1"}]}]}"#,
            "",
        ),
        (
            "ft-ok-does-not-change",
            "alpha\n",
            "  alpha: 1 pane\n",
            r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha"}]}"#,
            r#"{"ok":true}"#,
        ),
        (
            "ok-true-alias",
            "alpha\n",
            "alpha\n",
            r#"{"ok":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha"}]}"#,
            "",
        ),
    ];

    for (name, tmux, list, snap, ft) in cases {
        let dir = root.join(name);
        write_fix(&dir, tmux, list, snap, ft);
        compared += 1;
        let (py_rc, py_out) = run_side(&shell(), &["--json"], &dir);
        let (rs_rc, rs_out) = run_side(&rust_bin(), &["--json"], &dir);
        let py = core(&py_out);
        let rs = core(&rs_out);
        if py != rs || py_rc != rs_rc {
            disagreements.push(format!(
                "{name}: shell={py:?}/rc={py_rc} rust={rs:?}/rc={rs_rc}\n  shell_out={}\n  rust_out={}",
                py_out.trim(),
                rs_out.trim()
            ));
        }
    }

    assert!(
        compared > 0,
        "rule anti_vacuity: a differential that compares ZERO cases is an ERROR, not a pass"
    );
    assert!(
        disagreements.is_empty(),
        "{} disagreement(s) of {compared} cases:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}
