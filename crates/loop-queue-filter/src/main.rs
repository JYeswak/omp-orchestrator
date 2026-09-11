#![forbid(unsafe_code)]

use loop_queue_filter::select::{
    assign_peer_grade_for_named_grader, assign_peer_grade_with_ledger, parse_observed_panes,
};
use loop_queue_filter::phase_gate::{
    apply_phase_gate, EXCEPTION_SET, GATED_PHASE, PHASE_GATE_ENABLED, SWITCH_ON_PRECONDITION,
};
use loop_queue_filter::selector::select_graph;
use std::io::{self, Read};
use std::process::ExitCode;

fn assign_grade_cli(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "usage: loop-queue-filter assign-grade --observer %N --observation FILE --jsonl FILE [--ledger FILE] [--grader %N]\n\
             Observer may be WORKING. Grader must be CONFIRMED_IDLE and not carrying a dispatch.\n\
             --ledger is the lifecycle JSONL (grader_pane, pane) third author source.\n\
             Prints GRADE_ASSIGNED bead=... grader_pane=... grader_assignee=... observer_pane=..."
        );
        return ExitCode::SUCCESS;
    }
    let mut observer = String::new();
    let mut observation_path = String::new();
    let mut jsonl_path = String::new();
    let mut ledger_path = String::new();
    let mut grader = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--observer" => {
                index += 1;
                observer = args.get(index).cloned().unwrap_or_default();
            }
            "--observation" => {
                index += 1;
                observation_path = args.get(index).cloned().unwrap_or_default();
            }
            "--jsonl" => {
                index += 1;
                jsonl_path = args.get(index).cloned().unwrap_or_default();
            }
            "--grader" => {
                index += 1;
                grader = args.get(index).cloned();
            }
            "--ledger" => {
                index += 1;
                ledger_path = args.get(index).cloned().unwrap_or_default();
            }
            other => {
                eprintln!("ASSIGN_GRADE_REFUSED unknown argument {other}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    if observer.trim().is_empty() || observation_path.is_empty() || jsonl_path.is_empty() {
        eprintln!("ASSIGN_GRADE_REFUSED usage: --observer %N --observation FILE --jsonl FILE");
        return ExitCode::from(2);
    }
    let observation = match std::fs::read(&observation_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("ASSIGN_GRADE_REFUSED observation={error}");
            return ExitCode::from(2);
        }
    };
    let jsonl = match std::fs::read_to_string(&jsonl_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("ASSIGN_GRADE_REFUSED jsonl={error}");
            return ExitCode::from(2);
        }
    };
    let ledger = if ledger_path.is_empty() {
        String::new()
    } else {
        match std::fs::read_to_string(&ledger_path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("ASSIGN_GRADE_REFUSED ledger={error}");
                return ExitCode::from(2);
            }
        }
    };
    let panes = match parse_observed_panes(&observation) {
        Ok(panes) => panes,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    // A named grader carries the eligibility gauntlet INSIDE the selector, so
    // its refusal cannot be overwritten by a later generic `no_idle_pane`.
    let assigned = match grader.as_deref() {
        Some(grader_pane) => {
            assign_peer_grade_for_named_grader(&observer, grader_pane, &panes, &jsonl, &ledger)
        }
        None => assign_peer_grade_with_ledger(&observer, &panes, &jsonl, &ledger),
    };
    match assigned {
        Ok(assignment) => {
            if let Some(grader_pane) = grader.as_deref() {
                if assignment.grader_pane != grader_pane {
                    eprintln!(
                        "NO_ELIGIBLE_GRADER reason=requested_grader_not_selected requested={grader_pane} selected={}",
                        assignment.grader_pane
                    );
                    return ExitCode::from(2);
                }
            }
            println!(
                "GRADE_ASSIGNED bead={} grader_pane={} grader_assignee={} observer_pane={}",
                assignment.bead,
                assignment.grader_pane,
                assignment.grader_assignee,
                assignment.observer_pane
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn select_graph_cli(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "usage: loop-queue-filter select-graph [--receipt FILE]\n\
             Resolves `bv` on PATH. Missing binary: SELECTOR_UNAVAILABLE program=bv (exit 2).\n\
             Present: one JSON receipt, exit 0. Recency is not a fallback."
        );
        return ExitCode::SUCCESS;
    }
    let mut receipt_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--receipt" => {
                index += 1;
                receipt_path = args.get(index).cloned();
                if receipt_path.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    eprintln!("SELECTOR_REFUSED reason=receipt_path_missing");
                    return ExitCode::from(2);
                }
            }
            other => {
                eprintln!("SELECTOR_REFUSED unknown argument {other}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let path = std::env::var("PATH").unwrap_or_default();
    match select_graph(&path) {
        Ok(receipt) => {
            let json = receipt.to_json();
            println!("{json}");
            if let Some(path) = receipt_path {
                if std::fs::write(&path, format!("{json}\n")).is_err() {
                    eprintln!("SELECTOR_RECEIPT_WRITE_FAILED path={path}");
                    return ExitCode::from(1);
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

/// `phase-gate` — report the gate's configuration and, given candidates on stdin, its decision.
///
/// ITEM 7 EXISTS BECAUSE OF THIS FUNCTION. `SWITCH_ON_PRECONDITION` was a `const` no code path
/// referenced, so the linker dropped it and `strings` on the operator binary found neither the
/// required NON-EPIC text nor the refuted `jplf.1` text — while the source read as compliant. A
/// `const` is not an artifact. This subcommand PRINTS it, which is what puts it in the binary and
/// what makes rule `8b` — ship the refuted form as a string plus a test that keeps it named —
/// true of the thing an operator actually runs rather than of the tree.
fn phase_gate_cli(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "usage: loop-queue-filter phase-gate [--arc-census N] [--phase-complete] [--enabled]\n\
             Reads candidate bead ids from stdin, one per line. Reports the gate decision.\n\
             Exit: 0 decision made, 2 usage, 30 arc census zero, 31 empty candidate set, \
             32 exception set incomplete."
        );
        return ExitCode::SUCCESS;
    }
    let mut arc_census: usize = 0;
    let mut phase_complete = false;
    let mut enabled = PHASE_GATE_ENABLED;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--arc-census" => {
                index += 1;
                arc_census = match args.get(index).and_then(|v| v.parse().ok()) {
                    Some(value) => value,
                    None => {
                        eprintln!("PHASE_GATE_USAGE --arc-census requires an integer");
                        return ExitCode::from(2);
                    }
                };
            }
            "--phase-complete" => phase_complete = true,
            // Explicit opt-in only. The shipped default stays off; see item 12.
            "--enabled" => enabled = true,
            other => {
                eprintln!("PHASE_GATE_USAGE unknown argument={other}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    // The configuration is printed BEFORE any decision, so an operator who pipes nothing still
    // learns the precondition and the shipped default. This is the line that carries the strings.
    println!(
        "PHASE_GATE_CONFIG shipped_default_enabled={PHASE_GATE_ENABLED} phase={GATED_PHASE} \
         exceptions={} precondition=\"{SWITCH_ON_PRECONDITION}\"",
        EXCEPTION_SET.len()
    );

    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("PHASE_GATE_USAGE could not read candidates from stdin");
        return ExitCode::from(2);
    }
    let candidates: Vec<String> = input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    match apply_phase_gate(&candidates, arc_census, phase_complete, enabled) {
        Ok(outcome) => {
            println!("{}", outcome.render());
            for id in &outcome.admitted {
                println!("PHASE_GATE_ADMITTED id={id}");
            }
            for (id, reason) in &outcome.withheld {
                println!("PHASE_GATE_WITHHELD id={id} code={} {}", reason.code(), reason.detail());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--selftest-guard") {
        println!("queue-filter guard: PASS (Rust binary present)");
        return ExitCode::SUCCESS;
    }
    if args.first().map(String::as_str) == Some("assign-grade") {
        return assign_grade_cli(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("phase-gate") {
        return phase_gate_cli(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("select-graph") {
        return select_graph_cli(&args[1..]);
    }
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return ExitCode::from(1);
    }
    let output = loop_queue_filter::run(&input, &args, &loop_queue_filter::Runtime::from_process());
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);
    ExitCode::from(output.code as u8)
}
