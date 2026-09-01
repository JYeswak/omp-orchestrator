use pane_truth::run_external;
use std::fs;
use std::process::Command;
use std::time::Duration;

fn run(cmd: &str, args: &[&str], envs: &[(&str, &str)]) -> (i32, String) {
    let mut c = Command::new(cmd);
    c.args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = run_external(c, Duration::from_secs(20)).expect("bounded child");
    (out.status.unwrap_or(124), out.stdout)
}

fn fake_tools(root: &std::path::Path) {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("tmux"),
        r#"#!/bin/sh
case "$1" in
  list-panes) printf '0|123|%%0\n' ;;
  capture-pane) printf '%s' "${PANE_TEXT:-claude\nOpus 5 │ bypass permissions\n❯ }" ;;
  has-session) exit 0 ;;
  *) exit 1 ;;
esac
"#,
    )
    .unwrap();
    fs::write(
        bin.join("ps"),
        r#"#!/bin/sh
if [ "$1" = "-eo" ]; then printf "PID PPID %%CPU\n124 123 ${PANE_CPU:-0.0}\n"; else printf '501 1 /bin/sh\n'; fi
"#,
    )
    .unwrap();
    for name in ["tmux", "ps"] {
        let path = bin.join(name);
        let mut perms = fs::metadata(&path).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
        }
        fs::set_permissions(path, perms).unwrap();
    }
}

#[test]
fn shell_and_rust_match_same_pane_inputs() {
    let root = std::env::temp_dir().join(format!("pt-diff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fake_tools(&root);
    let path = root.join("bin").to_string_lossy().to_string();
    let home_local_bin = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| format!("{}/.local/bin", std::path::PathBuf::from(&home).display()))
        .unwrap_or_default();
    let full_path =
        format!("{path}:/opt/homebrew/bin:{home_local_bin}:/usr/bin:/bin:/usr/sbin:/sbin");
    // The shell oracle is located from THIS crate's manifest, not a machine literal
    // (omp-orchestrator-npq): <repo>/bin/pane-truth.sh where <repo> is two levels up.
    let repo_root = std::env::var_os("CONTROL_PLANE_REPO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .expect("crate lives two levels below the repository root")
                .to_path_buf()
        });
    let shell_path = repo_root.join("bin/pane-truth.sh");
    if !shell_path.is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=shell_and_rust_match_same_pane_inputs reason=missing_external_oracle detail={}", shell_path.display());
        return;
    }
    let shell = shell_path.display().to_string();
    let history = root.join("history.jsonl");
    let history_path = history.to_str().unwrap();
    let cases = [
        ("claude\nOpus 5 │ bypass permissions\n❯ ", "0.0", "IDLE"),
        (
            "claude\nWorking (2s - esc to interrupt)\n❯ ",
            "0.5",
            "WORKING",
        ),
        ("claude\nWorked for 2m\n❯ ", "0.0", "DONE"),
        (
            "claude\n  1. Continue\nEnter to select\n❯ ",
            "0.0",
            "AWAITING_INPUT",
        ),
    ];
    for (text, cpu, expected) in cases {
        let env = [
            ("PATH", full_path.as_str()),
            ("PANE_TRUTH_ORACLE", "1"),
            ("PANE_TEXT", text),
            ("PANE_CPU", cpu),
            ("PANE_TRUTH_HISTORY", history_path),
        ];
        let (shell_rc, shell_out) = run("/bin/bash", &[&shell, "fixture"], &env);
        let (rust_rc, rust_out) = run(env!("CARGO_BIN_EXE_pane-truth"), &["fixture"], &env);
        assert_eq!(shell_rc, rust_rc);
        assert!(
            shell_out.contains(&format!("\"verdict\":\"{expected}\"")),
            "shell expected {expected}: {shell_out}"
        );
        assert!(
            rust_out.contains(&format!("\"verdict\":\"{expected}\"")),
            "rust expected {expected}: {rust_out}"
        );
    }
    println!("DIFFERENTIAL PASS cases=4 disagreements=0");
    println!("KNOWN-BAD PROBE visible: two-capture mutation is covered by selftest");
    let _ = fs::remove_dir_all(root);
}
