#![forbid(unsafe_code)]

//! Live pane-dispatch-ready binary. Verdicts on STDOUT. stderr is usage only.

use pane_dispatch_ready::{
    apply_composer_rc, capture_snapshot, classify, confirm_free, missing_composer, spawn_timeout,
    PaneDispatchReadyRules, PaneDispatchReadyState, TWO_CAPTURE_MIN_SECS,
};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output_stdin, BoundedOutcome};

fn say(line: &str) {
    println!("{line}");
}
fn completed(label: &str, outcome: BoundedOutcome) -> Option<Output> {
    match outcome {
        BoundedOutcome::Completed(output) => Some(output),
        BoundedOutcome::TimedOut => {
            eprintln!("pane-dispatch-ready: {label} timed out before its deadline");
            None
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("pane-dispatch-ready: {label} could not spawn: {error}");
            None
        }
    }
}

/// Per-pane `local_state.is_rate_limited`, keyed by pane index, from `ntm --robot-agent-health`.
///
/// ⛔ BRANCH ON `success` FIRST. A not-found payload from these verbs still carries populated,
/// ZEROED objects, so a consumer reading a field without checking the envelope gets a plausible
/// answer from a call that failed. Measured on two verbs, 2026-09-11.
///
/// ⛔ AND NEVER `local_state.safe_to_dispatch`: measured the same day, it is TRUE on two panes
/// the same payload concurrently flags rate-limited and which are dead for ~83 hours. ANDing
/// with it would inherit the exact defect this refusal exists to fix.
///
/// Unreachable, unparsable or unsuccessful -> EMPTY MAP, i.e. no additional refusal. This layer
/// only ever SUBTRACTS from dispatchability; when it cannot ask, it leaves `classify`'s verdict
/// exactly as it found it rather than inventing a refusal it cannot support.
fn rate_limited_panes(session: &str) -> std::collections::BTreeMap<String, bool> {
    let mut command = Command::new("ntm");
    command.args([&format!("--robot-agent-health={session}"), "--no-caut"]);
    let Some(output) = completed("ntm agent-health", spawn_timeout(command, Duration::from_secs(30)))
    else {
        return std::collections::BTreeMap::new();
    };
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return std::collections::BTreeMap::new();
    };
    if document.get("success").and_then(serde_json::Value::as_bool) != Some(true) {
        return std::collections::BTreeMap::new();
    }
    let Some(panes) = document.get("panes").and_then(serde_json::Value::as_object) else {
        return std::collections::BTreeMap::new();
    };
    panes
        .iter()
        .filter_map(|(index, pane)| {
            pane.get("local_state")?
                .get("is_rate_limited")?
                .as_bool()
                .map(|limited| (index.clone(), limited))
        })
        .collect()
}
fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn composer_rc(tail: &str, path: &str) -> i32 {
    if !PathBuf::from(path).is_file() {
        return 99;
    }
    let mut cmd = if path.ends_with(".py") {
        let mut command = Command::new("python3");
        command.arg(path);
        command
    } else {
        Command::new(path)
    };
    match bounded_output_stdin(&mut cmd, Duration::from_secs(5), tail.as_bytes()) {
        BoundedOutcome::Completed(output) => output.status.code().unwrap_or(99),
        BoundedOutcome::TimedOut => {
            eprintln!("pane-dispatch-ready: composer probe timed out before its deadline");
            124
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("pane-dispatch-ready: composer probe could not spawn: {error}");
            77
        }
    }
}

fn tail_n(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= n {
        text.to_string()
    } else {
        lines[lines.len() - n..].join("\n")
    }
}

