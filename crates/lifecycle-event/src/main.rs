//! Thin emit frontend. Logic lives in the lib.
//!
//! usage: lifecycle-event emit --layer L0 --stage-from HUMAN --stage-to S1.L0 \
//!          --actor installer --outcome emitted --reason-code INSTALL_VERIFIED \
//!          [--journal PATH] [--pane %9] [--blocker TEXT] [--step ID --status S --next CMD]

#![forbid(unsafe_code)]

use lifecycle_event::{
    default_host_journal, DurableJournal, EmitError, Layer, LifecycleEvent, Outcome, ReasonCode,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == name {
            return it.next().map(String::as_str);
        }
    }
    None
}

fn require<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    flag(args, name).ok_or_else(|| format!("missing {name}"))
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("emit") => emit(&args[1..]),
        _ => Err(
            "usage: lifecycle-event emit --layer L0..L5 --stage-from X --stage-to Y --actor A --outcome emitted|refused|idle --reason-code CODE [--journal PATH]"
                .to_owned(),
        ),
    }
}

fn emit(args: &[String]) -> Result<(), String> {
    let layer = Layer::parse(require(args, "--layer")?).map_err(|e| e.to_string())?;
    let outcome = Outcome::parse(require(args, "--outcome")?).map_err(|e| e.to_string())?;
    let reason = ReasonCode::new(require(args, "--reason-code")?).map_err(|e| e.to_string())?;
    let mut event = LifecycleEvent::new(
        layer,
        require(args, "--stage-from")?,
        require(args, "--stage-to")?,
        require(args, "--actor")?,
        outcome,
        reason,
    );
    if let Some(pane) = flag(args, "--pane") {
        event = event.with_pane(pane);
    }
    if let Some(blocker) = flag(args, "--blocker") {
        event = event.with_blocker(blocker);
    }
    if let (Some(step), Some(status), Some(next)) = (
        flag(args, "--step"),
        flag(args, "--status"),
        flag(args, "--next"),
    ) {
        event = event.with_step(step, status, next);
    }
    let journal_path = flag(args, "--journal")
        .map(PathBuf::from)
        .unwrap_or_else(default_host_journal);
    let journal = DurableJournal::open(&journal_path).map_err(|e| e.to_string())?;
    let back = lifecycle_event::emit_one_host(&journal, event).map_err(|e: EmitError| e.to_string())?;
    println!(
        "LIFECYCLE_EVENT_WRITTEN path={} lines={}",
        back.path.display(),
        back.lines
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
