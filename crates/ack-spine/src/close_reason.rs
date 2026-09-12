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
    /// The work was re-executed green by a grader, WITHOUT a mutation leg — so it
    /// is verified to pass and NOT verified to be load-bearing. Admitted because
    /// forcing it behind `DONE` would delete the distinction a grader deliberately
    /// recorded, which is `uqnut`'s EXTEND ruling: the refused property is that a
    /// reason must state its verdict CLASS, and this states one.
    BuiltAndTested,
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
        Self::BuiltAndTested,
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
            Self::BuiltAndTested => "BUILT-AND-TESTED",
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
            // The admitted set is RENDERED FROM `ClosePrefix::ALL`, not retyped.
            // It was a hand-written literal, so extending the set left the refusal
            // naming eight tokens while nine were admitted — a message that lies
            // about the rule it is enforcing, and one more copy of one law.
            Self::PolicyRefused { leading } => {
                let admitted: Vec<&str> =
                    ClosePrefix::ALL.iter().map(|prefix| prefix.as_str()).collect();
                write!(
                    formatter,
                    "CLOSE_REASON_POLICY_REFUSED leading={leading} -- a reason must start with one of {}; \
                     the local guard refuses this, but a direct br close bypasses it, so read the status back",
                    admitted.join(", ")
                )
            }
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

/// WHO closed the bead, as a typed value rather than prose.
///
/// # Why this is a SEPARATE classifier and not a new [`CloseReasonVerdict`] variant
///
/// The prefix verdict is asked of EVERY closed row, including the 447 already in
/// the mirror. None of them carries an actor, so folding "no recorded actor" into
/// that enum would redden the entire backlog the moment it shipped — and a gate
/// that reddens the backlog is reverted by the first person it blocks. Keeping the
/// axes apart lets the obligation land only where it can be complied with: rows
/// whose close is NEW in the staged mirror.
///
/// # Three values, because the third is the whole point
///
/// `Missing` is NOT `Malformed` and neither is `Recorded`. A row that names
/// `closed_by=InvMapRed` recorded a NAME, and AGENTS.md measures that a name is
/// not an identity — WildStone carried three panes — so that row is Malformed and
/// must not read as compliant. A row with nothing is Missing, which is the
/// migration state the ratchet drains. Collapsing either into a boolean is how a
/// third state dies at its call sites: measured tonight in `omp-orchestrator`,
/// where `Undetermined` was added correctly and then erased by
/// `refusing_gates()`/`advisory_gates()` both keying on `!is_reachable()`.
///
/// # NO-CLAIM
///
/// `closed_by=` is SELF-REPORTED AND FORGEABLE, exactly like the ACK comment. It
/// converts an unrecorded fact into a CHECKABLE CLAIM — a claim can be compared
/// against the comment record and the assignee history, silence cannot — and it is
/// not an identity proof. Nothing here authenticates the pane it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseActor {
    /// `closed_by=pane=%NN` was present and well-formed.
    Recorded {
        /// The pane id as written, including the leading `%`.
        pane: String,
    },
    /// No `closed_by=` at all. The migration state, not a lie.
    Missing,
    /// `closed_by=` was written with something that is not a pane id — most
    /// often an agent NAME, which is the exact substitution this token exists
    /// to refuse.
    Malformed {
        /// What followed `closed_by=`, truncated to one token.
        found: String,
    },
}

/// The token that records the closing actor.
pub const CLOSE_ACTOR_TOKEN: &str = "closed_by=";
/// The only admitted shape after the token.
pub const CLOSE_ACTOR_PANE_PREFIX: &str = "pane=%";

impl CloseActor {
    /// A stable label for logs and ledger rows, distinct from every
    /// [`CloseReasonVerdict`] label so a reader can tell the two causes apart.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Recorded { .. } => "CLOSE_ACTOR_RECORDED",
            Self::Missing => "CLOSE_ACTOR_MISSING",
            Self::Malformed { .. } => "CLOSE_ACTOR_MALFORMED",
        }
    }

    /// True ONLY for a well-formed pane id.
    ///
    /// Deliberately false for `Malformed`: a recorded name is not a recorded
    /// actor, and a caller that cannot tell them apart has re-created the
    /// name-is-not-an-identity defect one layer up.
    #[must_use]
    pub const fn is_recorded(&self) -> bool {
        matches!(self, Self::Recorded { .. })
    }
}

