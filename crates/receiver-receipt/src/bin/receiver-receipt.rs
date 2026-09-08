#![forbid(unsafe_code)]

//! Observation-only receiver receipt harness. Transport-specific sends stay outside this binary.

use receiver_receipt::{
    assess_receiver_receipt, observe_capture, ObservationIdentity, PostSendObservation,
    ReceiptVerdict,
};
use std::env;
use std::fmt;
use std::fs;
use std::process::{Command, ExitCode};
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

const CAPTURE_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum CaptureError {
    Failed {
        target: String,
        status: String,
    },
    Timeout {
        target: String,
        deadline_secs: u64,
    },
    Unspawned {
        target: String,
        error: String,
    },
    Write {
        target: String,
        file: String,
        error: String,
    },
}

impl CaptureError {
    fn exit_code(&self) -> ExitCode {
        match self {
            Self::Failed { .. } | Self::Write { .. } => ExitCode::from(1),
            Self::Timeout { .. } | Self::Unspawned { .. } => ExitCode::from(2),
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed { target, status } => {
                write!(formatter, "CAPTURE_FAILED target={target} status={status}")
            }
            Self::Timeout {
                target,
                deadline_secs,
            } => write!(
                formatter,
                "CAPTURE_TIMEOUT target={target} deadline_secs={deadline_secs}"
            ),
            Self::Unspawned { target, error } => {
                write!(formatter, "CAPTURE_UNSPAWNED target={target} error={error}")
            }
            Self::Write {
                target,
                file,
                error,
            } => write!(
                formatter,
                "CAPTURE_WRITE_FAILED target={target} file={file} error={error}"
            ),
        }
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: receiver-receipt capture <tmux-target> <file> <unix-seconds>\n\
         usage: receiver-receipt assess <pane-id> <pre-file> <post-file> <pre-seconds> <post-seconds>\n\
         usage: receiver-receipt scan-sender-exit-mapping <path>"
    );
    ExitCode::from(2)
}

fn identity(at: u64) -> ObservationIdentity {
    ObservationIdentity {
        epoch: "receiver-receipt-cli".into(),
        sequence: at,
        changed_at: at.to_string(),
    }
}

fn capture_with(program: &str, target: &str, path: &str, at: u64) -> Result<(), CaptureError> {
    let mut command = Command::new(program);
    command.args([tick_monitor::CAPTURE_PANE, "-p", "-t", target, "-S", "-200"]);
    let output = match bounded_output(&mut command, CAPTURE_DEADLINE) {
        BoundedOutcome::Completed(output) => output,
        BoundedOutcome::TimedOut => {
            return Err(CaptureError::Timeout {
                target: target.to_owned(),
                deadline_secs: CAPTURE_DEADLINE.as_secs(),
            });
        }
        BoundedOutcome::Unspawned(error) => {
            return Err(CaptureError::Unspawned {
                target: target.to_owned(),
                error: error.to_string(),
            });
        }
    };
    if !output.status.success() {
        return Err(CaptureError::Failed {
            target: target.to_owned(),
            status: output.status.to_string(),
        });
    }
    fs::write(path, &output.stdout).map_err(|error| CaptureError::Write {
        target: target.to_owned(),
        file: path.to_owned(),
        error: error.to_string(),
    })?;
    println!("CAPTURED target={target} file={path} at={at}");
    Ok(())
}

fn capture(target: &str, path: &str, at: u64) -> ExitCode {
    match capture_with(tick_monitor::TMUX, target, path, at) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
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
    let pre = observe_capture(pane_id, &pre_text, pre_at, identity(pre_at));
    let post = observe_capture(pane_id, &post_text, post_at, identity(post_at));
    let result = assess_receiver_receipt(pane_id, &pre, PostSendObservation::Present(post));
    println!("{} reason={:?}", result.label(), result.reason());
    match result {
        ReceiptVerdict::ReceiptConfirmed { .. } | ReceiptVerdict::AckConfirmed { .. } => {
            ExitCode::SUCCESS
        }
        ReceiptVerdict::NoReceipt { .. } => ExitCode::from(1),
        ReceiptVerdict::Indeterminate { .. } => ExitCode::from(2),
        ReceiptVerdict::Dead { .. } => ExitCode::from(1),
    }
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("capture") => {
            let (Some(target), Some(path), Some(at)) = (args.next(), args.next(), args.next())
            else {
                return usage();
            };
            let Ok(at) = at.parse() else { return usage() };
            capture(&target, &path, at)
        }
        Some("assess") => {
            let (Some(pane), Some(pre), Some(post), Some(pre_at), Some(post_at)) = (
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
            ) else {
                return usage();
            };
            let (Ok(pre_at), Ok(post_at)) = (pre_at.parse(), post_at.parse()) else {
                return usage();
            };
            assess(&pane, &pre, &post, pre_at, post_at)
        }
        Some("scan-sender-exit-mapping") => {
            let Some(path) = args.next() else {
                return usage();
            };
            match receiver_receipt::refuse_sender_exit_mapping_file(std::path::Path::new(&path)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(msg) => {
                    eprintln!("{msg}");
                    ExitCode::from(1)
                }
            }
        }
        _ => usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn capture_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("receiver-receipt-{label}-{}", std::process::id()))
    }

    #[test]
    fn known_good_capture_writes_exact_bounded_output() {
        let path = capture_path("known-good");
        capture_with("/usr/bin/printf", "fixture", path.to_str().unwrap(), 7).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"capture-pane");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn nonzero_capture_pins_typed_message_and_exit_code() {
        let path = capture_path("known-bad");
        let error =
            capture_with("/usr/bin/false", "missing-pane", path.to_str().unwrap(), 7).unwrap_err();
        assert_eq!(
            error.to_string(),
            "CAPTURE_FAILED target=missing-pane status=exit status: 1"
        );
        assert_eq!(error.exit_code(), ExitCode::from(1));
        assert!(!path.exists());
    }

    #[test]
    fn large_dual_pipe_timeout_is_bounded_and_restrictive() {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "trap '' TERM; (trap '' TERM; while :; do printf child; done) & while :; do printf parent; printf parent >&2; done",
        ]);
        let started = Instant::now();
        let result = bounded_output(&mut command, Duration::from_millis(100));
        assert!(matches!(result, BoundedOutcome::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
