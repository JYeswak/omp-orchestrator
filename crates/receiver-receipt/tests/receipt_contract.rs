#![forbid(unsafe_code)]
//! THE CONTRACT VALIDATOR — `omp-orchestrator-receiver-receipt-contract-pwm`.
//!
//! # Why this replaced a `python3` heredoc
//!
//! `docs/contracts/receiver_receipt_contract.md` shipped its own pasteable
//! validation command as an inline `python3` script. That is a `.py` payload in a
//! repository whose **one rule** is *"No `.sh`. No `.py`."* — the extension gate
//! only sees files, so a heredoc slipped past it while being the same thing. Every
//! sibling contract that has a validator has a **Rust** one:
//! `crates/finding/tests/finding_contract.rs`,
//! `crates/pane-dispatch-ready/tests/readiness_contract.rs`.
//!
//! # Why it checks doc-to-SOURCE agreement and not only doc shape
//!
//! The old validator checked nine structural facts and **passed while the document
//! was wrong**. MEASURED 2026-09-02: the contract said the crate has *"six public
//! types"* and it has **seven** — `AckWaitVerdict` landed the same day in
//! `d4e8453`. A shape-only validator cannot see that, because the shape did not
//! change; only the world did.
//!
//! That is the drift class `NUMBERS.toml` exists for, one document over: *a figure
//! CORRECT WHEN WRITTEN and wrong now, because the repo moved under it.* So the
//! legs below assert claims the document makes **about the source**, and the source
//! is the authority.

use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// A section's body, sliced on a LINE-ANCHORED heading.
///
/// # Instance nine, and it was mine three times running
///
/// The first version used `text.find("## Non-Coverage")`, which matched **my own
/// sentence explaining that `## Non-Coverage` appears inside a code block** — the
/// prose landed five lines above the real heading, so the slicer read the
/// explanation instead of the section. The forbidden-token leg failed the same way:
/// it found `python3` inside the sentence saying the payload USED to be `python3`.
///
/// Same defect as `crates_emitting`, where a doc comment warning about a needle
/// contained the needle, and as the `path-literal-guard` refusal whose explanation
/// contained the forbidden literal. **The cure is structural, not verbal:** anchor
/// on `\n## <heading>\n` so only a real heading matches, and scope token checks to
/// the fenced block rather than the prose. AGENTS.md census item 5 states the
/// general rule — strip or structure before matching, never trust raw text.
fn section_body<'doc>(text: &'doc str, heading: &str) -> &'doc str {
    let anchor = format!("\n## {heading}\n");
    let start = text
        .find(&anchor)
        .unwrap_or_else(|| panic!("no line-anchored heading `## {heading}`"))
        + anchor.len();
    let end = text[start..]
        .find("\n## ")
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    &text[start..end]
}

/// The contents of the first fenced block inside a section — the pasteable command,
/// separated from the prose about it.
fn fenced_block(section: &str) -> &str {
    let start = section.find("```").expect("a fenced block");
    let after = &section[start + 3..];
    let body_start = after.find('\n').expect("a fence newline") + 1;
    let body = &after[body_start..];
    let end = body.find("```").expect("a closing fence");
    &body[..end]
}

fn contract() -> String {
    fs::read_to_string(repo_root().join("docs/contracts/receiver_receipt_contract.md"))
        .expect("the contract must exist — it is the artifact this suite validates")
}

fn production_source() -> String {
    fs::read_to_string(repo_root().join("crates/receiver-receipt/src/lib.rs"))
        .expect("the classifier source must exist")
}

/// ACCEPTANCE: <=25 KB, and every required section present.
#[test]
fn the_contract_has_every_required_section_within_the_size_bound() {
    let text = contract();
    let bytes = text.len();
    assert!(
        bytes <= 25_000,
        "contract is {bytes} bytes, over the 25 KB bound"
    );
    // ANTI-VACUITY: a truncated or empty file satisfies every "contains" check
    // below by containing nothing to contradict them — except that it cannot
    // contain the headings. Assert a floor anyway, because an empty file read as a
    // pass is the failure shape this repo keeps finding.
    assert!(
        bytes > 8_000,
        "contract is only {bytes} bytes: a stub cannot state five laws and a cross-check"
    );
    for section in [
        "\nBead: `omp-orchestrator-receiver-receipt-contract-pwm`",
        "\n## Purpose\n",
        "\n## Contract Artifacts\n",
        "\n## Laws\n",
        "\n## Cross-Check Against `pane_observation_contract`\n",
        "\n## Validation\n",
        "\n## Cross-References\n",
        "\n## Non-Coverage\n",
        "\n## NO-CLAIM\n",
    ] {
        assert!(
            text.contains(section),
            "missing required section: {section:?}"
        );
    }
    // The Artifacts section must name the invariant suite, not merely list files.
    assert!(
        text.contains("Invariant suite"),
        "Contract Artifacts must name the invariant suite"
    );
}

