#![forbid(unsafe_code)]

use dispatch_saga::grading::{decide_all, GradingDecision};
use std::io::{self, Read};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("grading-transition") {
        eprintln!("usage: dispatch-saga grading-transition [--apply] [--grader NAME]...");
        return ExitCode::from(2);
    }
    let apply = args.iter().any(|a| a == "--apply");
    let mut graders = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--grader" && i + 1 < args.len() {
            graders.push(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    if graders.is_empty() {
        graders.push("Orchestrator".into());
    }
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        eprintln!("dispatch-saga: stdin unread");
        return ExitCode::from(1);
    }
    let candidates: Vec<dispatch_saga::grading::GradingCandidate> = match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("dispatch-saga: candidate json: {e}");
            return ExitCode::from(1);
        }
    };
    let decisions = match decide_all(&candidates, &graders) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("dispatch-saga: {e:?}");
            return ExitCode::from(1);
        }
    };
    for d in &decisions {
        println!("{d:?}");
        if apply {
            if let GradingDecision::Transition { br_update, .. } = d {
                if br_update.first().map(String::as_str) == Some("br") {
                    let status = Command::new(&br_update[0])
                        .args(&br_update[1..])
                        .status();
                    eprintln!("dispatch-saga: apply {br_update:?} -> {status:?}");
                }
            }
        }
    }
    ExitCode::from(0)
}
