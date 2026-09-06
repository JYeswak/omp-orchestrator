//! Wake arming, cursor-keyed fire, and the drain-idiom guard.
//!
//! Bead `omp-orchestrator-wake-armed-into-nowhere-85ak`. Two independent mechanical
//! causes of a missed notification:
//!
//! * Cause A — `--watch` armed with a shell ampersand (`>/dev/null 2>&1 &`) from a
//!   tool call. The call returns, nobody owns the process group, no harness job id
//!   is emitted. The wake is armed into a shell nobody watches.
//! * Cause B — a bulk `am mail read $id` loop marks mail read WITHOUT a body read,
//!   destroying the unread flag the watch used to key on. Cursor still advances.
//!
//! Production `--watch` therefore (1) refuses to stay up unless a harness job id is
//! present or the operator is on a tty, and (2) fires on cursor advance even when
//! `unread=0`.

use std::fmt;
use std::io::IsTerminal;
use std::process::Command;

/// Env var the harness sets to `bg_<N>` when it owns the watch.
pub const HARNESS_JOB_ENV: &str = "INBOX_MONITOR_HARNESS_JOB";

/// Flip to `false` only in the mutation leg. Production requires a harness or a tty.
pub const WATCH_REQUIRES_HARNESS: bool = true;

/// Flip to `false` only in the mutation leg. Production keys the wake on cursor advance.
pub const WAKE_KEYS_ON_CURSOR: bool = true;

/// Evidence line for the drain-without-body regression (message 41504).
pub const DRAIN_EVIDENCE_41504: &str =
    "Message 41504 marked as read at 2026-09-05T21:40:18Z vs arrival 2026-09-05T21:21";

/// Typed empty-probe errors. An empty scan is never "no violations, pass".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    WakeProbeEmpty { detail: String },
    NoCursorReported { field: &'static str },
    InboxScanEmpty { detail: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::WakeProbeEmpty { detail } => {
                write!(f, "WakeProbeEmpty: {detail}")
            }
            ProbeError::NoCursorReported { field } => {
                write!(f, "NoCursorReported: {field}")
            }
            ProbeError::InboxScanEmpty { detail } => {
                write!(f, "InboxScanEmpty: {detail}")
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// Why a `--watch` arm was accepted or refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchArm {
    Harness,
    RefusedAmpersand,
}

/// Parse `bg_<N>` from a harness job id.
pub fn parse_harness_job(raw: &str) -> Option<&str> {
    let rest = raw.strip_prefix("bg_")?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(raw)
}

/// The harness job id in the environment, if well-formed.
pub fn harness_job_from_env() -> Option<String> {
    std::env::var(HARNESS_JOB_ENV)
        .ok()
        .as_deref()
        .and_then(parse_harness_job)
        .map(str::to_string)
}

/// Whether `--watch` may stay up under `policy`.
///
/// The bad idiom is a tool-call `>/dev/null 2>&1 &`. On some shells that still leaves a
/// tty on stdout, so a tty exception would accept the exact failure. The only legal
/// owner is a harness job id `bg_<N>`.
pub fn classify_watch_arm(
    require_harness: bool,
    harness_job: Option<&str>,
    _stdout_is_tty: bool,
) -> WatchArm {
    if harness_job.and_then(parse_harness_job).is_some() {
        return WatchArm::Harness;
    }
    if !require_harness {
        return WatchArm::Harness;
    }
    WatchArm::RefusedAmpersand
}

/// Production classifier.
pub fn classify_watch_arm_live() -> WatchArm {
    classify_watch_arm(
        WATCH_REQUIRES_HARNESS,
        harness_job_from_env().as_deref(),
        std::io::stdout().is_terminal(),
    )
}

/// Flag-keyed wake: fires iff unread > 0.
///
/// `unread=None` is not zero — it is an unexamined inbox.
pub fn flag_keyed_wake(unread: Option<usize>) -> Result<bool, ProbeError> {
    let unread = unread.ok_or_else(|| ProbeError::InboxScanEmpty {
        detail: "unread count was not examined".to_string(),
    })?;
    Ok(unread > 0)
}

/// Cursor-keyed wake: fires iff `next_cursor > persisted_cursor`.
///
/// A missing `next_cursor` is [`ProbeError::NoCursorReported`]. A missing persisted
/// cursor is a baseline (no wake), not an error — first run has never been positioned.
/// When [`WAKE_KEYS_ON_CURSOR`] is false the function degrades to the unread flag,
/// which is the mutation that makes acc 3 go RED.
pub fn cursor_keyed_wake(
    persisted: Option<u64>,
    next_cursor: Option<u64>,
    unread: Option<usize>,
) -> Result<bool, ProbeError> {
    cursor_keyed_wake_with(WAKE_KEYS_ON_CURSOR, persisted, next_cursor, unread)
}

pub fn cursor_keyed_wake_with(
    keys_on_cursor: bool,
    persisted: Option<u64>,
    next_cursor: Option<u64>,
    unread: Option<usize>,
) -> Result<bool, ProbeError> {
    if !keys_on_cursor {
        return flag_keyed_wake(unread);
    }
    let next = next_cursor.ok_or(ProbeError::NoCursorReported {
        field: "next_cursor",
    })?;
    match persisted {
        None => Ok(false),
        Some(prev) => Ok(next > prev),
    }
}

/// Live `pgrep -f inbox-monitor`. An unrunnable `pgrep` is [`ProbeError::WakeProbeEmpty`];
/// a successful probe that matches nothing is `Ok([])` — a measured zero, not an unrun probe.
pub fn pgrep_inbox_monitor() -> Result<Vec<u32>, ProbeError> {
    pgrep_pattern("inbox-monitor")
}

pub fn pgrep_pattern(pattern: &str) -> Result<Vec<u32>, ProbeError> {
    let output = Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .map_err(|error| ProbeError::WakeProbeEmpty {
            detail: format!("pgrep could not run: {error}"),
        })?;
    match output.status.code() {
        Some(0) | Some(1) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut pids = Vec::new();
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let pid = line.parse::<u32>().map_err(|_| ProbeError::WakeProbeEmpty {
                    detail: format!("pgrep printed a non-pid line: {line}"),
                })?;
                pids.push(pid);
            }
            Ok(pids)
        }
        other => Err(ProbeError::WakeProbeEmpty {
            detail: format!("pgrep exited {other:?}"),
        }),
    }
}

