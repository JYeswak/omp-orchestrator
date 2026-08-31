#![forbid(unsafe_code)]

//! Observation-only receiver receipt harness. Transport-specific sends stay outside this binary.

use receiver_receipt::{assess_receiver_receipt, observe_capture, PostSendObservation, ReceiptVerdict};
use std::env;
use std::fs;
use std::process::{Command, ExitCode};

fn usage() -> ExitCode {
    eprintln!(
        "usage: receiver-receipt capture <tmux-target> <file> <unix-seconds>\n\
         usage: receiver-receipt assess <pane-id> <pre-file> <post-file> <pre-seconds> <post-seconds>"
    );
    ExitCode::from(2)
}

fn capture(target: &str, path: &str, at: u64) -> ExitCode {
    let output = match Command::new("tmux")
        .args(["capture-pane", "-p", "-t", target, "-S", "-200"])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("capture: tmux spawn failed: {error}");
            return ExitCode::from(1);
        }
    };
    if !output.status.success() {
        eprintln!("capture: tmux exited with {}", output.status);
        return ExitCode::from(1);
    }
    if let Err(error) = fs::write(path, &output.stdout) {
        eprintln!("capture: cannot write {path}: {error}");
        return ExitCode::from(1);
    }
    println!("CAPTURED target={target} file={path} at={at}");
    ExitCode::SUCCESS
}

fn assess(pane_id: &str, pre_path: &str, post_path: &str, pre_at: u64, post_at: u64) -> ExitCode {
    let pre_text = match fs::read_to_string(pre_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("assess: cannot read {pre_path}: {error}");
            return ExitCode::from(2);
        }
    };
    let post_text = match fs::read_to_string(post_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("assess: cannot read {post_path}: {error}");
            return ExitCode::from(2);
        }
    };
    let pre = observe_capture(pane_id, &pre_text, pre_at);
    let post = observe_capture(pane_id, &post_text, post_at);
    let result = assess_receiver_receipt(pane_id, &pre, PostSendObservation::Present(post));
    println!("{} reason={:?}", result.label(), result.reason());
    match result {
        ReceiptVerdict::ReceiptConfirmed { .. } => ExitCode::SUCCESS,
        ReceiptVerdict::NoReceipt { .. } => ExitCode::from(1),
        ReceiptVerdict::Indeterminate { .. } => ExitCode::from(2),
        ReceiptVerdict::Dead { .. } => ExitCode::from(1),
    }
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("capture") => {
            let (Some(target), Some(path), Some(at)) = (args.next(), args.next(), args.next()) else {
                return usage();
            };
            let Ok(at) = at.parse() else { return usage() };
            capture(&target, &path, at)
        }
        Some("assess") => {
            let (Some(pane), Some(pre), Some(post), Some(pre_at), Some(post_at)) = (
                args.next(), args.next(), args.next(), args.next(), args.next()
            ) else {
                return usage();
            };
            let (Ok(pre_at), Ok(post_at)) = (pre_at.parse(), post_at.parse()) else {
                return usage();
            };
            assess(&pane, &pre, &post, pre_at, post_at)
        }
        _ => usage(),
    }
}
