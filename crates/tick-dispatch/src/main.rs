#![forbid(unsafe_code)]

use oracle_compare::{spawn_timeout, spawn_timeout_stdin};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;
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
        if REPO_MARKERS.iter().any(|marker| directory.join(marker).exists()) {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

fn resolve_repo_root(flag: Option<&str>, env_value: Option<String>, start: &std::path::Path) -> Result<PathBuf, RepoRootError> {
    if let Some(flag) = flag {
        if flag.trim().is_empty() {
            return Err(RepoRootError::ExplicitEmpty { source: "--repo".to_owned() });
        }
        return Ok(PathBuf::from(flag));
    }
    if let Some(value) = env_value {
        if value.trim().is_empty() {
            return Err(RepoRootError::ExplicitEmpty { source: "TICK_DISPATCH_REPO".to_owned() });
        }
        return Ok(PathBuf::from(value));
    }
    discover_repo_root(start).ok_or_else(|| RepoRootError::NotFound { from: start.to_path_buf() })
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
    let repo = match resolve_repo_root(repo_flag.as_deref(), std::env::var("TICK_DISPATCH_REPO").ok(), &cwd) {
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

fn run_live(session: &str, pane: &str, vars: &[String], rules: &TickDispatchRules, repo: &std::path::Path) -> ExitCode {
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
        Some(out) => parse_verdict(&String::from_utf8_lossy(&out.stdout), pane),
        None => "UNREADABLE".into(),
    };
    println!("── pane {pane} verdict: {verdict}");
    let force = std::env::var("FORCE_BUSY").ok().as_deref() == Some("1");
    if let Some(code) = pane_step(&verdict, force, rules) {
        return code;
    }

    let mut disc = Command::new(dir.join("pane-error-discriminator.sh"));
    disc.args([session, pane]);
    let (disc_rc, disc_out) = match spawn_timeout(disc, t) {
        Some(o) => (
            o.status.code().unwrap_or(99),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        ),
        None => (99, String::new()),
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
    let rendered = spawn_timeout_stdin(rcmd, t, RENDER_PY.as_bytes())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
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
        if let Some(o) = spawn_timeout(c, Duration::from_secs(10)) {
            let clause = String::from_utf8_lossy(&o.stdout);
            if !clause.trim().is_empty() {
                body = format!("{body}\n{}", clause.trim_end());
            }
        }
    }
    println!(
        "── preflight: scanning the RENDERED packet ({} bytes)",
        body.len()
    );
    let mut pf = Command::new(&ntm_bin);
    pf.args(["preflight", "-", "--json"]);
    if let Some(o) = spawn_timeout_stdin(pf, t, body.as_bytes()) {
        let txt = String::from_utf8_lossy(&o.stdout);
        if txt.trim().is_empty() {
            println!("   preflight: produced nothing (NOT a pass — note it)");
        } else {
            print_preflight(&txt);
        }
    } else {
        println!("   preflight: produced nothing (NOT a pass — note it)");
    }

    let mut chk = Command::new(dir.join("check.sh"));
    chk.arg("--run");
    let check_rc = spawn_timeout(chk, t)
        .map(|o| o.status.code().unwrap_or(99))
        .unwrap_or(99);
    if let TickDispatchDecision::Refuse { exit, detail, .. } = tick_dispatch::check_decision(check_rc, rules) {
        println!("── {detail}");
        return ExitCode::from(exit as u8);
    }

    let mut ready = Command::new(dir.join("pane-dispatch-ready.sh"));
    ready.arg(session).arg(format!("--pane={pane}"));
    let ready_rc = spawn_timeout(ready, t)
        .map(|o| o.status.code().unwrap_or(99))
        .unwrap_or(99);
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
        Err(_) => match std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from) {
            Some(home) => format!("{}/.local/state/flywheel", home.display()),
            None => {
                println!("tick-dispatch RED reason=home_unset: set TICK_DISPATCH_STATE_DIR to an absolute path");
                return ExitCode::from(77);
            }
        },
    };
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
        Some(o) => (
            o.status.code().unwrap_or(99),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        ),
        None => (99, String::new()),
    };
    println!("{send_out}");
    let jq_ok = jq_success(&jq_bin, &send_out, t);
    match send_decision(send_rc, jq_ok, rules) {
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

fn jq_success(jq: &PathBuf, body: &str, t: Duration) -> bool {
    let mut cmd = Command::new(jq);
    cmd.args(["-e", ".success == true"]);
    spawn_timeout_stdin(cmd, t, body.as_bytes())
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn which(name: &str) -> Option<PathBuf> {
    let mut cmd = Command::new("/usr/bin/which");
    cmd.arg(name);
    spawn_timeout(cmd, Duration::from_secs(5)).and_then(|o| {
        if !o.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    })
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
