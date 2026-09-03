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

use dispatch_claim_fence::{authorize, parse_br_show_json, BeadSnapshot, DispatchIntent};
use refill_idle_panes::{
    conflict_verdict, decide, measurability_refusal, measurability_verdict, packet_is_sendable,
    parse_activity_view, parse_oracle_view, parse_ready_fallback,
    parse_recommendations_with_skips, plan, reconciliation_failure, run_outcome, Assignment,
    SkippedPick,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output, bounded_status, BoundedOutcome};

const DEFAULT_MAX_PANES: usize = 8;
const PENDING_DISPATCH_MAX_AGE_SECS: u64 = 600;
const CLAIM_TIMEOUT_SECS: u64 = 30;

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

/// Run a probe with a wall bound.
///
/// Bounded because an unbounded child in a scheduled lane is how this box accumulated 13
/// orphans blocked forever in `write(2)` on a full pipe. `None` on any failure, which
/// the caller turns into "refuse everything" rather than "nothing is busy".
fn probe(bin: &str, args: &[String], secs: u64) -> Option<String> {
    probe_in(None, bin, args, secs)
}

/// Run a probe INSIDE the resolved target repository.
///
/// MEASURED 2026-09-02T21:24Z: with `REFILL_REPO` pinned, the packet's `Target:` line named
/// control-plane while `bv --robot-triage` and `br ready` ran in the PROCESS cwd — `$HOME`
/// under cron (its own `.beads`, prefix `fc-`), or whichever checkout the operator happened
/// to be in (`omp-orchestrator-ack-spine-oj6.3` was sent to a control-plane pane). The
/// repository that names the work and the repository that selects it must be the same
/// directory, so every DAG probe is pinned to the target.
fn probe_in(dir: Option<&str>, bin: &str, args: &[String], secs: u64) -> Option<String> {
    let mut command = Command::new(bin);
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    match bounded_output(&mut command, Duration::from_secs(secs)) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            String::from_utf8(output.stdout).ok()
        }
        BoundedOutcome::Completed(output) => {
            eprintln!(
                "refill-idle-panes: {bin} exited={}",
                output
                    .status
                    .code()
                    .map_or_else(|| "signal".to_owned(), |code| code.to_string())
            );
            None
        }
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
fn packet_from_row(
    bead: &str,
    footer: Option<&str>,
    target: &str,
    row: &serde_json::Value,
) -> Result<String, String> {
    let observed_id = row
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| "target_br_show_missing_id".to_owned())?;
    if observed_id != bead {
        return Err(format!("target_br_show_id_mismatch observed={observed_id}"));
    }
    let title = row
        .get("title")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "target_br_show_missing_title".to_owned())?;
    let body = row
        .get("description")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "target_br_show_missing_description".to_owned())?;
    let mut packet = format!(
        "Objective: work bead {bead} to completion.\n\
         Target: {target}. Run br show {bead} --json and read it IN FULL.\n\n\
         === BEAD BODY (authoritative) ===\n{title}\n\n{body}\n"
    );
    if let Some(f) = footer {
        packet.push_str(f);
    }
    packet_is_sendable(packet.len())
        .then_some(packet)
        .ok_or_else(|| "packet_too_small".to_owned())
}

/// Read br show INSIDE the target repository and reject every other shape.
fn render_packet(bead: &str, footer: Option<&str>, target: &str) -> Result<String, String> {
    let raw = probe_in(
        Some(target),
        "br",
        &["show".into(), bead.into(), "--json".into()],
        CLAIM_TIMEOUT_SECS,
    )
    .ok_or_else(|| format!("target_br_show_unreadable target={target}"))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|_| "target_br_show_invalid_json".to_owned())?;
    let row = match &value {
        serde_json::Value::Array(rows) => rows
            .first()
            .ok_or_else(|| "target_br_show_empty".to_owned())?,
        object @ serde_json::Value::Object(_) => object,
        _ => return Err("target_br_show_wrong_shape".to_owned()),
    };
    packet_from_row(bead, footer, target, row)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn refill_build_id() -> String {
    std::env::var("REFILL_BUILD_ID").unwrap_or_else(|_| {
        option_env!("OMP_BUILD_ID")
            .unwrap_or("unversioned")
            .to_owned()
    })
}

