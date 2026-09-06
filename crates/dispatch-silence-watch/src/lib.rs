#![forbid(unsafe_code)]

//! Detects DISPATCHED-THEN-SILENT: after a dispatch, this crate asks whether
//! the work produced a verdict.
//!
//! THE CONTRACT (bead: the conductor must not let a session go idle until
//! Joshua says so): a dispatched bead receives one assignee and a deadline.
//! After the deadline, the conductor asks one question — did the assignee post
//! a verdict? The answer must come from READING BACK the tracker's comment
//! list, never from a send's exit code (cp-z42vu: ntm send-flag returned
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
/// Tracker unreadability is owned by [`tracker_read_from`] / [`classify_from_read`].
/// This function classifies a successful `br` stdout payload only.
/// Empty payload remains TrackerError: even a bead with zero comments produces
/// a `Comments for ...` header, so empty means the read failed.
pub fn classify(
    comments_output: &str,
    current_assignee: &str,
    dispatch_assignee: &str,
    dispatch_epoch: i64,
    now_epoch: i64,
    deadline_secs: i64,
) -> SilenceVerdict {
    if comments_output.trim().is_empty() {
        return SilenceVerdict::TrackerError;
    }

    if current_assignee != dispatch_assignee {
        return SilenceVerdict::Reassigned;
    }

    if has_posted_verdict(comments_output) {
        return SilenceVerdict::VerdictPosted;
    }

    if now_epoch - dispatch_epoch >= deadline_secs {
        return SilenceVerdict::SilentPastDeadline;
    }

    SilenceVerdict::SilentPastDeadline
}

/// Restrictive tracker terminals clear the pending-dispatch intent so a
/// false or genuine TRACKER_ERROR cannot withhold a pane for the full 600s
/// marker expiry. VerdictPosted already clears; SilentPastDeadline does not
/// (the packet may still be in flight).
pub fn clears_pending_dispatch_intent(verdict: SilenceVerdict) -> bool {
    matches!(
        verdict,
        SilenceVerdict::VerdictPosted | SilenceVerdict::TrackerError
    )
}

/// Apply [`tracker_read_from`] then [`classify`]. Nonzero `br` exit never
/// becomes a payload for the substring-sensitive classifier.
pub fn classify_from_read(
    read: TrackerRead,
    current_assignee: &str,
    dispatch_assignee: &str,
    dispatch_epoch: i64,
    now_epoch: i64,
    deadline_secs: i64,
) -> SilenceVerdict {
    match read {
        TrackerRead::TrackerError(_) => SilenceVerdict::TrackerError,
        TrackerRead::Read(text) => classify(
            &text,
            current_assignee,
            dispatch_assignee,
            dispatch_epoch,
            now_epoch,
            deadline_secs,
        ),
    }
}

/// How br show matched the caller's bead identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadIdMatchKind {
    /// The caller supplied the canonical tracker id.
    Exact,
    /// br resolved the caller's suffix to the canonical tracker id.
    Suffix,
}

/// Canonical bead identity returned by br show <id> --json.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBeadId {
    /// The identifier supplied by the agent or dispatch record.
    pub requested_id: String,
    /// The full identifier emitted by br and safe for exact readers.
    pub canonical_id: String,
    /// Whether resolution was exact or suffix-based.
    pub match_kind: BeadIdMatchKind,
}

/// Why a br show identifier could not be canonicalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeadIdResolutionError {
    /// The requested identifier was empty.
    EmptyRequestedId,
    /// The br show response was not valid JSON.
    InvalidJson,
    /// br show returned no bead rows.
    EmptyResult { requested_id: String },
    /// br could not resolve a short identifier.
    UnresolvedShortId { requested_id: String },
    /// The suffix matched more than one bead.
    Ambiguous {
        requested_id: String,
        matches: Vec<String>,
    },
    /// A successful response did not include a usable id.
    MissingCanonicalId { requested_id: String },
    /// The response id did not equal or end with the requested suffix.
    UnexpectedCanonicalId {
        requested_id: String,
        canonical_id: String,
    },
    /// br returned an error response this reader does not understand.
    TrackerError { requested_id: String, code: String },
}

