#![forbid(unsafe_code)]

use oracle_compare::{spawn_timeout, spawn_timeout_stdin};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subprocess_contract::BoundedOutcome;
use tick_dispatch::{admit, send_decision, TickDispatchDecision, TickDispatchRules};

const RENDER_PY: &str = r#"
import re, sys
tpl_path = sys.argv[1]
args = sys.argv[2:]
vars = {}
i = 0
while i < len(args):
    if args[i] == "--var" and i + 1 < len(args):
        k, _, v = args[i+1].partition("=")
        vars[k] = v
        i += 2
    else:
        i += 1
body = open(tpl_path).read()
body = body.split("---", 2)[-1] if body.startswith("---") else body
def cond(m):
    neg, key, inner = m.group(1) == "^", m.group(2), m.group(3)
    has = bool(vars.get(key))
    return inner if (has != neg) else ""
body = re.sub(r"\{\{([#^])(\w+)\}\}(.*?)\{\{/\2\}\}", cond, body, flags=re.S)
body = re.sub(r"\{\{(\w+)\}\}", lambda m: vars.get(m.group(1), ""), body)
sys.stdout.write(body)
"#;

/// Marker entries that identify a repository root while walking up from the cwd.
/// `.git` may be a directory (plain checkout) or a file (worktree/submodule).
const REPO_MARKERS: [&str; 2] = [".git", ".beads"];

/// Fail-closed repository resolution (omp-orchestrator-npq, the same mechanism as
/// omp-idle-dispatch): `--repo` flag > `TICK_DISPATCH_REPO` env > upward marker walk
/// from the cwd. Every failure names what could not be found and the escape hatch.
#[derive(Debug)]
enum RepoRootError {
    ExplicitEmpty { source: String },
    NotFound { from: PathBuf },
}

impl std::fmt::Display for RepoRootError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExplicitEmpty { source } => write!(formatter, "{source} is set but empty"),
            Self::NotFound { from } => write!(
                formatter,
                "no repository marker ({}) found at or above {}; pass --repo <PATH> or set TICK_DISPATCH_REPO",
                REPO_MARKERS.join(" or "),
                from.display()
            ),
        }
    }
}

fn discover_repo_root(start: &std::path::Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(directory) = current {
        if REPO_MARKERS
            .iter()
            .any(|marker| directory.join(marker).exists())
        {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

fn resolve_repo_root(
    flag: Option<&str>,
    env_value: Option<String>,
    start: &std::path::Path,
) -> Result<PathBuf, RepoRootError> {
    if let Some(flag) = flag {
        if flag.trim().is_empty() {
            return Err(RepoRootError::ExplicitEmpty {
                source: "--repo".to_owned(),
            });
        }
        return Ok(PathBuf::from(flag));
    }
    if let Some(value) = env_value {
        if value.trim().is_empty() {
            return Err(RepoRootError::ExplicitEmpty {
                source: "TICK_DISPATCH_REPO".to_owned(),
            });
        }
        return Ok(PathBuf::from(value));
    }
    discover_repo_root(start).ok_or_else(|| RepoRootError::NotFound {
        from: start.to_path_buf(),
    })
}

fn main() -> ExitCode {
    let mut selftest = false;
    let mut eval_admission = false;
    let mut mutation = false;
    let mut repo_flag: Option<String> = None;
    let mut disabled: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => selftest = true,
            "--eval-admission" => eval_admission = true,
            "--mutation" => mutation = true,
            "--disable-rule" => {
                if let Some(v) = args.next() {
                    disabled.push(v);
                }
            }
            "--repo" => {
                repo_flag = args.next();
            }
            "-h" | "--help" => {
                eprintln!("usage: tick-dispatch <session> <pane> --var k=v ...");
                return ExitCode::SUCCESS;
            }
            other => rest.push(other.to_string()),
        }
    }
    let mut rules = TickDispatchRules::default();
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
    if eval_admission {
        let mut buf = String::new();
        let _ = io::stdin().read_to_string(&mut buf);
        return eval_adm(&buf, &rules);
    }
    if rest.len() < 2 {
        eprintln!("usage: tick-dispatch.sh <session> <pane> --var k=v ...");
        return ExitCode::from(2);
    }
    let session = rest[0].clone();
    let pane = rest[1].clone();
    let vars: Vec<String> = rest[2..].to_vec();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            println!("tick-dispatch RED reason=cwd_unreadable detail={error}");
            return ExitCode::from(77);
        }
    };
    let repo = match resolve_repo_root(
        repo_flag.as_deref(),
        std::env::var("TICK_DISPATCH_REPO").ok(),
        &cwd,
    ) {
        Ok(repo) => repo,
        Err(error) => {
            println!("tick-dispatch RED reason=repo_root_not_found detail={error}");
            return ExitCode::from(77);
        }
    };
    run_live(&session, &pane, &vars, &rules, &repo)
}