fn pending_marker_base(session: &str) -> PathBuf {
    std::env::var_os("OMP_PENDING_DISPATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env_or("HOME", ""))
                .join(".local/state/flywheel")
                .join(format!("omp-orchestrator-{session}.pending-dispatch"))
        })
}

fn pending_marker_path(base: &Path, pane: &str) -> PathBuf {
    let slug: String = pane.chars().filter(char::is_ascii_alphanumeric).collect();
    if slug.is_empty() {
        return base.to_path_buf();
    }
    let name = base
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "omp-orchestrator.pending-dispatch".to_owned());
    base.with_file_name(format!("{name}.{slug}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingMarker {
    Missing,
    Live {
        path: PathBuf,
        detail: String,
        age_secs: u64,
    },
    Expired {
        path: PathBuf,
        detail: String,
        age_secs: u64,
    },
    Undatable {
        path: PathBuf,
        detail: String,
        reason: &'static str,
    },
}

fn classify_pending_marker(text: &str, path: PathBuf, now: u64) -> PendingMarker {
    let detail = text.trim().to_owned();
    let issued_at = serde_json::from_str::<serde_json::Value>(&detail)
        .ok()
        .and_then(|value| value.get("issued_at").and_then(serde_json::Value::as_u64));
    let Some(issued_at) = issued_at else {
        return PendingMarker::Undatable {
            path,
            detail,
            reason: "INTENT_ISSUED_AT_MISSING",
        };
    };
    if issued_at > now {
        return PendingMarker::Undatable {
            path,
            detail,
            reason: "INTENT_ISSUED_IN_FUTURE",
        };
    }
    let age_secs = now - issued_at;
    if age_secs <= PENDING_DISPATCH_MAX_AGE_SECS {
        PendingMarker::Live {
            path,
            detail,
            age_secs,
        }
    } else {
        PendingMarker::Expired {
            path,
            detail,
            age_secs,
        }
    }
}

fn read_pending_marker(base: &Path, pane: &str, now: u64) -> Result<PendingMarker, String> {
    let pane_path = pending_marker_path(base, pane);
    let read = match std::fs::read_to_string(&pane_path) {
        Ok(text) => Ok((pane_path.clone(), text)),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound && pane_path != base =>
        {
            std::fs::read_to_string(base).map(|text| (base.to_path_buf(), text))
        }
        Err(error) => Err(error),
    };
    match read {
        Ok((path, text)) => Ok(classify_pending_marker(&text, path, now)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PendingMarker::Missing),
        Err(error) => Err(format!(
            "DISPATCH_RETRY_BLOCKED pane={pane} marker={} reason=MARKER_UNREADABLE error={error}",
            pane_path.display()
        )),
    }
}

fn clear_pending_marker(path: &Path, pane: &str) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "DISPATCH_BLOCKED pane={pane} marker={} reason=MARKER_CLEAR_FAILED error={error}",
            path.display()
        )),
    }
}

fn write_pending_marker(
    base: &Path,
    session: &str,
    pane: &str,
    bead: &str,
    target: &str,
) -> Result<PathBuf, String> {
    let path = pending_marker_path(base, pane);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pane={pane} bead={bead} marker={} reason=MARKER_PARENT error={error}",
                parent.display()
            )
        })?;
    }
    let row = serde_json::json!({
        "event": "dispatch_intent",
        "build_id": refill_build_id(),
        "pid": std::process::id(),
        "repo": target,
        "session": session,
        "pane": pane,
        "bead": bead,
        "issued_at": now_unix(),
    });
    let bytes = serde_json::to_vec(&row)
        .map_err(|error| format!("DISPATCH_BLOCKED pane={pane} bead={bead} reason=MARKER_SERIALIZE error={error}"))?;
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "DISPATCH_RETRY_BLOCKED pane={pane} bead={bead} marker={} reason=MARKER_EXISTS error={error}",
                path.display()
            )
        })?;
    marker
        .write_all(&bytes)
        .and_then(|_| marker.write_all(b"\n"))
        .and_then(|_| marker.sync_data())
        .map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pane={pane} bead={bead} marker={} reason=MARKER_WRITE error={error}",
                path.display()
            )
        })?;
    Ok(path)
}

