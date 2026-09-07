//! THE OPERATOR SURFACE for the finding kernel (`omp-orchestrator-2sx1`).
//!
//! # Why this file exists
//!
//! `crates/finding` was the most-wired library in this workspace and had **zero**
//! bin targets: 14 manifest callers, 19 src callers, `command -v finding` absent.
//! `AGENTS.md` records its author writing it "thirty minutes earlier to make an
//! unfiled gap impossible, then bypassing it in the next tool call". The reason is
//! now measured rather than confessed: **the bypass was the only available path.**
//! A library an operator cannot invoke is not a kernel, it is a proposal.
//!
//! # This bin CALLS the library, it does not re-implement it
//!
//! Every guarantee stays where it was already correct and tested:
//! `Finding::new` validation, the FNV-1a spool stem, the write-then-rename,
//! `BrPublisher`'s argv, `mark_published`'s rename-never-delete, and
//! `pending`'s typed refusal on an unreadable directory. This file is argv
//! parsing, a runtime, and exit codes — nothing else. Any logic added here
//! would be a second implementation of a guarantee that already has one.
//!
//! # NO-CLAIM
//!
//! A reachable bin makes the kernel INVOKABLE, not invoked. It removes the
//! excuse, not the behaviour: nothing here stops an operator running `br create`
//! by hand. That half needs a `PreToolUse` hook and is explicitly not this bead.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use asupersync::Cx;
use asupersync::runtime::RuntimeBuilder;
use asupersync::types::Budget;

use finding::{BR, BrPublisher, Finding, FindingError, pending};

/// Exit codes are DISTINCT PER CAUSE, deliberately.
///
/// `AGENTS.md` rule 7: a known-bad leg that matches only `rc != 0` goes green on
/// any unrelated breakage — `DJ-D-OT-UNGATED` rested on a `101` that a
/// workspace-loading outage also produced. A caller here can tell a missing
/// field from an unwritable spool from a dead publisher without parsing prose.
const EXIT_USAGE: u8 = 2;
const EXIT_MISSING_FIELD: u8 = 3;
const EXIT_SPOOL: u8 = 4;
const EXIT_PUBLISH: u8 = 5;
const EXIT_CANCELLED: u8 = 6;
const EXIT_SPOOL_UNREADABLE: u8 = 7;

const USAGE: &str = "\
usage: finding <subcommand>

subcommands:
  file      validate, spool, publish, and mark a finding published
  pending   the recovery sweep: report durable spool rows awaiting publication
  recover   publish every pending spool row, retiring each only on a confirmed id

file options (WHAT, WHY, ACCEPTANCE and LABELS are all REQUIRED by the kernel):
  --what <text> | --what-file <path>
  --why <text> | --why-file <path>
  --acceptance <text> | --acceptance-file <path>
  --labels <csv>            at least one non-empty label
  --priority <0-255>        default 1
  --spool <dir>             default <repo>/.flywheel/findings
  --repo <dir>              default .
  --br <program>            default br

