#![forbid(unsafe_code)]

use oracle_compare::{spawn_timeout, OracleCompareRules, OracleCompareVerdict};
use pane_oracle_diff::{census, is_agent_command, parse_subject_json};
use std::io::{self, Read};
use std::process::{Command, ExitCode};
use std::time::Duration;

fn say(line: &str) {
    println!("{line}");
}

fn main() -> ExitCode {
    // Home-relative PATH/TMUX segments derive from `$HOME` when set and are omitted
    // when not: omitted is a true statement, never a guess (omp-orchestrator-npq).
    let path = match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => format!(
            "/opt/homebrew/bin:{}/.local/bin:/usr/local/bin:/usr/bin:/bin",
            std::path::PathBuf::from(&home).display()
        ),
        None => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_owned(),
    };
    std::env::set_var("PATH", &path);
    if std::env::var("TMUX_TMPDIR").is_err() {
        if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(std::path::PathBuf::from) {
            std::env::set_var("TMUX_TMPDIR", home.join(".tmux-sockets"));
        }
    }
    let mut selftest = false;
    let mut eval_census = false;
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut session = "control-plane".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => selftest = true,
            "--eval-census" => eval_census = true,
            "--mutation" => mutation = true,
            "--disable-rule" => {
                if let Some(v) = args.next() {
                    disabled.push(v);
                }
            }
            "-h" | "--help" => {
                eprintln!("usage: pane-oracle-diff [session|--selftest]");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
            other => session = other.to_string(),
        }
    }
    let mut rules = OracleCompareRules::default();
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!("usage error: unknown rule {name}");
            return ExitCode::from(2);
        }
    }
    if selftest {
        return run_selftest(&rules);
    }
    if eval_census {
        let mut buf = String::new();
        let _ = io::stdin().read_to_string(&mut buf);
        // oracle_n subject_n session_visible
        let mut it = buf.split_whitespace();
        let o: u64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let subj = it.next().unwrap_or("UNPARSEABLE");
        let vis = it.next() == Some("1");
        let product = if subj == "UNPARSEABLE" {
            Err(())
        } else {
            subj.parse::<u64>().map_err(|_| ())
        };
        let v = census(o, product, vis, &rules);
        emit_census(&session, o, product.ok(), &v);
        return ExitCode::from(v.exit_code() as u8);
    }
    run_live(&session, &rules)
}

fn emit_census(session: &str, oracle_n: u64, subject: Option<u64>, v: &OracleCompareVerdict) {
    match v {
        OracleCompareVerdict::Agree { n } if *n == 0 && oracle_n == 0 => {
            say(&format!(
                "PASS pane-oracle-diff {session}: oracle=0 subject={} (session holds only bare shells; zero agents is the true answer)",
                subject.unwrap_or(0)
            ));
        }
        OracleCompareVerdict::Agree { n } => {
            say(&format!(
                "PASS pane-oracle-diff {session}: oracle={n} subject={n} (agree)"
            ));
        }
        OracleCompareVerdict::Disagree {
            oracle_n,
            product_n,
        } => {
            say(&format!(
                "FINDING pane-oracle-diff {session}: oracle(tmux)={oracle_n} subject(ntm)={product_n} — the surfaces disagree on how many panes exist"
            ));
            if product_n < oracle_n {
                say(&format!(
                    "  NTM UNDERCOUNTS by {}: a pane ntm cannot see is a pane the controller will never dispatch to.",
                    oracle_n - product_n
                ));
            } else {
                say(&format!(
                    "  NTM OVERCOUNTS by {}: a stale projection row can send work to a pane that no longer exists.",
                    product_n - oracle_n
                ));
            }
        }
        OracleCompareVerdict::Unmeasurable { why } if *why == "session_not_visible" => {
            // Verdicts on STDOUT (rubric stdout-verdict). Mirror on stderr for the
            // original `2>&1` capture in controller-tick.sh.
            let line = format!(
                "ERROR pane-oracle-diff: session '{session}' is not visible to tmux at all (tmux unreachable, wrong TMUX_TMPDIR, or session gone) — refusing to report agreement"
            );
            say(&line);
            eprintln!("{line}");
        }
        OracleCompareVerdict::Unmeasurable { .. } => {
            let line = format!(
                "ERROR pane-oracle-diff: subject (ntm --robot-activity={session}) returned no usable projection — fail closed, not a pass"
            );
            say(&line);
            eprintln!("{line}");
        }
    }
}

fn run_live(session: &str, rules: &OracleCompareRules) -> ExitCode {
    let oracle = oracle_panes(session);
    let oracle_n = oracle.len() as u64;
    let visible = session_visible(session);
    let product = subject_count(session);
    let v = census(oracle_n, product, visible, rules);
    emit_census(session, oracle_n, product.ok(), &v);
    if let OracleCompareVerdict::Disagree { .. } = v {
        say(&format!("  oracle panes: {}", oracle.join(",")));
    }
    ExitCode::from(v.exit_code() as u8)
}