fn load_target_snapshot(bead: &str, target: &str) -> Result<BeadSnapshot, String> {
    let raw = probe_in(
        Some(target),
        "br",
        &["show".into(), bead.into(), "--json".into()],
        CLAIM_TIMEOUT_SECS,
    )
    .ok_or_else(|| format!("target_br_show_unreadable target={target}"))?;
    parse_br_show_json(raw.as_bytes())
        .map_err(|error| format!("target_br_show_invalid bead={bead} target={target} error={error}"))
}

fn claim_bead_for_refill(bead: &str, target: &str, pane: &str) -> Result<BeadSnapshot, String> {
    let owner = format!("supervisor:{}", std::process::id());
    let initial = load_target_snapshot(bead, target)?;
    if initial.status_label() == "open" && initial.assignee().is_none() {
        let claim_args = vec![
            "update".to_owned(),
            bead.to_owned(),
            "--claim".to_owned(),
            "--actor".to_owned(),
            owner.clone(),
        ];
        probe_in(Some(target), "br", &claim_args, CLAIM_TIMEOUT_SECS).ok_or_else(|| {
            format!(
                "DISPATCH_BLOCKED bead={bead} pane={pane} target={target} reason=CLAIM_COMMAND_FAILED"
            )
        })?;
    }
    let snapshot = if initial.status_label() == "open" && initial.assignee().is_none() {
        load_target_snapshot(bead, target)?
    } else {
        initial
    };
    authorize(&DispatchIntent::bead(bead, &owner), Some(&snapshot)).map_err(|error| {
        format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} target={target} reason={} detail={error}",
            error.code()
        )
    })?;
    Ok(snapshot)
}

fn ledger_row(
    event: &str,
    session: &str,
    pane: &str,
    bead: &str,
    target: &str,
    ts_unix: u64,
    build_id: &str,
    reason: Option<&str>,
) -> Result<String, String> {
    let mut row = serde_json::json!({
        "event": event,
        "session": session,
        "pane": pane,
        "bead": bead,
        "target": target,
        "ts_unix": ts_unix,
        "build_id": build_id,
    });
    if let Some(reason) = reason {
        row["reason"] = serde_json::Value::String(reason.to_owned());
    }
    serde_json::to_string(&row).map_err(|error| format!("ledger serialization failed: {error}"))
}