fn timeout() -> Duration {
    Duration::from_secs(
        std::env::var("TICK_DISPATCH_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120),
    )
}
fn completed(label: &str, outcome: BoundedOutcome) -> Result<Output, String> {
    match outcome {
        BoundedOutcome::Completed(output) => Ok(output),
        BoundedOutcome::TimedOut => Err(format!("{label} timed out before its deadline")),
        BoundedOutcome::Unspawned(error) => Err(format!("{label} could not spawn: {error}")),
    }
}

const DISPATCH_RESULT_PANE: &str = "1";
const DISPATCH_RESULT_SCHEMA: &str = "zs.dispatch-result.v1";
const DISPATCH_RESULT_CONFIDENCE: &str = "unquantified";

fn dispatch_result_row(
    sender: &str,
    session: &str,
    target_pane: &str,
    bead_or_epic: &str,
    outcome: &str,
    detail: &str,
    surface: &str,
    surface_value: &str,
    surface_source: &str,
    result_pane_id: Option<&str>,
    result_pane_resolved_at_unix: Option<u64>,
    result_pane_resolution: &str,
) -> String {
    serde_json::json!({
        "schema": DISPATCH_RESULT_SCHEMA,
        "event": "dispatch_result",
        "sender": sender,
        "session": session,
        "target_pane": target_pane,
        "bead_or_epic": bead_or_epic,
        "outcome": outcome,
        "detail": detail,
        "surface": {
            "name": surface,
            "value": surface_value,
            "source": surface_source,
            "confidence": DISPATCH_RESULT_CONFIDENCE,
        },
        "result_pane_index": DISPATCH_RESULT_PANE,
        "result_pane_id": result_pane_id,
        "result_pane_resolution": result_pane_resolution,
        "result_pane_resolved_at_unix": result_pane_resolved_at_unix,
    })
    .to_string()
}
fn resolve_result_pane_id(session: &str) -> Result<(String, u64), String> {
    let mut command = Command::new("tmux");
    command.args([
        "list-panes",
        "-a",
        "-F",
        "#{pane_id} #{session_name}:#{window_index}.#{pane_index}",
    ]);
    let output = completed(
        "tmux pane-one resolution",
        spawn_timeout(command, Duration::from_secs(10)),
    )?;
    if !output.status.success() {
        return Err(format!(
            "tmux pane-one resolution failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let pane_id = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let pane_id = fields.next()?;
            let location = fields.next()?;
            let (session_window, pane_index) = location.rsplit_once('.')?;
            let (session_name, _) = session_window.rsplit_once(':')?;
            (session_name == session && pane_index == DISPATCH_RESULT_PANE)
                .then(|| pane_id.to_owned())
        })
        .ok_or_else(|| {
            format!(
                "no pane index {} in session {session}",
                DISPATCH_RESULT_PANE
            )
        })?;
    let resolved_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok((pane_id, resolved_at_unix))
}

fn dispatch_result_args(session: &str, pane_id: &str, row: &str) -> Vec<String> {
    vec![
        format!("--robot-send={session}"),
        format!("--panes={pane_id}"),
        format!("--msg={row}"),
    ]
}

fn append_dispatch_result(path: &Path, row: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("dispatch result ledger open {}: {error}", path.display()))?;
    writeln!(file, "{row}")
        .map_err(|error| format!("dispatch result ledger write {}: {error}", path.display()))?;
    file.sync_data()
        .map_err(|error| format!("dispatch result ledger sync {}: {error}", path.display()))
}

fn ntm_result_succeeded(output: &Output) -> bool {
    output.status.success()
        && serde_json::from_slice::<Value>(&output.stdout)
            .ok()
            .and_then(|value| value.get("success").and_then(Value::as_bool))
            == Some(true)
}

fn notify_dispatch_result(
    ntm: &Path,
    session: &str,
    pane_id: Option<&str>,
    row: &str,
    deadline: Duration,
) -> Result<(), String> {
    let pane_id = pane_id.ok_or_else(|| "result pane index 1 could not be resolved".to_owned())?;
    let mut command = Command::new(ntm);
    command.args(dispatch_result_args(session, pane_id, row));
    let output = completed(
        "dispatch result notification",
        spawn_timeout(command, deadline),
    )?;
    if ntm_result_succeeded(&output) {
        return Ok(());
    }
    Err(format!(
        "ntm result notification was not acknowledged: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn dispatch_work_identity(vars: &[String]) -> String {
    for window in vars.windows(2) {
        if window[0] == "--var" {
            if let Some((key, value)) = window[1].split_once('=') {
                if matches!(key, "bead" | "epic") && !value.is_empty() {
                    return value.to_owned();
                }
            }
        }
    }
    "unknown".to_owned()
}

fn eval_adm(buf: &str, rules: &TickDispatchRules) -> ExitCode {
    let mut verdict = "UNREADABLE".to_string();
    let mut force = false;
    let mut disc_rc = 0;
    let mut empty = false;
    let mut check_rc = 0;
    let mut ready_rc = 0;
    let mut pane = "1".to_string();
    let mut send_rc = 0;
    let mut jq_ok = true;
    for line in buf.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "verdict" => verdict = v.trim().to_string(),
                "force_busy" => force = v.trim() == "1",
                "disc_rc" => disc_rc = v.trim().parse().unwrap_or(99),
                "rendered_empty" => empty = v.trim() == "1",
                "check_rc" => check_rc = v.trim().parse().unwrap_or(99),
                "ready_rc" => ready_rc = v.trim().parse().unwrap_or(99),
                "pane" => pane = v.trim().to_string(),
                "send_rc" => send_rc = v.trim().parse().unwrap_or(0),
                "jq_success" => jq_ok = v.trim() == "1" || v.trim() == "true",
                _ => {}
            }
        }
    }
    println!("── pane {pane} verdict: {verdict}");
    match admit(
        &verdict, force, disc_rc, empty, check_rc, ready_rc, &pane, rules,
    ) {
        TickDispatchDecision::Refuse {
            exit,
            reason,
            detail,
        } => {
            if force && reason == "refuse_busy" {
                // unreachable: admit already allowed force
            }
            if verdict != "DONE" && verdict != "IDLE" && force {
                println!("   FORCE_BUSY=1 — dispatching over a {verdict} pane deliberately.");
            }
            println!("   {detail}");
            println!("MUTATION RED {reason}: {detail}");
            return ExitCode::from(exit as u8);
        }
        TickDispatchDecision::Allow => {}
    }
    if verdict != "DONE" && verdict != "IDLE" && force {
        println!("   FORCE_BUSY=1 — dispatching over a {verdict} pane deliberately.");
    }
    match send_decision(send_rc, jq_ok, rules) {
        TickDispatchDecision::Refuse {
            exit,
            reason,
            detail,
        } => {
            println!("── {detail}");
            println!("MUTATION RED {reason}: {detail}");
            ExitCode::from(exit as u8)
        }
        TickDispatchDecision::Allow => {
            println!("ALLOW tick-dispatch admission");
            ExitCode::SUCCESS
        }
    }
}