fn session_visible(session: &str) -> bool {
    let mut cmd = Command::new("tmux");
    cmd.args(["list-panes", "-a", "-F", "#{session_name}"]);
    let Some(out) = spawn_timeout(cmd, Duration::from_secs(15)) else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|l| l.trim() == session)
}

fn subject_count(session: &str) -> Result<u64, ()> {
    let mut cmd = Command::new("ntm");
    cmd.arg(format!("--robot-activity={session}")).arg("--all");
    let Some(out) = spawn_timeout(cmd, Duration::from_secs(30)) else {
        return Err(());
    };
    parse_subject_json(&String::from_utf8_lossy(&out.stdout))
}

fn oracle_panes(session: &str) -> Vec<String> {
    let mut cmd = Command::new("tmux");
    cmd.args([
        "list-panes",
        "-a",
        "-F",
        "#{pane_id} #{session_name} #{pane_pid} #{pane_current_command}",
    ]);
    let Some(out) = spawn_timeout(cmd, Duration::from_secs(15)) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut it = line.splitn(4, ' ');
        let (Some(pane), Some(sess), Some(pid), Some(fgcmd)) =
            (it.next(), it.next(), it.next(), it.next())
        else {
            continue;
        };
        if sess != session {
            continue;
        }
        if is_agent_command(fgcmd) {
            ids.push(pane.to_string());
            continue;
        }
        if let Some(child) = child_command(pid) {
            if is_agent_command(&child) {
                ids.push(pane.to_string());
            }
        }
    }
    ids.sort();
    ids
}

fn child_command(ppid: &str) -> Option<String> {
    let mut cmd = Command::new("ps");
    cmd.args(["-eo", "ppid,command"]);
    let out = spawn_timeout(cmd, Duration::from_secs(5))?;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let t = line.trim_start();
        let Some((pid, rest)) = t.split_once(char::is_whitespace) else {
            continue;
        };
        if pid == ppid {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn run_selftest(rules: &OracleCompareRules) -> ExitCode {
    let mut fails = 0;
    let chk = |ok: bool, msg: &str, fails: &mut i32| {
        if !ok {
            println!("{msg}");
            *fails += 1;
        }
    };
    let v = census(3, Ok(3), true, rules);
    chk(
        matches!(v, OracleCompareVerdict::Agree { n: 3 }),
        "SELFTEST RED: matching surfaces did not report agreement",
        &mut fails,
    );
    let v = census(4, Ok(3), true, rules);
    chk(
        matches!(
            v,
            OracleCompareVerdict::Disagree {
                oracle_n: 4,
                product_n: 3
            }
        ),
        "SELFTEST RED: ntm undercount was not reported as a finding",
        &mut fails,
    );
    let v = census(2, Ok(5), true, rules);
    chk(
        matches!(
            v,
            OracleCompareVerdict::Disagree {
                oracle_n: 2,
                product_n: 5
            }
        ),
        "SELFTEST RED: ntm overcount was not reported as a finding",
        &mut fails,
    );
    let v = census(0, Ok(0), false, rules);
    chk(
        matches!(v, OracleCompareVerdict::Unmeasurable { .. }),
        "SELFTEST RED: unmeasurable session did not ERROR (vacuous green is the defect)",
        &mut fails,
    );
    let v = census(0, Ok(0), true, rules);
    chk(
        matches!(v, OracleCompareVerdict::Agree { n: 0 }),
        "SELFTEST RED: shells-only session did not PASS (alarm on a legitimate state)",
        &mut fails,
    );
    let v = census(2, Err(()), true, rules);
    chk(
        matches!(v, OracleCompareVerdict::Unmeasurable { .. }),
        "SELFTEST RED: unreadable subject did not ERROR",
        &mut fails,
    );
    let home_codex = format!(
        "node {}/.local/bin/codex --dangerously-bypass-approvals",
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|home| home.display().to_string())
            .unwrap_or_default()
    );
    let cases = [
        (
            "claude --dangerously-skip-permissions --model claude-opus-4-8",
            true,
        ),
        (
            "claude --dangerously-skip-permissions --resume 3db92271-36d8",
            true,
        ),
        (home_codex.as_str(), true),
        ("grok --always-approve", true),
        ("npm exec comfyui-mcp", false),
        ("-zsh", false),
        ("node /some/other/server.js", false),
    ];
    for (cmd, want) in cases {
        let got = is_agent_command(cmd);
        chk(
            got == want,
            &format!(
                "SELFTEST RED: agent allow-list wrong for [{cmd}] — expected {want} got {got}"
            ),
            &mut fails,
        );
    }
    if fails == 0 {
        say("SELFTEST PASS pane-oracle-diff: agreement PASSes; undercount and overcount BOTH trip; empty oracle and unreadable subject ERROR rather than pass");
        ExitCode::SUCCESS
    } else {
        say(&format!(
            "SELFTEST FAIL pane-oracle-diff ({fails} leg(s) red)"
        ));
        ExitCode::from(1)
    }
}