fn append_ledger(
    event: &str,
    session: &str,
    pane: &str,
    bead: &str,
    target: &str,
    reason: Option<&str>,
) -> Result<(), String> {
    let path = std::env::var("REFILL_LEDGER").map_or_else(
        |_| PathBuf::from(env_or("HOME", "")).join(".local/state/flywheel/refill-idle-panes.jsonl"),
        PathBuf::from,
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("ledger parent={} error={error}", parent.display()))?;
    }
    let row = ledger_row(
        event,
        session,
        pane,
        bead,
        target,
        now_unix(),
        &refill_build_id(),
        reason,
    )?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("ledger path={} error={error}", path.display()))?;
    writeln!(file, "{row}")
        .and_then(|_| file.sync_data())
        .map_err(|error| format!("ledger path={} write error={error}", path.display()))
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

    let triage = probe_in(Some(&target), "bv", &["--robot-triage".into()], 90).unwrap_or_default();
    let (mut picks, refused) = parse_recommendations_with_skips(&triage);
    // Name every refusal so a reader of the log can see WHY a top-ranked bead was not
    // sent, instead of inferring it from its absence.
    for SkippedPick { bead, reason } in &refused {
        println!("SKIP  bead={bead} reason={reason}");
    }
    if picks.is_empty() {
        // bv's top-N was entirely epics/grading/blocked (measured 2026-09-02T21:23Z, 10 of
        // 10, beside 28 ready beads). The ready list is the second oracle.
        let ready = probe_in(Some(&target), "br", &["ready".into(), "--json".into()], 60).unwrap_or_default();
        let (ready_picks, ready_refused) = parse_ready_fallback(&ready);
        for SkippedPick { bead, reason } in &ready_refused {
            println!("SKIP  bead={bead} reason={reason} source=br-ready");
        }
        if !ready_picks.is_empty() {
            println!(
                "refill: bv top-{} all refused; falling back to br ready ({} dispatchable)",
                refused.len(),
                ready_picks.len()
            );
        }
        picks = ready_picks;
    }
    if picks.is_empty() {
        println!(
            "refill: {} idle pane(s) but bv returned NO picks — queue empty or triage unreadable",
            panes.len()
        );
        return ExitCode::from(outcome.code);
    }

    let assignments = plan(&panes, &picks, max);
    let pending_base = pending_marker_base(&session);
    let (mut sent, mut skipped) = (0usize, 0usize);
    for Assignment { pane, bead } in &assignments {
        let packet = match render_packet(bead, footer.as_deref(), &target) {
            Ok(packet) => packet,
            Err(reason) => {
                println!(
                    "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={reason}"
                );
                if apply {
                    let _ = append_ledger(
                        "refill_refused",
                        &session,
                        pane,
                        bead,
                        &target,
                        Some(&reason),
                    );
                }
                skipped += 1;
                continue;
            }
        };
        if !apply {
            println!("PLAN  pane={pane} bead={bead} bytes={}", packet.len());
            continue;
        }

        match read_pending_marker(&pending_base, pane, now_unix()) {
            Ok(PendingMarker::Missing) => {}
            Ok(PendingMarker::Expired {
                path,
                age_secs,
                ..
            }) => {
                if let Err(error) = clear_pending_marker(&path, pane) {
                    println!(
                        "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={error}"
                    );
                    let _ = append_ledger(
                        "refill_refused",
                        &session,
                        pane,
                        bead,
                        &target,
                        Some(&error),
                    );
                    skipped += 1;
                    continue;
                }
                println!("PENDING_MARKER_EXPIRED pane={pane} age_secs={age_secs} cleared");
            }
            Ok(PendingMarker::Live {
                path,
                detail,
                age_secs,
            }) => {
                let reason = format!(
                    "PENDING_MARKER_LIVE age_secs={age_secs} marker={} detail={detail}",
                    path.display()
                );
                println!(
                    "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={reason}"
                );
                let _ = append_ledger(
                    "refill_refused",
                    &session,
                    pane,
                    bead,
                    &target,
                    Some(&reason),
                );
                skipped += 1;
                continue;
            }
            Ok(PendingMarker::Undatable { path, detail, reason }) => {
                let reason = format!(
                    "{reason} marker={} detail={detail}",
                    path.display()
                );
                println!(
                    "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={reason}"
                );
                let _ = append_ledger(
                    "refill_refused",
                    &session,
                    pane,
                    bead,
                    &target,
                    Some(&reason),
                );
                skipped += 1;
                continue;
            }
            Err(error) => {
                println!(
                    "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={error}"
                );
                let _ = append_ledger(
                    "refill_refused",
                    &session,
                    pane,
                    bead,
                    &target,
                    Some(&error),
                );
                skipped += 1;
                continue;
            }
        }

        let marker = match write_pending_marker(&pending_base, &session, pane, bead, &target) {
            Ok(path) => path,
            Err(error) => {
                println!(
                    "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={error}"
                );
                let _ = append_ledger(
                    "refill_refused",
                    &session,
                    pane,
                    bead,
                    &target,
                    Some(&error),
                );
                skipped += 1;
                continue;
            }
        };
        if let Err(error) = claim_bead_for_refill(bead, &target, pane) {
            let _ = clear_pending_marker(&marker, pane);
            println!(
                "REFILL_REFUSED pane={pane} bead={bead} target={target} reason={error}"
            );
            let _ = append_ledger(
                "refill_refused",
                &session,
                pane,
                bead,
                &target,
                Some(&error),
            );
            skipped += 1;
            continue;
        }

        let staged = std::env::temp_dir().join(format!("refill-{session}-{pane}-{bead}.txt"));
        if std::fs::write(&staged, &packet).is_err() {
            let reason = "stage_write_failed";
            println!("REFILL_REFUSED pane={pane} bead={bead} target={target} reason={reason}");
            let _ = append_ledger(
                "refill_refused",
                &session,
                pane,
                bead,
                &target,
                Some(reason),
            );
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
        let _ = std::fs::remove_file(&staged);
        if ok {
            println!("SENT  pane={pane} bead={bead} (sent, not yet verified received)");
            if let Err(error) = append_ledger(
                "refill_sent",
                &session,
                pane,
                bead,
                &target,
                None,
            ) {
                eprintln!("refill-idle-panes: {error}");
            }
            sent += 1;
        } else {
            let reason = format!("{failure}");
            println!("REFILL_FAILED pane={pane} bead={bead} target={target} reason={reason}");
            let _ = append_ledger(
                "refill_refused",
                &session,
                pane,
                bead,
                &target,
                Some(&reason),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "refill-ve4w-{label}-{}-{}",
            std::process::id(),
            now_unix()
        ));
        std::fs::create_dir_all(&path).expect("scratch directory");
        path
    }

    #[test]
    fn live_marker_refuses_the_same_pane_before_send() {
        let root = scratch("marker");
        let base = root.join("omp-orchestrator.pending-dispatch");
        let marker = write_pending_marker(&base, "control-plane", "%1414", "oj6.3", "/repo")
            .expect("write marker");
        assert!(marker.exists(), "marker path must be staged: {}", marker.display());
        let state = read_pending_marker(&base, "%1414", now_unix()).expect("read marker");
        assert!(
            matches!(state, PendingMarker::Live { age_secs, .. } if age_secs <= 1),
            "a fresh marker must block the pane: {state:?}"
        );
        let error = write_pending_marker(&base, "control-plane", "%1414", "other", "/repo")
            .expect_err("a second send to the pane must refuse");
        assert!(error.contains("MARKER_EXISTS"), "{error}");
        std::fs::remove_dir_all(root).expect("remove scratch");
    }

    #[test]
    fn ledger_row_carries_timestamp_build_and_target() {
        let text = ledger_row(
            "refill_refused",
            "control-plane",
            "%1414",
            "oj6.3",
            "/repo",
            1_788_000_000,
            "build-abc",
            Some("target_br_show_unreadable"),
        )
        .expect("serialize ledger row");
        let value: serde_json::Value = serde_json::from_str(&text).expect("ledger JSON");
        assert_eq!(value["ts_unix"], 1_788_000_000);
        assert_eq!(value["build_id"], "build-abc");
        assert_eq!(value["target"], "/repo");
        assert_eq!(value["reason"], "target_br_show_unreadable");
    }

    #[test]
    fn missing_target_repository_is_refused_with_target_name() {
        let target = format!("/definitely-missing-refill-target-{}", std::process::id());
        let error = render_packet("oj6.3", None, &target)
            .expect_err("br show in a missing target repository must refuse");
        assert!(error.contains("target_br_show_unreadable"), "{error}");
        assert!(error.contains(&target), "{error}");
    }

    #[test]
    fn target_row_mismatch_is_refused_before_packet_construction() {
        let row = serde_json::json!({
            "id": "different-repository-bead",
            "title": "wrong target",
            "description": "wrong target body"
        });
        let error = packet_from_row("oj6.3", None, "/target/repo", &row)
            .expect_err("a target that returned another bead must refuse");
        assert!(error.contains("target_br_show_id_mismatch"), "{error}");
    }

    #[test]
    fn claim_fence_refusal_names_bead_pane_and_target() {
        let snapshot = BeadSnapshot::new(
            "oj6.3",
            "dispatch bead",
            "dispatch body",
            "in_progress",
            Some("BlueLantern"),
        );
        let error = authorize(
            &DispatchIntent::bead("oj6.3", "supervisor:123"),
            Some(&snapshot),
        )
        .expect_err("another owner's bead must not dispatch");
        let refusal = format!(
            "DISPATCH_BLOCKED bead=oj6.3 pane=%1414 target=/repo reason={} detail={error}",
            error.code()
        );
        assert!(refusal.contains("ASSIGNED_ELSEWHERE"), "{refusal}");
        assert!(refusal.contains("bead=oj6.3"), "{refusal}");
        assert!(refusal.contains("pane=%1414"), "{refusal}");
        assert!(refusal.contains("target=/repo"), "{refusal}");
    }
}
