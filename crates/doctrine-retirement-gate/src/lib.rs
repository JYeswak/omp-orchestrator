#![forbid(unsafe_code)]

//! Machine-distinguishable retracted doctrine for markdown — `omp-orchestrator-cq4fb`.
//!
//! # The defect this exists for
//!
//! A retraction must QUOTE the sentence it retires to be intelligible. Every substring match for
//! the retired sentence therefore also hits the retraction, and the reader concludes the false
//! claim is still live. That channel misled two agents in one evening — `ShimAudit` reported a
//! corrected claim as live, and pane 1 hit the identical trap twenty minutes earlier while
//! verifying its own commit. **Both parties knew the rule.** Care is demonstrably not the fix.
//!
//! # THE COUNT IS NOT A FIXED NUMBER, AND THAT IS THE DESIGN CONSTRAINT
//!
//! Measured 2026-09-10: the instance count INCREMENTED WHILE THIS CRATE WAS BEING WRITTEN.
//! Commit `fe7534c` — the commit that fixed the finding this crate was filed from — created a
//! third instance at `AGENTS.md:250-251`, because its correction had to quote the sentence it
//! was retiring. **Every correction manufactures a new instance.** So:
//!
//! - nothing here stores an instance count or a line number;
//! - nothing here stores a census of retired phrases, which would need re-deriving after every
//!   future correction and would refuse nothing in the meantime;
//! - the subjects are DERIVED FROM THE DOCUMENT at scan time, out of the markers themselves, and
//!   every leg is keyed on the raw-vs-effective DISAGREEMENT, which is count-independent.
//!
//! # The precedent, deliberately reused rather than reinvented
//!
//! `crates/gate-runner/tests/workflow_shape.rs` solved this shape for YAML. Its citation guards
//! first ran as raw `text.contains(…)` and BOTH went red on the step's own provenance comment,
//! which quotes the defective line. The remedy was `effective_yaml()` — compute the EFFECTIVE
//! text by removing the segments that merely QUOTE, and scan that — plus
//! `the_raw_and_effective_scans_disagree_on_this_file`, which proves WHICH scan is in force.
//!
//! **Carried over:** the two-scan split (`raw` vs `effective`), scanning the effective text only,
//! and the disagreement leg asserting raw-contains / effective-lacks / effective-shorter against
//! the REAL file rather than a fixture.
//!
//! **What markdown forced to change**, and none of it is cosmetic:
//!
//! 1. **No comment syntax with parser-level meaning.** YAML's `#` is understood by the consumer,
//!    so `effective_yaml` needs no cooperation from the author. Markdown has none, so retirement
//!    must be an EXPLICIT authored marker.
//! 2. **The retired text must stay VISIBLE.** Preserving the record of what was believed is the
//!    whole point — deleting the quote is the `FREEZE`/`AUTHORIZED` failure inverted — so the
//!    "hide the text" form is unavailable. The marker is an HTML-comment SPAN whose *delimiters*
//!    are invisible to a renderer while its *content* keeps rendering: the inverse of YAML,
//!    where the comment body is what disappears.
//! 3. **A span can be UNBALANCED; a `#` cannot.** An unclosed [`RETIRED_OPEN`] silently swallows
//!    the rest of the file, converting over-stripping into a green. New failure mode, typed
//!    [`Verdict::InstrumentError`], never a pass.
//! 4. **The subject list is derived, not enumerated.** `effective_yaml` needs no list because
//!    `#` is self-identifying; here the span IS the declaration, so [`retired_spans`] reads the
//!    subjects back out of the document instead of a const table.
//!
//! # Boundary — the exact limit of what this refuses
//!
//! It refuses a span that is empty, a span that is unbalanced, and a marked-dead string that
//! ALSO occurs unmarked elsewhere in the same document. It CANNOT discover a retraction nobody
//! marked and no marked span quotes verbatim: deciding that a sentence is a quotation of dead
//! doctrine is the judgement the marker exists to record. That judgement stays with the author;
//! this crate makes it machine-readable once made, and refuses every way of making it silently.

use std::fmt;

/// Opening delimiter of a retirement span.
///
/// An HTML comment because that is the only markdown construct invisible to a renderer and inert
/// to every consumer. Deliberately ugly and deliberately greppable: a marker a truncation could
/// plausibly produce by accident would not be evidence of intent.
pub const RETIRED_OPEN: &str = "<!--RETIRED-->";

/// Closing delimiter of a retirement span.
pub const RETIRED_CLOSE: &str = "<!--/RETIRED-->";

/// The documents in scope.
///
/// A SCOPE declaration, not a census: it names files, never instances, so a new retraction in a
/// listed file needs no edit here. Adding a file is the only maintenance this const ever needs.
pub const SCANNED_DOCUMENTS: &[&str] = &["AGENTS.md"];

