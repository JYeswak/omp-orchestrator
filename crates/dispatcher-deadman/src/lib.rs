#![forbid(unsafe_code)]

//! Consecutive no-delivery watchdog. Port of `bin/dispatcher-deadman.sh`.
//!
//! Two consecutive scheduled ticks with eligible work and zero delivered packets
//! are RED. A first stall (merely slow) is PASS. A healthy tick is PASS and
//! resets the counter. This is an observer, not an admission override.

use std::fs;
use std::path::Path;
use std::process::Command;
use subprocess_contract::{bounded_output, BoundedOutcome};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatcherDeadmanRule {
    ConsecutiveThreshold,
    ReadyWithoutDeliveryCounts,
    DeliveryResets,
}

impl DispatcherDeadmanRule {
    pub const ALL: &'static [DispatcherDeadmanRule] = &[
        DispatcherDeadmanRule::ConsecutiveThreshold,
        DispatcherDeadmanRule::ReadyWithoutDeliveryCounts,
        DispatcherDeadmanRule::DeliveryResets,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            DispatcherDeadmanRule::ConsecutiveThreshold => "consecutive_threshold",
            DispatcherDeadmanRule::ReadyWithoutDeliveryCounts => "ready_without_delivery_counts",
            DispatcherDeadmanRule::DeliveryResets => "delivery_resets",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct DispatcherDeadmanRules {
    pub consecutive_threshold: bool,
    pub ready_without_delivery_counts: bool,
    pub delivery_resets: bool,
}

impl Default for DispatcherDeadmanRules {
    fn default() -> Self {
        Self {
            consecutive_threshold: true,
            ready_without_delivery_counts: true,
            delivery_resets: true,
        }
    }
}

impl DispatcherDeadmanRules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = DispatcherDeadmanRule::parse(name) else {
            return false;
        };
        match rule {
            DispatcherDeadmanRule::ConsecutiveThreshold => self.consecutive_threshold = false,
            DispatcherDeadmanRule::ReadyWithoutDeliveryCounts => {
                self.ready_without_delivery_counts = false
            }
            DispatcherDeadmanRule::DeliveryResets => self.delivery_resets = false,
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub ready_count: u64,
    pub delivered_count: u64,
    pub threshold: u64,
    pub tick_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatcherDeadmanVerdict {
    pub verdict: &'static str,
    pub consecutive: u64,
    pub exit: i32,
}

pub fn nonnegative(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

pub fn read_consecutive(text: &str) -> u64 {
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("consecutive_no_delivery=") {
            return nonnegative(v.trim()).unwrap_or(0);
        }
    }
    0
}

pub fn apply_record(
    prev: u64,
    rec: &Record,
    rules: &DispatcherDeadmanRules,
) -> (u64, DispatcherDeadmanVerdict) {
    let stall = rec.ready_count > 0 && rec.delivered_count == 0;
    let consecutive = if stall && rules.ready_without_delivery_counts {
        prev + 1
    } else if !stall && rules.delivery_resets {
        0
    } else {
        prev
    };
    let threshold = if rec.threshold == 0 { 1 } else { rec.threshold };
    let fires = stall
        && if rules.consecutive_threshold {
            consecutive >= threshold
        } else {
            true
        };
    if fires {
        (
            consecutive,
            DispatcherDeadmanVerdict {
                verdict: "RED",
                consecutive,
                exit: 1,
            },
        )
    } else {
        (
            consecutive,
            DispatcherDeadmanVerdict {
                verdict: "PASS",
                consecutive,
                exit: 0,
            },
        )
    }
}

pub fn emit_json(verdict: &str, rec: &Record, consecutive: u64) -> String {
    format!(
        r#"{{"schema":"zs.dispatch-deadman.v1","verdict":"{verdict}","ready_count":{},"delivered_count":{},"consecutive_no_delivery":{consecutive},"threshold":{},"tick_id":"{}","reason":"{}"}}"#,
        rec.ready_count, rec.delivered_count, rec.threshold, rec.tick_id, rec.reason
    )
}

pub fn state_body(consecutive: u64, rec: &Record) -> String {
    format!(
        "schema=zs.dispatch-deadman.v1\nconsecutive_no_delivery={consecutive}\nlast_tick_id={}\nlast_ready_count={}\nlast_delivered_count={}\nlast_reason={}\n",
        rec.tick_id, rec.ready_count, rec.delivered_count, rec.reason
    )
}

pub fn write_state_atomic(path: &Path, body: &str) -> Result<(), i32> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let _ = fs::create_dir_all(dir);
    let tmp = dir.join(format!(".dispatcher-deadman.{}.tmp", std::process::id()));
    fs::write(&tmp, body).map_err(|_| 77)?;
    fs::rename(&tmp, path).map_err(|_| {
        let _ = fs::remove_file(&tmp);
        77
    })
}

pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> BoundedOutcome {
    bounded_output(&mut cmd, timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_stall_is_not_red() {
        let rec = Record {
            ready_count: 2,
            delivered_count: 0,
            threshold: 2,
            tick_id: "stall-1".into(),
            reason: "pane_busy".into(),
        };
        let (c, v) = apply_record(0, &rec, &DispatcherDeadmanRules::default());
        assert_eq!(c, 1);
        assert_eq!(v.verdict, "PASS", "merely slow: first stall must not fire");
    }

    #[test]
    fn second_stall_is_red() {
        let rec = Record {
            ready_count: 2,
            delivered_count: 0,
            threshold: 2,
            tick_id: "stall-2".into(),
            reason: "pane_busy".into(),
        };
        let (c, v) = apply_record(1, &rec, &DispatcherDeadmanRules::default());
        assert_eq!(c, 2);
        assert_eq!(
            v.verdict, "RED",
            "rule consecutive_threshold: genuinely stopped dispatcher fires"
        );
    }

    #[test]
    fn delivery_resets() {
        let rec = Record {
            ready_count: 2,
            delivered_count: 1,
            threshold: 2,
            tick_id: "recovered".into(),
            reason: "delivered".into(),
        };
        let (c, v) = apply_record(2, &rec, &DispatcherDeadmanRules::default());
        assert_eq!(c, 0);
        assert_eq!(v.verdict, "PASS");
    }

    #[test]
    fn zero_work_is_pass() {
        let rec = Record {
            ready_count: 0,
            delivered_count: 0,
            threshold: 2,
            tick_id: "healthy".into(),
            reason: "no_work".into(),
        };
        let (_, v) = apply_record(0, &rec, &DispatcherDeadmanRules::default());
        assert_eq!(v.verdict, "PASS");
    }

    #[test]
    fn spawn_timeout_kills_a_hung_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "rule bounded_waits"
        );
        assert!(matches!(out, BoundedOutcome::TimedOut));
    }

    #[test]
    fn spawn_timeout_child_does_not_inherit_our_file_fd() {
        use std::os::unix::io::AsRawFd;
        let dir = std::env::temp_dir().join(format!("dd-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held");
        let fd = guard.as_raw_fd();
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "exec 3<>/dev/fd/$CHECK_FD"])
            .env("CHECK_FD", fd.to_string());
        match spawn_timeout(cmd, Duration::from_secs(2)) {
            BoundedOutcome::Completed(output) => {
                assert!(!output.status.success(), "rule lock_not_inheritable");
            }
            BoundedOutcome::TimedOut => panic!("fd probe must complete before its deadline"),
            BoundedOutcome::Unspawned(error) => panic!("fd probe must spawn: {error}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "ADMISSION", "FRESH");
        assert!(!main.contains(&token));
    }
}