/// ACCEPTANCE: at least five stable IDs, and each must be a real one rather than an
/// accidental capitalised token.
#[test]
fn the_contract_declares_at_least_five_stable_ids() {
    let text = contract();
    let mut ids: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|token| token.starts_with("RR-") && token.len() > 4)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert!(
        ids.len() >= 5,
        "only {} stable RR- ids: {ids:?}",
        ids.len()
    );
    // The five LAWS must each be present by id, or "states L1-L5" is unmet.
    for law in [
        "RR-L1-TWO-CAPTURE",
        "RR-L2-FRESH-IDLE-TO-WORKING",
        "RR-L3-COMPOSER-ARRIVAL-NOT-DELIVERY",
        "RR-L4-ABSENCE-IS-UNKNOWN",
        "RR-L5-OBSCURED-UNPROVEN-INHABITED",
    ] {
        assert!(text.contains(law), "law {law} is not stated");
    }
}

/// ACCEPTANCE: cross-checks L1-L5 against `pane_observation_contract`.
///
/// The document previously referenced `PO-L1` only. All five PO laws must now be
/// named, and each must be named in the cross-check section rather than in passing.
#[test]
fn every_pane_observation_law_is_cross_checked() {
    let text = contract();
    let section = section_body(&text, "Cross-Check Against `pane_observation_contract`");
    for law in [
        "PO-L1-TWO-CAPTURE-DOMINANCE",
        "PO-L2-UNKNOWN-INHABITED",
        "PO-L3-LAST-LINE",
        "PO-L4-NO-CONTRADICTION",
        "PO-L5-DISPATCH-SEPARATE",
    ] {
        assert!(
            section.contains(law),
            "{law} is not cross-checked in the cross-check section"
        );
    }
    // And the counterpart document must actually declare those law ids, or the
    // cross-check cites laws that do not exist.
    let sibling = fs::read_to_string(repo_root().join("docs/contracts/pane_observation_contract.md"))
        .expect("the sibling contract must exist to be cross-checked");
    for law in [
        "PO-L1-TWO-CAPTURE-DOMINANCE",
        "PO-L2-UNKNOWN-INHABITED",
        "PO-L3-LAST-LINE",
        "PO-L4-NO-CONTRADICTION",
        "PO-L5-DISPATCH-SEPARATE",
    ] {
        assert!(
            sibling.contains(law),
            "{law} is cross-checked here but not declared in pane_observation_contract"
        );
    }
}

