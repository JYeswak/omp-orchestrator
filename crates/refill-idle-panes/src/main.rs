#![forbid(unsafe_code)]

//! `refill-idle-panes` — find every idle pane, take the DAG's picks, send bead BODIES.
//!
//! Thin I/O shell around [`refill_idle_panes`]. Every decision lives in the lib so it is
//! testable without a fleet; this binary only spawns probes, renders packets, and sends.
//!
//! Port of `bin/refill-idle-panes.sh`, which remains the differential oracle per
//! `registries/dispatch_chain_migration.toml`.
//!
//! Verbs: `--plan` (default, mutates nothing) | `--apply` | `--selftest`

use refill_idle_panes::{
    conflict_verdict, decide, measurability_refusal, measurability_verdict, packet_is_sendable,
    parse_activity_view, parse_oracle_view, parse_recommendations, plan, reconciliation_failure,
    run_outcome, Assignment,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

const DEFAULT_MAX_PANES: usize = 8;

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

/// Run a probe with a wall bound.
///
/// Bounded because an unbounded child in a scheduled lane is how this box accumulated 13
/// orphans blocked forever in `write(2)` on a full pipe. `None` on any failure, which
/// the caller turns into "refuse everything" rather than "nothing is busy".
fn probe(bin: &str, args: &[String], secs: u64) -> Option<String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
    let out = child.wait_with_output().ok()?;
    String::from_utf8(out.stdout).ok()
}
/// Reconcile NTM and tmux before interpreting an empty idle-pane intersection.
///
/// This calls the shared fleet-reconcile library rather than duplicating its
/// fail-closed empty-success and name-set checks. A non-PASS result is returned
/// as a named nonzero refill outcome.
fn reconcile_fleet() -> Result<(), String> {
    let tmux = probe(
        "tmux",
        &["list-sessions".into(), "-F".into(), "#{session_name}".into()],
        45,
    )
    .unwrap_or_default();
    let list = probe("ntm", &["list".into()], 45).unwrap_or_default();
    let snapshot = probe("ntm", &["--robot-snapshot".into()], 45).unwrap_or_default();
    let verdict = fleet_reconcile::reconcile_inner(
        &tmux,
        &list,
        &snapshot,
        &fleet_reconcile::FleetReconcileRules::default(),
    );
    reconciliation_failure(&verdict).map_or(Ok(()), Err)
}

/// Render one bead's body into a packet.
///
/// SEND BODIES, NEVER IDS. An opaque id makes the worker do the conductor's
/// interpretation, and a worker that has to guess the spec guesses differently each time.
///
/// The `Target:` line names the RESOLVED repository root (`REFILL_REPO` env > upward
/// `.git`/`.beads` marker walk from the cwd) — never a literal, because a packet
/// naming a wrong checkout compiles into a worker that reads the wrong repo.
fn render_packet(bead: &str, footer: Option<&str>, target: &str) -> Option<String> {
    let raw = probe(
        "br",
        &[
            "show".into(),
            bead.into(),
            "--json".into(),
        ],
        30,
    )?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let row = match &value {
        serde_json::Value::Array(a) => a.first()?,
        other => other,
    };
    let title = row.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
    let body = row
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let mut packet = format!(
        "Objective: work bead {bead} to completion.\n\
         Target: {target}. Run `br show {bead} --json` and read it IN FULL.\n\n\
         === BEAD BODY (authoritative) ===\n{title}\n\n{body}\n"
    );
    if let Some(f) = footer {
        packet.push_str(f);
    }
    packet_is_sendable(packet.len()).then_some(packet)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--selftest") => selftest(),
        Some("--apply") => run(true),
        Some("--plan") | None => run(false),
        _ => {
            eprintln!("usage: refill-idle-panes [--plan|--apply|--selftest]");
            ExitCode::from(2)
        }
    }
}
/// Marker entries that identify a repository root while walking up from the cwd.
const REPO_MARKERS: [&str; 2] = [".git", ".beads"];

fn resolve_target() -> Result<String, String> {
    if let Some(root) = std::env::var_os("REFILL_REPO").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(root).display().to_string());
    }
    let mut current = std::env::current_dir().ok();
    while let Some(directory) = current {
        if REPO_MARKERS.iter().any(|marker| directory.join(marker).exists()) {
            return Ok(directory.display().to_string());
        }
        current = directory.parent().map(|p| p.to_path_buf());
    }
    Err(format!(
        "refill-idle-panes: no repository marker ({}) found at or above the cwd; set REFILL_REPO or run from a checkout",
        REPO_MARKERS.join(" or ")
    ))
}

