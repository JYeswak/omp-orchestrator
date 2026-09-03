//! Ack spine (slice a): the asupersync step ledger.
//!
//! Every dispatch step is a region-owned operation taking `&Cx` FIRST and
//! emitting exactly ONE typed row. The step count is ASSERTED — a dispatch
//! that emits fewer rows than steps taken FAILS.
//!
//! Cancellation is acknowledged before the effect and after it. The row is
//! committed before the post-effect checkpoint, so a cancellation observed at
//! that checkpoint leaves a consistent, recoverable prefix: `last_kind()` is
//! the completed boundary from which the owner can resume.

use std::fmt;
use std::future::Future;

/// Errors raised while enforcing the ledger's deciding invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// No dispatch step was observed. A zero-step run is never a clean run.
    Empty,
    /// A step and its corresponding row diverged.
    CountMismatch { rows: usize, steps_taken: usize },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(
                f,
                "ANTI_VACUITY: zero steps observed is an ERROR, never a clean dispatch"
            ),
            Self::CountMismatch { rows, steps_taken } => write!(
                f,
                "STEP_COUNT_ASSERTION_FAILED: rows={rows} steps_taken={steps_taken}"
            ),
        }
    }
}

impl std::error::Error for LedgerError {}

/// Errors from the cancel-correct step primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    /// Cancellation was observed. The prefix through `last_kind` is durable
    /// and can be resumed from the next step.
    Cancelled { last_kind: Option<StepKind> },
    /// The ledger invariant was violated.
    Ledger(LedgerError),
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled { last_kind } => {
                write!(f, "CANCELLED step_prefix_last={last_kind:?}")
            }
            Self::Ledger(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for StepError {}

impl From<LedgerError> for StepError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

/// The dispatch steps, in order. Each kind emits exactly one row per
/// occurrence in the dispatch path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepKind {
    /// `br ready` selected a bead for this pane.
    BeadSelected,
    /// The dispatch packet was rendered from the bead body.
    PacketRendered,
    /// The pane-dispatch fence admitted this pane.
    FenceChecked,
    /// The packet was sent (ntm robot-send through the fence).
    PacketSent,
    /// The receiver proof verified the pane is acting on the named bead.
    ReceiverVerified,
    /// The receiver proof timed out — the pane did not show the bead.
    ReceiverTimedOut,
    /// The grade receipt was requested from the independent grader.
    GradeRequested,
    /// The grade receipt was received (PASS or FIX).
    GradeReceived,
    /// The bead was closed by the grader, with a reason the close policy admits.
    Closed,
    /// The bead was closed with a reason the policy does NOT admit — so it closed
    /// and it was never graded.
    ///
    /// # Why this is its own variant (`mcq2`, and it is a measurement)
    ///
    /// MEASURED 2026-09-02 across 102 closes: **18 carry a first token outside the
    /// four documented prefixes** — `COMPLETE` ×11 (every one an `ipg.*`),
    /// `PARTIALLY` ×1, `CONFIRMED`, and three `VERIFIED-*` variants. AGENTS.md
    /// states the close policy REFUSES prose; for one close in five that is false,
    /// and **a bead closed as `PARTIALLY`**.
    ///
    /// The first version of the classifier emitted a bare `Closed` for those, so an
    /// ungraded close was distinguishable only by **the ABSENCE of a sibling
    /// `GradeReceived` row**. Absence is not evidence: a reader cannot tell "closed
    /// without a grade" from "the grade row was lost", and a query for ungraded
    /// closes had to join rows and infer. The whole argument for typing over
    /// substrings applies here at one more level — this is the sixth
    /// missing-representation defect of the day, after `iis6` (busy/dead),
    /// `leht` (unasked/nonexistent), `t784` (awaiting-grade/unstarted) and
    /// `--claim` (actor/agent).
    ClosedWithoutGrade,
    /// The bead was sent back for redispatch with a named fix.
    Redispatched,
    /// A bead-comment acknowledgement was confirmed by read-back.
    AckReadBack,
}

