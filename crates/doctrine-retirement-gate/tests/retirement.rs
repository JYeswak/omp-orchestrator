//! `omp-orchestrator-cq4fb` — the five named legs, run against `AGENTS.md` itself.
//!
//! NAMED TARGET: cite as `cargo test -j 2 -p doctrine-retirement-gate --test retirement`.
//!
//! # Why these run on the real file and not on a fixture
//!
//! `workflow_shape.rs` established the rule this file follows: the disagreement leg must run on
//! the REAL document, because a fixture proves the FUNCTION works while saying nothing about
//! which scan is in force on the file everyone actually reads. A fixture-only suite is how a
//! gate ships green over a document it never looked at.
//!
//! # NO LEG HARD-CODES AN INSTANCE COUNT OR A LINE NUMBER
//!
//! Measured 2026-09-10: a third instance appeared in `AGENTS.md` mid-implementation, created by
//! `fe7534c` — the commit that fixed the finding this crate was filed from — because its
//! correction had to quote what it retired. Every correction manufactures a new instance, so any
//! leg asserting "2 hits at :195 and :204" is stale within the hour. Every leg below derives its
//! subjects from the document at run time and keys on the raw-vs-effective DISAGREEMENT, which
//! is count-independent by construction.

use doctrine_retirement_gate::{
    effective_doctrine, retired_spans, scan, span_count, Document, RetirementReason, SpanError, Verdict,
    RETIRED_CLOSE, RETIRED_OPEN, SCANNED_DOCUMENTS,
};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn agents_md() -> String {
    let path = repo_root().join("AGENTS.md");
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "RETIREMENT_FILE_UNREADABLE path={} detail={error} — an unreadable document is an \
             ERROR, never a pass",
            path.display()
        )
    })
}

fn live_documents() -> Vec<Document> {
    SCANNED_DOCUMENTS
        .iter()
        .map(|relative| {
            let path = repo_root().join(relative);
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("unreadable {relative}: {error}"));
            Document {
                path: (*relative).to_owned(),
                raw,
            }
        })
        .collect()
}

/// LEG 4 — THE KNOWN-BAD, AND THE WHOLE POINT OF THE CRATE.
///
/// For every retired subject the document itself declares:
/// RAW `AGENTS.md` CONTAINS it (the record of what was believed is intact);
/// EFFECTIVE does NOT (a substring match can now tell a retraction from its subject);
/// and EFFECTIVE IS SHORTER than raw (something was actually stripped).
///
/// All three on `AGENTS.md`, never a fixture. A leg that only checks a marker is present is an
/// ABSENCE PROOF: it establishes that someone typed a token, not that any scan honours it.
///
/// The subjects are READ OUT OF THE FILE, so this leg covers a retraction landed after it was
/// written without being edited.
#[test]
fn fires_on_known_bad() {
    let raw = agents_md();
    let subjects = retired_spans(&raw).expect("AGENTS.md retirement spans must balance");
    let effective = effective_doctrine(&raw).expect("AGENTS.md retirement spans must balance");

    assert!(
        !subjects.is_empty(),
        "AGENTS.md declares NO retirement spans, so this leg compares nothing. An empty scan is \
         an ERROR, never a pass — restore the markers, do not delete this leg"
    );

    for subject in &subjects {
        let trimmed = subject.trim();
        assert!(
            !trimmed.is_empty(),
            "RETIREMENT_RECORD_DESTROYED — a span in AGENTS.md is EMPTY. The marker survived and \
             the quotation inside it was deleted, which destroys the record of what was believed"
        );
        assert!(
            raw.contains(trimmed),
            "the raw text does not contain its own span body {trimmed:?} — the walker and the \
             document disagree, which is an instrument defect, not a finding"
        );
        assert!(
            !effective.contains(trimmed),
            "RETIREMENT_UNMARKED subject={trimmed:?} — present in the EFFECTIVE doctrine, so \
             every substring match still reports this retracted claim as live. That channel \
             misled two agents in one evening. Wrap the other occurrence too: \
             {RETIRED_OPEN}…{RETIRED_CLOSE}"
        );
    }

    assert!(
        effective.len() < raw.len(),
        "raw={} effective={} — the two scans AGREE on AGENTS.md, so nothing establishes which \
         one is in force. Either every marker was removed or the stripper is a no-op",
        raw.len(),
        effective.len()
    );
}