fn run(apply: bool) -> ExitCode {
    let target = match resolve_target() {
        Ok(target) => target,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(64);
        }
    };
    let session = env_or("REFILL_SESSION", "control-plane");
    let max: usize = env_or("REFILL_MAX_PANES", "")
        .parse()
        .unwrap_or(DEFAULT_MAX_PANES);
    let footer = std::env::var("REFILL_FOOTER")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok());
    if let Err(message) = reconcile_fleet() {
        eprintln!("{message}");
        return ExitCode::from(1);
    }

    let activity = probe("ntm", &[format!("--robot-activity={session}")], 45).unwrap_or_default();
    let ready_bin = PathBuf::from(env_or("HOME", "")).join(".local/bin/pane-dispatch-ready");
    let oracle = probe(
        &ready_bin.display().to_string(),
        &[session.clone(), "--json".into()],
        90,
    )
    .unwrap_or_default();

    // MEASURABILITY FIRST, through the shared kernel. An empty or unreadable roster is a
    // broken probe (ntm#254 class), never consensus that the fleet has no work.
    if let Some(outcome) = measurability_refusal(&measurability_verdict(&activity, &oracle)) {
        eprintln!("{}", outcome.message);
        return ExitCode::from(outcome.code);
    }
    // Both parse — measurability_refusal already proved it — so these cannot be None.
    let (Some(activity_view), Some(oracle_view)) =
        (parse_activity_view(&activity), parse_oracle_view(&oracle))
    else {
        eprintln!(
            "refill: UNMEASURABLE detector=probe_reparse why=a probe that parsed for the roster \
             comparison failed to parse again; this is a defect in refill, not in the fleet"
        );
        return ExitCode::from(2);
    };

    let decision = decide(&activity_view, &oracle_view);
    let conflict = conflict_verdict(&activity_view, &oracle_view);
    let outcome = run_outcome(&decision, &conflict);
    let panes = decision.dispatchable.clone();
    if panes.is_empty() {
        if outcome.code == 0 {
            println!("{}", outcome.message);
        } else {
            eprintln!("{}", outcome.message);
        }
        return ExitCode::from(outcome.code);
    }
    // A conflict or an unknowable pane is reported even while the established panes are
    // fed. Starving the fleet to report a problem is the failure this crate exists to end.
    if outcome.code != 0 {
        eprintln!("{}", outcome.message);
    } else {
        println!("{}", outcome.message);
    }

    let triage = probe("bv", &["--robot-triage".into()], 90).unwrap_or_default();
    let picks = parse_recommendations(&triage);
    if picks.is_empty() {
        println!(
            "refill: {} idle pane(s) but bv returned NO picks — queue empty or triage unreadable",
            panes.len()
        );
        return ExitCode::from(outcome.code);
    }

    let assignments = plan(&panes, &picks, max);
    let (mut sent, mut skipped) = (0usize, 0usize);
    for Assignment { pane, bead } in &assignments {
        let Some(packet) = render_packet(bead, footer.as_deref(), &target) else {
            println!("SKIP  pane={pane} bead={bead} reason=render_failed_or_too_small");
            skipped += 1;
            continue;
        };
        if !apply {
            println!("PLAN  pane={pane} bead={bead} bytes={}", packet.len());
            continue;
        }
        let staged = std::env::temp_dir().join(format!("refill-{session}-{pane}-{bead}.txt"));
        if std::fs::write(&staged, &packet).is_err() {
            println!("SKIP  pane={pane} bead={bead} reason=stage_write_failed");
            skipped += 1;
            continue;
        }
        let ok = Command::new("ntm")
            .arg(format!("--robot-send={session}"))
            .arg(format!("--panes={pane}"))
            .arg(format!("--msg-file={}", staged.display()))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            // SENDER SUCCESS IS NOT RECEIVER RECEIPT. ntm returns success while a packet
            // sits unsubmitted in a composer. Verification is controller-tick's job, and
            // conflating the two is the defect that cost four days.
            println!("SENT  pane={pane} bead={bead} (sent, not yet verified received)");
            append_ledger(&session, pane, bead);
            sent += 1;
        } else {
            println!("FAIL  pane={pane} bead={bead} reason=send_failed");
            skipped += 1;
        }
    }
    println!("---");
    println!(
        "refill mode={} idle={} picks={} sent={sent} skipped={skipped}",
        if apply { "apply" } else { "plan" },
        panes.len(),
        picks.len()
    );
    // The dispatch happened; the exit code still carries the unresolved observation.
    ExitCode::from(outcome.code)
}

fn append_ledger(session: &str, pane: &str, bead: &str) {
    let path = std::env::var("REFILL_LEDGER").map_or_else(
        |_| PathBuf::from(env_or("HOME", "")).join(".local/state/flywheel/refill-idle-panes.jsonl"),
        PathBuf::from,
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            f,
            r#"{{"event":"refill_sent","session":"{session}","pane":"{pane}","bead":"{bead}"}}"#
        );
    }
}

