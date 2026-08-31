#![forbid(unsafe_code)]

//! One subprocess boundary for cancel-correct, process-group-owned children.
//!
//! The helper deliberately configures every child with a fresh process group and
//! group-targeted cancellation. `asupersync::process::Command::output_async`
//! drains stdout and stderr concurrently while observing the caller's `Cx`.
//! A timeout or deadline cancellation is surfaced as [`RunError::Timeout`],
//! never as a child failure or an invented output verdict.

use asupersync::Cx;
use asupersync::process::{
    Command, ExitStatus, Output, ProcessError, ProcessGroupMode, ProcessSignalTarget, Stdio,
};
use asupersync::types::CancelKind;
use std::fmt;

/// The only failure categories exposed by the shared process boundary.
#[derive(Debug)]
pub enum RunError {
    /// The child could not be spawned or waited for.
    Process(ProcessError),
    /// The owning context was cancelled by a timeout or deadline.
    Timeout,
    /// The owning context was cancelled for another reason.
    Cancelled(CancelKind),
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => write!(formatter, "process error: {error}"),
            Self::Timeout => formatter.write_str("TIMEOUT reason=process_context_deadline"),
            Self::Cancelled(kind) => write!(formatter, "CANCELLED reason={kind:?}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            Self::Timeout | Self::Cancelled(_) => None,
        }
    }
}

impl From<ProcessError> for RunError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

fn cancellation_error(cx: &Cx) -> RunError {
    if cx.any_cause_is(CancelKind::Timeout) || cx.any_cause_is(CancelKind::Deadline) {
        RunError::Timeout
    } else {
        RunError::Cancelled(
            cx.cancel_reason()
                .map_or(CancelKind::User, |reason| reason.kind),
        )
    }
}

fn checkpoint(cx: &Cx) -> Result<(), RunError> {
    cx.checkpoint().map_err(|_| cancellation_error(cx))
}

fn configure_group(command: &mut Command) {
    command
        .process_group_mode(ProcessGroupMode::NewProcessGroup)
        .signal_target(ProcessSignalTarget::ProcessGroup)
        .kill_on_drop(true);
}

/// Spawn a child in its own process group and capture both output streams.
///
/// `&Cx` is first so cancellation is owned by the caller's region. The
/// asupersync child wait drains stdout and stderr concurrently and escalates
/// group termination on cancellation.
pub async fn run_output(cx: &Cx, mut command: Command) -> Result<Output, RunError> {
    checkpoint(cx)?;
    configure_group(&mut command);
    command
        .stdin(Stdio::Null)
        .stdout(Stdio::Pipe)
        .stderr(Stdio::Pipe);

    match command.output_async(cx).await {
        Ok(output) => {
            checkpoint(cx)?;
            Ok(output)
        }
        Err(_error) if cx.is_cancel_requested() => Err(cancellation_error(cx)),
        Err(error) => Err(RunError::Process(error)),
    }
}

/// Spawn a child in its own process group and return its exit status.
///
/// This uses the same captured-output path as [`run_output`], ensuring callers
/// cannot accidentally reintroduce an undrained pipe while only inspecting the
/// exit status.
pub async fn run_status(cx: &Cx, command: Command) -> Result<ExitStatus, RunError> {
    Ok(run_output(cx, command).await?.status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asupersync::runtime::RuntimeBuilder;
    use asupersync::types::CancelKind;
    use std::fs;
    use std::process::{Command as StdCommand, Stdio as StdStdio};
    use std::thread;
    use std::time::{Duration, Instant};

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    fn wait_until_process_gone(pid: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let still_exists = StdCommand::new("/bin/kill")
                .args(["-0", pid])
                .stderr(StdStdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if !still_exists {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn completed_child_returns_output() {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime installs Cx");
            let output = run_output(&cx, shell("printf stdout; printf stderr >&2"))
                .await
                .expect("completed child should return output");
            assert!(output.status.success());
            assert_eq!(output.stdout, b"stdout");
            assert_eq!(output.stderr, b"stderr");
        });
    }

    #[test]
    fn both_large_pipes_are_drained_without_deadlock() {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime installs Cx");
            let output = run_output(
                &cx,
                shell("/usr/bin/head -c 131072 /dev/zero; /usr/bin/head -c 131072 /dev/zero >&2"),
            )
            .await
            .expect("large dual-pipe child should complete");
            assert!(output.status.success());
            assert!(output.stdout.len() >= 131_072, "stdout was truncated");
            assert!(output.stderr.len() >= 131_072, "stderr was truncated");
        });
    }

    #[test]
    fn timeout_kills_the_process_group_and_is_not_a_failure_verdict() {
        let pid_file = std::env::temp_dir().join(format!(
            "subprocess-contract-grandchild-{}.pid",
            std::process::id()
        ));
        let _ = fs::remove_file(&pid_file);
        let script = format!(
            "trap '' TERM; (trap '' TERM; echo $$ > '{}'; sleep 30) & wait",
            pid_file.display()
        );
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime installs Cx");
            let cancel = cx.clone();
            let pid_path = pid_file.clone();
            let trigger = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(2);
                while !pid_path.is_file() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                cancel.cancel_with(CancelKind::Timeout, Some("test deadline"));
            });

            let result = run_output(&cx, shell(&script)).await;
            trigger.join().expect("cancellation trigger thread");
            assert!(matches!(result, Err(RunError::Timeout)), "got {result:?}");
        });

        let pid = fs::read_to_string(&pid_file)
            .expect("grandchild must publish its pid before cancellation")
            .trim()
            .to_owned();
        assert!(
            wait_until_process_gone(&pid),
            "grandchild {pid} survived group cancellation"
        );
        let _ = fs::remove_file(pid_file);
    }
}