/// LEG 5 — THE KNOWN-GOOD, AND THE FAILURE DIRECTION THAT KILLS GATES QUIETLY.
///
/// Over-stripping deletes REAL doctrine from the scan, and an attack-only suite ships an
/// over-strict gate that gets routed around — a slower death than no gate at all.
///
/// Two live sentences, both at HEAD, both named here, and neither a line number:
///
/// 1. `THE POLICY IS ABSOLUTE: build on Contabo, never locally.` — the surviving RULING at the
///    top of the corrected section, explicitly what the retraction did NOT overturn.
/// 2. `THE SHIM DOES NOT STOP A BARE VERB` — the live half of the `### CORRECTED` heading, which
///    sits on the SAME LINE as a retirement span. It is the tightest available guard against a
///    span that swallows its own line: if the marker over-stripped, this sentence disappears.
#[test]
fn passes_known_good() {
    let raw = agents_md();
    let effective = effective_doctrine(&raw).expect("AGENTS.md retirement spans must balance");

    for live in [
        "THE POLICY IS ABSOLUTE: build on Contabo, never locally.",
        "THE SHIM DOES NOT STOP A BARE VERB",
    ] {
        assert!(
            raw.contains(live),
            "the live sentence {live:?} is no longer in AGENTS.md, so this leg proves nothing — \
             repoint it at a sentence that exists, do not delete it"
        );
        assert!(
            effective.contains(live),
            "OVER-STRIPPING: the live sentence {live:?} survives in raw but was removed from the \
             EFFECTIVE doctrine. A retirement span swallowed live text, which deletes real \
             doctrine from every scan that runs on the effective view"
        );
    }

    // And the gate as a whole is GREEN on the tree it ships with.
    let verdict = scan(&live_documents());
    assert_eq!(
        verdict.exit_code(),
        0,
        "the live tree must PASS: {}",
        detail(&verdict)
    );
    assert_eq!(verdict.status(), "PASS");
}

/// LEG 6 — AN EMPTY SCAN IS AN ERROR, NEVER A PASS.
///
/// Both vacuity shapes, each pinned on BOTH the message and the exit code per AGENTS.md gate
/// rule 7: a message-only assertion survives two causes collapsing into one code, and a
/// code-only assertion survives an unrelated breakage returning the same code.
#[test]
fn empty_scan_is_error() {
    // 1. No documents.
    let verdict = scan(&[]);
    assert_eq!(verdict.exit_code(), 2, "an empty document set must be UNRUN");
    assert_eq!(verdict.status(), "UNRUN");
    let Verdict::Unrun { reason } = &verdict else {
        panic!("expected Unrun, got {verdict:?}");
    };
    assert!(
        reason.contains("RETIREMENT_SCAN_EMPTY") && reason.contains("no_documents"),
        "the refusal must name its own cause: {reason}"
    );

    // 2. A document carrying ZERO retirement spans. With no spans `effective == raw`, so every
    //    comparison is trivially satisfied — this is the shape a marker-stripping edit produces,
    //    and reporting it clean is the vacuous green.
    let verdict = scan(&[Document {
        path: "AGENTS.md".to_owned(),
        raw: "a document with no markers at all, however much doctrine it carries".to_owned(),
    }]);
    assert_eq!(
        verdict.exit_code(),
        2,
        "a scanned document with no retirement spans must be UNRUN, not a clean pass"
    );
    assert_eq!(verdict.status(), "UNRUN");
    let Verdict::Unrun { reason } = &verdict else {
        panic!("expected Unrun, got {verdict:?}");
    };
    assert!(
        reason.contains("no_retirement_spans"),
        "the refusal must name its own cause: {reason}"
    );
}