/// Exercises the decision layer against the measured fixtures. The unit tests in the lib
/// are the real coverage; this is the operator-facing smoke check.
fn selftest() -> ExitCode {
    let mut fails = 0;
    let mut check = |name: &str, ok: bool| {
        println!("  {} {name}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            fails += 1;
        }
    };

    // THE MEASURED DEFECT, 2026-09-02 03:14:55Z: ntm cannot classify a codex pane and
    // coerces its UNKNOWN into safe_to_dispatch=false. pane-dispatch-ready CONFIRMS free.
    let codex_activity = r#"{"agents":[
        {"pane":"2","agent_type":"codex","state":"UNKNOWN","confidence":0.5,"safe_to_dispatch":false},
        {"pane":"4","agent_type":"omp-glm","state":"ERROR","confidence":0.95,"safe_to_dispatch":false}]}"#;
    let codex_oracle =
        r#"{"panes":[{"pane":"2","state":"FREE"},{"pane":"4","state":"BUSY"}]}"#;
    let codex = decide(
        &parse_activity_view(codex_activity).expect("fixture parses"),
        &parse_oracle_view(codex_oracle).expect("fixture parses"),
    );
    check(
        "a codex pane ntm cannot classify IS dispatchable when the oracle confirms it",
        codex.dispatchable == vec!["2".to_string()],
    );
    check(
        "the old two-valued rule selected NOTHING here (known-bad reproduced)",
        !codex_activity.contains(r#""safe_to_dispatch":true"#),
    );

    // The measured 2026-08-27 disagreement: a CONFIDENT conflict is never dispatched.
    let conflict = decide(
        &parse_activity_view(
            r#"{"agents":[{"pane":"4","state":"IDLE","confidence":0.95,"safe_to_dispatch":true}]}"#,
        )
        .expect("fixture parses"),
        &parse_oracle_view(r#"{"panes":[{"pane":"4","state":"NO_AGENT"}]}"#).expect("fixture parses"),
    );
    check(
        "a bare shell is refused even when activity confidently says free",
        conflict.dispatchable.is_empty() && conflict.conflicts == vec!["4".to_string()],
    );

    let bad_reconcile = fleet_reconcile::InnerVerdict {
        detector: "ntm_empty_success_with_live_tmux".into(),
        verdict: "FAIL".into(),
        tmux_count: 1,
        ntm_count: 0,
        detail: "snapshot reported no sessions while tmux had one".into(),
    };
    let bad_message = reconciliation_failure(&bad_reconcile).unwrap_or_default();
    check(
        "empty-success disagreement is named and nonzero",
        bad_message.starts_with("refill: SURFACE_DISAGREEMENT")
            && bad_message.contains("detector=ntm_empty_success_with_live_tmux"),
    );

    let good_reconcile = fleet_reconcile::InnerVerdict {
        detector: "ntm_tmux_agree".into(),
        verdict: "PASS".into(),
        tmux_count: 1,
        ntm_count: 1,
        detail: "ntm and tmux agree".into(),
    };
    let busy_activity =
        r#"{"agents":[{"pane":"2","state":"THINKING","confidence":0.9,"safe_to_dispatch":false}]}"#;
    let busy_oracle = r#"{"panes":[{"pane":"2","state":"BUSY"}]}"#;
    let busy = decide(
        &parse_activity_view(busy_activity).expect("fixture parses"),
        &parse_oracle_view(busy_oracle).expect("fixture parses"),
    );
    let busy_outcome = run_outcome(
        &busy,
        &conflict_verdict(
            &parse_activity_view(busy_activity).expect("fixture parses"),
            &parse_oracle_view(busy_oracle).expect("fixture parses"),
        ),
    );
    check(
        "a CONFIDENTLY busy fleet remains the healthy no-work case at exit 0",
        reconciliation_failure(&good_reconcile).is_none()
            && busy.dispatchable.is_empty()
            && busy_outcome.code == 0,
    );

    let unreadable = measurability_refusal(&measurability_verdict("not json", "not json"));
    check(
        "an unreadable probe is a typed NONZERO refusal, not zero candidates",
        unreadable.as_ref().is_some_and(|o| o.code == 2),
    );

    let empty_ntm = measurability_refusal(&measurability_verdict(
        r#"{"success":true,"agents":[]}"#,
        r#"{"panes":[{"pane":"2","state":"FREE"}]}"#,
    ));
    check(
        "an EMPTY ntm roster against a live oracle refuses nonzero and names the probe",
        empty_ntm
            .as_ref()
            .is_some_and(|o| o.code == 1 && o.message.contains("ntm --robot-activity")),
    );

    check("an undersized packet is refused", !packet_is_sendable(10));
    check("a full-size packet is accepted", packet_is_sendable(7_347));

    println!("---");
    println!("selftest fails={fails}");
    if fails == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