/// Why one span was refused.
///
/// Two arms, not one, because the two failures have OPPOSITE remedies. Collapsing them would
/// reproduce this crate's own subject defect inside the crate: one channel, two populations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A string marked dead in one place occurs UNMARKED somewhere else in the same document, so
    /// a substring match still reports the retracted claim as live.
    UnmarkedOccurrence,
    /// A span with no content: the marker survived and the quotation inside it was deleted. That
    /// destroys the record of what was believed, which is the failure non-goal 10 forbids.
    RecordDestroyed,
}

impl Reason {
    /// The stable code a caller may match on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnmarkedOccurrence => "RETIREMENT_UNMARKED",
            Self::RecordDestroyed => "RETIREMENT_RECORD_DESTROYED",
        }
    }

    const fn remedy(self) -> &'static str {
        match self {
            Self::UnmarkedOccurrence => {
                "wrap the other occurrence too — do NOT delete the quote to silence this"
            }
            Self::RecordDestroyed => {
                "restore the quoted sentence verbatim inside the markers; an empty span records \
                 nothing and a deleted quote destroys what was believed"
            }
        }
    }
}

/// One refused span, named well enough to act on without opening the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    /// The span's content, bounded for the message — never the whole document.
    pub subject: String,
    pub reason: Reason,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} file={} subject={:?} remedy={}",
            self.reason.code(),
            self.file,
            elide(&self.subject),
            self.reason.remedy()
        )
    }
}

/// Bound a quoted subject so a refusal message stays readable.
fn elide(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 90 {
        return flat;
    }
    let head: String = flat.chars().take(87).collect();
    format!("{head}...")
}

/// A structurally broken retirement span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanError {
    /// An opener with no closer. Left unguarded this swallows the rest of the file, turning
    /// over-stripping — the direction that silently deletes LIVE doctrine — into a green.
    Unclosed { byte_offset: usize },
    /// A closer with no opener: text stopped being retired that never started.
    Unopened { byte_offset: usize },
}

impl fmt::Display for SpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unclosed { byte_offset } => write!(
                formatter,
                "RETIREMENT_SPAN_UNCLOSED byte={byte_offset} — `{RETIRED_OPEN}` with no \
                 `{RETIRED_CLOSE}`; an unclosed span strips the remainder of the file and would \
                 report a vacuous green"
            ),
            Self::Unopened { byte_offset } => write!(
                formatter,
                "RETIREMENT_SPAN_UNOPENED byte={byte_offset} — `{RETIRED_CLOSE}` with no opener"
            ),
        }
    }
}

/// Walk the spans once: `(content, byte_offset_of_content)` per span, in document order.
///
/// One traversal shared by [`effective_doctrine`] and [`retired_spans`], so the two can never
/// disagree about where a span starts — a second parser is how two views of one document drift.
///
/// # Errors
///
/// [`SpanError`] when the delimiters do not balance.
fn walk(text: &str) -> Result<(String, Vec<(String, usize)>), SpanError> {
    let mut effective = String::with_capacity(text.len());
    let mut spans = Vec::new();
    let mut rest = text;
    let mut consumed = 0usize;
    loop {
        let Some(open_at) = rest.find(RETIRED_OPEN) else {
            break;
        };
        // A closer BEFORE the next opener is unopened: check the segment we are about to keep.
        if let Some(stray) = rest[..open_at].find(RETIRED_CLOSE) {
            return Err(SpanError::Unopened {
                byte_offset: consumed + stray,
            });
        }
        effective.push_str(&rest[..open_at]);
        let after_open = open_at + RETIRED_OPEN.len();
        let Some(close_rel) = rest[after_open..].find(RETIRED_CLOSE) else {
            return Err(SpanError::Unclosed {
                byte_offset: consumed + open_at,
            });
        };
        let body = &rest[after_open..after_open + close_rel];
        // A nested opener is an unclosed opener by another name: the inner one never gets a
        // closer of its own, and silently treating it as content hides a typo'd delimiter.
        if body.contains(RETIRED_OPEN) {
            return Err(SpanError::Unclosed {
                byte_offset: consumed + after_open,
            });
        }
        spans.push((body.to_owned(), consumed + after_open));
        let after_close = after_open + close_rel + RETIRED_CLOSE.len();
        consumed += after_close;
        rest = &rest[after_close..];
    }
    if let Some(stray) = rest.find(RETIRED_CLOSE) {
        return Err(SpanError::Unopened {
            byte_offset: consumed + stray,
        });
    }
    effective.push_str(rest);
    Ok((effective, spans))
}

/// The EFFECTIVE doctrine: every retirement span removed, delimiters included.
///
/// The markdown analogue of `workflow_shape.rs`'s `effective_yaml()`, and the same one-line idea:
/// a scan must run over the text that is CLAIMED, never over the text that is merely QUOTED.
///
/// # Errors
///
/// [`SpanError`] when the delimiters do not balance. Returning a best-effort string here is what
/// would make an unclosed opener read as "clean".
pub fn effective_doctrine(text: &str) -> Result<String, SpanError> {
    walk(text).map(|(effective, _)| effective)
}