fn main() -> ExitCode {
    // Home-relative PATH/TMUX segments derive from `$HOME` when set and are omitted
    // when not: omitted is a true statement, never a guess (omp-orchestrator-npq).
    let path = match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => format!(
            "/opt/homebrew/bin:{}/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            std::path::PathBuf::from(&home).display()
        ),
        None => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    };
    std::env::set_var("PATH", &path);
    if std::env::var("TMUX_TMPDIR").is_err() {
        if let Some(home) = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
        {
            std::env::set_var("TMUX_TMPDIR", home.join(".tmux-sockets"));
        }
    }

    let mut json = false;
    let mut selftest = false;
    let mut eval = false;
    let mut mutation = false;
    let mut pane_filter: Option<String> = None;
    let mut sessions: Vec<String> = Vec::new();
    let mut disabled: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
            "--selftest" => selftest = true,
            "--eval" => eval = true,
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!(
                    "usage: pane-dispatch-ready [session ...] [--pane=N] [--json|--selftest]"
                );
                return ExitCode::SUCCESS;
            }
            other if other.starts_with("--pane=") => {
                let n = &other[7..];
                if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) {
                    eprintln!("invalid --pane: {n}");
                    return ExitCode::from(2);
                }
                pane_filter = Some(n.to_string());
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
            other => sessions.push(other.to_string()),
        }
    }
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    let mut rules = PaneDispatchReadyRules::default();
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                PaneDispatchReadyRules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }
    if selftest {
        return run_selftest(&rules);
    }
    if eval {
        let mut buf = String::new();
        let _ = io::stdin().read_to_string(&mut buf);
        let changed = std::env::var("BUFFER_CHANGED").ok().as_deref() == Some("1");
        // Composer is an EXTERNAL command (bin/composer-typed.py). The shell
        // classify() always consults it on the FREE path; --eval must too or
        // the differential would compare a weaker classifier.
        let v = classify_with_composer(&buf, changed, &rules);
        say(&v.pipe_line());
        return if v.state == PaneDispatchReadyState::Free {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    run_live(json, &sessions, pane_filter.as_deref(), &rules)
}

/// Repository root for `bin/` helpers: `CP` env > upward `.git`/`.beads` marker walk from
/// the cwd — the omp-orchestrator-npq mechanism, never a literal, because a wrong-but-
/// plausible root silently scans the wrong repo.
fn composer_repo_root() -> Result<std::path::PathBuf, String> {
    if let Some(root) = std::env::var_os("CP").filter(|v| !v.is_empty()) {
        return Ok(std::path::PathBuf::from(root));
    }
    let mut current = std::env::current_dir()
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    loop {
        if [".git", ".beads"]
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            return Ok(current);
        }
        let Some(parent) = current.parent() else {
            return Err(format!(
                "no repository marker (.git or .beads) found at or above {}; set CP or run from a checkout",
                current.display()
            ));
        };
        current = parent.to_path_buf();
    }
}

fn composer_path() -> String {
    // An EMPTY `COMPOSER_TYPED` is NOT a configured path. `var()` returns `Ok("")`
    // for `COMPOSER_TYPED=`, which silently suppressed the discovery ladder below
    // and fail-closed every FREE pane with a BLANK path in the operator's reason
    // ("composer discriminator missing at  "). Same empty-filter idiom `HOME` and
    // `CP` already use in this file: omitted is a true statement, never a guess.
    if let Some(p) = std::env::var_os("COMPOSER_TYPED").filter(|v| !v.is_empty()) {
        return p.to_string_lossy().into_owned();
    }
    let cp = match composer_repo_root() {
        Ok(root) => root.display().to_string(),
        Err(message) => {
            eprintln!("pane-dispatch-ready: {message}");
            std::process::exit(64);
        }
    };
    // Prefer the ported Rust discriminator, falling back to the legacy `.py` only if it
    // is still present. The `.py` was deleted when composer-typed became a crate, so the
    // old unconditional path made composer_rc return 99 and fail-closed EVERY pane to
    // BUSY -- a silent fleet-wide starve that needed no config to trigger.
    // Ordered by trust: an explicit sibling install, then the repo's own build output.
    for candidate in [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("composer-typed"))),
        Some(PathBuf::from(format!("{cp}/target/release/composer-typed"))),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_file() {
            return candidate.display().to_string();
        }
    }
    format!("{cp}/bin/composer-typed.py")
}

