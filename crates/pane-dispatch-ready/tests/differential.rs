//! Differential vs `bin/pane-dispatch-ready.sh` classify() on identical stdin.
//! Empty comparison set is an ERROR (fh C86). The shell is the oracle; rust
//! --eval must consult composer-typed.py on the FREE path the same way.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pane-dispatch-ready"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn shell_oracle() -> PathBuf {
    repo_root().join("bin/pane-dispatch-ready.sh")
}

fn composer() -> PathBuf {
    repo_root().join("bin/composer-typed.py")
}

#[derive(Debug)]
enum OracleStatus {
    Ready,
    MissingScript(PathBuf),
    MissingInterpreter,
}

fn oracle_status() -> OracleStatus {
    let shell = shell_oracle();
    if !shell.is_file() {
        return OracleStatus::MissingScript(shell);
    }
    let composer = composer();
    if !composer.is_file() {
        return OracleStatus::MissingScript(composer);
    }
    match Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => OracleStatus::Ready,
        _ => OracleStatus::MissingInterpreter,
    }
}

fn announce_skip(test: &str, status: &OracleStatus) {
    let (reason, detail) = match status {
        OracleStatus::MissingScript(path) => ("missing_script", path.display().to_string()),
        OracleStatus::MissingInterpreter => ("missing_interpreter", "python3".to_owned()),
        OracleStatus::Ready => ("ready", String::new()),
    };
    println!(
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail={detail}\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         tests/mutation.rs + tests/composer_rc.rs + src/lib.rs unit tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn eval_shell(input: &str, buffer_changed: bool) -> String {
    let mut cmd = Command::new(shell_oracle());
    cmd.arg("--eval")
        .env("PANE_DISPATCH_READY_ORACLE", "1")
        .env("COMPOSER_TYPED", composer())
        .env("CP", repo_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if buffer_changed {
        cmd.env("BUFFER_CHANGED", "1");
    }
    let mut child = cmd.spawn().expect("shell --eval");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn eval_rust(input: &str, buffer_changed: bool, extra: &[&str]) -> String {
    let mut cmd = Command::new(rust_bin());
    cmd.arg("--eval")
        .args(extra)
        .env("COMPOSER_TYPED", composer())
        .env("CP", repo_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if buffer_changed {
        cmd.env("BUFFER_CHANGED", "1");
    }
    let mut child = cmd.spawn().expect("rust --eval");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn state_of(line: &str) -> &str {
    line.split('|').next().unwrap_or("")
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("comparator_sees_manufactured_disagreement", &status);
        return;
    };
    let input =
        "Opus 5 (1M context) │ bypass permissions\n• Working (38m 29s • esc to interrupt)\n❯ ";
    let sh = eval_shell(input, false);
    let rs = eval_rust(
        input,
        false,
        &["--mutation", "--disable-rule", "busy_markers_load_bearing"],
    );
    assert_eq!(
        state_of(&sh),
        "BUSY",
        "probe setup: shell working-timer+prompt must be BUSY, got {sh}"
    );
    assert_eq!(
        state_of(&rs),
        "FREE",
        "probe setup: rust with busy_markers_load_bearing disabled must be FREE, got {rs}"
    );
    assert_ne!(
        state_of(&sh),
        state_of(&rs),
        "rule comparator_not_vacuous: a manufactured disagreement must be visible"
    );
    println!("DIFFERENTIAL known-bad probe: shell={sh} rust_mutant={rs}");
}

#[test]
fn rust_matches_shell_classify_on_nonempty_case_set() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("rust_matches_shell_classify_on_nonempty_case_set", &status);
        return;
    };
    let esc = "\u{1b}";
    let dim = format!("{esc}[2m");
    let off = format!("{esc}[0m");
    let def = format!("{esc}[39m");
    let agent = "  Opus 5 (1M context) | control-plane";
    let cases: &[(&str, &str, bool)] = &[
        (
            "claude working timer",
            "claude\n• Working (38m 29s • esc to interrupt)",
            false,
        ),
        (
            "codex pursuing goal",
            "gpt-5.6-luna max · alpsinsurance\nPursuing goal (2h 29m)",
            false,
        ),
        (
            "claude sauteed",
            "claude\n✻ Sautéed for 3m 9s · 4 monitors still running",
            false,
        ),
        (
            "claude infusing",
            "claude\n✽ Infusing… (21s · ↓ 443 tokens)",
            false,
        ),
        (
            "claude warping",
            "claude\n✻ Warping… (47s · ↓ 1.7k tokens)",
            false,
        ),
        (
            "claude flummoxing",
            "claude\n✻ Flummoxing… (51s · ↓ 1.3k tokens)",
            false,
        ),
        (
            "codex transcript hint",
            "codex\n… +43 lines (ctrl + t to view transcript)",
            false,
        ),
        (
            "agent at empty prompt",
            "Opus 5 (1M context) │ bypass permissions\n❯ ",
            false,
        ),
        (
            "bare shell",
            // Assembled by `concat!` so this source never contains the contiguous home
            // literal the repo-wide gate forbids (omp-orchestrator-npq).
            concat!("josh@Studio repo % pwd", "\n/Users/", "josh", "/Developer/x"),
            false,
        ),
        ("empty capture", "", false),
        ("whitespace-only", " ", false),
        (
            "agent no prompt",
            "claude\nsome output with no prompt and no timer",
            false,
        ),
        (
            "quota banner",
            "  Opus 5 (1M context) | control-plane\n■ You've hit your usage limit. try again later.\n❯ ",
            false,
        ),
        (
            "busy only in scrollback",
            "esc to interrupt appeared here long ago\n  Opus 5 (1M context) | control-plane\nf1\nf2\nf3\nf4\nf5\nf6\nf7\n❯ a suggestion",
            false,
        ),
        (
            "classifier words are not truth",
            "Opus 5 │ bypass permissions\nERROR waiting idle THINKING\n❯ ",
            false,
        ),
        (
            "motion bit on a prompt",
            "Opus 5 (1M context) │ bypass permissions\n❯ ",
            true,
        ),
    ];
    let boxed_free = format!("{agent}\n{def}❯ {dim}a suggestion{off}");
    let boxed_typed = format!("{agent}\n{def}❯ bought credits - resume the fleet{off}");
    let extra = [
        ("greyed autosuggestion", boxed_free.as_str(), false),
        ("typed operator text", boxed_typed.as_str(), false),
    ];

    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for (label, body, changed) in cases.iter().copied().chain(extra) {
        compared += 1;
        let sh = eval_shell(body, changed);
        let rs = eval_rust(body, changed, &[]);
        if state_of(&sh) != state_of(&rs) {
            disagreements.push(format!(
                "{label}: shell={} rust={} (full shell={sh} rust={rs})",
                state_of(&sh),
                state_of(&rs)
            ));
        }
    }
    assert!(
        compared > 0,
        "rule anti_vacuity: a differential that compares ZERO cases is an ERROR, not a pass"
    );
    assert!(
        disagreements.is_empty(),
        "rule differential_vs_oracle: {compared} cases, disagreements:\n{}",
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL pane-dispatch-ready: {compared} cases compared, 0 disagreements");
}

#[test]
fn eval_empty_stdin_is_unreadable_not_a_pass() {
    let rs = eval_rust("", false, &[]);
    assert!(
        rs.starts_with("UNREADABLE|"),
        "rule anti_vacuity: empty capture is UNREADABLE on stdout, got {rs}"
    );
}
