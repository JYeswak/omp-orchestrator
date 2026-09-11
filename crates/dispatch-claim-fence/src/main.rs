//! THE OPERATOR SURFACE for the dispatch claim fence (`omp-orchestrator-dda5`).
//!
//! # Why this file exists
//!
//! `dispatch-claim-fence` is the fail-closed admission check that a bead must be
//! `in_progress` and owned by the exact receiver before a dispatch packet is
//! built. It existed only as a library: an operator or a shell-driven
//! orchestrator tick could not run the fence, so the fence was skipped and the
//! packet was sent anyway. A check that only a linked Rust caller can reach does
//! not fence the lane an operator actually dispatches from.
//!
//! # This bin CALLS the library, it does not re-implement it
//!
//! `parse_br_show_json`, `authorize`, `authorize_with_identities`,
//! `ClaimFenceError::code` and `claim_required_advice` stay the single
//! implementation. This file is argv parsing, stdin/file reading, and an exit
//! code per refusal cause — nothing else. Any admission logic added here would
//! be a second fence that could disagree with the first.
//!
//! # NO-CLAIM
//!
//! A permit is authorization to CONSTRUCT a packet from a point-in-time
//! projection. It is not proof that transport happened, and it is not a lock:
//! the tracker can change between this exit and the send. The dispatch ledger
//! remains the authority for "was it dispatched".

#![forbid(unsafe_code)]

use std::io::Read;
use std::process::ExitCode;

use dispatch_claim_fence::{
    ClaimFenceError, DispatchIntent, DispatchLedgerEvidence, DispatchPermit, SnapshotParseError,
    authorize,
};

/// Exit codes are DISTINCT PER CAUSE, deliberately.
///
/// A known-bad leg that matches only `rc != 0` goes green on the wrong failure.
/// A caller that wants "did the fence refuse because the bead is claimed by
/// someone else" must be able to ask that without parsing English.
mod exit {
    /// Usage error: argv did not name a well-formed intent.
    pub const USAGE: u8 = 2;
    /// The snapshot could not be read or parsed into a typed bead row.
    pub const SNAPSHOT: u8 = 3;
    /// Bead is not claimed by the receiver (`CLAIM_REQUIRED`).
    pub const CLAIM_REQUIRED: u8 = 10;
    /// Bead is claimed by a different agent (`ASSIGNED_ELSEWHERE`).
    pub const ASSIGNED_ELSEWHERE: u8 = 11;
    /// The tracker answered about a different bead (`SNAPSHOT_ID_MISMATCH`).
    pub const SNAPSHOT_ID_MISMATCH: u8 = 12;
    /// The tracker status is not one the fence recognizes (`UNKNOWN_STATUS`).
    pub const UNKNOWN_STATUS: u8 = 13;
    /// A bead dispatch was requested with no snapshot at all.
    pub const MISSING_SNAPSHOT: u8 = 14;
    /// An intent field required by the fence was empty.
    pub const MISSING_FIELD: u8 = 15;
    /// Any other typed refusal, including the identity-namespace family.
    pub const OTHER_REFUSAL: u8 = 16;
}

fn exit_code_for(error: &ClaimFenceError) -> u8 {
    match error.code() {
        "CLAIM_REQUIRED" => exit::CLAIM_REQUIRED,
        "ASSIGNED_ELSEWHERE" => exit::ASSIGNED_ELSEWHERE,
        "SNAPSHOT_ID_MISMATCH" => exit::SNAPSHOT_ID_MISMATCH,
        "UNKNOWN_STATUS" => exit::UNKNOWN_STATUS,
        "MISSING_SNAPSHOT" => exit::MISSING_SNAPSHOT,
        "MISSING_BEAD_ID" | "MISSING_RECEIVER_AGENT" | "MISSING_OPERATION" => exit::MISSING_FIELD,
        _ => exit::OTHER_REFUSAL,
    }
}

const USAGE: &str = "\
usage: dispatch-claim-fence authorize --receiver <agent>
                            (--bead <id> --snapshot <path|-> | --operation <name> --kind broadcast|correction)
                            [--ledger present|absent|unavailable:<reason>]

Reads a `br show <id> --json` projection and authorizes one dispatch intent.
Exit 0 permits packet construction. Every refusal has its own exit code:
  2 usage  3 snapshot  10 claim_required  11 assigned_elsewhere
  12 snapshot_id_mismatch  13 unknown_status  14 missing_snapshot
  15 missing_field  16 other_refusal";

struct Args {
    receiver: Option<String>,
    bead: Option<String>,
    snapshot: Option<String>,
    operation: Option<String>,
    kind: Option<String>,
    ledger: Option<String>,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        receiver: None,
        bead: None,
        snapshot: None,
        operation: None,
        kind: None,
        ledger: None,
    };
    let mut index = 0;
    while index < argv.len() {
        let flag = argv[index].as_str();
        let mut take = |slot: &mut Option<String>| -> Result<(), String> {
            let value = argv
                .get(index + 1)
                .ok_or_else(|| format!("{flag} requires a value"))?;
            *slot = Some(value.clone());
            Ok(())
        };
        match flag {
            "--receiver" => take(&mut args.receiver)?,
            "--bead" => take(&mut args.bead)?,
            "--snapshot" => take(&mut args.snapshot)?,
            "--operation" => take(&mut args.operation)?,
            "--kind" => take(&mut args.kind)?,
            "--ledger" => take(&mut args.ledger)?,
            other => return Err(format!("unrecognized argument {other}")),
        }
        index += 2;
    }
    Ok(args)
}