/// THE LEG THE OLD VALIDATOR LACKED: the document's claims about the source must be
/// TRUE.
///
/// It said "six public types" while seven exist. A shape-only validator passed,
/// because the shape had not changed — only the world had.
#[test]
fn the_documents_claims_about_the_source_are_current() {
    let text = contract();
    let src = production_source();

    // Every public enum the contract names must exist in the source.
    for ty in [
        "PanePresence",
        "PostSendObservation",
        "ReceiptReason",
        "ReceiptVerdict",
        "AckWaitVerdict",
        "ComposerEvidence",
        "NonDeliveryEscalation",
    ] {
        assert!(
            src.contains(&format!("pub enum {ty}")),
            "the contract names {ty} and the source has no such public enum"
        );
        assert!(
            text.contains(ty),
            "the source declares {ty} and the contract never names it — that is the \
             six-versus-seven drift recurring"
        );
    }

    // The COUNT stated in prose must match the source. This is the exact sentence
    // that was wrong.
    let declared = src.matches("\npub enum ").count();
    assert_eq!(
        declared, 7,
        "the source declares {declared} public enums; update the contract's count \
         and this assertion together, deliberately"
    );
    assert!(
        text.contains("seven public types"),
        "the contract must state the current count in prose; it said \"six\" while \
         seven existed"
    );

    // Constants the contract quotes by value.
    assert!(
        src.contains("OBSERVATION_WINDOW_MIN_SECS: u64 = 75"),
        "the 75-second floor moved; the contract quotes it"
    );
    assert!(
        src.contains("IDLE_TO_WORKING_TIMER_TOLERANCE_SECS: u64 = 30"),
        "the 30-second capture-jitter tolerance moved; the contract quotes it"
    );
    // The bound must stay SPAN-RELATIVE. An absolute comparison here was the defect:
    // widening RECEIPT_TIMEOUT from 30s to 90s made `timer_too_large_after_idle` fire on
    // panes that were working correctly (after_secs=32/31 vs max_secs=30, measured live
    // on %9 and %8 2026-09-05), because a pane that starts work immediately shows
    // timer_secs ~= elapsed_since_send. If this reverts to a constant comparison the
    // guard silently mis-fires again the next time a wait length changes.
    assert!(
        src.contains("span_secs.saturating_add(IDLE_TO_WORKING_TIMER_TOLERANCE_SECS)"),
        "the idle->working timer bound must be derived from the observed span, not absolute"
    );
    assert!(
        text.contains("75-second") && text.contains("30 seconds"),
        "the contract must quote both bounds so a change to either is caught here"
    );
}

/// The Cross-References section must point at paths that EXIST. A contract citing a
/// moved file is the close-evidence defect in a document.
#[test]
fn every_cited_source_path_exists() {
    let text = contract();
    let root = repo_root();
    let mut checked = 0usize;
    for token in text.split(['`', ' ', '\n']) {
        // Strip a trailing `:LINE` reference and any sentence punctuation.
        let candidate = token.split(':').next().unwrap_or(token).trim_end_matches(',');
        if !(candidate.starts_with("crates/") || candidate.starts_with("docs/")) {
            continue;
        }
        if !candidate.ends_with(".rs") && !candidate.ends_with(".md") {
            continue;
        }
        assert!(
            root.join(candidate).is_file(),
            "cited path does not exist: {candidate}"
        );
        checked += 1;
    }
    // ANTI-VACUITY, and it is the point: a path extractor that matches nothing
    // passes this test over zero paths, which is indistinguishable from every path
    // being valid.
    assert!(
        checked >= 8,
        "only {checked} cited paths were checked: the extractor is not matching"
    );
}

/// ACCEPTANCE: the pasteable Validation command must be THIS suite, not a foreign
/// interpreter. The repo's one rule forbids `.py`; a heredoc is the same payload.
#[test]
fn the_validation_command_is_this_rust_suite() {
    let text = contract();
    let section = section_body(&text, "Validation");
    // THE COMMAND, from the fenced block only. The surrounding prose legitimately
    // NAMES the interpreter it replaced, and a check that reads the prose fails on
    // the sentence describing the fix.
    let command = fenced_block(section);
    assert!(
        command.contains("cargo test -p receiver-receipt --test receipt_contract"),
        "the pasteable block must be this suite's own command, got: {command:?}"
    );
    for forbidden in ["python3", "python ", "#!/bin/sh", "bash <<"] {
        assert!(
            !command.contains(forbidden),
            "the pasteable block still carries a {forbidden:?} payload"
        );
    }
    // ANTI-VACUITY: an empty fenced block passes every `!contains` above.
    assert!(
        command.trim().lines().count() == 1,
        "the acceptance asks for ONE pasteable command, got {} lines",
        command.trim().lines().count()
    );
}

/// NON-COVERAGE and NO-CLAIM must be substantive, not headings over nothing. A
/// contract whose limits section is empty claims everything by omission.
#[test]
fn the_limits_sections_are_substantive() {
    let text = contract();
    for heading in ["Non-Coverage", "NO-CLAIM"] {
        let body = section_body(&text, heading).trim();
        assert!(
            body.len() > 300,
            "## {heading} body is only {} chars: a limits section that says nothing \
             claims everything",
            body.len()
        );
    }
    // The known implementation gaps must be NAMED, per the acceptance: identify
    // gaps without refactoring the crate.
    for gap in [
        "does not carry the 75-second interval",
        "can be called without a receipt object",
    ] {
        assert!(
            text.contains(gap),
            "the contract must name this measured implementation gap: {gap}"
        );
    }
}