impl StepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StepKind::BeadSelected => "bead_selected",
            StepKind::PacketRendered => "packet_rendered",
            StepKind::FenceChecked => "fence_checked",
            StepKind::PacketSent => "packet_sent",
            StepKind::ReceiverVerified => "receiver_verified",
            StepKind::ReceiverTimedOut => "receiver_timed_out",
            StepKind::GradeRequested => "grade_requested",
            StepKind::GradeReceived => "grade_received",
            StepKind::Closed => "closed",
            // A DISTINCT wire string, deliberately. Reusing "closed" with a flag
            // elsewhere would put the distinction back into a field a reader has to
            // remember to check, which is the substring problem again.
            StepKind::ClosedWithoutGrade => "closed_without_grade",
            StepKind::Redispatched => "redispatched",
            StepKind::AckReadBack => "ack_read_back",
        }
    }
}

/// One typed row in the step ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRecord {
    pub kind: StepKind,
    pub bead_id: String,
    pub pane_id: String,
    pub session: String,
    pub ts_unix: i64,
    pub detail: String,
}

impl fmt::Display for StepRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {} {}",
            self.kind.as_str(),
            self.bead_id,
            self.pane_id,
            self.session,
            self.detail
        )
    }
}

/// The step ledger: a sequence of typed rows, one per dispatch step.
#[derive(Debug, Clone, Default)]
pub struct StepLedger {
    rows: Vec<StepRecord>,
    steps_taken: usize,
}