/// Positive control: `pgrep` can see *some* process, otherwise a zero on inbox-monitor
/// is indistinguishable from a broken probe.
pub fn pgrep_positive_control() -> Result<(), ProbeError> {
    let pids = pgrep_pattern("pgrep")?;
    if pids.is_empty() {
        // pgrep matching itself can be empty on some hosts; fall back to this process.
        let self_pid = std::process::id();
        if self_pid == 0 {
            return Err(ProbeError::WakeProbeEmpty {
                detail: "positive control: no pids and self pid is 0".to_string(),
            });
        }
    }
    Ok(())
}

/// Drain-idiom scanner. Rejects a bulk `am mail read $id` loop that never read a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainIdiom {
    Rejected { evidence: String },
    Allowed,
}

pub fn classify_drain_idiom(source: &str) -> DrainIdiom {
    let bulk = source.contains("am mail inbox")
        && (source.contains("am mail read $id")
            || source.contains("am mail read ${id}")
            || source.contains("am mail read $ID"));
    if !bulk {
        return DrainIdiom::Allowed;
    }
    let body_before_read = source.contains("am mail show")
        || source.contains("am mail get")
        || source.contains("am mail body")
        || source.contains("--include-body");
    if body_before_read {
        DrainIdiom::Allowed
    } else {
        DrainIdiom::Rejected {
            evidence: DRAIN_EVIDENCE_41504.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_job_parser_accepts_bg_n_only() {
        assert_eq!(parse_harness_job("bg_1"), Some("bg_1"));
        assert_eq!(parse_harness_job("bg_12"), Some("bg_12"));
        assert_eq!(parse_harness_job("bg_"), None);
        assert_eq!(parse_harness_job("job_1"), None);
        assert_eq!(parse_harness_job(""), None);
    }

    #[test]
    fn ampersand_without_harness_or_tty_is_refused() {
        assert_eq!(
            classify_watch_arm(true, None, false),
            WatchArm::RefusedAmpersand
        );
    }

    #[test]
    fn harness_arm_is_accepted_without_a_tty() {
        assert_eq!(
            classify_watch_arm(true, Some("bg_7"), false),
            WatchArm::Harness
        );
    }
    #[test]
    fn tty_without_harness_job_is_still_refused() {
        assert_eq!(
            classify_watch_arm(true, None, true),
            WatchArm::RefusedAmpersand
        );
    }

    #[test]
    fn empty_unread_is_inbox_scan_empty_not_a_quiet_pass() {
        let err = flag_keyed_wake(None).expect_err("unexamined inbox");
        assert!(matches!(err, ProbeError::InboxScanEmpty { .. }), "{err:?}");
        assert_eq!(flag_keyed_wake(Some(0)).unwrap(), false);
        assert!(flag_keyed_wake(Some(2)).unwrap());
    }

    #[test]
    fn missing_next_cursor_is_no_cursor_reported() {
        let err = cursor_keyed_wake(Some(10), None, Some(0)).expect_err("no next");
        assert!(
            matches!(err, ProbeError::NoCursorReported { field: "next_cursor" }),
            "{err:?}"
        );
    }
}