impl fmt::Display for CloseActor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recorded { pane } => write!(formatter, "CLOSE_ACTOR_RECORDED pane={pane}"),
            Self::Missing => write!(
                formatter,
                "CLOSE_ACTOR_MISSING -- a close must record WHO closed it as \
                 `{CLOSE_ACTOR_TOKEN}{CLOSE_ACTOR_PANE_PREFIX}NN`; without it no later check can \
                 ask whether the closer was the implementer. This is a DIFFERENT refusal from a \
                 bad prefix: the prefix says what evidence was claimed, this says who claimed it"
            ),
            Self::Malformed { found } => write!(
                formatter,
                "CLOSE_ACTOR_MALFORMED found={found} -- `{CLOSE_ACTOR_TOKEN}` must name a PANE \
                 (`{CLOSE_ACTOR_PANE_PREFIX}NN`), never an agent name: one agent carries several \
                 panes, so a name cannot answer whether the closer was the implementer"
            ),
        }
    }
}

/// Classify the closing actor recorded in a close reason.
///
/// Takes the reason by value rather than as `Option`, deliberately: "the observer
/// did not read the reason" is [`CloseReasonVerdict::Unread`] and belongs to that
/// axis. Mixing it in here would give this enum a fourth state that means
/// something about the caller rather than about the close.
#[must_use]
pub fn classify_close_actor(reason: &str) -> CloseActor {
    let Some(index) = reason.find(CLOSE_ACTOR_TOKEN) else {
        return CloseActor::Missing;
    };
    let rest = &reason[index + CLOSE_ACTOR_TOKEN.len()..];
    if let Some(pane) = rest.strip_prefix(CLOSE_ACTOR_PANE_PREFIX) {
        let digits: String = pane.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return CloseActor::Recorded {
                pane: format!("%{digits}"),
            };
        }
    }
    CloseActor::Malformed {
        found: rest
            .split_whitespace()
            .next()
            .unwrap_or("<end of reason>")
            .to_owned(),
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
///
/// Trailing and leading PUNCTUATION is trimmed before comparison, because a
/// measured row was refused on a trailing BACKTICK: `pxhmd` wrote `` `local` ``
/// and token equality could not see it, so an honest execution authority read as
/// absent. Prose is still refused — the word "workers", a bare "worker", and a
/// path like `~/.local/bin` are NOT authorities, which is the direction a
/// hand-rolled `contains("local")` check got wrong on four rows in one pass.
#[must_use]
pub fn has_worker_authority(reason: &str) -> bool {
    reason.split_whitespace().any(|raw| {
        let token = raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '=' && c != ':');
        token == "local" || token.starts_with("worker=") || token.starts_with("worker:")
    })
}

/// Classify a close reason against the local prefix policy.
///
/// `None` means the caller did not read the reason and yields
/// [`CloseReasonVerdict::Unread`] — never a verified verdict.
#[must_use]
pub fn classify_close_reason(reason: Option<&str>) -> CloseReasonVerdict {
    classify_close_reason_with_external_authority(reason, false)
}

