//! K9 acceptance 4: the close-reason policy, enforced rather than documented.
//!
//! # The measured defect this module exists to remove
//!
//! `followup.rs` reported every closed bead as
//! `close_verdict: "MUTATION-VERIFIED-or-equivalent"` — a **hardcoded string
//! literal**. `classify_followup` did not take the close reason as an input at
//! all, so the verdict could not have been observed even in principle. It was a
//! constant wearing the shape of a measurement.
//!
//! That is the same defect class as `RECEIVER_RECEIPT=ntm_robot_send` recording
//! that a send RETURNED rather than that a message ARRIVED, and as a
//! `build_id` that never re-derives: **a derived value that cannot vary is not
//! derived.** Here the consequence is specific — a bead closed with a prose
//! reason, which the local classifier REFUSES but direct `br` stores, would still have been reported as
//! `MUTATION-VERIFIED-or-equivalent` by the follow-up stage.
//!
//! # Why `Unread` is a first-class verdict and not an error
//!
//! A caller that has not read the close reason must be able to SAY SO. Folding
//! "I did not look" into either "verified" or "refused" is what produced the
//! fabricated constant: the code had no way to express the honest state, so it
//! asserted the flattering one. `Unread` is that missing state, and it is
//! deliberately NOT a policy failure — the bead may be perfectly well closed.
//! It is a statement about the OBSERVER.
//!
//! # No I/O
//!
//! This module classifies a string. It does not run `br`, so it cannot confuse a
//! tracker failure with a policy refusal — that distinction belongs to
//! `classify_followup`'s `tracker_readable` input, which is checked first and
//! outranks everything.
//!
//! The installed `br 0.4.1` command accepts and stores arbitrary close-reason
//! strings; it is not the validator. The eight-token set below is the policy
//! enforced by this classifier for callers that pass a reason through it.
//! Direct `br close` calls bypass this module, which is why the live closed-row
//! census is a required separate control rather than proof that `br` refused a
//! bad close.

use std::fmt;

/// A close-reason prefix the local close-policy classifier admits.
///
/// # What this does NOT mean
///
/// The prefix records WHAT KIND of evidence the closer claimed, never that the
/// evidence is good. `DONE` and `MUTATION-VERIFIED` are both sanctioned and they
/// are not equivalent: the second asserts a mutation was proven to fire, the
/// first does not. Collapsing them would make the strongest close
/// indistinguishable from the weakest, which is the same defect as folding a
/// working refusal into a crash's exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosePrefix {
    /// A mutation was planted and the guard was shown to go RED on it.
    MutationVerified,
    /// The mutation leg was not required because the gate was already known to fire.
    MutationNotRequired,
    /// The mutation was attributed and the live subject was proven independently.
    MutationAttributed,
    /// The work was completed and verified, without a mutation leg.
    Done,
    /// A grader approved the work.
    Approved,
    /// The work is deliberately not needed; the premise was false.
    PremiseFalse,
    /// The work was already completed by another landed change.
    AlreadyFixed,
    /// The work will not be done, with the reason recorded.
    WontFix,
}

impl ClosePrefix {
    /// Every sanctioned prefix, in the order the policy lists them.
    ///
    /// Ordered longest-first WHERE ONE IS A PREFIX OF ANOTHER is not needed
    /// today — none of the eight shares a prefix with another — but the matcher
    /// below iterates this slice, so adding a future prefix that shadows an
    /// existing one would need that care. Stated because the next editor will
    /// not otherwise know it was considered.
    pub const ALL: &'static [Self] = &[
        Self::MutationVerified,
        Self::MutationNotRequired,
        Self::MutationAttributed,
        Self::Done,
        Self::Approved,
        Self::PremiseFalse,
        Self::AlreadyFixed,
        Self::WontFix,
    ];

    /// The exact token a close reason must start with.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MutationVerified => "MUTATION-VERIFIED",
            Self::MutationNotRequired => "MUTATION-NOT-REQUIRED",
            Self::MutationAttributed => "MUTATION-ATTRIBUTED",
            Self::Done => "DONE",
            Self::Approved => "APPROVED",
            Self::PremiseFalse => "PREMISE-FALSE",
            Self::AlreadyFixed => "ALREADY-FIXED",
            Self::WontFix => "WONTFIX",
        }
    }
}

impl fmt::Display for ClosePrefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// What a close reason establishes about the close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReasonVerdict {
    /// The reason begins with a sanctioned prefix.
    Verified {
        /// Which prefix was found.
        prefix: ClosePrefix,
    },
    /// A reason is present and begins with none of the sanctioned prefixes. The
    /// local classifier refuses this. The tracker command itself stores arbitrary
    /// reasons, so a direct tracker close can bypass this verdict; the status must
    /// be read back rather than inferred from the command appearing to succeed.
    PolicyRefused {
        /// The first whitespace-delimited token, so the caller can name what was
        /// written instead of echoing an entire prose paragraph into a log line.
        leading: String,
    },
    /// A cargo test figure omitted the worker or local execution authority.
    /// The number cannot be compared across an offloaded and local tree without it.
    CargoWorkerMissing {
        /// The first token of the unqualified cargo claim.
        leading: String,
    },
    /// A reason is present but blank once trimmed.
    Empty,
    /// No reason was supplied to this classifier. **The observer did not look.**
    /// Not a policy failure — a statement about the caller.
    Unread,
}