pending / recover options:
  --spool <dir>
  --repo <dir>              recover only
  --br <program>            recover only
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("file") => run(Op::File, &args[1..]),
        Some("pending") => run(Op::Pending, &args[1..]),
        Some("recover") => run(Op::Recover, &args[1..]),
        Some("--help" | "-h" | "help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("FINDING_USAGE unknown subcommand {other:?}");
            eprint!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
        None => {
            eprint!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    File,
    Pending,
    Recover,
}

struct Config {
    what: String,
    why: String,
    acceptance: String,
    labels: Vec<String>,
    priority: u8,
    spool: PathBuf,
    repo: PathBuf,
    br: PathBuf,
}

fn run(op: Op, args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let config = match parse(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("FINDING_USAGE {message}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    // `pending` is a pure read and needs no runtime: keeping it off the async
    // path means the recovery sweep can be inspected even if a runtime cannot
    // be built, which is exactly when an operator most wants to look.
    if op == Op::Pending {
        return report_pending(&config.spool);
    }
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("FINDING_RUNTIME_ERROR error={error}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let cx = runtime.request_cx_with_budget(Budget::INFINITE);
    runtime.block_on(async move { run_async(&cx, op, config).await })
}

async fn run_async(cx: &Cx, op: Op, config: Config) -> ExitCode {
    let publisher = BrPublisher::new(config.br.clone(), config.repo.clone());
    match op {
        Op::File => {
            // `Finding::new` is the ONLY constructor and it refuses an incomplete
            // finding. This bin does not pre-check the fields: duplicating the
            // precondition here would let the two copies drift.
            let finding = match Finding::new(
                config.what,
                config.why,
                config.acceptance,
                config.labels,
                config.priority,
            ) {
                Ok(finding) => finding,
                Err(error) => return fail(&error),
            };
            match finding.file(cx, &config.spool, &publisher).await {
                Ok(filed) => {
                    println!("{}", filed.id());
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        Op::Recover => match Finding::recover_pending(cx, &config.spool, &publisher).await {
            Ok(0) => {
                println!("FINDING_RECOVER_EMPTY spool={} recovered=0", display(&config.spool));
                ExitCode::SUCCESS
            }
            Ok(count) => {
                println!(
                    "FINDING_RECOVER_OK spool={} recovered={count}",
                    display(&config.spool)
                );
                ExitCode::SUCCESS
            }
            Err(error) => fail(&error),
        },
        Op::Pending => unreachable!("pending is handled before the runtime is built"),
    }
}

/// ANTI-VACUITY. "I could not look" and "there is nothing there" are OPPOSITE
/// conditions and get different tokens and different exit codes. Conflating them
/// is the defect `AGENTS.md` blames for twelve confident zeros in one session, and
/// the library already refuses an unreadable directory — this only keeps the
/// distinction visible at the operator boundary instead of flattening it to 0.
fn report_pending(spool: &Path) -> ExitCode {
    match pending(spool) {
        Ok(rows) if rows.is_empty() => {
            println!("FINDING_PENDING_EMPTY spool={} count=0", display(spool));
            ExitCode::SUCCESS
        }
        Ok(rows) => {
            println!(
                "FINDING_PENDING_ROWS spool={} count={}",
                display(spool),
                rows.len()
            );
            for row in rows {
                println!("{}", display(&row));
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("FINDING_SPOOL_UNREADABLE spool={} error={error}", display(spool));
            ExitCode::from(EXIT_SPOOL_UNREADABLE)
        }
    }
}

/// The message NAMES THE CAUSE, and the code is distinct per cause. A known-bad
/// leg asserting only the code cannot tell a missing ACCEPTANCE from a dead `br`.
fn fail(error: &FindingError) -> ExitCode {
    let code = match error {
        FindingError::MissingField(field) => {
            eprintln!("FINDING_MISSING_FIELD field={field}");
            EXIT_MISSING_FIELD
        }
        FindingError::WaiverNeedsReason => {
            eprintln!("FINDING_WAIVER_NEEDS_REASON");
            EXIT_MISSING_FIELD
        }
        FindingError::SpoolUnwritable(detail) => {
            eprintln!("FINDING_SPOOL_UNWRITABLE detail={detail}");
            EXIT_SPOOL
        }
        FindingError::PublishFailed(detail) => {
            eprintln!("FINDING_PUBLISH_FAILED detail={detail}");
            EXIT_PUBLISH
        }
        // A cancelled file() leaves a RECOVERABLE spool row; naming its path is
        // the whole point of the spool guarantee, so it goes to the operator.
        FindingError::Cancelled { spool_path } => {
            eprintln!(
                "FINDING_CANCELLED recoverable_spool_row={}",
                display(spool_path)
            );
            EXIT_CANCELLED
        }
    };
    ExitCode::from(code)
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

fn parse(args: &[String]) -> Result<Config, String> {
    let mut what: Option<String> = None;
    let mut why: Option<String> = None;
    let mut acceptance: Option<String> = None;
    let mut labels: Vec<String> = Vec::new();
    let mut priority: u8 = 1;
    let mut spool: Option<PathBuf> = None;
    let mut repo = PathBuf::from(".");
    let mut br = PathBuf::from(BR);

    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let mut value = |name: &str| -> Result<String, String> {
            args.get(index + 1)
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match flag {
            "--what" => what = Some(set_once(what.take(), value("--what")?, "WHAT")?),
            "--why" => why = Some(set_once(why.take(), value("--why")?, "WHY")?),
            "--acceptance" => {
                acceptance = Some(set_once(
                    acceptance.take(),
                    value("--acceptance")?,
                    "ACCEPTANCE",
                )?);
            }
            "--what-file" => {
                what = Some(set_once(what.take(), read(&value("--what-file")?)?, "WHAT")?);
            }
            "--why-file" => {
                why = Some(set_once(why.take(), read(&value("--why-file")?)?, "WHY")?);
            }
            "--acceptance-file" => {
                acceptance = Some(set_once(
                    acceptance.take(),
                    read(&value("--acceptance-file")?)?,
                    "ACCEPTANCE",
                )?);
            }
            "--labels" => {
                labels = value("--labels")?
                    .split(',')
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
            }
            "--priority" => {
                let raw = value("--priority")?;
                priority = raw
                    .parse()
                    .map_err(|_| format!("--priority must be 0-255, got {raw:?}"))?;
            }
            "--spool" => spool = Some(PathBuf::from(value("--spool")?)),
            "--repo" => repo = PathBuf::from(value("--repo")?),
            "--br" => br = PathBuf::from(value("--br")?),
            other => return Err(format!("unknown flag {other:?}")),
        }
        index += 2;
    }

    // Relative to the repo, so a checkout is self-contained and no absolute path
    // is baked into a binary — `path-literal-guard` refuses the alternative.
    let spool = spool.unwrap_or_else(|| repo.join(".flywheel").join("findings"));
    Ok(Config {
        // Empty strings are passed THROUGH to `Finding::new` rather than caught
        // here, so the refusal and its field name come from the kernel.
        what: what.unwrap_or_default(),
        why: why.unwrap_or_default(),
        acceptance: acceptance.unwrap_or_default(),
        labels,
        priority,
        spool,
        repo,
        br,
    })
}

fn set_once(existing: Option<String>, value: String, field: &str) -> Result<String, String> {
    if existing.is_some() {
        return Err(format!("{field} given twice"));
    }
    Ok(value)
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("cannot read {path:?}: {error}"))
}