/// [`classify_close_reason`], with the execution authority allowed to live
/// SOMEWHERE ELSE THAN THE REASON.
///
/// `external_authority` is true when the caller has already found `worker=<name>`,
/// `worker:<name>` or `local` outside the reason string — in practice in the row's
/// COMMENTS, which is where the evidence actually lives: the reason field is
/// UNAMENDABLE without reopening a closed bead, and `zero-open-S1` is the
/// predicate the S1 done-bar is defined over, so a gate that can only be satisfied
/// by a status flap is asking for a state change it does not want.
///
/// ⛔ THIS WIDENS THE SEARCH SURFACE, NOT THE RULE. A cargo figure still needs a
/// named tree; `external_authority = false` reproduces the old behaviour exactly,
/// which is what [`classify_close_reason`] passes and what its own legs pin. And a
/// comment is no more forgery-proof than a reason — both are writable by any
/// agent — so this is a provenance convention, never a proof.
#[must_use]
pub fn classify_close_reason_with_external_authority(
    reason: Option<&str>,
    external_authority: bool,
) -> CloseReasonVerdict {
    let Some(raw) = reason else {
        return CloseReasonVerdict::Unread;
    };
    let trimmed = raw.trim_start();
    if trimmed.trim().is_empty() {
        return CloseReasonVerdict::Empty;
    }
    if has_cargo_test_figure(trimmed) && !has_worker_authority(trimmed) && !external_authority {
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

/// What a grade citation establishes about the tree its figures ran on.
///
/// A grade is a `Reason` or comment citing a commit sha AND cargo figures
/// (`passed`/`failed`, `test result`, `rc=`). Anything less is not a grade
/// claim and this classifier has nothing to say about it ([`GradePin::NotAGrade`]).
/// A grade must pin three things in text: the porcelain state (`porcelain
/// empty`, or a `TREE_DIRTY` disclosure), the merge-base ancestry, and the
/// cargo-figure tree (`HEAD`, `sha` or `worktree` named beside `tree`). The
/// pins live in text because reasons and comments are the only surfaces that
/// survive a close; there is no side channel to consult.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradePin {
    /// The grade carries all three tree pins.
    Clean,
    /// A grade is cited but no tree pinning is present at all.
    TreeUnpinned,
    /// The text carries dirty markers with no `TREE_DIRTY` disclosure.
    TreeDirtyUndisclosed,
    /// No sha and no cargo figure: not a grade claim, never a refusal.
    NotAGrade,
    /// A grade is cited but the comment set is empty: error, never a pass.
    NoComments,
}

impl GradePin {
    /// A stable label for logs and ledger rows.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Clean => "GRADE_PIN_CLEAN",
            Self::TreeUnpinned => "GRADE_TREE_UNPINNED",
            Self::TreeDirtyUndisclosed => "GRADE_TREE_DIRTY_UNDISCLOSED",
            Self::NotAGrade => "NOT_A_GRADE",
            Self::NoComments => "GRADE_NO_COMMENTS",
        }
    }

    /// True for `Clean` and for `NotAGrade` (nothing claimed, nothing owed).
    /// False for every refusal, including `NoComments`.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Clean | Self::NotAGrade)
    }
}

impl fmt::Display for GradePin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => write!(formatter, "GRADE_PIN_CLEAN -- grade pins its tree"),
            Self::TreeUnpinned => write!(
                formatter,
                "GRADE_TREE_UNPINNED -- a grade cites a sha and cargo figures with no \
                 porcelain state, no merge-base ancestry, and no named tree; cite \
                 `porcelain empty` (or a TREE_DIRTY disclosure), the merge-base, and HEAD|sha|worktree"
            ),
            Self::TreeDirtyUndisclosed => write!(
                formatter,
                "GRADE_TREE_DIRTY_UNDISCLOSED -- the text carries dirty markers with no \
                 TREE_DIRTY disclosure; a dirty tree with a silent grade reads as clean"
            ),
            Self::NotAGrade => write!(
                formatter,
                "NOT_A_GRADE -- no sha and no cargo figure cited; nothing to pin"
            ),
            Self::NoComments => write!(
                formatter,
                "GRADE_NO_COMMENTS -- a grade is cited but the comment set is empty; \
                 tree pins live in comments, so there is nowhere for them to be"
            ),
        }
    }
}

/// Word tokens of a text, lowercased, split on non-alphanumerics.
fn grade_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_lowercase())
        .collect()
}

/// A hex token of sha length carrying at least one digit. The digit clause
/// excludes English words that happen to be hex (`deadbee`); a digitless
/// 7-plus hex run is rarer than the false positive it would admit.
fn has_sha(text: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric()).any(|token| {
        let len = token.len();
        (7..=40).contains(&len)
            && token.chars().all(|c| c.is_ascii_hexdigit())
            && token.chars().any(|c| c.is_ascii_digit())
    })
}

