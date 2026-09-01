#![forbid(unsafe_code)]

//! Shared comparator for oracle vs product.
//!
//! A COMPARATOR THAT CANNOT SEE A DIVERGENCE REPORTS PERFECT AGREEMENT.
//! `Agree` and "the comparator is broken" render identically. The `disagree_is_finding`
//! rule is load-bearing: disabling it is the blinding mutation this crate exists to catch.
//!
//! Empty / unreadable arms are NEVER agreement (vacuous green). An empty product
//! against a live oracle is DISAGREEMENT (ntm#254), not a quiet pass.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleCompareRule {
    DisagreeIsFinding,
    EmptyOracleIsError,
    UnreadableIsError,
    EmptyProductIsDisagreement,
}

impl OracleCompareRule {
    pub const ALL: &'static [OracleCompareRule] = &[
        OracleCompareRule::DisagreeIsFinding,
        OracleCompareRule::EmptyOracleIsError,
        OracleCompareRule::UnreadableIsError,
        OracleCompareRule::EmptyProductIsDisagreement,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            OracleCompareRule::DisagreeIsFinding => "disagree_is_finding",
            OracleCompareRule::EmptyOracleIsError => "empty_oracle_is_error",
            OracleCompareRule::UnreadableIsError => "unreadable_is_error",
            OracleCompareRule::EmptyProductIsDisagreement => "empty_product_is_disagreement",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct OracleCompareRules {
    pub disagree_is_finding: bool,
    pub empty_oracle_is_error: bool,
    pub unreadable_is_error: bool,
    pub empty_product_is_disagreement: bool,
}

impl Default for OracleCompareRules {
    fn default() -> Self {
        Self {
            disagree_is_finding: true,
            empty_oracle_is_error: true,
            unreadable_is_error: true,
            empty_product_is_disagreement: true,
        }
    }
}

impl OracleCompareRules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = OracleCompareRule::parse(name) else {
            return false;
        };
        match rule {
            OracleCompareRule::DisagreeIsFinding => self.disagree_is_finding = false,
            OracleCompareRule::EmptyOracleIsError => self.empty_oracle_is_error = false,
            OracleCompareRule::UnreadableIsError => self.unreadable_is_error = false,
            OracleCompareRule::EmptyProductIsDisagreement => self.empty_product_is_disagreement = false,
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CountArm {
    Unreadable,
    Value(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetArm {
    Unreadable,
    Value(BTreeSet<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OracleCompareVerdict {
    Agree { n: u64 },
    Disagree { oracle_n: u64, product_n: u64 },
    Unmeasurable { why: &'static str },
}

impl OracleCompareVerdict {
    pub fn exit_code(&self) -> i32 {
        match self {
            OracleCompareVerdict::Agree { .. } => 0,
            OracleCompareVerdict::Disagree { .. } => 1,
            OracleCompareVerdict::Unmeasurable { .. } => 2,
        }
    }
}

/// Census compare used by pane-oracle-diff (agent COUNT, not busy labels).
///
/// `session_visible`: tmux can see the session. Combined with oracle=0 this is
/// the shells-only PASS (zero agents is the true answer), not an unmeasurable ERROR.
pub fn compare_counts(
    oracle: CountArm,
    product: CountArm,
    session_visible: bool,
    rules: &OracleCompareRules,
) -> OracleCompareVerdict {
    match (oracle, product) {
        (CountArm::Unreadable, _) | (_, CountArm::Unreadable) => {
            if rules.unreadable_is_error {
                OracleCompareVerdict::Unmeasurable {
                    why: "unreadable_subject",
                }
            } else {
                OracleCompareVerdict::Agree { n: 0 }
            }
        }
        (CountArm::Value(0), CountArm::Value(p)) if !session_visible => {
            if rules.empty_oracle_is_error {
                OracleCompareVerdict::Unmeasurable {
                    why: "session_not_visible",
                }
            } else {
                OracleCompareVerdict::Agree { n: p }
            }
        }
        (CountArm::Value(0), CountArm::Value(0)) => OracleCompareVerdict::Agree { n: 0 },
        (CountArm::Value(o), CountArm::Value(p)) if o == p => OracleCompareVerdict::Agree { n: o },
        (CountArm::Value(o), CountArm::Value(p)) => {
            if rules.disagree_is_finding {
                OracleCompareVerdict::Disagree {
                    oracle_n: o,
                    product_n: p,
                }
            } else {
                OracleCompareVerdict::Agree { n: o }
            }
        }
    }
}

/// Set compare used by oracle-pane-state-differential (session:index, not %N, not busy labels).
pub fn compare_sets(oracle: SetArm, product: SetArm, rules: &OracleCompareRules) -> OracleCompareVerdict {
    match (oracle, product) {
        (SetArm::Unreadable, _) | (_, SetArm::Unreadable) => {
            if rules.unreadable_is_error {
                OracleCompareVerdict::Unmeasurable {
                    why: "unreadable_arm",
                }
            } else {
                OracleCompareVerdict::Agree { n: 0 }
            }
        }
        (SetArm::Value(o), _) if o.is_empty() => {
            if rules.empty_oracle_is_error {
                OracleCompareVerdict::Unmeasurable {
                    why: "empty_oracle",
                }
            } else {
                OracleCompareVerdict::Agree { n: 0 }
            }
        }
        (SetArm::Value(o), SetArm::Value(p)) if p.is_empty() => {
            if rules.empty_product_is_disagreement {
                OracleCompareVerdict::Disagree {
                    oracle_n: o.len() as u64,
                    product_n: 0,
                }
            } else {
                OracleCompareVerdict::Agree { n: o.len() as u64 }
            }
        }
        (SetArm::Value(o), SetArm::Value(p)) if o == p => OracleCompareVerdict::Agree { n: o.len() as u64 },
        (SetArm::Value(o), SetArm::Value(p)) => {
            if rules.disagree_is_finding {
                OracleCompareVerdict::Disagree {
                    oracle_n: o.len() as u64,
                    product_n: p.len() as u64,
                }
            } else {
                OracleCompareVerdict::Agree { n: o.len() as u64 }
            }
        }
    }
}

pub fn set_delta<'a>(
    oracle: &'a BTreeSet<String>,
    product: &'a BTreeSet<String>,
) -> (Vec<&'a str>, Vec<&'a str>) {
    let only_oracle: Vec<&str> = oracle.difference(product).map(|s| s.as_str()).collect();
    let only_product: Vec<&str> = product.difference(oracle).map(|s| s.as_str()).collect();
    (only_oracle, only_product)
}

/// Harvest selector: ready>0 expects exactly 1 selected row; ready=0 expects 0.
pub fn harvest_expect(ready: u64) -> u64 {
    if ready > 0 {
        1
    } else {
        0
    }
}

/// Compare harvest ready-count against selected-count through the shared comparator.
pub fn harvest_verdict(ready: u64, selected: u64, rules: &OracleCompareRules) -> OracleCompareVerdict {
    compare_counts(
        CountArm::Value(harvest_expect(ready)),
        CountArm::Value(selected),
        true,
        rules,
    )
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn kill_process_group(child: &mut std::process::Child) {
    let group = format!("-{}", child.id());
    let killed_group = Command::new("/bin/kill")
        .args(["-KILL", &group])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !killed_group {
        let _ = child.kill();
    }
}

#[cfg(not(unix))]
fn kill_process_group(child: &mut std::process::Child) {
    let _ = child.kill();
}

fn wait_deadline(
    mut child: std::process::Child,
    timeout: Duration,
    stdin: Option<&[u8]>,
) -> Option<Output> {
    std::thread::scope(|scope| {
        let stdout_reader = child.stdout.take().map(|mut stream| {
            scope.spawn(move || {
                let mut output = Vec::new();
                stream.read_to_end(&mut output).ok().map(|_| output)
            })
        });
        let stderr_reader = child.stderr.take().map(|mut stream| {
            scope.spawn(move || {
                let mut output = Vec::new();
                stream.read_to_end(&mut output).ok().map(|_| output)
            })
        });
        let stdin_writer = stdin.and_then(|bytes| {
            child.stdin.take().map(|mut stream| {
                scope.spawn(move || {
                    let _ = stream.write_all(bytes);
                })
            })
        });

        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(exit_status)) => break Some(exit_status),
                Ok(None) if start.elapsed() >= timeout => {
                    kill_process_group(&mut child);
                    break child.wait().ok();
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => {
                    kill_process_group(&mut child);
                    break child.wait().ok();
                }
            }
        };

        let io_finished = || {
            stdout_reader
                .as_ref()
                .is_none_or(|reader| reader.is_finished())
                && stderr_reader
                    .as_ref()
                    .is_none_or(|reader| reader.is_finished())
                && stdin_writer
                    .as_ref()
                    .is_none_or(|writer| writer.is_finished())
        };
        while !io_finished() {
            if start.elapsed() >= timeout {
                kill_process_group(&mut child);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let stdout = match stdout_reader {
            Some(reader) => reader.join().ok().flatten()?,
            None => Vec::new(),
        };
        let stderr = match stderr_reader {
            Some(reader) => reader.join().ok().flatten()?,
            None => Vec::new(),
        };
        let status = status?;

        Some(Output {
            status,
            stdout,
            stderr,
        })
    })
}

/// Bounded spawn. Both output streams are drained concurrently before the child is observed.
/// The control-plane workspace has no `subprocess-contract` crate; this preserves its synchronous
/// `std::process::Command` API while applying that crate's dual-pipe/process-group contract here.
pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> Option<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut cmd);
    let child = cmd.spawn().ok()?;
    wait_deadline(child, timeout, None)
}

/// Bounded spawn with stdin bytes (EOF after write). Used to drive loop-queue-filter.py.
pub fn spawn_timeout_stdin(mut cmd: Command, timeout: Duration, stdin: &[u8]) -> Option<Output> {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut cmd);
    let child = cmd.spawn().ok()?;
    wait_deadline(child, timeout, Some(stdin))
}

/// Prove a File we hold is CLOEXEC: a child cannot open our raw fd.
pub fn child_cannot_open_fd(fd: i32, timeout: Duration) -> bool {
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", "exec 3<>/dev/fd/$CHECK_FD"])
        .env("CHECK_FD", fd.to_string());
    spawn_timeout(cmd, timeout)
        .map(|o| !o.status.success())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disagree_is_finding() {
        let v = compare_counts(
            CountArm::Value(4),
            CountArm::Value(3),
            true,
            &OracleCompareRules::default(),
        );
        assert_eq!(
            v,
            OracleCompareVerdict::Disagree {
                oracle_n: 4,
                product_n: 3
            }
        );
    }

    #[test]
    fn blinding_reports_false_agreement() {
        let mut r = OracleCompareRules::default();
        assert!(r.disable("disagree_is_finding"));
        let v = compare_counts(CountArm::Value(4), CountArm::Value(3), true, &r);
        assert_eq!(v, OracleCompareVerdict::Agree { n: 4 });
    }

    #[test]
    fn empty_oracle_unmeasurable_when_session_gone() {
        let v = compare_counts(
            CountArm::Value(0),
            CountArm::Value(0),
            false,
            &OracleCompareRules::default(),
        );
        assert!(matches!(v, OracleCompareVerdict::Unmeasurable { .. }));
    }

    #[test]
    fn shells_only_is_agree() {
        let v = compare_counts(
            CountArm::Value(0),
            CountArm::Value(0),
            true,
            &OracleCompareRules::default(),
        );
        assert_eq!(v, OracleCompareVerdict::Agree { n: 0 });
    }

    #[test]
    fn empty_product_against_live_oracle_is_disagree() {
        let mut o = BTreeSet::new();
        o.insert("s:1".into());
        let v = compare_sets(
            SetArm::Value(o),
            SetArm::Value(BTreeSet::new()),
            &OracleCompareRules::default(),
        );
        assert_eq!(
            v,
            OracleCompareVerdict::Disagree {
                oracle_n: 1,
                product_n: 0
            }
        );
    }

    #[test]
    fn empty_oracle_set_is_unmeasurable() {
        let v = compare_sets(
            SetArm::Value(BTreeSet::new()),
            SetArm::Value(BTreeSet::new()),
            &OracleCompareRules::default(),
        );
        assert!(matches!(
            v,
            OracleCompareVerdict::Unmeasurable {
                why: "empty_oracle"
            }
        ));
    }

    #[test]
    fn harvest_ready_positive_expects_one() {
        assert_eq!(harvest_expect(0), 0);
        assert_eq!(harvest_expect(1), 1);
        assert_eq!(harvest_expect(9), 1);
        let v = harvest_verdict(5, 1, &OracleCompareRules::default());
        assert_eq!(v, OracleCompareVerdict::Agree { n: 1 });
        let v = harvest_verdict(5, 0, &OracleCompareRules::default());
        assert_eq!(
            v,
            OracleCompareVerdict::Disagree {
                oracle_n: 1,
                product_n: 0
            }
        );
    }

    #[test]
    fn harvest_blinding_hides_starvation() {
        let mut r = OracleCompareRules::default();
        assert!(r.disable("disagree_is_finding"));
        let v = harvest_verdict(5, 0, &r);
        assert_eq!(v, OracleCompareVerdict::Agree { n: 1 });
    }

    #[test]
    fn spawn_timeout_collects_stdout() {
        let mut cmd = Command::new("/bin/echo");
        cmd.arg("oracle-compare-hello");
        let out = spawn_timeout(cmd, Duration::from_secs(2)).expect("echo");
        assert!(out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("oracle-compare-hello"),
            "rule bounded_waits: timeout wrapper must return the child's stdout"
        );
    }

    #[test]
    fn spawn_timeout_kills_a_hung_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "rule bounded_waits: hung child must not be waited unbounded, elapsed={:?}",
            start.elapsed()
        );
        assert!(
            out.is_some(),
            "rule bounded_waits: timeout path must return"
        );
    }

    #[test]
    fn spawn_timeout_child_does_not_inherit_our_file_fd() {
        use std::os::unix::io::AsRawFd;
        let dir = std::env::temp_dir().join(format!("oc-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held file");
        let fd = guard.as_raw_fd();
        assert!(
            child_cannot_open_fd(fd, Duration::from_secs(2)),
            "rule lock_not_inheritable: child opened our File fd {fd}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spawn_timeout_does_not_wait_for_descendant_pipe_holder() {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "sleep 30 & exit 0"]);
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "descendant-held pipes must remain deadline-bounded, elapsed={:?}",
            start.elapsed()
        );
        assert!(out.is_some(), "post-exit pipe cleanup must return output");
    }

    #[test]
    fn spawn_timeout_stdin_roundtrip() {
        let cmd = Command::new("/bin/cat");
        let out = spawn_timeout_stdin(cmd, Duration::from_secs(2), b"piped-body\n").expect("cat");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "piped-body\n");
    }

    #[test]
    fn spawn_timeout_drains_large_stdout_and_stderr() {
        let mut cmd = Command::new("/bin/sh");
        cmd.args([
            "-c",
            "/usr/bin/head -c 131072 /dev/zero & /usr/bin/head -c 131072 /dev/zero >&2 & wait",
        ]);
        let out = spawn_timeout(cmd, Duration::from_secs(2)).expect("large dual-pipe child");
        assert!(
            out.status.success(),
            "child must complete after both pipes are drained: {:?}",
            out.status
        );
        assert!(
            out.stdout.len() >= 131_072,
            "stdout must be fully captured, got {} bytes",
            out.stdout.len()
        );
        assert!(
            out.stderr.len() >= 131_072,
            "stderr must be fully captured, got {} bytes",
            out.stderr.len()
        );
    }
}
