#![forbid(unsafe_code)]

//! Detects DISPATCHED-THEN-SILENT: after a dispatch, this crate asks whether
//! the work produced a verdict.
//!
//! THE CONTRACT (bead: the conductor must not let a session go idle until
//! Joshua says so): a dispatched bead receives one assignee and a deadline.
//! After the deadline, the conductor asks one question — did the assignee post
//! a verdict? The answer must come from READING BACK the tracker's comment
//! list, never from a send's exit code (cp-z42vu: `ntm --robot-send` returned
//! `successful:["4"]` while the packet never reached the pane).
//!
//! THE `br comment` SINGULAR TRAP: `br comment <id> <text>` prefix-matches to
//! `br comments`, prints a usage error to stderr, and EXITS 0. An agent that
//! checks only the exit code believes the comment landed. The fix: parse the
//! OUTPUT for actual `[Author] at date` comment blocks — the attribution line
//! is the thing that cannot exist unless a real comment was stored.
//!
//! NO-CLAIM: this crate classifies one bead's post-dispatch state. It does not
//! dispatch, grade, close, or re-route. The caller (the conductor) decides
//! what to do with the verdict.

use std::fmt;

/// The typed verdict for one dispatched bead's follow-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SilenceVerdict {
    /// A real comment block was read back from the tracker.
    VerdictPosted,
    /// No comment, and the dispatch deadline has passed.
    SilentPastDeadline,
    /// The bead's assignee changed since dispatch — the original dispatch
    /// is moot regardless of whether comments exist.
    Reassigned,
    /// The tracker output was unreadable. An ERROR — never VERDICT_POSTED,
    /// never SILENT_PAST_DEADLINE. The caller must re-read before acting.
    TrackerError,
}

impl SilenceVerdict {
    /// The detector name a harness asserts — not a bare exit code.
    pub fn detector(&self) -> &'static str {
        match self {
            SilenceVerdict::VerdictPosted => "VERDICT_POSTED",
            SilenceVerdict::SilentPastDeadline => "SILENT_PAST_DEADLINE",
            SilenceVerdict::Reassigned => "REASSIGNED",
            SilenceVerdict::TrackerError => "TRACKER_ERROR",
        }
    }
}

impl fmt::Display for SilenceVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.detector())
    }
}

/// Determine whether the raw stdout of `br comments list <bead_id>` contains
/// at least one real comment attribution block.
///
/// The attribution line format is `[Author] at YYYY-MM-DD HH:MM UTC` — this is
/// what `br comments list` emits for every stored comment, and what CANNOT
/// exist unless a real comment was stored in the tracker.
///
/// The bare header (`Comments for cp-xxx:`) does NOT count: it is emitted for
/// every bead whether or not comments exist.
///
/// A usage error on stderr with exit 0 (the `br comment` singular trap) never
/// reaches this function: the caller must capture stdout, not the exit code.
pub fn has_posted_verdict(comments_output: &str) -> bool {
    for line in comments_output.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('[') {
            continue;
        }
        if let Some(close) = trimmed.find(']') {
            let author = &trimmed[1..close];
            if author.is_empty() || author.contains(' ') {
                continue;
            }
            let after = &trimmed[close + 1..];
            let after_trim = after.trim_start();
            if after_trim.starts_with("at ")
                && after_trim[3..]
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit())
            {
                return true;
            }
        }
    }
    false
}

/// Classify one dispatched bead's post-dispatch state.
///
/// LEGS (bead omp-orchestrator-dispatch-silence-watch):
/// * A bead whose comments output contains a real `[Author] at date` block is
///   VERDICT_POSTED — verified by read-back, never by exit code.
/// * A bead whose assignee changed since dispatch is REASSIGNED, even if
///   comments exist — the original dispatch is moot.
/// * A bead with no comment past its deadline is SILENT_PAST_DEADLINE.
/// * An unreadable tracker (empty output, error markers) is TrackerError —
///   never VERDICT_POSTED, never SILENT_PAST_DEADLINE.
///
/// The precedence order matters: TrackerError > Reassigned > VerdictPosted >
/// SilentPastDeadline. A tracker error contaminates every other reading.
pub fn classify(
    comments_output: &str,
    current_assignee: &str,
    dispatch_assignee: &str,
    dispatch_epoch: i64,
    now_epoch: i64,
    deadline_secs: i64,
) -> SilenceVerdict {
    // Unreadable tracker: an ERROR, never VERDICT_POSTED. An empty output is
    // the strongest signal — even a bead with zero comments produces the
    // "Comments for cp-xxx:" header.
    if comments_output.trim().is_empty() {
        return SilenceVerdict::TrackerError;
    }
    if comments_output.contains("Error:") || comments_output.contains("error:") {
        return SilenceVerdict::TrackerError;
    }

    // Assignee changed: the original dispatch is moot. This outranks
    // VerdictPosted because a REASSIGNED bead's comments belong to the
    // PREVIOUS assignee's work, not to the new assignee's.
    if current_assignee != dispatch_assignee {
        return SilenceVerdict::Reassigned;
    }

    // A real comment was read back from the tracker.
    if has_posted_verdict(comments_output) {
        return SilenceVerdict::VerdictPosted;
    }

    // No comment. If the deadline has passed, the bead is silent.
    if now_epoch - dispatch_epoch >= deadline_secs {
        return SilenceVerdict::SilentPastDeadline;
    }

    // Within deadline, no comment yet: the conductor should not be asking
    // yet, but the honest answer is still "no verdict posted." We report
    // SILENT_PAST_DEADLINE only past the deadline; before it, the caller
    // should not have asked — but if it did, the answer is the same: no
    // verdict. This is NOT TrackerError (the tracker is readable); it is
    // simply too early to escalate.
    SilenceVerdict::SilentPastDeadline
}

/// Extract the current assignee from the raw stdout of
/// `br show <bead_id> --json`. Returns None if the JSON cannot be parsed,
/// the bead id does not match, or the assignee field is absent/empty/"none".
pub fn parse_bead_assignee(text: &str, expected_id: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let row = match &value {
        serde_json::Value::Array(rows) => rows.first()?,
        value => value,
    };
    if row.get("id").and_then(serde_json::Value::as_str) != Some(expected_id) {
        return None;
    }
    row.get("assignee")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "none")
        .map(str::to_owned)
}