fn parse_ledger(value: Option<&str>) -> Result<DispatchLedgerEvidence, String> {
    match value {
        None => Ok(DispatchLedgerEvidence::Unavailable {
            reason: "not supplied".to_owned(),
        }),
        Some("present") => Ok(DispatchLedgerEvidence::Present),
        Some("absent") => Ok(DispatchLedgerEvidence::Absent),
        Some(other) => match other.strip_prefix("unavailable:") {
            Some(reason) if !reason.trim().is_empty() => Ok(DispatchLedgerEvidence::Unavailable {
                reason: reason.trim().to_owned(),
            }),
            _ => Err(format!(
                "--ledger expects present|absent|unavailable:<reason>, got {other}"
            )),
        },
    }
}

fn read_snapshot_bytes(source: &str) -> Result<Vec<u8>, String> {
    if source == "-" {
        let mut buffer = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buffer)
            .map_err(|error| format!("cannot read snapshot from stdin: {error}"))?;
        return Ok(buffer);
    }
    std::fs::read(source).map_err(|error| format!("cannot read snapshot {source}: {error}"))
}

fn permit_line(permit: &DispatchPermit) -> String {
    match permit {
        DispatchPermit::Bead {
            bead_id,
            receiver_agent,
        } => format!("DISPATCH_PERMIT kind=bead bead={bead_id} receiver={receiver_agent}"),
        DispatchPermit::Broadcast {
            operation,
            receiver_agent,
        } => format!(
            "DISPATCH_PERMIT kind=broadcast operation={operation} receiver={receiver_agent}"
        ),
        DispatchPermit::Correction {
            operation,
            receiver_agent,
        } => format!(
            "DISPATCH_PERMIT kind=correction operation={operation} receiver={receiver_agent}"
        ),
    }
}

fn snapshot_exit(error: &SnapshotParseError) -> u8 {
    eprintln!("DISPATCH_BLOCKED SNAPSHOT_UNPARSEABLE detail={error}");
    exit::SNAPSHOT
}

fn run(argv: Vec<String>) -> u8 {
    let mut argv = argv.into_iter();
    let Some(subcommand) = argv.next() else {
        eprintln!("{USAGE}");
        return exit::USAGE;
    };
    if subcommand == "--help" || subcommand == "-h" || subcommand == "help" {
        println!("{USAGE}");
        return 0;
    }
    if subcommand != "authorize" {
        eprintln!("DISPATCH_ERROR unknown subcommand {subcommand}\n{USAGE}");
        return exit::USAGE;
    }
    let rest: Vec<String> = argv.collect();
    let args = match parse_args(&rest) {
        Ok(args) => args,
        Err(detail) => {
            eprintln!("DISPATCH_ERROR {detail}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let receiver = args.receiver.unwrap_or_default();

    let intent = match (&args.bead, &args.operation) {
        (Some(_), Some(_)) => {
            eprintln!("DISPATCH_ERROR --bead and --operation are mutually exclusive\n{USAGE}");
            return exit::USAGE;
        }
        (None, None) => {
            eprintln!("DISPATCH_ERROR one of --bead or --operation is required\n{USAGE}");
            return exit::USAGE;
        }
        (Some(bead), None) => DispatchIntent::bead(bead, &receiver),
        (None, Some(operation)) => match args.kind.as_deref() {
            Some("broadcast") => DispatchIntent::broadcast(operation, &receiver),
            Some("correction") => DispatchIntent::correction(operation, &receiver),
            other => {
                eprintln!(
                    "DISPATCH_ERROR --operation requires --kind broadcast|correction, got {}\n{USAGE}",
                    other.unwrap_or("nothing")
                );
                return exit::USAGE;
            }
        },
    };

    let snapshot = if args.bead.is_some() {
        match args.snapshot.as_deref() {
            None => None,
            Some(source) => {
                let bytes = match read_snapshot_bytes(source) {
                    Ok(bytes) => bytes,
                    Err(detail) => {
                        eprintln!("DISPATCH_BLOCKED SNAPSHOT_UNREADABLE detail={detail}");
                        return exit::SNAPSHOT;
                    }
                };
                match dispatch_claim_fence::parse_br_show_json(&bytes) {
                    Ok(snapshot) => Some(snapshot),
                    Err(error) => return snapshot_exit(&error),
                }
            }
        }
    } else {
        None
    };

    let evidence = match parse_ledger(args.ledger.as_deref()) {
        Ok(evidence) => evidence,
        Err(detail) => {
            eprintln!("DISPATCH_ERROR {detail}\n{USAGE}");
            return exit::USAGE;
        }
    };

    match authorize(&intent, snapshot.as_ref()) {
        Ok(permit) => {
            println!("{}", permit_line(&permit));
            0
        }
        Err(error) => {
            eprintln!("{} code={}", error, error.code());
            if let Some(advice) = error.claim_required_advice(evidence) {
                eprintln!("{advice}");
            }
            exit_code_for(&error)
        }
    }
}

fn main() -> ExitCode {
    ExitCode::from(run(std::env::args().skip(1).collect()))
}
