#![forbid(unsafe_code)]

use blocker_taxonomy::{
    baseline_292_fixture, decide_redispatch, report, BeadRecord, BlockerKind, LiveSet,
    RedispatchDecision, BASELINE_BLOCKED_TOTAL, BASELINE_DEP_BLOCKED, BASELINE_DEP_BLOCKED_P0,
    BASELINE_STATUS_BLOCKED,
};
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("report") => cmd_report(&args[1..]),
        Some("redispatch") => cmd_redispatch(&args[1..]),
        _ => {
            eprintln!(
                "usage: blocker-taxonomy report [--fixture-292 | --stdin-json]\n       blocker-taxonomy redispatch --bead ID --kind KIND --hold-pane PANE --live P,P"
            );
            ExitCode::from(2)
        }
    }
}

fn cmd_report(args: &[String]) -> ExitCode {
    let beads = if args.iter().any(|a| a == "--stdin-json") {
        let mut buf = String::new();
        if io::stdin().read_to_string(&mut buf).is_err() {
            eprintln!("blocker-taxonomy: failed to read stdin");
            return ExitCode::from(1);
        }
        match serde_json::from_str::<Vec<BeadRecord>>(&buf) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("blocker-taxonomy: invalid census json: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        baseline_292_fixture()
    };
    match report(&beads) {
        Ok(r) => {
            println!(
                "status_blocked={} dependency_blocked={} dependency_blocked_p0={} other={} total={} both_classes={} baseline={}={}+{} p0={}",
                r.status_blocked,
                r.dependency_blocked,
                r.dependency_blocked_p0,
                r.other,
                r.total_listed,
                r.both_classes_reported,
                BASELINE_BLOCKED_TOTAL,
                BASELINE_STATUS_BLOCKED,
                BASELINE_DEP_BLOCKED,
                BASELINE_DEP_BLOCKED_P0
            );
            if let Ok(json) = serde_json::to_string(&r) {
                println!("{json}");
            }
            if !r.both_classes_reported {
                eprintln!("blocker-taxonomy: report collapsed tracker classes");
                return ExitCode::from(1);
            }
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("blocker-taxonomy: {e:?}");
            ExitCode::from(1)
        }
    }
}

fn parse_kind(s: &str) -> Option<BlockerKind> {
    Some(match s {
        "tracker-blocked" => BlockerKind::TrackerBlocked,
        "dependency-blocked" => BlockerKind::DependencyBlocked,
        "cross-pane-hold" => BlockerKind::CrossPaneHold,
        "pending-dispatch-marker" => BlockerKind::PendingDispatchMarker,
        "stranded-grading-claim" => BlockerKind::StrandedGradingClaim,
        "ack-indeterminate" => BlockerKind::AckIndeterminate,
        "packet-refused" => BlockerKind::PacketRefused,
        "queue-unranked" => BlockerKind::QueueUnranked,
        "missing-receiver-agent" => BlockerKind::MissingReceiverAgent,
        "own-child-blocks-epic" => BlockerKind::OwnChildBlocksEpic,
        "half-claim" => BlockerKind::HalfClaim,
        _ => return None,
    })
}

fn cmd_redispatch(args: &[String]) -> ExitCode {
    let mut bead = None;
    let mut kind = None;
    let mut hold = None;
    let mut live_raw = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bead" if i + 1 < args.len() => {
                bead = Some(args[i + 1].as_str());
                i += 2;
            }
            "--kind" if i + 1 < args.len() => {
                kind = parse_kind(&args[i + 1]);
                i += 2;
            }
            "--hold-pane" if i + 1 < args.len() => {
                hold = Some(args[i + 1].as_str());
                i += 2;
            }
            "--live" if i + 1 < args.len() => {
                live_raw = Some(args[i + 1].as_str());
                i += 2;
            }
            other => {
                eprintln!("blocker-taxonomy: unknown flag {other}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(bead), Some(kind)) = (bead, kind) else {
        eprintln!("blocker-taxonomy: --bead and --kind required");
        return ExitCode::from(2);
    };
    let live_panes: Vec<String> = live_raw
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let live = match LiveSet::new(live_panes) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("blocker-taxonomy: live set refused: {e:?}");
            return ExitCode::from(1);
        }
    };
    let decision = decide_redispatch(bead, kind, hold, &live);
    match serde_json::to_string(&decision) {
        Ok(s) => println!("{s}"),
        Err(_) => println!("{decision:?}"),
    }
    match decision {
        RedispatchDecision::Release { .. } => ExitCode::from(0),
        RedispatchDecision::KeepAlive { .. }
        | RedispatchDecision::RefuseHumanGated { .. }
        | RedispatchDecision::RefuseNotKernelClearable { .. } => ExitCode::from(3),
    }
}
