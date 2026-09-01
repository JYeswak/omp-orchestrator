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
/// Spawn a child in its own process group, drain both pipes concurrently on
/// dedicated readers, and bound the whole wait by `deadline`.
///
/// Sync on purpose: census readers, commit hooks, and gate bins run outside
/// any asupersync runtime and must not grow an async ripple to get the
/// five-rule contract. The child is a process-group leader (`process_group(0)`
/// is std-stable and unsafe-free), so its pid IS the pgid and the group kill
/// needs no libc. On deadline the group is TERMed, graced, then KILLed —
/// grandchildren included, which is the measured admission-lock trap.
///
/// PRE-FIX note: the failing-first stub of this function mirrored unbounded
/// `.output()` and the deadline tests below failed against it on purpose.
/// `READER_JOIN_GRACE` bounds how long the stdout/stderr readers may take to
/// observe EOF after the group was killed. Orphaned group members holding the
/// pipe are the measured admission-lock trap; a bounded join turns that
/// pathology into a restrictive terminal instead of a hang.
const READER_JOIN_GRACE: std::time::Duration = std::time::Duration::from_secs(1);

/// Wait out the reader join, but never longer than the grace: after the
/// group KILL, EOF is immediate for every in-group pipe holder.
fn join_reader(handle: std::thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    let deadline = std::time::Instant::now() + READER_JOIN_GRACE;
    while std::time::Instant::now() < deadline {
        if handle.is_finished() {
            return handle.join().unwrap_or_default();
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Vec::new()
}

/// Spawn a child in its own process group, drain both pipes concurrently on
/// dedicated readers, and bound the whole wait by `deadline`.
///
/// Sync on purpose: census readers, commit hooks, and gate bins run outside
/// any asupersync runtime and must not grow an async ripple to get the
/// five-rule contract. The child is a process-group leader (`process_group(0)`
/// is std-stable and unsafe-free), so its pid IS the pgid and the group kill
/// needs no libc. On deadline the group is TERMed, graced, then KILLed —
/// grandchildren included, which is the measured admission-lock trap.
pub fn bounded_output(
    command: &mut std::process::Command,
    deadline: std::time::Duration,
) -> BoundedOutcome {
    use std::io::Read;

    let mut child = match command
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return BoundedOutcome::Unspawned(error),
    };
    let pid = child.id();

    use std::os::unix::process::CommandExt as _;

    // Dedicated readers own the pipes for the child's whole life: the wait
    // loop below can poll as coarsely as it likes without ever letting a
    // pipe buffer fill, so the ~64 KiB undrained-pipe deadlock cannot form.
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stdout_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }
        if started.elapsed() >= deadline {
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };

    match status {
        Some(status) => {
            // The child exited on its own. Reap (no-op after try_wait Some)
            // and collect what the readers drained.
            let _ = child.wait();
            let stdout = stdout_reader.join().unwrap_or_default();
            let stderr = stderr_reader.join().unwrap_or_default();
            BoundedOutcome::Completed(std::process::Output {
                status,
                stdout,
                stderr,
            })
        }
        None => {
            // Deadline elapsed: signal the GROUP (-pid, the child is its own
            // group leader), TERM then graced KILL, grandchildren included.
            // TimedOut carries NO output: a killed read is not an answer.
            let group = format!("-{pid}");
            let _ = std::process::Command::new("/bin/kill")
                .args(["-TERM", &group])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", &group])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            // BOUNDED reap: after the group KILL the child is dead, but if a
            // future mutation or regression drops the KILL leg, an unbounded
            // wait here would hang the restrictive path itself - the adapter
            // rule is that no wait, including shutdown, is unbounded.
            let reap_deadline = std::time::Instant::now() + READER_JOIN_GRACE;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) | Err(_) => break,
                    Ok(None) if std::time::Instant::now() >= reap_deadline => break,
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                }
            }
            let _ = join_reader(stdout_reader);
            let _ = join_reader(stderr_reader);
            BoundedOutcome::TimedOut
        }
    }
}