fn run_live(
    session: &str,
    pane: &str,
    vars: &[String],
    rules: &TickDispatchRules,
    repo: &std::path::Path,
) -> ExitCode {
    let t = timeout();
    let dir = std::env::var("TICK_DISPATCH_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo.join("bin"));
    let py = which("python3");
    let ntm = which("ntm");
    let jq = which("jq");
    if py.is_none() || ntm.is_none() || jq.is_none() {
        println!("tick-dispatch RED reason=required-child-unavailable");
        return ExitCode::from(77);
    }
    let py = py.unwrap();
    let ntm_bin = ntm.unwrap();
    let jq_bin = jq.unwrap();

    let mut pt = Command::new(dir.join("pane-truth.sh"));
    pt.arg(session);
    let verdict = match spawn_timeout(pt, t) {
        BoundedOutcome::Completed(out) => parse_verdict(&String::from_utf8_lossy(&out.stdout), pane),
        BoundedOutcome::TimedOut => {
            println!("── pane-truth timed out before its deadline");
            return ExitCode::from(124u8);
        }
        BoundedOutcome::Unspawned(error) => {
            println!("── pane-truth could not spawn: {error}");
            return ExitCode::from(77u8);
        }
    };
    println!("── pane {pane} verdict: {verdict}");
    let force = std::env::var("FORCE_BUSY").ok().as_deref() == Some("1");
    if let Some(code) = pane_step(&verdict, force, rules) {
        return code;
    }

    let mut disc = Command::new(dir.join("pane-error-discriminator.sh"));
    disc.args([session, pane]);
    let (disc_rc, disc_out) = match spawn_timeout(disc, t) {
        BoundedOutcome::Completed(output) => (
            output.status.code().unwrap_or(99),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ),
        BoundedOutcome::TimedOut => {
            println!("── pane-error-discriminator timed out before its deadline");
            return ExitCode::from(124u8);
        }
        BoundedOutcome::Unspawned(error) => {
            println!("── pane-error-discriminator could not spawn: {error}");
            return ExitCode::from(77u8);
        }
    };
    print!("{disc_out}");
    if !disc_out.ends_with('\n') && !disc_out.is_empty() {
        println!();
    }
    match tick_dispatch::disc_decision(disc_rc, rules) {
        TickDispatchDecision::Refuse { exit, detail, .. } => {
            println!("   {detail}");
            return ExitCode::from(exit as u8);
        }
        TickDispatchDecision::Allow => {}
    }

    let tpl = std::env::var("TEMPLATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dir.join("../ntm-templates/dispatch.md"));
    let mut rcmd = Command::new(&py);
    rcmd.arg("-").arg(tpl.to_str().unwrap_or(""));
    let mut i = 0;
    while i < vars.len() {
        rcmd.arg(&vars[i]);
        i += 1;
    }
    let rendered = match spawn_timeout_stdin(rcmd, t, RENDER_PY.as_bytes()) {
        BoundedOutcome::Completed(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        BoundedOutcome::TimedOut => {
            println!("── packet template render timed out before its deadline");
            return ExitCode::from(124u8);
        }
        BoundedOutcome::Unspawned(error) => {
            println!("── packet template render could not spawn: {error}");
            return ExitCode::from(77u8);
        }
    };
    if let TickDispatchDecision::Refuse { exit, detail, .. } =
        tick_dispatch::render_decision(rendered.is_empty(), rules)
    {
        println!("── {detail}");
        return ExitCode::from(exit as u8);
    }
    let corpus = dir.join("lib/corpus-first.sh");
    let mut body = rendered;
    if corpus.is_file() {
        let mut c = Command::new("/bin/bash");
        c.args([
            "-c",
            "set -uo pipefail; . \"$1\"; corpus_first_contract",
            "corpus-first",
            corpus.to_str().unwrap_or(""),
        ]);
        match spawn_timeout(c, Duration::from_secs(10)) {
            BoundedOutcome::Completed(output) => {
                let clause = String::from_utf8_lossy(&output.stdout);
                if !clause.trim().is_empty() {
                    body = format!("{body}\n{}", clause.trim_end());
                }
            }
            BoundedOutcome::TimedOut => println!("   corpus-first timed out before its deadline"),
            BoundedOutcome::Unspawned(error) => println!("   corpus-first could not spawn: {error}"),
        }
    }
    println!(
        "── preflight: scanning the RENDERED packet ({} bytes)",
        body.len()
    );
    let mut pf = Command::new(&ntm_bin);
    pf.args(["preflight", "-", "--json"]);
    match spawn_timeout_stdin(pf, t, body.as_bytes()) {
        BoundedOutcome::Completed(output) => {
            let txt = String::from_utf8_lossy(&output.stdout);
            if txt.trim().is_empty() {
                println!("   preflight: produced nothing (NOT a pass — note it)");
            } else {
                print_preflight(&txt);
            }
        }
        BoundedOutcome::TimedOut => println!("   preflight timed out before its deadline (NOT a pass)"),
        BoundedOutcome::Unspawned(error) => println!("   preflight could not spawn: {error} (NOT a pass)"),
    }

    let mut chk = Command::new(dir.join("check.sh"));
    chk.arg("--run");
    let check_rc = match spawn_timeout(chk, t) {
        BoundedOutcome::Completed(output) => output.status.code().unwrap_or(99),
        BoundedOutcome::TimedOut => {
            println!("── check timed out before its deadline");
            return ExitCode::from(124u8);
        }
        BoundedOutcome::Unspawned(error) => {
            println!("── check could not spawn: {error}");
            return ExitCode::from(77u8);
        }
    };
    if let TickDispatchDecision::Refuse { exit, detail, .. } =
        tick_dispatch::check_decision(check_rc, rules)
    {
        println!("── {detail}");
        return ExitCode::from(exit as u8);
    }

    let mut ready = Command::new(dir.join("pane-dispatch-ready.sh"));
    ready.arg(session).arg(format!("--pane={pane}"));
    let ready_rc = match spawn_timeout(ready, t) {
        BoundedOutcome::Completed(output) => output.status.code().unwrap_or(99),
        BoundedOutcome::TimedOut => {
            println!("── pane-dispatch-ready timed out before its deadline");
            return ExitCode::from(124u8);
        }
        BoundedOutcome::Unspawned(error) => {
            println!("── pane-dispatch-ready could not spawn: {error}");
            return ExitCode::from(77u8);
        }
    };
    if let TickDispatchDecision::Refuse { exit, detail, .. } =
        tick_dispatch::ready_decision(ready_rc, pane, rules)
    {
        println!("── {detail}");
        return ExitCode::from(exit as u8);
    }

    println!("── sending");
    let fence = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".local/bin/pane-dispatch-fence"))
        .unwrap_or_else(|| PathBuf::from("pane-dispatch-fence"));
    let state = match std::env::var("TICK_DISPATCH_STATE_DIR") {
        Ok(state) => state,
        Err(_) => match std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
        {
            Some(home) => format!("{}/.local/state/flywheel", home.display()),
            None => {
                println!(
                    "tick-dispatch RED reason=home_unset: set TICK_DISPATCH_STATE_DIR to an absolute path"
                );
                return ExitCode::from(77);
            }
        },
    };
    let result_ledger = PathBuf::from(&state).join("tick-ledger.jsonl");
    let mut send = Command::new(&fence);
    send.args([
        "--state-dir",
        &state,
        "--session",
        session,
        "--pane",
        pane,
        "--owner",
        "tick-dispatch",
        "--ready-probe",
        dir.join("pane-dispatch-ready.sh").to_str().unwrap_or(""),
        "--",
        ntm_bin.to_str().unwrap_or("ntm"),
        &format!("--robot-send={session}"),
        &format!("--panes={pane}"),
        &format!("--msg={body}"),
    ]);
    let (send_rc, send_out) = match spawn_timeout(send, t) {
        BoundedOutcome::Completed(output) => (
            output.status.code().unwrap_or(99),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ),
        BoundedOutcome::TimedOut => {
            println!("── pane dispatch fence timed out before its deadline");
            return ExitCode::from(124u8);
        }
        BoundedOutcome::Unspawned(error) => {
            println!("── pane dispatch fence could not spawn: {error}");
            return ExitCode::from(77u8);
        }
    };
    let jq_ok = match jq_success(&jq_bin, &send_out, t) {
        Ok(value) => value,
        Err(error) => {
            println!("── jq result verification unavailable: {error}");
            return ExitCode::from(77u8);
        }
    };
    let decision = send_decision(send_rc, jq_ok, rules);
    let sent = matches!(&decision, TickDispatchDecision::Allow);
    let outcome = if sent {
        "dispatch_transport_succeeded"
    } else {
        "dispatch_transport_failed"
    };
    let detail = if sent {
        "ntm robot-send returned success=true".to_owned()
    } else {
        format!("send_rc={send_rc} jq_success={jq_ok}: {}", send_out.trim())
    };
    let (result_pane_id, result_pane_resolved_at_unix, result_pane_resolution) =
        match resolve_result_pane_id(session) {
            Ok((pane_id, resolved_at_unix)) => (
                Some(pane_id.clone()),
                Some(resolved_at_unix),
                format!("resolved from pane index {DISPATCH_RESULT_PANE}: {pane_id}"),
            ),
            Err(error) => (None, None, format!("pane index resolution failed: {error}")),
        };
    let result_row = dispatch_result_row(
        "tick-dispatch",
        session,
        pane,
        &dispatch_work_identity(vars),
        outcome,
        &detail,
        "pane-truth",
        &verdict,
        "tick-dispatch::pane-truth.sh",
        result_pane_id.as_deref(),
        result_pane_resolved_at_unix,
        &result_pane_resolution,
    );
    if let Err(error) = append_dispatch_result(&result_ledger, &result_row) {
        println!("tick-dispatch RED DISPATCH_RESULT_LEDGER_FAILED {error}");
        return ExitCode::from(77);
    }
    if let Err(error) = notify_dispatch_result(
        ntm_bin.as_path(),
        session,
        result_pane_id.as_deref(),
        &result_row,
        t,
    ) {
        let notify_row = dispatch_result_row(
            "tick-dispatch",
            session,
            pane,
            &dispatch_work_identity(vars),
            "dispatch_result_send_failed",
            &error,
            "pane-truth",
            &verdict,
            "tick-dispatch::pane-truth.sh",
            result_pane_id.as_deref(),
            result_pane_resolved_at_unix,
            &result_pane_resolution,
        );
        if let Err(ledger_error) = append_dispatch_result(&result_ledger, &notify_row) {
            println!(
                "tick-dispatch DISPATCH_RESULT_SEND_FAILED {error}; LEDGER_FAILED {ledger_error}"
            );
        } else {
            println!("tick-dispatch DISPATCH_RESULT_SEND_FAILED {error}");
        }
    }
    match decision {
        TickDispatchDecision::Refuse { exit, detail, .. } => {
            println!("── {detail}");
            ExitCode::from(exit as u8)
        }
        TickDispatchDecision::Allow => {
            println!("── robot send: delivered");
            ExitCode::SUCCESS
        }
    }
}

fn pane_step(verdict: &str, force: bool, rules: &TickDispatchRules) -> Option<ExitCode> {
    match tick_dispatch::pane_decision(verdict, force, rules) {
        TickDispatchDecision::Allow => {
            if verdict != "DONE" && verdict != "IDLE" && force {
                println!("   FORCE_BUSY=1 — dispatching over a {verdict} pane deliberately.");
            }
            None
        }
        TickDispatchDecision::Refuse { exit, detail, .. } => {
            println!("   {detail}");
            println!("   Read its own words first; set FORCE_BUSY=1 only if you mean it.");
            Some(ExitCode::from(exit as u8))
        }
    }
}

fn parse_verdict(json: &str, pane: &str) -> String {
    let Ok(d) = serde_json::from_str::<serde_json::Value>(json) else {
        return "UNREADABLE".into();
    };
    let want: i64 = pane.parse().unwrap_or(-1);
    let Some(arr) = d.get("panes").and_then(|x| x.as_array()) else {
        return "NO_PANE".into();
    };
    for p in arr {
        if p.get("pane_index").and_then(|x| x.as_i64()) == Some(want) {
            return p
                .get("verdict")
                .and_then(|x| x.as_str())
                .unwrap_or("UNREADABLE")
                .to_string();
        }
    }
    "NO_PANE".into()
}

fn print_preflight(txt: &str) {
    let Ok(d) = serde_json::from_str::<serde_json::Value>(txt) else {
        println!("   preflight: unparseable output");
        return;
    };
    let f = d.get("findings").and_then(|x| x.as_array());
    match f {
        Some(arr) if arr.is_empty() => println!("   preflight: clean"),
        Some(arr) => {
            println!("   preflight: {} finding(s)", arr.len());
            for i in arr.iter().take(6) {
                let sev = i.get("severity").and_then(|x| x.as_str()).unwrap_or("");
                let msg = i.get("message").map(|x| x.to_string()).unwrap_or_default();
                let clip: String = msg.chars().take(90).collect();
                println!("     [{sev}] {clip}");
            }
        }
        None => println!("   preflight: clean"),
    }
}

fn jq_success(jq: &PathBuf, body: &str, t: Duration) -> Result<bool, String> {
    let mut cmd = Command::new(jq);
    cmd.args(["-e", ".success == true"]);
    match spawn_timeout_stdin(cmd, t, body.as_bytes()) {
        BoundedOutcome::Completed(output) => Ok(output.status.success()),
        BoundedOutcome::TimedOut => Err("jq verification timed out before its deadline".to_owned()),
        BoundedOutcome::Unspawned(error) => Err(format!("jq verification could not spawn: {error}")),
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let mut cmd = Command::new("/usr/bin/which");
    cmd.arg(name);
    match spawn_timeout(cmd, Duration::from_secs(5)) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!s.is_empty()).then(|| PathBuf::from(s))
        }
        BoundedOutcome::Completed(_) => None,
        BoundedOutcome::TimedOut => {
            eprintln!("which {name} timed out before its deadline");
            None
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("which {name} could not spawn: {error}");
            None
        }
    }
}