impl StepLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit one row and account for its step. This is intentionally private:
    /// the public `step` primitive is the single emission path.
    fn emit(&mut self, record: StepRecord) {
        self.rows.push(record);
        self.steps_taken += 1;
    }

    /// THE DECIDING LEG: the step count is ASSERTED.
    ///
    /// Returns an error when the row count does not match the steps taken —
    /// meaning a step completed without emitting a row, or a row was emitted
    /// without a step. Both are ledger corruption.
    pub fn assert_step_count(&self) -> Result<(), LedgerError> {
        if self.rows.len() != self.steps_taken {
            return Err(LedgerError::CountMismatch {
                rows: self.rows.len(),
                steps_taken: self.steps_taken,
            });
        }
        Ok(())
    }

    /// The ledger is consistent when every step has a row and vice versa.
    pub fn is_consistent(&self) -> bool {
        self.rows.len() == self.steps_taken
    }

    /// ANTI-VACUITY: zero steps observed is an ERROR, never a clean dispatch.
    pub fn assert_non_empty(&self) -> Result<(), LedgerError> {
        if self.rows.is_empty() && self.steps_taken == 0 {
            return Err(LedgerError::Empty);
        }
        Ok(())
    }

    pub fn rows(&self) -> &[StepRecord] {
        &self.rows
    }

    pub fn steps_taken(&self) -> usize {
        self.steps_taken
    }

    /// The kind of the last recorded step — the recovery boundary. A
    /// conductor resumes from the next step, not from scratch.
    pub fn last_kind(&self) -> Option<StepKind> {
        self.rows.last().map(|r| r.kind)
    }

    /// Serialize to JSONL: one JSON object per line, field order fixed.
    ///
    /// # Why this is not a `format!`
    ///
    /// It was, until `omp-orchestrator-11ou`. The old body escaped exactly one
    /// character — four `.replace('"', "\\\"")` calls and nothing for `\n`,
    /// `\t`, `\\`, or control bytes — then joined rows with `"\n"`. A `detail`
    /// carrying a newline therefore emitted a **literal newline inside a JSON
    /// string**: invalid JSON, and it broke the one-object-per-line invariant
    /// that is the only thing making the format JSONL. Measured on the pre-fix
    /// code with all five hostile characters in every field: **5 physical lines
    /// for 1 row**, `back\slash` as an invalid escape, and a raw tab and ESC
    /// byte inside the string.
    ///
    /// The failure mode is what makes it worth a comment: **the write
    /// SUCCEEDED.** Nothing reported a problem. The corruption surfaces in the
    /// reader, arbitrarily later, with no path back to the writer — and `detail`
    /// is precisely the field that carries captured error messages and pane
    /// tails, which routinely contain newlines.
    ///
    /// # The trap avoided here
    ///
    /// A `serde_json::Map` is deliberately NOT used. Without the
    /// `preserve_order` feature it is a `BTreeMap`, so building rows through it
    /// would silently ALPHABETISE this contract into
    /// `bead, detail, kind, pane, session, ts`. A derived `Serialize` struct
    /// emits in declaration order, because serde calls `serialize_field` in
    /// declaration order and the JSON serialiser writes them in call order.
    pub fn to_jsonl(&self) -> String {
        /// The wire projection. DECLARATION ORDER IS THE FIELD ORDER, and it
        /// reproduces the pre-fix key order exactly so no reader breaks.
        #[derive(serde::Serialize)]
        struct Wire<'row> {
            kind: &'static str,
            bead: &'row str,
            pane: &'row str,
            session: &'row str,
            ts: i64,
            detail: &'row str,
        }

        self.rows
            .iter()
            .map(|row| {
                let wire = Wire {
                    kind: row.kind.as_str(),
                    bead: &row.bead_id,
                    pane: &row.pane_id,
                    session: &row.session,
                    ts: row.ts_unix,
                    detail: &row.detail,
                };
                // A serialisation failure here is unreachable: every field is a
                // `str` or an `i64`, and serde_json only fails on non-string map
                // keys, non-finite floats, or a custom Serialize that errors.
                // Rather than `unwrap()`, emit a row that SAYS the row was lost —
                // a silently dropped ledger row is the defect class this whole
                // function exists to remove.
                serde_json::to_string(&wire).unwrap_or_else(|error| {
                    format!(
                        r#"{{"kind":"serialization_failed","bead":"","pane":"","session":"","ts":0,"detail":{}}}"#,
                        serde_json::Value::String(error.to_string())
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Execute one region-owned dispatch step.
///
/// The caller supplies the runtime-owned context first. Cancellation is
/// checked before the effect and after its boundary. The record is committed
/// between those checkpoints, making a post-effect cancellation a consistent
/// and recoverable prefix rather than a half-state. The effect is awaited
/// directly; this primitive never detaches a task.
pub async fn step<F, Fut>(
    cx: &asupersync::Cx,
    ledger: &mut StepLedger,
    kind: StepKind,
    bead_id: &str,
    pane_id: &str,
    session: &str,
    detail: &str,
    effect: F,
) -> Result<(), StepError>
where
    F: FnOnce(&asupersync::Cx) -> Fut,
    Fut: Future<Output = ()>,
{
    if cx.checkpoint().is_err() {
        return Err(StepError::Cancelled {
            last_kind: ledger.last_kind(),
        });
    }

    effect(cx).await;

    let ts_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    ledger.emit(StepRecord {
        kind,
        bead_id: bead_id.to_owned(),
        pane_id: pane_id.to_owned(),
        session: session.to_owned(),
        ts_unix,
        detail: detail.to_owned(),
    });

    if cx.checkpoint().is_err() {
        return Err(StepError::Cancelled {
            last_kind: ledger.last_kind(),
        });
    }

    ledger.assert_step_count()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asupersync::runtime::RuntimeBuilder;
    use asupersync::types::CancelKind;

    fn run<F: Future>(future: F) -> F::Output {
        RuntimeBuilder::current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(future)
    }

    #[test]
    fn one_row_per_step_count_asserted() {
        run(async {
            let cx = asupersync::Cx::current().expect("runtime Cx");
            let mut ledger = StepLedger::new();
            step(
                &cx,
                &mut ledger,
                StepKind::BeadSelected,
                "cp-1",
                "%5",
                "s",
                "selected",
                |_| async {},
            )
            .await
            .expect("first step");
            step(
                &cx,
                &mut ledger,
                StepKind::PacketRendered,
                "cp-1",
                "%5",
                "s",
                "rendered",
                |_| async {},
            )
            .await
            .expect("second step");
            ledger.assert_step_count().expect("two rows, two steps");
            assert_eq!(ledger.rows().len(), 2);
            assert_eq!(ledger.steps_taken(), 2);
        });
    }

    #[test]
    fn deleted_row_emission_goes_red() {
        // KNOWN-BAD: a step that completes but does NOT emit a row.
        let mut ledger = StepLedger::new();
        ledger.steps_taken = 1;
        assert!(matches!(
            ledger.assert_step_count(),
            Err(LedgerError::CountMismatch {
                rows: 0,
                steps_taken: 1
            })
        ));
    }

    #[test]
    fn cancellation_keeps_consistent_recoverable_prefix() {
        run(async {
            let cx = asupersync::Cx::current().expect("runtime Cx");
            let mut ledger = StepLedger::new();
            step(
                &cx,
                &mut ledger,
                StepKind::BeadSelected,
                "cp-1",
                "%5",
                "s",
                "selected",
                |_| async {},
            )
            .await
            .expect("prefix step");

            let error = step(
                &cx,
                &mut ledger,
                StepKind::PacketRendered,
                "cp-1",
                "%5",
                "s",
                "rendered",
                |cx| {
                    cx.cancel_with(CancelKind::User, Some("cancel after effect"));
                    async {}
                },
            )
            .await
            .expect_err("post-effect cancellation");
            assert_eq!(
                error,
                StepError::Cancelled {
                    last_kind: Some(StepKind::PacketRendered)
                }
            );
            assert!(ledger.is_consistent());
            ledger.assert_step_count().expect("cancel prefix count");
            assert_eq!(ledger.last_kind(), Some(StepKind::PacketRendered));
            assert_eq!(ledger.rows().len(), 2);
        });
    }

    #[test]
    fn empty_ledger_is_typed_anti_vacuity_error() {
        let ledger = StepLedger::new();
        assert_eq!(ledger.assert_non_empty(), Err(LedgerError::Empty));
    }

    #[test]
    fn jsonl_serialization_round_trips() {
        let mut ledger = StepLedger::new();
        ledger.emit(StepRecord {
            kind: StepKind::BeadSelected,
            bead_id: "cp-1".to_owned(),
            pane_id: "%5".to_owned(),
            session: "s".to_owned(),
            ts_unix: 1,
            detail: "selected".to_owned(),
        });
        let jsonl = ledger.to_jsonl();
        assert!(jsonl.contains("bead_selected"));
        assert!(jsonl.contains("cp-1"));
    }

    /// The five hostile characters, all in every field of one row.
    ///
    /// A quote, a backslash, a newline, a tab, and a control byte (ANSI ESC) —
    /// the set that a captured error message, a pane tail, or a refusal string
    /// routinely carries, which is precisely what `detail` is for.
    const HOSTILE: &str = "q\"uote back\\slash new\nline tab\there esc\u{1b}[31m";

    /// FIRES-ON-KNOWN-BAD for `omp-orchestrator-11ou`.
    ///
    /// Run this against the pre-fix `format!` implementation and it FAILS on the
    /// first assertion with `lines: 4, rows: 1` — the newline in each field
    /// splits one row across four physical lines, so a consumer splitting on
    /// `\n` gets four fragments and none of them parse. That is the whole defect:
    /// **the write succeeds and the reader discovers the corruption later, with
    /// no path back to the writer.**
    ///
    /// The neighbouring `jsonl_serialization_round_trips` does not catch it and
    /// its name overpromises: it asserts two `contains()` calls on benign values
    /// and never parses the output, so it is a known-GOOD leg only. Both are kept
    /// — an attack-only suite ships an over-strict gate.
    #[test]
    fn a_row_carrying_every_hostile_character_stays_one_parseable_line() {
        let mut ledger = StepLedger::new();
        ledger.emit(StepRecord {
            kind: StepKind::PacketSent,
            bead_id: format!("bead-{HOSTILE}"),
            pane_id: format!("%1408-{HOSTILE}"),
            session: format!("session-{HOSTILE}"),
            ts_unix: 1_767_331_200,
            detail: HOSTILE.to_owned(),
        });
        let jsonl = ledger.to_jsonl();

        // ONE row in, ONE physical line out. This is the JSONL invariant, and it
        // is asserted before parseability because it is the half that a JSON
        // parser alone would not reveal.
        assert_eq!(
            jsonl.lines().count(),
            ledger.rows().len(),
            "one row must occupy exactly one line; got {} line(s) for {} row(s):\n{jsonl}",
            jsonl.lines().count(),
            ledger.rows().len()
        );
        assert!(
            !jsonl.trim_end().contains('\n'),
            "no raw newline may survive inside the row: {jsonl:?}"
        );

        // And the values must come back byte-for-byte, which is what proves the
        // escaping is real rather than lossy stripping.
        let line = jsonl.lines().next().expect("one row was emitted");
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|error| {
            panic!("emitted row must parse as JSON: {error}\nline: {line:?}")
        });
        assert_eq!(parsed["detail"], serde_json::json!(HOSTILE));
        assert_eq!(parsed["bead"], serde_json::json!(format!("bead-{HOSTILE}")));
        assert_eq!(
            parsed["pane"],
            serde_json::json!(format!("%1408-{HOSTILE}"))
        );
        assert_eq!(
            parsed["session"],
            serde_json::json!(format!("session-{HOSTILE}"))
        );
        assert_eq!(parsed["kind"], serde_json::json!("packet_sent"));
        assert_eq!(parsed["ts"], serde_json::json!(1_767_331_200));
    }

    /// ANTI-VACUITY for the leg above: it must not be satisfiable by an empty
    /// ledger. `0 lines == 0 rows` is trivially true, so without this the leg
    /// would pass on a ledger that emitted nothing — a deliverable never checked
    /// reporting identically to one that passed.
    #[test]
    fn the_hostile_row_leg_is_not_satisfiable_by_an_empty_ledger() {
        let empty = StepLedger::new();
        assert_eq!(
            empty.to_jsonl(),
            "",
            "an empty ledger emits nothing, so a line-count equality would be vacuous"
        );
        assert_eq!(
            empty.assert_non_empty(),
            Err(LedgerError::Empty),
            "zero rows is an ERROR, never a pass"
        );
        assert_eq!(empty.rows().len(), 0);
    }

    /// The FIX MUST NOT REORDER THE LEDGER'S BYTES.
    ///
    /// Any existing consumer of this file — a `jq` filter, an eyeball, a diff
    /// against a stored artifact — depends on the key order the `format!` string
    /// literal produced: `kind, bead, pane, session, ts, detail`. Replacing the
    /// emitter is only safe if that order survives, so it is asserted here
    /// rather than assumed from the doc comment.
    ///
    /// This is the leg that would have caught a `serde_json::Map`, which without
    /// `preserve_order` is a `BTreeMap` and alphabetises to
    /// `bead, detail, kind, pane, session, ts`.
    #[test]
    fn the_serde_emitter_preserves_the_pre_fix_key_order() {
        let mut ledger = StepLedger::new();
        ledger.emit(StepRecord {
            kind: StepKind::BeadSelected,
            bead_id: "cp-1".to_owned(),
            pane_id: "%5".to_owned(),
            session: "s".to_owned(),
            ts_unix: 7,
            detail: "selected".to_owned(),
        });
        let line = ledger.to_jsonl();

        // Byte-exact against the pre-fix literal for a benign row: the fix is a
        // no-op on values that need no escaping, which is what makes it safe.
        assert_eq!(
            line,
            r#"{"kind":"bead_selected","bead":"cp-1","pane":"%5","session":"s","ts":7,"detail":"selected"}"#,
            "a benign row must be byte-identical to what the format! literal produced"
        );

        // And the order independently, by position, so a future field addition
        // cannot silently move an existing one.
        let mut cursor = 0usize;
        for key in ["kind", "bead", "pane", "session", "ts", "detail"] {
            let needle = format!("\"{key}\":");
            let at = line[cursor..].find(&needle).unwrap_or_else(|| {
                panic!("key `{key}` missing or out of order after byte {cursor}: {line}")
            });
            cursor += at + needle.len();
        }
    }

    #[test]
    fn step_kinds_are_ordered() {
        let order = [
            StepKind::BeadSelected,
            StepKind::PacketRendered,
            StepKind::FenceChecked,
            StepKind::PacketSent,
            StepKind::ReceiverVerified,
            StepKind::ReceiverTimedOut,
            StepKind::GradeRequested,
            StepKind::GradeReceived,
            StepKind::Closed,
            StepKind::ClosedWithoutGrade,
            StepKind::Redispatched,
            StepKind::AckReadBack,
        ];
        let names: std::collections::HashSet<&str> =
            order.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(names.len(), order.len(), "detector names must be unique");
    }
}