/// The outcome of a bounded synchronous spawn. The two restrictive variants
/// are the whole contract: a child the deadline killed can NEVER surface as
/// [`BoundedOutcome::Completed`], and a child that could not start is named
/// rather than silently folded into either a completed read or a timeout.
#[derive(Debug)]
pub enum BoundedOutcome {
    /// The child exited on its own within the deadline - success or failure
    /// is the caller's business; this variant only says WE did not kill it.
    Completed(std::process::Output),
    /// The deadline elapsed and the process group was signalled. Restrictive:
    /// carries no output verdict.
    TimedOut,
    /// The child could not be spawned at all. Restrictive, and distinct from
    /// TimedOut because the remedy differs (path/env vs. deadline/slow child).
    Unspawned(std::io::Error),
}


#[cfg(test)]
mod tests {
    use super::*;
    use asupersync::runtime::RuntimeBuilder;
    use asupersync::types::CancelKind;
    use std::fs;
    use std::process::{Command as StdCommand, Stdio as StdStdio};
    use std::thread;

    fn pid_alive(pid: u32) -> bool {
        StdCommand::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(StdStdio::null())
            .stderr(StdStdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    static LAST_SLEEP_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    #[test]
    fn sleep_child_past_deadline_is_timedout_never_completed() {
        let mut command = StdCommand::new("/bin/sh");
        command.args(["-c", "sleep 5"]);
        let started = Instant::now();
        let outcome = bounded_output(&mut command, Duration::from_millis(300));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "bounded_output must not wait out the child's full runtime"
        );
        match outcome {
            BoundedOutcome::TimedOut => {}
            BoundedOutcome::Completed(_) => {
                panic!("a deadline-killed child must NEVER surface as Completed")
            }
            BoundedOutcome::Unspawned(error) => {
                panic!("sleep spawns everywhere; unspawned: {error}")
            }
        }
    }

    #[test]
    fn fast_child_completes_with_output_and_status() {
        let mut command = StdCommand::new("/bin/sh");
        command.args(["-c", "printf out; printf err >&2; exit 3"]);
        match bounded_output(&mut command, Duration::from_secs(5)) {
            BoundedOutcome::Completed(output) => {
                assert_eq!(output.stdout, b"out");
                assert_eq!(output.stderr, b"err");
                assert_eq!(output.status.code(), Some(3));
            }
            BoundedOutcome::TimedOut => panic!("a fast child must complete"),
            BoundedOutcome::Unspawned(error) => panic!("/bin/sh must spawn: {error}"),
        }
    }
    #[test]
    fn deadline_killed_child_is_reaped_not_orphaned() {
        // The child records its own pid; after bounded_output returns, the
        // group leader must be gone promptly. The measured admission-lock
        // trap was grandchildren that outlived every timeout.
        let pid_file = std::env::temp_dir().join(format!(
            "subprocess-contract-{}-pid",
            std::process::id()
        ));
        // The DIRECT child ignores TERM and runs a 30s loop: TERM kills its
        // transient sleep children but never the shell, so only the group
        // KILL can reap it. A mutation dropping the KILL leg leaves the pid
        // alive and this test goes RED.
        let mut command = StdCommand::new("/bin/sh");
        command.args(["-c", "echo $$ > \"$PIDFILE\"; trap '' TERM; i=0; while [ $i -lt 300 ]; do sleep 0.1; i=$((i+1)); done"]);
        command.env("PIDFILE", &pid_file);
        let _ = bounded_output(&mut command, Duration::from_millis(250));
        let deadline = Instant::now() + Duration::from_secs(3);
        let pid = loop {
            match fs::read_to_string(&pid_file) {
                Ok(text) => break text.trim().parse::<u32>().expect("pid"),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("child never recorded its pid: {error}"),
            }
        };
        while Instant::now() < deadline {
            if !pid_alive(pid) {
                let _ = fs::remove_file(&pid_file);
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = fs::remove_file(&pid_file);
        panic!("killed group leader {pid} still alive: the kill path is broken");
    }

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