impl fmt::Display for BeadIdResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequestedId => formatter.write_str("EMPTY-BEAD-ID"),
            Self::InvalidJson => formatter.write_str("INVALID-BR-ID-RESPONSE"),
            Self::EmptyResult { requested_id } => {
                write!(formatter, "EMPTY-BEAD-ID-RESULT requested={requested_id}")
            }
            Self::UnresolvedShortId { requested_id } => {
                write!(formatter, "UNRESOLVED-SHORT-ID requested={requested_id}")
            }
            Self::Ambiguous {
                requested_id,
                matches,
            } => write!(
                formatter,
                "AMBIGUOUS-BEAD-ID requested={requested_id} matches={}",
                matches.join(",")
            ),
            Self::MissingCanonicalId { requested_id } => {
                write!(formatter, "MISSING-CANONICAL-ID requested={requested_id}")
            }
            Self::UnexpectedCanonicalId {
                requested_id,
                canonical_id,
            } => write!(
                formatter,
                "UNEXPECTED-CANONICAL-ID requested={requested_id} canonical={canonical_id}"
            ),
            Self::TrackerError { requested_id, code } => {
                write!(
                    formatter,
                    "BR-ID-ERROR requested={requested_id} code={code}"
                )
            }
        }
    }
}

/// Resolve the canonical id from one successful br show JSON response.
///
/// The caller must use CanonicalBeadId::canonical_id for exact-match readers.
/// A multi-row response is never reduced to its first row.
pub fn resolve_br_id(
    text: &str,
    requested_id: &str,
) -> Result<CanonicalBeadId, BeadIdResolutionError> {
    if requested_id.trim().is_empty() {
        return Err(BeadIdResolutionError::EmptyRequestedId);
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| BeadIdResolutionError::InvalidJson)?;
    let rows = match &value {
        serde_json::Value::Array(rows) => rows.as_slice(),
        serde_json::Value::Object(object) if object.contains_key("error") => {
            let error = object.get("error").and_then(serde_json::Value::as_object);
            let code = error
                .and_then(|entry| entry.get("code"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("UNKNOWN")
                .to_owned();
            if code == "AMBIGUOUS_ID" {
                let matches = error
                    .and_then(|entry| entry.get("context"))
                    .and_then(|context| context.get("matches"))
                    .and_then(serde_json::Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                return Err(BeadIdResolutionError::Ambiguous {
                    requested_id: requested_id.to_owned(),
                    matches,
                });
            }
            if code == "ISSUE_NOT_FOUND" && !requested_id.starts_with("omp-orchestrator-") {
                return Err(BeadIdResolutionError::UnresolvedShortId {
                    requested_id: requested_id.to_owned(),
                });
            }
            return Err(BeadIdResolutionError::TrackerError {
                requested_id: requested_id.to_owned(),
                code,
            });
        }
        serde_json::Value::Object(_) => std::slice::from_ref(&value),
        _ => return Err(BeadIdResolutionError::InvalidJson),
    };
    let row = match rows {
        [] => {
            return Err(BeadIdResolutionError::EmptyResult {
                requested_id: requested_id.to_owned(),
            })
        }
        [row] => row,
        rows => {
            let matches = rows
                .iter()
                .filter_map(|row| row.get("id").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .collect();
            return Err(BeadIdResolutionError::Ambiguous {
                requested_id: requested_id.to_owned(),
                matches,
            });
        }
    };
    let canonical_id = row
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BeadIdResolutionError::MissingCanonicalId {
            requested_id: requested_id.to_owned(),
        })?;
    let match_kind = if canonical_id == requested_id {
        BeadIdMatchKind::Exact
    } else if canonical_id.ends_with(&format!("-{requested_id}")) {
        BeadIdMatchKind::Suffix
    } else {
        return Err(BeadIdResolutionError::UnexpectedCanonicalId {
            requested_id: requested_id.to_owned(),
            canonical_id: canonical_id.to_owned(),
        });
    };
    Ok(CanonicalBeadId {
        requested_id: requested_id.to_owned(),
        canonical_id: canonical_id.to_owned(),
        match_kind,
    })
}

/// Extract the current assignee from a br show response after canonicalizing
/// the caller's identifier. Passing a suffix to this exact reader returns the
/// typed UNRESOLVED-SHORT-ID diagnostic instead of silently reporting absence.
pub fn parse_bead_assignee(
    text: &str,
    expected_id: &str,
) -> Result<Option<String>, BeadIdResolutionError> {
    let resolution = resolve_br_id(text, expected_id)?;
    if resolution.match_kind != BeadIdMatchKind::Exact {
        return Err(BeadIdResolutionError::UnresolvedShortId {
            requested_id: expected_id.to_owned(),
        });
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| BeadIdResolutionError::InvalidJson)?;
    let row = match &value {
        serde_json::Value::Array(rows) => {
            rows.first()
                .ok_or_else(|| BeadIdResolutionError::EmptyResult {
                    requested_id: expected_id.to_owned(),
                })?
        }
        serde_json::Value::Object(_) => &value,
        _ => return Err(BeadIdResolutionError::InvalidJson),
    };
    Ok(row
        .get("assignee")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "none")
        .map(str::to_owned))
}

/// How a tracker read-back turned out, typed so a bounded timeout can never
/// wear a successful read's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerRead {
    /// `br` exited 0; the text is the authoritative read-back.
    Read(String),
    /// The read-back could not complete: nonzero exit, deadline kill, or a
    /// spawn failure. Every one of these is a TRACKER_ERROR - a restrictive
    /// terminal, never an empty read-back a caller might parse as a verdict.
    TrackerError(&'static str),
}

/// Map a bounded tracker spawn outcome onto the typed read. A timed-out or
/// unspawned `br` MUST NOT surface as an empty successful read: an empty
/// comment list parses as "no verdict posted", which would re-classify a
/// wedged tracker as a silent bead - the false-green class this lane exists
/// to close.
pub fn tracker_read_from(outcome: subprocess_contract::BoundedOutcome) -> TrackerRead {
    match outcome {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            TrackerRead::Read(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        subprocess_contract::BoundedOutcome::Completed(output) => {
            TrackerRead::TrackerError(if output.status.code().is_none() {
                "br exited by signal"
            } else {
                "br exited nonzero"
            })
        }
        subprocess_contract::BoundedOutcome::TimedOut => {
            TrackerRead::TrackerError("br read exceeded deadline; group killed")
        }
        subprocess_contract::BoundedOutcome::Unspawned(_) => {
            TrackerRead::TrackerError("br could not be spawned")
        }
    }
}

#[cfg(test)]
mod bounded_read_tests {
    use super::*;

    #[cfg(unix)]
    fn completed(status: i32, stdout: &str) -> subprocess_contract::BoundedOutcome {
        use std::os::unix::process::ExitStatusExt;
        subprocess_contract::BoundedOutcome::Completed(std::process::Output {
            // Unix raw encoding: an exited process is `code << 8`; a bare
            // small integer would encode a signal death instead.
            status: std::process::ExitStatus::from_raw(status << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    #[test]
    fn successful_read_carries_text() {
        let read = tracker_read_from(completed(0, "bead=x verdict=VERDICT_POSTED"));
        assert_eq!(
            read,
            TrackerRead::Read("bead=x verdict=VERDICT_POSTED".to_owned())
        );
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_is_tracker_error_not_empty_read() {
        assert_eq!(
            tracker_read_from(completed(1, "")),
            TrackerRead::TrackerError("br exited nonzero")
        );
    }

    #[test]
    fn timed_out_is_typed_error_never_a_read() {
        assert_eq!(
            tracker_read_from(subprocess_contract::BoundedOutcome::TimedOut),
            TrackerRead::TrackerError("br read exceeded deadline; group killed")
        );
    }

    #[test]
    fn unspawned_is_typed_error_never_a_read() {
        assert_eq!(
            tracker_read_from(subprocess_contract::BoundedOutcome::Unspawned(
                std::io::Error::new(std::io::ErrorKind::NotFound, "no br")
            )),
            TrackerRead::TrackerError("br could not be spawned")
        );
    }

    #[cfg(unix)]
    #[test]
    fn successful_exit_with_error_colon_in_stdout_is_not_tracker_error() {
        let payload = "Comments for dp21:\n[WildStone] at 2026-09-06 13:00 UTC\nError: cannot claim blocked issue\n";
        let read = tracker_read_from(completed(0, payload));
        let v = classify_from_read(read, "a", "a", 1_000, 10_000, 3_600);
        assert_ne!(v, SilenceVerdict::TrackerError);
        assert_eq!(v, SilenceVerdict::VerdictPosted);
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_classifies_as_tracker_error_even_if_stdout_quotes_error() {
        let read = tracker_read_from(completed(1, "Error: Issue not found\n"));
        let v = classify_from_read(read, "a", "a", 1_000, 10_000, 3_600);
        assert_eq!(v, SilenceVerdict::TrackerError);
        assert_eq!(v.detector(), "TRACKER_ERROR");
        assert!(clears_pending_dispatch_intent(v));
    }
}