/// The retired subjects, READ BACK OUT OF THE DOCUMENT rather than declared.
///
/// This is what keeps every leg count-independent: a correction landed an hour from now adds its
/// own subject here with no edit to this crate.
///
/// # Errors
///
/// [`SpanError`] when the delimiters do not balance.
pub fn retired_spans(text: &str) -> Result<Vec<String>, SpanError> {
    walk(text).map(|(_, spans)| spans.into_iter().map(|(body, _)| body).collect())
}

/// How many retirement spans the text declares.
///
/// Counted on the RAW text, because the entire job of [`effective_doctrine`] is to make them
/// vanish — asking the effective text how many spans it had would always answer zero.
#[must_use]
pub fn span_count(text: &str) -> usize {
    text.matches(RETIRED_OPEN).count()
}

/// The gate's verdict over one scan.
///
/// Four arms and the 0/1/2/3 exit lattice from the crate atom's parts 2 and 3: a timeout or an
/// unreadable input is `Unrun`, never `Pass` and never `Refused`. An instrument that could not
/// look must never render as a subject that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every span is non-empty and every marked-dead string occurs only inside a span.
    Pass { spans: usize },
    /// A span is empty, or a marked-dead string occurs unmarked elsewhere.
    Refused { findings: Vec<Finding> },
    /// Nothing was scanned, or a scanned document declares no retirement at all.
    ///
    /// **An empty scan is an ERROR, never a pass.** A gate that reports clean because it looked
    /// at nothing is the vacuous green this repo has already paid for.
    Unrun { reason: String },
    /// The gate itself could not run correctly — unreadable input, unbalanced spans.
    InstrumentError { reason: String },
}

impl Verdict {
    /// 0 pass, 1 refuse, 2 unrun, 3 instrument error.
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Pass { .. } => 0,
            Self::Refused { .. } => 1,
            Self::Unrun { .. } => 2,
            Self::InstrumentError { .. } => 3,
        }
    }

    /// The closed status vocabulary for the JSON envelope.
    pub const fn status(&self) -> &'static str {
        match self {
            Self::Pass { .. } => "PASS",
            Self::Refused { .. } => "REFUSED",
            Self::Unrun { .. } => "UNRUN",
            Self::InstrumentError { .. } => "INSTRUMENT_ERROR",
        }
    }
}

/// One document handed to [`scan`]: its repo-relative name and its RAW text.
///
/// Reading is the caller's job so the kernel stays pure and testable without a filesystem — part
/// 1 of the crate atom, and the reason `state-wildcard-lint` splits the same way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub path: String,
    pub raw: String,
}

/// Scan documents for retirements that a substring match cannot distinguish from live doctrine.
///
/// # Anti-vacuity, in two shapes rather than one
///
/// 1. No documents → `Unrun`. Nothing was looked at.
/// 2. A scanned document with ZERO spans → `Unrun`. With no spans `effective == raw`, so every
///    comparison this gate makes is trivially satisfied and it would report clean forever. That
///    is precisely "clean because empty", and it is the state a marker-stripping edit produces.
#[must_use]
pub fn scan(documents: &[Document]) -> Verdict {
    if documents.is_empty() {
        return Verdict::Unrun {
            reason: "RETIREMENT_SCAN_EMPTY reason=no_documents — an empty scan set is an ERROR, \
                     never a pass"
                .to_owned(),
        };
    }

    let mut findings = Vec::new();
    let mut total_spans = 0usize;
    for document in documents {
        let (effective, spans) = match walk(&document.raw) {
            Ok(parsed) => parsed,
            Err(error) => {
                return Verdict::InstrumentError {
                    reason: format!("file={} {error}", document.path),
                }
            }
        };
        if spans.is_empty() {
            return Verdict::Unrun {
                reason: format!(
                    "RETIREMENT_SCAN_EMPTY file={} reason=no_retirement_spans — with no spans \
                     the raw and effective scans are identical, so this gate compares nothing \
                     and would report clean forever",
                    document.path
                ),
            };
        }
        total_spans += spans.len();
        for (body, _) in &spans {
            let subject = body.trim();
            if subject.is_empty() {
                findings.push(Finding {
                    file: document.path.clone(),
                    subject: body.clone(),
                    reason: Reason::RecordDestroyed,
                });
                continue;
            }
            // THE COUNT-INDEPENDENT ENFORCEMENT. Marking one occurrence declares the string
            // dead; any OTHER occurrence still reachable by a substring match is exactly the
            // defect. `AGENTS.md` had two `HARD-REFUSED` sites for this reason.
            if effective.contains(subject) {
                findings.push(Finding {
                    file: document.path.clone(),
                    subject: subject.to_owned(),
                    reason: Reason::UnmarkedOccurrence,
                });
            }
        }
    }

    if findings.is_empty() {
        Verdict::Pass { spans: total_spans }
    } else {
        Verdict::Refused { findings }
    }
}
