#![forbid(unsafe_code)]

//! A NAMED GAP IS AN OBLIGATION.
//!
//! MEASURED 2026-08-31: in one session this repo produced **25 headed rules** in
//! `WAVE.md` and **29 beads**, and at least **8 gaps were named in prose and never
//! filed** — the wildcard lint, `Duty` left unwired, a close_reason citing a path
//! that never existed, `retryable: true` on a permanent condition, `vanished()`
//! reported but never acted on, and more. Eight is only what one agent could
//! recall; the true count is unbounded because nothing counts it.
//!
//! ## Why findings leak
//!
//! There are three places a finding can land, and ALL THREE are write-only with
//! respect to the work queue:
//!
//! | sink | durable? | schedulable? |
//! |---|---|---|
//! | a chat report | no — it scrolls away | no |
//! | `WAVE.md` / `AGENTS.md` | yes | **no** — prose is not a work item |
//! | a comment on bead A | yes | **no** — it creates no bead B |
//!
//! And the incentive is inverted: **filing is discretionary and the finder is
//! always mid-task.** The cost is paid now, by the finder; the benefit accrues
//! later, to someone else. That is why "I noted it in the report" felt like
//! discharge and was not.
//!
//! ## The kernel answer
//!
//! [`Finding`] is `#[must_use]`. You cannot *have* a finding and drop it — the
//! compiler objects. The only exits are [`Finding::file`] (becomes a bead) and
//! [`Finding::waive`] (requires a written reason). There is no third path, and
//! "mention it in prose" is not one of them.
//!
//! Construction enforces the Jeff-bead standard rather than documenting it:
//! WHAT, WHY, and ACCEPTANCE are all required and non-empty, plus at least one
//! label, because `bv` scoping and the alert surface only work on labelled beads.
//! **A title-only bead is unrepresentable.**
//!
//! ## Cancel-correctness (asupersync contract)
//!
//! Filing is SPOOL-THEN-PUBLISH, and the ordering is the whole point:
//!
//! 1. Write a durable spool row. Pure, synchronous, no cancellable effect.
//! 2. `cx.checkpoint()` — the ONLY cancellation point.
//! 3. Publish via `subprocess_contract::run_output`, which drains both pipes.
//! 4. Mark the spool row published only after a confirmed exit.
//!
//! **A cancellation between (1) and (4) leaves a recoverable spool row.** So a
//! cancelled file is a DEFERRED finding, never a lost one — which is the same
//! rule as "a timeout is not a verdict" applied to durability. `&Cx` comes first
//! in every async signature, no task is detached, and the spool sweep is how the
//! region's owner recovers work its child could not finish.

use std::fmt;
use std::path::{Path, PathBuf};

use asupersync::Cx;

/// Why a finding could not be constructed or filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingError {
    /// The Jeff-bead standard, enforced at construction rather than reviewed.
    MissingField(&'static str),
    /// A waiver with no reason is a silent drop wearing a decision.
    WaiverNeedsReason,
    /// The spool could not be written. Filing MUST NOT proceed: publishing
    /// without a durable record is how a cancellation loses the finding.
    SpoolUnwritable(String),
    /// The publish command failed or was refused.
    PublishFailed(String),
    /// The region was cancelled. The spool row survives; this is DEFERRED.
    Cancelled { spool_path: PathBuf },
}

impl fmt::Display for FindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(
                f,
                "FINDING_INCOMPLETE: {field} is required — a title-only bead cannot be worked, only adjudicated"
            ),
            Self::WaiverNeedsReason => write!(
                f,
                "WAIVER_NEEDS_REASON: dropping a finding requires a written reason"
            ),
            Self::SpoolUnwritable(why) => write!(f, "SPOOL_UNWRITABLE: {why}"),
            Self::PublishFailed(why) => write!(f, "PUBLISH_FAILED: {why}"),
            Self::Cancelled { spool_path } => write!(
                f,
                "FILING_DEFERRED: cancelled after spooling; recoverable at {}",
                spool_path.display()
            ),
        }
    }
}

impl std::error::Error for FindingError {}

/// A named gap that MUST be filed or explicitly waived.
///
/// `#[must_use]` is the enforcement. Every other mechanism in this repo that
/// relied on someone remembering has failed at least once tonight.
#[must_use = "an unfiled Finding is the gap that never became a bead: call file() or waive()"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    what: String,
    why: String,
    acceptance: String,
    labels: Vec<String>,
    priority: u8,
}