impl CloseReasonVerdict {
    /// A stable label for logs and ledger rows.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Verified { .. } => "CLOSE_REASON_VERIFIED",
            Self::PolicyRefused { .. } => "CLOSE_REASON_POLICY_REFUSED",
            Self::CargoWorkerMissing { .. } => "CLOSE_REASON_WORKER_MISSING",
            Self::Empty => "CLOSE_REASON_EMPTY",
            Self::Unread => "CLOSE_REASON_UNREAD",
        }
    }

    /// True only when a sanctioned prefix was actually observed.
    ///
    /// Deliberately false for [`CloseReasonVerdict::Unread`]: an unread reason
    /// may name a perfectly good close, and treating it as verified is exactly
    /// the fabrication this module removes.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }
}

impl fmt::Display for CloseReasonVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verified { prefix } => write!(formatter, "CLOSE_REASON_VERIFIED prefix={prefix}"),
            Self::PolicyRefused { leading } => write!(
                formatter,
                "CLOSE_REASON_POLICY_REFUSED leading={leading} -- a reason must start with one of \
                 MUTATION-VERIFIED, MUTATION-NOT-REQUIRED, MUTATION-ATTRIBUTED, DONE, APPROVED, \
                 PREMISE-FALSE, ALREADY-FIXED, WONTFIX; the local guard refuses this, but a \
                 direct br close bypasses it, so read the status back"
            ),
            Self::CargoWorkerMissing { leading } => write!(
                formatter,
                "CLOSE_REASON_WORKER_MISSING leading={leading} -- cargo test figures must name worker=<name> or local"
            ),
            Self::Empty => write!(
                formatter,
                "CLOSE_REASON_EMPTY -- a close with no reason records nothing"
            ),
            Self::Unread => write!(
                formatter,
                "CLOSE_REASON_UNREAD -- the close reason was not read, so no verdict about it is \
                 available; this is a fact about the observer, not about the bead"
            ),
        }
    }
}

/// True when a close reason cites a `cargo test` figure.
///
/// THE OWNER OF THE INVARIANT EXPORTS IT. Tree provenance is demanded of a
/// NUMBER, so a reason that cites no cargo figure has no number to attribute and
/// this predicate is the precondition every consumer must gate on. It is public
/// because `pre-delete-citation-check` re-implemented the demand WITHOUT the
/// precondition and emitted this crate's error name for it; two copies of a rule
/// drift, and this one drifted into refusing rows that made no execution claim.
#[must_use]
pub fn has_cargo_test_figure(reason: &str) -> bool {
    reason.contains("cargo test")
}

/// True when a close reason names where the figure ran: `worker=<name>`,
/// `worker:<name>`, or `local`.
#[must_use]
pub fn has_worker_authority(reason: &str) -> bool {
    reason.split_whitespace().any(|token| {
        token == "local" || token.starts_with("worker=") || token.starts_with("worker:")
    })
}