/// A cargo-figure claim: passed/failed counts, a test-result line, or an rc= code.
fn has_grade_figure(text: &str) -> bool {
    let words = grade_words(text);
    words.iter().any(|word| word == "passed" || word == "failed")
        || text.to_lowercase().contains("test result")
        || text
            .split_whitespace()
            .any(|raw| raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '=' && c != ':').starts_with("rc="))
}

/// A git-status porcelain line: two status columns, a space, a path. Only
/// the verbatim pasted form counts; prose about dirt does not.
fn has_porcelain_lines(text: &str) -> bool {
    const CODES: &[char] = &['M', 'A', 'D', 'R', 'C', 'U', '?', '!', ' '];
    text.lines().any(|line| {
        let mut chars = line.chars();
        matches!((chars.next(), chars.next(), chars.next()), (Some(a), Some(b), Some(' ')) if CODES.contains(&a) && CODES.contains(&b))
            && chars.as_str().split_whitespace().next().is_some_and(|path| !path.is_empty())
    })
}

/// Classify a close's grade evidence: the reason plus every comment.
///
/// The `comments` slice is load-bearing, not decorative: porcelain state,
/// merge-base ancestry and the tree label live in comment evidence, so a
/// grade cited with an empty comment set is an error, never a pass.
#[must_use]
pub fn classify_grade_pin(reason: Option<&str>, comments: &[String]) -> GradePin {
    let mut corpus = reason.unwrap_or_default().to_owned();
    for comment in comments {
        corpus.push('\n');
        corpus.push_str(comment);
    }
    let lowered = corpus.to_lowercase();
    let grade = has_sha(&corpus) && has_grade_figure(&corpus);
    if !grade {
        return GradePin::NotAGrade;
    }
    if comments.is_empty() {
        return GradePin::NoComments;
    }
    let words = grade_words(&corpus);
    let has_word = |word: &str| words.iter().any(|w| w == word);
    let tree_dirty = corpus.contains("TREE_DIRTY");
    if has_porcelain_lines(&corpus) && !tree_dirty {
        return GradePin::TreeDirtyUndisclosed;
    }
    if lowered.contains("dirty") && !tree_dirty && !lowered.contains("porcelain empty") {
        return GradePin::TreeDirtyUndisclosed;
    }
    let porcelain = lowered.contains("porcelain empty") || tree_dirty;
    let ancestry = lowered.contains("merge-base") || has_word("ancestor");
    let tree = (has_word("tree") || has_word("worktree"))
        && (has_word("head") || has_word("worktree") || has_word("sha"));
    if !(porcelain && ancestry && tree) {
        return GradePin::TreeUnpinned;
    }
    GradePin::Clean
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
            9,
            "the policy names exactly nine prefixes; BUILT-AND-TESTED is the ninth, \
             added under uqnut's EXTEND ruling"
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

    /// 5erif: a measured row wrote `` `local` `` and was refused on a trailing
    /// BACKTICK. Punctuation is trimmed; the authority is the token, not its
    /// typography.
    #[test]
    fn punctuation_around_an_authority_token_does_not_hide_it() {
        for reason in [
            "DONE: cargo test 4 passed, `local`",
            "DONE: cargo test 4 passed (worker=contabo-2)",
            "DONE: cargo test 4 passed on worker:contabo-4.",
        ] {
            assert!(
                classify_close_reason(Some(reason)).is_verified(),
                "{reason:?} names its tree and must verify"
            );
        }
    }

    /// The other direction, and the one a hand-rolled `contains` check got wrong on
    /// four rows in a single pass: PROSE IS NOT AN AUTHORITY.
    #[test]
    fn prose_and_paths_are_not_execution_authorities() {
        for reason in [
            "DONE: cargo test 4 passed, ran on the workers",
            "DONE: cargo test 4 passed via ~/.local/bin/cargo",
            "DONE: cargo test 4 passed, worker unknown",
        ] {
            assert!(
                !classify_close_reason(Some(reason)).is_verified(),
                "{reason:?} names no tree and must refuse"
            );
        }
    }

    /// THE THIRD VALUE, AND THE TWO WRONG ANSWERS IT MUST NOT COLLAPSE INTO.
    ///
    /// A typed error is only as sharp as its call site: `Undetermined` was added
    /// correctly to `omp-orchestrator`'s reachability and then erased by two
    /// consumers that both keyed on `!is_reachable()`, so an UNMEASURED row
    /// bucketed exactly as a MEASURED-and-failed one. These legs pin the WRONG
    /// answers, not just the right one, so a boolean at a future call site
    /// cannot quietly flatten `Missing` into either compliance or a prefix
    /// violation.
    #[test]
    fn a_missing_actor_is_neither_recorded_nor_a_prefix_violation() {
        let reason = "DONE the work landed and the leg is green";
        let actor = classify_close_actor(reason);
        assert_eq!(actor, CloseActor::Missing);
        assert!(!actor.is_recorded(), "Missing must never read as recorded");

        // AND THE OTHER AXIS IS UNTOUCHED: the very same reason is a perfectly
        // good PREFIX. If these two ever move together, one of them is reading
        // the other's evidence.
        let verdict = classify_close_reason(Some(reason));
        assert!(
            verdict.is_verified(),
            "a missing actor must not make a sanctioned prefix fail: {verdict}"
        );
        assert_ne!(
            actor.label(),
            verdict.label(),
            "the two axes must not share a label, or one refusal will be read as the other"
        );
    }

    /// A NAME IS NOT AN IDENTITY, and this is the substitution the token exists
    /// to refuse. `WildStone` carried three panes, so `closed_by=WildStone`
    /// cannot answer whether the closer was the implementer.
    #[test]
    fn an_agent_name_is_malformed_not_recorded() {
        let actor = classify_close_actor("APPROVED by me closed_by=InvMapRed on a good grade");
        assert_eq!(
            actor,
            CloseActor::Malformed {
                found: "InvMapRed".to_owned()
            }
        );
        assert!(
            !actor.is_recorded(),
            "a recorded NAME is not a recorded ACTOR"
        );
        assert_eq!(actor.label(), "CLOSE_ACTOR_MALFORMED");

        // A half-written token is malformed too, not missing: the difference is
        // between nobody having tried and somebody having tried wrongly.
        assert_eq!(
            classify_close_actor("DONE closed_by=pane=abc"),
            CloseActor::Malformed {
                found: "pane=abc".to_owned()
            }
        );
    }

    /// KNOWN-GOOD, and the pane survives verbatim including its `%`.
    #[test]
    fn a_well_formed_pane_is_recorded_verbatim() {
        assert_eq!(
            classify_close_actor("APPROVED non-author grade closed_by=pane=%33 -- legs green"),
            CloseActor::Recorded {
                pane: "%33".to_owned()
            }
        );
        // Trailing punctuation must not be eaten into the pane id.
        assert_eq!(
            classify_close_actor("DONE closed_by=pane=%7, mutation reverted"),
            CloseActor::Recorded {
                pane: "%7".to_owned()
            }
        );
        assert!(classify_close_actor("DONE closed_by=pane=%7").is_recorded());
    }

    /// ANTI-VACUITY ON THE CLASSIFIER ITSELF: it must be capable of all three
    /// answers. A classifier that can only ever say one thing is a constant
    /// wearing the shape of a measurement -- the defect this whole module was
    /// written to remove.
    #[test]
    fn the_actor_classifier_can_return_each_of_its_three_values() {
        let seen = [
            classify_close_actor("DONE closed_by=pane=%1"),
            classify_close_actor("DONE"),
            classify_close_actor("DONE closed_by=somebody"),
        ];
        let labels: Vec<&str> = seen.iter().map(CloseActor::label).collect();
        assert_eq!(
            labels,
            vec![
                "CLOSE_ACTOR_RECORDED",
                "CLOSE_ACTOR_MISSING",
                "CLOSE_ACTOR_MALFORMED"
            ],
            "all three states must be reachable from real reasons"
        );
    }
}