impl Finding {
    /// The ONLY constructor. Refuses an incomplete finding, so the bead standard
    /// is a type precondition rather than a review comment.
    pub fn new(
        what: impl Into<String>,
        why: impl Into<String>,
        acceptance: impl Into<String>,
        labels: Vec<String>,
        priority: u8,
    ) -> Result<Self, FindingError> {
        let (what, why, acceptance) = (what.into(), why.into(), acceptance.into());
        if what.trim().is_empty() {
            return Err(FindingError::MissingField("WHAT"));
        }
        if why.trim().is_empty() {
            return Err(FindingError::MissingField("WHY"));
        }
        if acceptance.trim().is_empty() {
            return Err(FindingError::MissingField("ACCEPTANCE"));
        }
        // Labels are not decoration: `bv -l <label> --robot-insights` and the
        // alert surface only work on labelled beads.
        if labels.iter().all(|l| l.trim().is_empty()) {
            return Err(FindingError::MissingField("LABELS"));
        }
        Ok(Self {
            what,
            why,
            acceptance,
            labels,
            priority,
        })
    }

    /// The bead body, in the standard's WHAT/WHY/ACCEPTANCE shape.
    pub fn body(&self) -> String {
        format!(
            "WHAT: {}\nWHY: {}\nACCEPTANCE: {}",
            self.what.trim(),
            self.why.trim(),
            self.acceptance.trim()
        )
    }

    pub fn labels(&self) -> &[String] {
        &self.labels
    }
    pub fn priority(&self) -> u8 {
        self.priority
    }

    /// STEP 1 of filing, and it is deliberately separable: a durable record
    /// written BEFORE any cancellable effect. Cancellation after this point
    /// leaves recoverable work.
    pub fn spool(&self, spool_dir: &Path) -> Result<SpooledFinding, FindingError> {
        std::fs::create_dir_all(spool_dir)
            .map_err(|e| FindingError::SpoolUnwritable(e.to_string()))?;
        // Content-derived name: re-spooling the same finding is idempotent rather
        // than duplicating it.
        let stem = stable_stem(&self.body());
        let path = spool_dir.join(format!("finding-{stem}.pending"));
        let payload = format!(
            "priority={}\nlabels={}\n---\n{}\n",
            self.priority,
            self.labels.join(","),
            self.body()
        );
        // Write-then-rename: a crash mid-write cannot leave a truncated row that
        // reads as a malformed finding.
        let tmp = path.with_extension("pending.tmp");
        std::fs::write(&tmp, payload).map_err(|e| FindingError::SpoolUnwritable(e.to_string()))?;
        std::fs::rename(&tmp, &path).map_err(|e| FindingError::SpoolUnwritable(e.to_string()))?;
        Ok(SpooledFinding { path })
    }

    /// The ONLY sanctioned way to not file. A reason is REQUIRED, because a
    /// waiver without one is a silent drop with extra steps.
    pub fn waive(self, reason: impl Into<String>) -> Result<Waived, FindingError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(FindingError::WaiverNeedsReason);
        }
        Ok(Waived {
            body: self.body(),
            reason,
        })
    }

    /// SPOOL-THEN-PUBLISH. `&Cx` first; the durable write precedes the only
    /// cancellation point; cancellation yields `Cancelled` naming the recoverable
    /// spool row rather than losing the finding.
    pub async fn file(
        self,
        cx: &Cx,
        spool_dir: &Path,
        publisher: &impl Publisher,
    ) -> Result<Filed, FindingError> {
        let spooled = self.spool(spool_dir)?;
        // THE ONLY CANCELLATION POINT. Before it, nothing external happened.
        // After it, the spool row exists and a sweep can finish the job.
        if cx.checkpoint().is_err() {
            return Err(FindingError::Cancelled {
                spool_path: spooled.path.clone(),
            });
        }
        let id = publisher.publish(cx, &self).await?;
        spooled.mark_published(&id)?;
        Ok(Filed {
            id,
            body: self.body(),
        })
    }
}

/// A durable spool row. Its existence is the recovery guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpooledFinding {
    path: PathBuf,
}

impl SpooledFinding {
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Retire the row only after a CONFIRMED publish. Renaming rather than
    /// deleting keeps the audit trail.
    pub fn mark_published(&self, id: &str) -> Result<(), FindingError> {
        let done = self.path.with_extension(format!("filed-{}", sanitize(id)));
        std::fs::rename(&self.path, &done).map_err(|e| FindingError::SpoolUnwritable(e.to_string()))
    }
}

/// Spool rows still awaiting publication — the recovery sweep.
///
/// ANTI-VACUITY: an unreadable spool directory is an ERROR, not an empty sweep.
/// "I could not look" and "there is nothing there" are opposite conditions, and
/// conflating them is the defect that produced twelve confident zeros tonight.
pub fn pending(spool_dir: &Path) -> Result<Vec<PathBuf>, FindingError> {
    let entries =
        std::fs::read_dir(spool_dir).map_err(|e| FindingError::SpoolUnwritable(e.to_string()))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| FindingError::SpoolUnwritable(e.to_string()))?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "pending") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// Proof a finding became a bead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filed {
    id: String,
    body: String,
}

impl Filed {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// Proof a finding was deliberately NOT filed, with the reason recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waived {
    body: String,
    reason: String,
}