fn run_selftest(rules: &TickDispatchRules) -> ExitCode {
    let mut fails = 0;
    if !matches!(
        admit("DONE", false, 0, false, 0, 0, "1", rules),
        TickDispatchDecision::Allow
    ) {
        println!("SELFTEST RED DONE did not admit");
        fails += 1;
    }
    if !matches!(
        admit("WORKING", false, 0, false, 0, 0, "1", rules),
        TickDispatchDecision::Refuse {
            reason: "refuse_busy",
            ..
        }
    ) {
        println!("SELFTEST RED WORKING did not refuse");
        fails += 1;
    }
    if !matches!(
        admit("IDLE", false, 1, false, 0, 0, "1", rules),
        TickDispatchDecision::Refuse {
            reason: "refuse_disc",
            ..
        }
    ) {
        println!("SELFTEST RED disc rc=1 did not refuse");
        fails += 1;
    }
    if fails == 0 {
        println!(
            "SELFTEST PASS tick-dispatch: DONE admits; WORKING refuses; discriminator rc=1 refuses"
        );
        ExitCode::SUCCESS
    } else {
        println!("SELFTEST FAIL tick-dispatch ({fails})");
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod dispatch_result_tests {
    use super::*;

    #[test]
    fn result_args_target_pane_one_and_retain_fields() {
        let row = dispatch_result_row(
            "tick-dispatch",
            "demo",
            "3",
            "bead-1",
            "dispatch_transport_succeeded",
            "ntm robot-send returned success=true",
            "pane-truth",
            "IDLE",
            "tick-dispatch::pane-truth.sh",
            Some("%99"),
            Some(1_710_000_000),
            "resolved from pane index 1: %99",
        );
        let args = dispatch_result_args("demo", "%99", &row);
        assert_eq!(args[0], "--robot-send=demo");
        assert_eq!(args[1], "--panes=%99");
        let value: Value = serde_json::from_str(args[2].strip_prefix("--msg=").unwrap()).unwrap();
        assert_eq!(value["sender"], "tick-dispatch");
        assert_eq!(value["surface"]["value"], "IDLE");
        assert_eq!(value["result_pane_id"], "%99");
        assert_eq!(value["surface"]["confidence"], "unquantified");
        assert_eq!(value["result_pane_resolved_at_unix"], 1_710_000_000);
    }

    #[test]
    fn failed_result_keeps_failure_outcome() {
        let row = dispatch_result_row(
            "tick-dispatch",
            "demo",
            "3",
            "bead-1",
            "dispatch_transport_failed",
            "send decision refused",
            "pane-truth",
            "UNREADABLE",
            "tick-dispatch::pane-truth.sh",
            Some("%99"),
            Some(1_710_000_000),
            "resolved from pane index 1: %99",
        );
        let value: Value = serde_json::from_str(&row).unwrap();
        assert_eq!(value["outcome"], "dispatch_transport_failed");
        assert_eq!(value["surface"]["source"], "tick-dispatch::pane-truth.sh");
    }
}