/// LEG 8 — MUTATION: THE KNOWN-BAD LEG IS ATTRIBUTABLE TO THE STRIPPING STEP.
///
/// The two ways the step can die are modelled here, and `fires_on_known_bad`'s central assertion
/// is re-run against each. A mutation that fails to bite means the leg is not attributable and
/// proves nothing.
///
/// This is the IN-TEST half. The operational half — mutating `src/lib.rs`, watching the leg go
/// red on the lane, and restoring byte-identically to `git diff --numstat` = 0 — is recorded on
/// the bead, because a test cannot edit its own source and remain a test.
#[test]
fn mutation_goes_red() {
    let raw = agents_md();
    let honest = effective_doctrine(&raw).expect("spans must balance");
    let subjects = retired_spans(&raw).expect("spans must balance");
    assert!(!subjects.is_empty(), "nothing to mutate against");

    // MUTANT A — the stripping step DELETED: effective is raw.
    let mutant_deleted = raw.clone();
    // MUTANT B — the stripping step CORRUPTED to remove the delimiters and KEEP the quoted body.
    // This is the plausible wrong fix ("strip the markers so the file looks clean") and it is
    // precisely the mutation the crate must not survive.
    let mutant_corrupted = raw.replace(RETIRED_OPEN, "").replace(RETIRED_CLOSE, "");

    for subject in &subjects {
        let trimmed = subject.trim();
        assert!(
            !honest.contains(trimmed),
            "precondition: the honest stripper already fails on {trimmed:?}, so no mutation can \
             be attributed to anything"
        );
        assert!(
            mutant_deleted.contains(trimmed),
            "MUTANT A DID NOT BITE for {trimmed:?}: with stripping deleted the subject must \
             still be found, otherwise leg 4 would pass with no stripper at all"
        );
        assert!(
            mutant_corrupted.contains(trimmed),
            "MUTANT B DID NOT BITE for {trimmed:?}: with only the delimiters removed the quoted \
             body must still be found"
        );
    }
    assert_eq!(
        mutant_deleted.len(),
        raw.len(),
        "MUTANT A must be byte-identical to raw, or it is not the deletion mutant"
    );
    assert!(
        mutant_corrupted.len() < raw.len() && mutant_corrupted.len() > honest.len(),
        "MUTANT B must sit strictly between honest and raw: raw={} corrupted={} honest={}",
        raw.len(),
        mutant_corrupted.len(),
        honest.len()
    );
}

/// The claim this crate makes, and the exact strength it is entitled to.
#[test]
fn claim_header() {
    let raw = agents_md();
    let subjects = retired_spans(&raw).expect("spans must balance");

    // CLAIM: every retirement AGENTS.md declares is machine-distinguishable from live doctrine.
    // The bound is derived from the document, never typed in: `span_count` and the walker must
    // agree, which is the only count this leg asserts and it is a self-consistency check.
    assert_eq!(
        span_count(&raw),
        subjects.len(),
        "the delimiter count and the walked spans disagree, so the gate does not know how many \
         retirements this document declares"
    );
    assert!(
        !subjects.is_empty(),
        "CLAIM VOID: AGENTS.md declares no retirement spans"
    );

    // STRENGTH, stated as a limit rather than a boast.
    assert_eq!(
        SCANNED_DOCUMENTS,
        &["AGENTS.md"],
        "SCOPE: this claim covers AGENTS.md only. Every other markdown file in the repo is OUT \
         OF SCOPE and an unmarked retraction there is undetected — a declared limit, not an \
         oversight. Widening it means adding a file here, never a count anywhere"
    );
}

/// UNBALANCED SPANS ARE AN INSTRUMENT ERROR, NOT A VERDICT — and both halves are pinned.
///
/// This is the failure mode markdown adds that YAML's `#` cannot have. An unclosed opener strips
/// the remainder of the file, so left unguarded it converts over-stripping into a green.
#[test]
fn an_unbalanced_span_is_an_instrument_error_not_a_pass() {
    let unclosed = format!("live text {RETIRED_OPEN} dead text and no closer");
    let error = effective_doctrine(&unclosed).expect_err("an unclosed span must not be stripped");
    assert!(matches!(error, SpanError::Unclosed { .. }), "{error:?}");
    assert!(
        error.to_string().contains("RETIREMENT_SPAN_UNCLOSED"),
        "{error}"
    );

    let unopened = format!("live text {RETIRED_CLOSE} more live text");
    let error = effective_doctrine(&unopened).expect_err("a stray closer must be refused");
    assert!(matches!(error, SpanError::Unopened { .. }), "{error:?}");
    assert!(
        error.to_string().contains("RETIREMENT_SPAN_UNOPENED"),
        "{error}"
    );

    // A nested opener is an unclosed opener by another name.
    let nested = format!("a {RETIRED_OPEN}b{RETIRED_OPEN}c{RETIRED_CLOSE}d");
    assert!(matches!(
        effective_doctrine(&nested),
        Err(SpanError::Unclosed { .. })
    ));

    // And the gate renders it as INSTRUMENT_ERROR (exit 3), never REFUSED (1) or PASS (0):
    // an instrument that could not look must not render as a subject that failed.
    let verdict = scan(&[Document {
        path: "AGENTS.md".to_owned(),
        raw: unclosed,
    }]);
    assert_eq!(verdict.exit_code(), 3);
    assert_eq!(verdict.status(), "INSTRUMENT_ERROR");
    let Verdict::InstrumentError { reason } = &verdict else {
        panic!("expected InstrumentError, got {verdict:?}");
    };
    assert!(reason.contains("RETIREMENT_SPAN_UNCLOSED"), "{reason}");
}