impl Waived {
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// The publish side, abstracted so the cancel-correct ordering is testable
/// without a live tracker. Implementors MUST drain both pipes — use
/// `subprocess_contract::run_output`, never a hand-rolled spawn.
pub trait Publisher {
    fn publish(
        &self,
        cx: &Cx,
        finding: &Finding,
    ) -> impl Future<Output = Result<String, FindingError>>;
}

fn stable_stem(text: &str) -> String {
    // Small, dependency-free, and only needs to be stable + collision-resistant
    // enough to make re-spooling idempotent.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("finding-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn ok_finding() -> Finding {
        Finding::new(
            "wildcard match arms on state enums are not linted",
            "an exhaustive match caught a real omission at compile time tonight; two `_ => 0` catch-alls silently shipped bugs",
            "a lint flags any wildcard arm matching a state enum; known-bad = a planted `_ =>` goes RED; known-good = a non-state match still passes",
            vec!["gate".into(), "kernel".into()],
            0,
        )
        .expect("complete finding")
    }

    #[test]
    fn a_finding_without_what_why_or_acceptance_cannot_exist() {
        // THE BEAD STANDARD AS A TYPE PRECONDITION. A P0 sat at the head of the
        // ready queue tonight with no ACCEPTANCE section; two agents in a row
        // triaged it and went idle, because a bead you cannot write run-X-expect-Y
        // for can only be adjudicated, not worked.
        for (what, why, acc, missing) in [
            ("", "w", "a", "WHAT"),
            ("x", "", "a", "WHY"),
            ("x", "w", "", "ACCEPTANCE"),
        ] {
            let err = Finding::new(what, why, acc, vec!["l".into()], 1)
                .expect_err("incomplete finding must be refused");
            assert_eq!(err, FindingError::MissingField(missing), "{missing} leg");
        }
    }

    #[test]
    fn a_finding_without_labels_cannot_exist() {
        // `bv -l <label> --robot-insights` and the alert surface only work on
        // labelled beads. An unlabelled bead is invisible to the planning brain.
        let err = Finding::new("x", "w", "a", vec!["  ".into()], 1)
            .expect_err("unlabelled finding must be refused");
        assert_eq!(err, FindingError::MissingField("LABELS"));
    }

    #[test]
    fn the_body_carries_the_standard_shape() {
        let body = ok_finding().body();
        for key in ["WHAT:", "WHY:", "ACCEPTANCE:"] {
            assert!(body.contains(key), "body must carry {key}: {body}");
        }
    }

    #[test]
    fn a_waiver_without_a_reason_is_refused() {
        let err = ok_finding()
            .waive("   ")
            .expect_err("a reasonless waiver is a silent drop");
        assert_eq!(err, FindingError::WaiverNeedsReason);
    }

    #[test]
    fn a_waiver_with_a_reason_records_it() {
        let waived = ok_finding()
            .waive("superseded by cp-g6sy8; same class, already owned")
            .expect("reasoned waiver");
        assert!(waived.reason().contains("superseded"));
        assert!(waived.body().contains("WHAT:"), "the body survives the waiver");
    }

    #[test]
    fn spooling_happens_before_any_publish_and_survives_as_recoverable() {
        // THE CANCEL-CORRECTNESS LEG. This is what makes a cancelled file a
        // DEFERRED finding rather than a lost one.
        let d = dir("spool");
        let spooled = ok_finding().spool(&d).expect("spool");
        assert!(spooled.path().exists(), "the durable row must exist");
        let recoverable = pending(&d).expect("sweep");
        assert_eq!(recoverable.len(), 1, "an unpublished finding must be sweepable");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn respooling_the_same_finding_is_idempotent_not_duplicated() {
        // A retry after a cancellation must not create a second bead.
        let d = dir("idem");
        let _ = ok_finding().spool(&d).expect("first");
        let _ = ok_finding().spool(&d).expect("second");
        assert_eq!(pending(&d).expect("sweep").len(), 1, "content-derived name");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_published_row_leaves_the_pending_sweep() {
        let d = dir("pub");
        let spooled = ok_finding().spool(&d).expect("spool");
        spooled.mark_published("omp-orchestrator-abc").expect("mark");
        assert!(
            pending(&d).expect("sweep").is_empty(),
            "a filed finding must not be re-filed by the sweep"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_unreadable_spool_dir_is_an_error_not_an_empty_sweep() {
        // ANTI-VACUITY. "I could not look" and "there is nothing there" are
        // opposite conditions; twelve confident zeros tonight came from conflating
        // exactly this.
        let missing = std::env::temp_dir().join("finding-does-not-exist-xyzzy");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(
            matches!(pending(&missing), Err(FindingError::SpoolUnwritable(_))),
            "an absent spool dir must ERROR, never report zero pending"
        );
    }
}