/// Classify a close reason against the local six-prefix policy.
///
/// `None` means the caller did not read the reason and yields
/// [`CloseReasonVerdict::Unread`] — never a verified verdict.
#[must_use]
pub fn classify_close_reason(reason: Option<&str>) -> CloseReasonVerdict {
    let Some(raw) = reason else {
        return CloseReasonVerdict::Unread;
    };
    let trimmed = raw.trim_start();
    if trimmed.trim().is_empty() {
        return CloseReasonVerdict::Empty;
    }
    if has_cargo_test_figure(trimmed) && !has_worker_authority(trimmed) {
        let leading = trimmed.split_whitespace().next().unwrap_or_default().to_owned();
        return CloseReasonVerdict::CargoWorkerMissing { leading };
    }
    for prefix in ClosePrefix::ALL {
        let tok = prefix.as_str();
        if let Some(rest) = trimmed.strip_prefix(tok) {
            let boundary_ok = rest.chars().next().map_or(true, |c| !c.is_alphanumeric());
            if boundary_ok {
                return CloseReasonVerdict::Verified { prefix: *prefix };
            }
        }
    }
    let leading = trimmed
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned();
    CloseReasonVerdict::PolicyRefused { leading }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KNOWN-GOOD: every sanctioned prefix is accepted, including with the
    /// trailing colon and prose the real reasons carry.
    #[test]
    fn every_sanctioned_prefix_is_accepted() {
        for prefix in ClosePrefix::ALL {
            let reason = format!(
                "{prefix}: re-ran the suite, 28 passed / 0 failed",
                prefix = prefix
            );
            let verdict = classify_close_reason(Some(&reason));
            assert_eq!(
                verdict,
                CloseReasonVerdict::Verified { prefix: *prefix },
                "{reason}"
            );
            assert!(verdict.is_verified());
        }
        // Leading whitespace is tolerated; a reason is not refused for indentation.
        assert!(classify_close_reason(Some("  DONE: fixed")).is_verified());
    }

    /// KNOWN-BAD: prose is refused, and the refusal NAMES the leading token
    /// rather than echoing the paragraph.
    #[test]
    fn prose_is_refused_and_the_leading_token_is_named() {
        let verdict = classify_close_reason(Some("fixed the thing, tests pass"));
        assert_eq!(
            verdict,
            CloseReasonVerdict::PolicyRefused {
                leading: "fixed".to_owned()
            }
        );
        assert!(!verdict.is_verified());
        let text = verdict.to_string();
        assert!(
            text.contains("CLOSE_REASON_POLICY_REFUSED")
                && text.contains("leading=fixed")
                && text.contains("read the status back"),
            "the refusal must name the token AND the readback duty: {text}"
        );
    }

    /// A lowercase prefix is NOT the prefix. The local classifier is
    /// case-sensitive, and accepting `done:` here would widen the enforced set
    /// without changing the documented policy.
    #[test]
    fn a_lowercase_prefix_is_refused() {
        assert_eq!(
            classify_close_reason(Some("done: lowercase")),
            CloseReasonVerdict::PolicyRefused {
                leading: "done:".to_owned()
            }
        );
    }

    /// THE CENTRAL LEG. An unread reason is `Unread`, never verified. This is
    /// the state the old code could not express, which is why it asserted
    /// `"MUTATION-VERIFIED-or-equivalent"` instead.
    #[test]
    fn an_unread_reason_is_never_verified() {
        let verdict = classify_close_reason(None);
        assert_eq!(verdict, CloseReasonVerdict::Unread);
        assert!(
            !verdict.is_verified(),
            "an unread reason must not read as verified"
        );
        assert!(
            verdict.to_string().contains("fact about the observer"),
            "the message must locate the gap in the observer: {verdict}"
        );
    }

    #[test]
    fn an_empty_reason_is_distinct_from_an_unread_one() {
        assert_eq!(
            classify_close_reason(Some("   ")),
            CloseReasonVerdict::Empty
        );
        assert_ne!(
            classify_close_reason(Some("   ")),
            classify_close_reason(None),
            "a close that recorded nothing and an observer that did not look are \
             different facts and must not collapse"
        );
    }

    /// The four labels must be distinct. Two verdicts sharing a label is the
    /// exit-code overload defect in a different alphabet.
    #[test]
    fn every_verdict_label_is_distinct() {
        let labels = [
            CloseReasonVerdict::Verified {
                prefix: ClosePrefix::Done,
            }
            .label(),
            CloseReasonVerdict::PolicyRefused {
                leading: "x".to_owned(),
            }
            .label(),
            CloseReasonVerdict::Empty.label(),
            CloseReasonVerdict::Unread.label(),
        ];
        let mut sorted = labels.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), labels.len(), "duplicate label in {labels:?}");
    }

    /// `DONE` and `MUTATION-VERIFIED` must not collapse: the second claims a
    /// mutation was proven to fire and the first does not.
    #[test]
    fn the_prefixes_do_not_collapse_into_one_another() {
        let done = classify_close_reason(Some("DONE: shipped"));
        let mutation = classify_close_reason(Some("MUTATION-VERIFIED: leg went RED"));
        assert_ne!(done, mutation);
        assert_eq!(
            ClosePrefix::ALL.len(),
            8,
            "the policy names exactly eight prefixes"
        );
        let mut tokens: Vec<&str> = ClosePrefix::ALL.iter().map(|p| p.as_str()).collect();
        let before = tokens.len();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), before, "duplicate prefix token");
    }
    #[test]
    fn extended_prefixes_do_not_accept_shadowing_tokens() {
        for (accepted, shadowed) in [
            ("PREMISE-FALSE", "PREMISE-FALSELY"),
            ("ALREADY-FIXED", "ALREADY-FIXEDLY"),
        ] {
            assert!(classify_close_reason(Some(&format!("{accepted}: detail"))).is_verified());
            assert!(
                !classify_close_reason(Some(&format!("{shadowed}: detail"))).is_verified(),
                "shadowed token must remain refused: {shadowed}"
            );
        }
    }

    #[test]
    fn cargo_figure_without_worker_is_refused_and_names_the_surface() {
        let verdict = classify_close_reason(Some("DONE: cargo test -p inbox-monitor 29 passed"));
        assert_eq!(
            verdict,
            CloseReasonVerdict::CargoWorkerMissing {
                leading: "DONE:".to_owned()
            }
        );
        let text = verdict.to_string();
        assert!(
            text.contains("CLOSE_REASON_WORKER_MISSING")
                && text.contains("worker=<name>")
                && text.contains("local"),
            "worker omission must be explicit: {text}"
        );
        assert!(!verdict.is_verified());
    }

    #[test]
    fn cargo_figure_with_worker_or_local_authority_is_verified() {
        assert!(classify_close_reason(Some("DONE: cargo test worker=contabo-3 29 passed")).is_verified());
        assert!(classify_close_reason(Some("DONE: local cargo test 29 passed")).is_verified());
    }
}
