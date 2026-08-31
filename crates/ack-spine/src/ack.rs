//! Ack detector (slice c): confirms a bead comment ACTUALLY LANDED by reading it back.
//!
//! THE TRAP (measured 2026-08-31, live): `br comment` (SINGULAR) prefix-matches to
//! `br comments`, prints 'error: unexpected argument' on stderr, and the comment
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
//! * A bead comment via `br comments add` is the only artifact that survives
//!   pane death, context compaction, and a daemon restart.
//!
//! THE CONTRACT: an ack is confirmed by READ-BACK — run `br comments list <id>`
//! and check for a unique marker from the comment. Exit 0 from the posting
//! command is necessary but not sufficient.

use std::fmt;
use std::process::Command;

/// The typed outcome of an ack read-back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckVerdict {
    /// The marker appears in the bead's comment list.
    Confirmed { bead_id: String, marker: String },
    /// The marker does not appear — the comment did not land.
    Missing { bead_id: String, marker: String },
    /// The tracker is unreadable — an ERROR, never "no ack".
    Unverifiable { bead_id: String, detail: String },
}

impl fmt::Display for AckVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confirmed { bead_id, marker } => {
                write!(formatter, "ACK_CONFIRMED: '{marker}' found in {bead_id} comments")
            }
            Self::Missing { bead_id, marker } => {
                write!(formatter, "ACK_MISSING: '{marker}' not found in {bead_id} — the comment did not land")
            }
            Self::Unverifiable { bead_id, detail } => {
                write!(formatter, "ACK_UNVERIFIABLE: {bead_id} — {detail} (an ERROR, not 'no ack')")
            }
        }
    }
}

/// Run `br comments list <bead_id>` and check for the marker.
///
/// This is the REAL read-back: it invokes br, captures the output, and
/// classifies the result. The exit code of the POSTING command is not
/// consulted — only the read-back matters.
pub fn detect_ack(bead_id: &str, marker: &str) -> AckVerdict {
    let output = Command::new("br")
        .args(["comments", "list", bead_id])
        .output();

    let Ok(raw) = output else {
        return AckVerdict::Unverifiable {
            bead_id: bead_id.to_owned(),
            detail: format!("br spawn failed: {}", std::io::Error::last_os_error()),
        };
    };

    if !raw.status.success() {
        return AckVerdict::Unverifiable {
            bead_id: bead_id.to_owned(),
            detail: format!(
                "br comments list exited {:?}: {}",
                raw.status.code(),
                String::from_utf8_lossy(&raw.stderr)
            ),
        };
    }

    let text = String::from_utf8_lossy(&raw.stdout);
    if text.contains(marker) {
        AckVerdict::Confirmed {
            bead_id: bead_id.to_owned(),
            marker: marker.to_owned(),
        }
    } else {
        AckVerdict::Missing {
            bead_id: bead_id.to_owned(),
            marker: marker.to_owned(),
        }
    }
}

/// Simulate the SINGULAR-verb trap: run `br comment <id> -m <text>` and return
/// what happened. The comment does NOT land, but the exit code and stderr
/// vary by argument shape — which is exactly why the read-back detector exists.
pub fn simulate_singular_trap(bead_id: &str, text: &str) -> SingularTrapResult {
    let output = Command::new("br")
        .args(["comment", bead_id, "-m", text])
        .output();

    match output {
        Ok(raw) => SingularTrapResult {
            exit_code: raw.status.code(),
            stderr: String::from_utf8_lossy(&raw.stderr).into_owned(),
            comment_landed: false, // br comment (SINGULAR) NEVER posts
        },
        Err(error) => SingularTrapResult {
            exit_code: None,
            stderr: format!("spawn failed: {error}"),
            comment_landed: false,
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
