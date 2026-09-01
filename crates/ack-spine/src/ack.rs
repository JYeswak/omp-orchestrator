//! Ack detector (slice c): confirms a bead comment ACTUALLY LANDED by reading it back.
//!
//! THE TRAP (measured 2026-08-31, live): br comment (SINGULAR) prefix-matches to
//! br comments, prints 'error: unexpected argument' on stderr, and the comment
//! does not land. Exit code varies (2 with -m, 0 without) and is NOT sufficient
//! to establish an ack. An agent that trusts the exit code believes the comment
//! posted when it did not.
//!
//! THE ALTERNATIVES, RULED OUT BY MEASUREMENT:
//! * ntm robot-send REFUSES codex panes entirely (cp-nq2s9) — structurally
//!   impossible for half the fleet.
//! * Agent Mail returned 'database is locked' on both writes with RSS
//!   130MB->687MB over 98 minutes and a restart that fixed it — state
//!   accumulation, not a fix (cp-4fsjw).
//! * A bead comment via br comments add is the only artifact that survives
//!   pane death, context compaction, and a daemon restart.
//!
//! THE CONTRACT: an ack is confirmed by READ-BACK — run br comments list <id>
//! and check for exactly one unique marker from the comment. Exit 0 from the
//! posting command is necessary but not sufficient.

use asupersync::Cx;
use asupersync::process::Command;
use std::fmt;
use subprocess_contract::run_output;

/// The typed outcome of an ack read-back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckVerdict {
    /// The marker appears exactly once in the bead's comment list.
    Confirmed { bead_id: String, marker: String },
    /// The marker does not appear — the comment was not confirmed.
    Missing { bead_id: String, marker: String },
    /// The tracker is unreadable or the marker is not singular — an ERROR,
    /// never "no ack".
    Unverifiable { bead_id: String, detail: String },
}

impl fmt::Display for AckVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confirmed { bead_id, marker } => {
                write!(
                    formatter,
                    "ACK_CONFIRMED: '{marker}' found exactly once in {bead_id} comments"
                )
            }
            Self::Missing { bead_id, marker } => {
                write!(
                    formatter,
                    "ACK_MISSING: '{marker}' not found in {bead_id} — the comment was not confirmed"
                )
            }
            Self::Unverifiable { bead_id, detail } => {
                write!(
                    formatter,
                    "ACK_UNVERIFIABLE: {bead_id} — {detail} (an ERROR, not 'no ack')"
                )
            }
        }
    }
}

/// Run br comments list <bead_id> and check for the marker.
///
/// This is the I/O-bound wrapper that the hermetic tests bypass by calling the
/// pure classify_ack_readback directly.
pub async fn detect_ack(cx: &Cx, bead_id: &str, marker: &str) -> AckVerdict {
    let mut command = Command::new("br");
    command.args(["comments", "list", bead_id]);

    match run_output(cx, command).await {
        Ok(raw) => {
            let exit_code = raw.status.code();
            let stdout = String::from_utf8_lossy(&raw.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&raw.stderr).into_owned();
            classify_ack_readback(bead_id, marker, exit_code, &stdout, &stderr)
        }
        Err(error) => classify_ack_readback(bead_id, marker, None, "", &error.to_string()),
    }
}

/// Simulate the SINGULAR-verb trap: run br comment <id> -m <text> and return
/// what happened. The comment does NOT land, but the exit code and stderr
/// vary by argument shape — which is exactly why the read-back detector exists.
pub async fn simulate_singular_trap(
    cx: &Cx,
    bead_id: &str,
    text: &str,
) -> SingularTrapResult {
    let mut command = Command::new("br");
    command.args(["comment", bead_id, "-m", text]);

    match run_output(cx, command).await {
        Ok(raw) => SingularTrapResult {
            exit_code: raw.status.code(),
            stderr: String::from_utf8_lossy(&raw.stderr).into_owned(),
            comment_landed: false,
        },
        Err(error) => SingularTrapResult {
            exit_code: None,
            stderr: error.to_string(),
            comment_landed: false,
        },
    }
}

/// Count non-overlapping occurrences of a non-empty marker.
fn marker_occurrences(read_back: &str, marker: &str) -> usize {
    read_back.match_indices(marker).count()
}

/// Pure classifier for an ack read-back, given pre-captured raw inputs.
///
/// BlueLantern's hermetic tests call this directly with synthetic
/// (exit_code, stdout, stderr) triples — no br subprocess required. The
/// I/O-bound detect_ack delegates here after capturing the real output.
///
/// A zero exit code only says that br comments list completed. Confirmation
/// additionally requires one, and only one, occurrence of a non-empty marker
/// in stdout. Missing or duplicate markers are never Confirmed.
pub fn classify_ack_readback(
    bead_id: &str,
    marker: &str,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> AckVerdict {
    match exit_code {
        Some(0) if marker.is_empty() => AckVerdict::Unverifiable {
            bead_id: bead_id.to_owned(),
            detail: "ack marker is empty; a singular marker is required".to_owned(),
        },
        Some(0) => match marker_occurrences(stdout, marker) {
            0 => AckVerdict::Missing {
                bead_id: bead_id.to_owned(),
                marker: marker.to_owned(),
            },
            1 => AckVerdict::Confirmed {
                bead_id: bead_id.to_owned(),
                marker: marker.to_owned(),
            },
            count => AckVerdict::Unverifiable {
                bead_id: bead_id.to_owned(),
                detail: format!(
                    "ack marker appears {count} times in br comments list output; exactly one is required"
                ),
            },
        },
        Some(code) => AckVerdict::Unverifiable {
            bead_id: bead_id.to_owned(),
            detail: format!("br comments list exited {code}: {stderr}"),
        },
        None => AckVerdict::Unverifiable {
            bead_id: bead_id.to_owned(),
            detail: format!("br spawn failed: {stderr}"),
        },
    }
}

/// What the singular-verb trap produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingularTrapResult {
    pub exit_code: Option<i32>,
    pub stderr: String,
    /// Always false: br comment (SINGULAR) never posts a comment.
    pub comment_landed: bool,
}