/// THE TWO REFUSALS DO NOT SHARE A CODE — one channel, two populations, refused by construction.
///
/// Message AND exit code pinned on both, per AGENTS.md gate rule 7. `RecordDestroyed` and
/// `UnmarkedOccurrence` have OPPOSITE remedies (restore the quote / mark the other occurrence),
/// so a single collapsed code would reproduce this crate's own subject defect inside the crate.
#[test]
fn the_two_refusal_causes_are_distinguishable_by_code_and_by_message() {
    // Population 1: a string marked dead in one place, still reachable unmarked in another.
    let unmarked = scan(&[Document {
        path: "AGENTS.md".to_owned(),
        raw: format!(
            "the correction says {RETIRED_OPEN}LOCAL BUILDS ARE HARD-REFUSED{RETIRED_CLOSE} is \
             false, but a heading further down still says LOCAL BUILDS ARE HARD-REFUSED unmarked"
        ),
    }]);
    assert_eq!(
        unmarked.exit_code(),
        1,
        "an unmarked second occurrence must REFUSE: {}",
        detail(&unmarked)
    );
    assert_eq!(unmarked.status(), "REFUSED");
    let Verdict::Refused { findings } = &unmarked else {
        panic!("expected Refused, got {unmarked:?}");
    };
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].reason, RetirementReason::UnmarkedOccurrence);
    assert_eq!(findings[0].reason.code(), "RETIREMENT_UNMARKED");
    assert!(
        findings[0].to_string().contains("do NOT delete the quote"),
        "the remedy must forbid the tempting wrong fix: {}",
        findings[0]
    );

    // Population 2: the marker kept, the quotation inside it deleted. Same gate, same file,
    // different code — and the remedy is the opposite one.
    let destroyed = scan(&[Document {
        path: "AGENTS.md".to_owned(),
        raw: format!("a correction that deleted its own quote: {RETIRED_OPEN}{RETIRED_CLOSE}"),
    }]);
    assert_eq!(destroyed.exit_code(), 1);
    assert_eq!(destroyed.status(), "REFUSED");
    let Verdict::Refused { findings } = &destroyed else {
        panic!("expected Refused, got {destroyed:?}");
    };
    assert_eq!(findings[0].reason, RetirementReason::RecordDestroyed);
    assert_eq!(findings[0].reason.code(), "RETIREMENT_RECORD_DESTROYED");
    assert!(
        findings[0].to_string().contains("restore the quoted sentence"),
        "{}",
        findings[0]
    );
    assert_ne!(
        RetirementReason::RecordDestroyed.code(),
        RetirementReason::UnmarkedOccurrence.code(),
        "two causes sharing one code IS the defect this crate exists for"
    );
}

/// The stripper removes the span and nothing else — the byte-level contract.
#[test]
fn a_span_removes_exactly_itself() {
    let text = format!("keep A {RETIRED_OPEN}drop{RETIRED_CLOSE} keep B");
    assert_eq!(effective_doctrine(&text).unwrap(), "keep A  keep B");
    assert_eq!(retired_spans(&text).unwrap(), vec!["drop".to_owned()]);

    let multiline = format!("head\n{RETIRED_OPEN}one\ntwo{RETIRED_CLOSE}\ntail");
    assert_eq!(effective_doctrine(&multiline).unwrap(), "head\n\ntail");
    assert_eq!(retired_spans(&multiline).unwrap(), vec!["one\ntwo".to_owned()]);

    // No markers at all: effective == raw. The stripper must not be lossy on ordinary prose.
    let plain = "no markers here at all";
    assert_eq!(effective_doctrine(plain).unwrap(), plain);
    assert_eq!(span_count(plain), 0);
    assert!(retired_spans(plain).unwrap().is_empty());
}

fn detail(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Pass { spans } => format!("PASS spans={spans}"),
        Verdict::Refused { findings } => findings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" | "),
        Verdict::Unrun { reason } | Verdict::InstrumentError { reason } => reason.clone(),
    }
}