fn classify_with_composer(
    text: &str,
    changed: bool,
    rules: &PaneDispatchReadyRules,
) -> pane_dispatch_ready::PaneDispatchReadyVerdict {
    let v = classify(text, changed, rules);
    if v.state != PaneDispatchReadyState::Free {
        return v;
    }
    let path = composer_path();
    if !PathBuf::from(&path).is_file() {
        // Symmetric with `apply_composer_rc`'s unknown-rc arm: ONE rule,
        // `composer_fail_closed`, governs BOTH composer failures. It defaults to
        // on, so live behaviour is unchanged; disabling it is what lets a mutation
        // leg isolate the rule actually under test from an absent discriminator.
        if rules.composer_fail_closed {
            return missing_composer(&path);
        }
        return v;
    }
    let tail = tail_n(text, 6);
    let rc = composer_rc(&tail, &path);
    apply_composer_rc(v, rc, &path, rules)
}

fn floored_capture_interval(requested: Option<u64>) -> u64 {
    requested
        .unwrap_or(TWO_CAPTURE_MIN_SECS)
        .max(TWO_CAPTURE_MIN_SECS)
}
fn run_live(
    json_out: bool,
    sessions: &[String],
    pane_filter: Option<&str>,
    rules: &PaneDispatchReadyRules,
) -> ExitCode {
    let mut tmv = Command::new(tick_monitor::TMUX);
    tmv.arg("-V");
    let tmux_unhealthy = match spawn_timeout(tmv, Duration::from_secs(5)) {
        BoundedOutcome::Completed(output) => !output.status.success(),
        BoundedOutcome::TimedOut => {
            eprintln!("pane-dispatch-ready: tmux version timed out before its deadline");
            true
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("pane-dispatch-ready: tmux version could not spawn: {error}");
            true
        }
    };
    if tmux_unhealthy {
        eprintln!("tmux not available");
        return ExitCode::from(2);
    }
    let sess_list: Vec<String> = if sessions.is_empty() {
        let mut cmd = Command::new(tick_monitor::TMUX);
        cmd.args(["list-sessions", "-F", "#{session_name}"]);
        let t = completed("tmux sessions", spawn_timeout(cmd, Duration::from_secs(15)))
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        if t.trim().is_empty() {
            eprintln!("no tmux sessions");
            return ExitCode::from(1);
        }
        t.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else {
        sessions.to_vec()
    };

    let capture_interval_secs = floored_capture_interval(
        std::env::var("BUFFER_MOTION_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok()),
    );

    let mut free_count = 0usize;
    let mut rows: Vec<String> = Vec::new();
    if json_out {
        print!("{{\"schema\":\"zs.dispatch-ready.v1\",\"panes\":[");
    }
    let mut first = true;
    for s in &sess_list {
        let mut cmd = Command::new(tick_monitor::TMUX);
        // ONE agent-health call per session, not per pane: the payload is keyed by pane index.
        let rate_limited = rate_limited_panes(s);
        cmd.args(["list-panes", "-t", s, "-F", "#{pane_index}"]);
        let panes = completed("tmux panes", spawn_timeout(cmd, Duration::from_secs(15)))
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        for pane in panes.lines().map(str::trim).filter(|p| !p.is_empty()) {
            if let Some(f) = pane_filter {
                if pane != f {
                    continue;
                }
            }
            let mut cap = Command::new(tick_monitor::TMUX);
            cap.args([
                tick_monitor::CAPTURE_PANE,
                "-p",
                "-e",
                "-t",
                &format!("{s}.{pane}"),
                "-S",
                "-40",
            ]);
            let txt = completed(
                "tmux first capture",
                spawn_timeout(cap, Duration::from_secs(10)),
            )
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
            let first_captured_at_secs = unix_seconds();
            let mut v = classify_with_composer(&txt, false, rules);
            if v.state == PaneDispatchReadyState::Free && rules.two_capture_liveness {
                let first_snapshot = capture_snapshot(first_captured_at_secs, &txt);
                std::thread::sleep(Duration::from_secs(capture_interval_secs));
                let mut cap2 = Command::new(tick_monitor::TMUX);
                cap2.args([
                    tick_monitor::CAPTURE_PANE,
                    "-p",
                    "-e",
                    "-t",
                    &format!("{s}.{pane}"),
                    "-S",
                    "-40",
                ]);
                let next = completed(
                    "tmux second capture",
                    spawn_timeout(cap2, Duration::from_secs(10)),
                )
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
                let current_snapshot = capture_snapshot(unix_seconds(), &next);
                v = confirm_free(v, &next, first_snapshot, current_snapshot, rules);
                if v.state == PaneDispatchReadyState::Free {
                    v = classify_with_composer(&next, false, rules);
                }
            }
            // ADDITIONAL REFUSAL, never a replacement oracle: the composer stays the authority
            // on FREE, and a pane whose AGENT cannot work is subtracted from that set. Measured
            // 2026-09-11: three panes of this session held a clean empty prompt under a live
            // ~5014-minute rate limit, so FREE was true and dispatch would have parked 83 hours.
            if v.state == PaneDispatchReadyState::Free {
                if let Some(reason) = pane_dispatch_ready::rate_limit_refusal(
                    rate_limited.get(pane).copied().unwrap_or(false),
                    &txt,
                ) {
                    v = pane_dispatch_ready::PaneDispatchReadyVerdict {
                        state: PaneDispatchReadyState::QuotaBlocked,
                        reason,
                    };
                }
            }
            if v.state == PaneDispatchReadyState::Free {
                free_count += 1;
            }
            if json_out {
                if !first {
                    print!(",");
                }
                first = false;
                let rec = serde_json::json!({
                    "session": s,
                    "pane": pane,
                    "state": v.state.as_str(),
                    "reason": v.reason,
                });
                print!("{rec}");
            } else {
                rows.push(format!(
                    "  {:<22} pane {:<3} {:<9} {}",
                    s,
                    pane,
                    v.state.as_str(),
                    v.reason
                ));
            }
        }
    }
    if json_out {
        println!("],\"free_count\":{free_count}}}");
    } else {
        for r in rows {
            say(&r);
        }
        say("");
        say(&format!("  FREE panes: {free_count}"));
        if free_count == 0 {
            say("  No dispatch target. Do NOT send — every pane is working or has no agent.");
        }
    }
    if free_count > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_selftest(rules: &PaneDispatchReadyRules) -> ExitCode {
    let mut fail = 0i32;
    let chk = |label: &str, text: &str, want: PaneDispatchReadyState, fail: &mut i32| {
        let got = classify(text, false, rules).state;
        if got == want {
            say(&format!("  [ ok ] {:<42} {}", label, got.as_str()));
        } else {
            say(&format!(
                "  [FAIL] {:<42} {} (want {})",
                label,
                got.as_str(),
                want.as_str()
            ));
            *fail += 1;
        }
    };
    say("=== BUSY markers must each fire (fires-on-known-bad) ===");
    chk(
        "claude working timer",
        "claude\n• Working (38m 29s • esc to interrupt)",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "claude sauteed",
        "claude\n✻ Sautéed for 3m 9s · 4 monitors still running",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "codex pursuing goal",
        "gpt-5.6-luna max · alpsinsurance\nPursuing goal (2h 29m)",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "claude infusing",
        "claude\n✽ Infusing… (21s · ↓ 443 tokens)",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "claude warping",
        "claude\n✻ Warping… (47s · ↓ 1.7k tokens)",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "claude flummoxing",
        "claude\n✻ Flummoxing… (51s · ↓ 1.3k tokens)",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "codex transcript hint",
        "codex\n… +43 lines (ctrl + t to view transcript)",
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    chk(
        "empty capture",
        "",
        PaneDispatchReadyState::Unreadable,
        &mut fail,
    );
    chk(
        "bare shell, no agent",
        // Assembled by `concat!` so this source never contains the contiguous home
        // literal the repo-wide gate forbids (omp-orchestrator-npq).
        concat!(
            "josh@Studio repo % pwd",
            "\n/Users/",
            "josh",
            "/Developer/x"
        ),
        PaneDispatchReadyState::NoAgent,
        &mut fail,
    );
    chk(
        "agent at empty prompt",
        "Opus 5 (1M context) │ bypass permissions\n❯ ",
        PaneDispatchReadyState::Free,
        &mut fail,
    );
    // FIRES-ON-KNOWN-BAD: a live Codex pane renders its model as "GPT-5.6-Luna"
    // (capital GPT). AGENT_RE matched only lowercase "gpt-", so every Codex pane in
    // the fleet classified NO_AGENT and refill-idle-panes refused to dispatch to any
    // of them. Measured 2026-09-02: 2 CONFIRMED_IDLE panes beside a 425-deep ready
    // queue. Fixture is the verbatim status line from zeststream-cast:0.5.
    chk(
        "codex pane at empty prompt (capitalised model name)",
        // WITH the SGR escapes, because the live callers capture with `-e`. A clean-text
        // fixture passed while every real pane still failed -- the escapes sit between
        // the line start and the glyph, so a whitespace trim can never reach it.
        // WITH the real FOOTER GEOMETRY, not just the prompt line. Two earlier fixtures
        // passed while every live pane still failed, because each modelled only the part
        // I had already guessed at: first clean text (missing the SGR escapes), then a
        // lone line (missing the trailing footer). The actual pane ends with the status
        // line, the box border, then blanks -- putting the prompt 7 lines from the end,
        // OUTSIDE the 6-line busy tail. That geometry IS the bug, so it belongs here.
        concat!(
            "\u{1b}[0m\u{1b}[48;2;15;18;22m \u{1b}[38;2;107;114;128m\u{3c0}\u{1b}[39m",
            "  > \u{25d5} GPT-5.6-Luna > \u{1f4c1} ~/Developer/zeststream-cast > \u{2442} main *134 ?367\n",
            "\u{2570}\u{2500} \n\n\n\n\n"
        ),
        PaneDispatchReadyState::Free,
        &mut fail,
    );
    // The NEGATIVE half, and the one that keeps the clause above honest: a WORKING
    // Codex pane carries the identical `>` separators and the same model name, and
    // differs only in the leading glyph (spinner + elapsed timer instead of the idle
    // `π`). If the prompt-marker clause ever widens to the separators themselves,
    // this fixture goes RED before a busy pane can be overwritten mid-thought.
    // Verbatim from zeststream-cast:0.3 while it was 16m into a task.
    chk(
        "codex pane MID-WORK must never read free (spinner, not idle glyph)",
        concat!(
            "\u{1b}[0m\u{1b}[48;2;15;18;22m \u{1b}[38;2;107;114;128m\u{2834}\u{1b}[39m",
            " 16m  > \u{25d5} GPT-5.6-Luna > \u{1f4c1} ~/Developer/zeststream-cast > \u{2442} main *138 +4 ?367\n",
            "\u{2570}\u{2500} \n\n\n\n\n"
        ),
        PaneDispatchReadyState::Busy,
        &mut fail,
    );
    say("");
    say("=== BUFFER MOTION: two-capture liveness ===");
    let first = classify("Opus 5 │ bypass permissions\n❯ ", false, rules);
    let moved = confirm_free(
        first.clone(),
        "Opus 5 │ bypass permissions\n❯ ",
        capture_snapshot(0, "first capture"),
        capture_snapshot(TWO_CAPTURE_MIN_SECS, "second capture"),
        rules,
    );
    if rules.two_capture_liveness && moved.state == PaneDispatchReadyState::Busy {
        say("  [ ok ] two-capture hash change -> BUSY");
    } else if !rules.two_capture_liveness && moved.state == PaneDispatchReadyState::Free {
        say("  [ ok ] two-capture disabled -> FREE (mutation)");
    } else {
        say(&format!(
            "  [FAIL] two-capture want BUSY got {}",
            moved.state.as_str()
        ));
        fail += 1;
    }
    say("");
    say(&format!("=== SELFTEST: {fail} failure(s) ==="));
    if fail == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_motion_override_cannot_lower_canonical_floor() {
        for requested in [Some(0), Some(10), Some(TWO_CAPTURE_MIN_SECS - 1)] {
            assert_eq!(
                floored_capture_interval(requested),
                TWO_CAPTURE_MIN_SECS,
                "override {requested:?} must not lower the canonical floor"
            );
        }
        assert_eq!(
            floored_capture_interval(Some(TWO_CAPTURE_MIN_SECS)),
            TWO_CAPTURE_MIN_SECS
        );
        assert_eq!(floored_capture_interval(Some(90)), 90);
        assert_eq!(
            floored_capture_interval(None),
            TWO_CAPTURE_MIN_SECS,
            "an absent override uses the canonical floor"
        );
    }
}
