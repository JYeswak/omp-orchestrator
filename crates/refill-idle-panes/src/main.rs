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

use refill_idle_panes::{SkippedPick, 
    conflict_verdict, decide, measurability_refusal, measurability_verdict, packet_is_sendable,
    parse_activity_view, parse_oracle_view, parse_recommendations_with_skips, plan, reconciliation_failure,
    run_outcome, Assignment,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;
use subprocess_contract::{bounded_output, bounded_status, BoundedOutcome};

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
    let mut command = Command::new(bin);
    command.args(args);
    match bounded_output(&mut command, Duration::from_secs(secs)) {
        BoundedOutcome::Completed(output) => String::from_utf8(output.stdout).ok(),
        BoundedOutcome::TimedOut => {
            eprintln!("refill-idle-panes: {bin} timed out before its deadline");
            None
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("refill-idle-panes: {bin} could not spawn: {error}");
            None
        }
    }
}
/// Reconcile NTM and tmux before interpreting an empty idle-pane intersection.
///
/// This calls the shared fleet-reconcile library rather than duplicating its
/// fail-closed empty-success and name-set checks. A non-PASS result is returned
/// as a named nonzero refill outcome.
fn reconcile_fleet() -> Result<(), String> {
    let tmux = probe(
        "tmux",
        &[
            "list-sessions".into(),
            "-F".into(),
            "#{session_name}".into(),
        ],
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
    let raw = probe("br", &["show".into(), bead.into(), "--json".into()], 30)?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let row = match &value {
        serde_json::Value::Array(a) => a.first()?,
        other => other,
    };
    let title = row
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
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
        if REPO_MARKERS
            .iter()
            .any(|marker| directory.join(marker).exists())
        {
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
    let (picks, refused) = parse_recommendations_with_skips(&triage);
    // Name every refusal so a reader of the log can see WHY a top-ranked bead was not
    // sent, instead of inferring it from its absence.
    for SkippedPick { bead, reason } in &refused {
        println!("SKIP  bead={bead} reason={reason}");
    }
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
        let mut command = Command::new("ntm");
        command
            .arg(format!("--robot-send={session}"))
            .arg(format!("--panes={pane}"))
            .arg(format!("--msg-file={}", staged.display()));
        let (ok, failure) = match bounded_status(&mut command, Duration::from_secs(45)) {
            BoundedOutcome::Completed(output) => (output.status.success(), "send_failed"),
            BoundedOutcome::TimedOut => (false, "send_timed_out"),
            BoundedOutcome::Unspawned(error) => {
                eprintln!("refill-idle-panes: ntm send could not spawn: {error}");
                (false, "send_unspawned")
            }
        };
        if ok {
            // SENDER SUCCESS IS NOT RECEIVER RECEIPT. ntm returns success while a packet
            // sits unsubmitted in a composer. Verification is controller-tick's job, and
            // conflating the two is the defect that cost four days.
            println!("SENT  pane={pane} bead={bead} (sent, not yet verified received)");
            append_ledger(&session, pane, bead);
            sent += 1;
        } else {
            println!("FAIL  pane={pane} bead={bead} reason={failure}");
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
        if ok == false {
            fails += 1;
        }
    };

    // THE MEASURED DEFECT, verbatim from 2026-09-02 03:14:55Z. Panes 2 and 3 are codex
    // workers pane-dispatch-ready CONFIRMS free while ntm reports them working at
    // observation_confidence 0.95. Two confident surfaces, flatly contradicting.
    let live_activity = r#"{"agents":[
        {"pane":"1","agent_type":"claude","state":"UNKNOWN","confidence":0.5,"observation_state":"idle",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
        {"pane":"2","agent_type":"codex","state":"UNKNOWN","confidence":0.5,"observation_state":"working",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
        {"pane":"3","agent_type":"codex","state":"UNKNOWN","confidence":0.5,"observation_state":"working",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false}]}"#;
    let live_oracle = r#"{"panes":[
        {"pane":"1","state":"BUSY"},{"pane":"2","state":"FREE"},{"pane":"3","state":"FREE"}]}"#;
    let live_a = parse_activity_view(live_activity).expect("fixture parses");
    let live_o = parse_oracle_view(live_oracle).expect("fixture parses");
    let live = decide(&live_a, &live_o);
    let live_outcome = run_outcome(&live, &conflict_verdict(&live_a, &live_o));
    check(
        "the live capture is a NAMED per-pane conflict, not a quiet fleet",
        live.conflicts == vec!["1".to_string(), "2".to_string(), "3".to_string()]
            && live_outcome.code == 1
            && live_outcome.message.contains("SURFACE_CONFLICT"),
    );
    check(
        "refusing to dispatch there is still CORRECT (this fix invents no dispatchability)",
        live.dispatchable.is_empty(),
    );
    // The retired quiet-success line is asserted absent by three lib tests. It is NOT
    // repeated here: a guard that embeds the string it forbids puts that string back
    // into the shipped binary, and `strings ~/.local/bin/refill-idle-panes` is how an
    // operator checks that the reinstall actually landed.

    // `state` is NOT the dispatch signal. On the 03:37:51Z capture a state=UNKNOWN pane
    // was dispatchable and a state=THINKING pane was not; gating on `state` refuses the
    // only panes that can receive work.
    let state_unknown_but_idle = parse_activity_view(
        r#"{"agents":[{"pane":"4","state":"UNKNOWN","confidence":0.5,"observation_state":"idle",
            "capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true}]}"#,
    )
    .expect("fixture parses");
    let oracle_4_free =
        parse_oracle_view(r#"{"panes":[{"pane":"4","state":"FREE"}]}"#).expect("fixture parses");
    check(
        "state=UNKNOWN does not blind the parser to a live idle observation",
        decide(&state_unknown_but_idle, &oracle_4_free).dispatchable == vec!["4".to_string()],
    );

    // The measured 2026-08-27 disagreement: a CONFIDENT conflict is never dispatched.
    let bare_shell = decide(
        &parse_activity_view(
            r#"{"agents":[{"pane":"4","observation_state":"idle","capture_provenance":"live",
                "observation_freshness":"fresh"}]}"#,
        )
        .expect("fixture parses"),
        &parse_oracle_view(r#"{"panes":[{"pane":"4","state":"NO_AGENT"}]}"#)
            .expect("fixture parses"),
    );
    check(
        "a bare shell is refused even when ntm confidently says idle",
        bare_shell.dispatchable.is_empty() && bare_shell.conflicts == vec!["4".to_string()],
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
    let busy_a = parse_activity_view(
        r#"{"agents":[{"pane":"2","observation_state":"working","capture_provenance":"live",
            "observation_freshness":"fresh"}]}"#,
    )
    .expect("fixture parses");
    let busy_o =
        parse_oracle_view(r#"{"panes":[{"pane":"2","state":"BUSY"}]}"#).expect("fixture parses");
    let busy = decide(&busy_a, &busy_o);
    let busy_outcome = run_outcome(&busy, &conflict_verdict(&busy_a, &busy_o));
    check(
        "a CONFIDENTLY busy fleet remains the healthy no-work case at exit 0",
        reconciliation_failure(&good_reconcile).is_none()
            && busy.dispatchable.is_empty()
            && busy_outcome.code == 0,
    );

    let stale = parse_activity_view(
        r#"{"agents":[{"pane":"2","observation_state":"working","capture_provenance":"stale",
            "observation_freshness":"stale"}]}"#,
    )
    .expect("fixture parses");
    check(
        "a stale capture is UNKNOWN, not a busy assertion",
        stale.confident().is_empty(),
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

    check(
        "an undersized packet is refused",
        packet_is_sendable(10) == false,
    );
    check("a full-size packet is accepted", packet_is_sendable(7_347));

    println!("---");
    println!("selftest fails={fails}");
    if fails == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
