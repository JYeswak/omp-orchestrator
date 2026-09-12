//! THE `--robot-agent-health` ADOPTION LEGS FOR THIS CRATE.
//!
//! The parse moved to `ntm-kernel` and this crate stopped keeping a private copy. What remains
//! here is the CONSUMER's contract: the census's three answers must stay three, the silence must
//! be carried rather than flattened, and the three traps this verb is known to set must be
//! provably not stepped in.
//!
//! Every payload below is the SHAPE measured live on session `omp-orchestrator`,
//! 2026-09-12T01:05-01:06Z, `--no-caut`, read-only.

use ntm_kernel::{classify, rate_limited_from_payload, RateLimitCensus};
use pane_dispatch_ready::RateLimitCoverage;

/// VERBATIM SHAPE of the not-found answer: exit 1, `success:false`, and — the trap — a populated
/// `panes:{}` beside a fully ZEROED `fleet_health`.
const NOT_FOUND: &[u8] = br#"{"success":false,"error":"pane selector \"99\" not found; available: 0 (%25), 1 (%33), 2 (%45), 3 (%26)","error_code":"PANE_NOT_FOUND","session":"omp-orchestrator","panes":{},"fleet_health":{"total_panes":0,"healthy_count":0,"warning_count":0,"critical_count":0,"avg_health_score":0,"overall_grade":""}}"#;

/// VERBATIM SHAPE of the healthy unsolicited answer. tmux enumerated FOUR panes in the same
/// minute; this names THREE and reports `total_panes: 3`. Pane `0` (a bare `zsh`) is ABSENT.
const HEALTHY_THREE: &str = r#"{"success":true,"session":"omp-orchestrator","panes":{
  "1":{"agent_type":"omp-claude","local_state":{"is_rate_limited":false,"safe_to_dispatch":false}},
  "2":{"agent_type":"omp-grok","local_state":{"is_rate_limited":true,"safe_to_dispatch":true}},
  "3":{"agent_type":"omp-muse","local_state":{"is_rate_limited":false,"safe_to_dispatch":true}}},
  "fleet_health":{"total_panes":3,"healthy_count":3,"overall_grade":"A"}}"#;

fn census_of(json: &str) -> RateLimitCensus {
    rate_limited_from_payload(&serde_json::from_str(json).expect("fixture parses"))
}

// ── TRAP 1: BRANCH ON EXIT STATUS FIRST ────────────────────────────────────────────────────
#[test]
fn rule_not_found_exit_is_refused_before_its_zeroed_payload_is_read() {
    // The bytes are a perfectly good JSON object carrying `panes:{}` and a zeroed fleet. A
    // consumer that parses before branching reads a zero-pane fleet in grade "" as measurement.
    let outcome = classify(false, Some(1), NOT_FOUND, b"");
    assert!(
        outcome.payload().is_none(),
        "RULE exit_first: a nonzero exit must expose NO payload, however parseable its bytes"
    );
    assert_eq!(
        outcome.state(),
        "REFUSED",
        "RULE exit_first: the not-found answer is a refusal, not an empty measurement"
    );
    // And the trap sprung: the same bytes, taken at face value, answer "nobody is limited".
    let flattened = rate_limited_from_payload(
        &serde_json::from_slice::<serde_json::Value>(NOT_FOUND).expect("fixture parses"),
    );
    assert_eq!(
        flattened,
        RateLimitCensus::Known(Default::default()),
        "RULE exit_first_matters: parsing the refusal's body yields a KNOWN-about-nobody census \
         — which is why the exit branch, not the body, decides"
    );
}

#[test]
fn rule_success_false_at_exit_zero_is_still_not_a_measurement() {
    // Defence in depth: if the verb ever exits 0 while saying `success:false`, the payload's own
    // envelope must still refuse. Neither check alone covers both shapes.
    let outcome = classify(true, Some(0), NOT_FOUND, b"");
    assert!(
        outcome.payload().is_none(),
        "RULE envelope_second: `payload.success != true` must refuse even at exit 0"
    );
    assert_eq!(outcome.state(), "UNPARSABLE");
}

// ── TRAP 2: NEVER KEY ON `error_code` ──────────────────────────────────────────────────────
#[test]
fn rule_two_spellings_of_one_condition_produce_one_outcome() {
    // `agent-health` says PANE_NOT_FOUND where `dialogs` says INVALID_FLAG for the same
    // condition. A consumer keyed on the spelling handles one verb and silently mishandles the
    // other.
    let pane_not_found = classify(false, Some(1), NOT_FOUND, b"");
    let invalid_flag = classify(
        false,
        Some(1),
        br#"{"success":false,"error_code":"INVALID_FLAG","panes":{}}"#,
        b"",
    );
    assert_eq!(
        pane_not_found.state(),
        invalid_flag.state(),
        "RULE no_error_code_key: two spellings of one condition must classify identically"
    );
    assert!(pane_not_found.payload().is_none() && invalid_flag.payload().is_none());

    // And the consumer must not have learned the spelling either.
    let src = include_str!("../src/main.rs");
    let code_reads = src
        .lines()
        .filter(|line| line.contains("error_code"))
        .filter(|line| !line.trim_start().starts_with("//") && !line.trim_start().starts_with("///"))
        .count();
    assert_eq!(
        code_reads, 0,
        "RULE no_error_code_key: this crate must never read `error_code` outside prose"
    );
}

// ── TRAP 3: NEVER DERIVE A PANE DENOMINATOR FROM `total_panes` ─────────────────────────────
#[test]
fn rule_pane_denominator_never_comes_from_the_agent_count() {
    let census = census_of(HEALTHY_THREE);
    let RateLimitCensus::Known(map) = &census else {
        panic!("RULE denominator: the healthy fixture must be a KNOWN census");
    };
    assert_eq!(
        map.len(),
        3,
        "RULE denominator: the census names three AGENT panes while tmux enumerated four"
    );
    // THE LIVE SPECIMEN: pane 0 is a bare `zsh` that the unsolicited call declines to describe.
    // The old `.unwrap_or(false)` read that silence as a measured "not rate-limited" and admitted
    // a shell as rate-limit-clear capacity.
    assert_eq!(
        RateLimitCoverage::of(census.is_rate_limited("0")),
        RateLimitCoverage::Unmeasured,
        "RULE denominator: a pane the census never named is UNMEASURED, never measured-free"
    );
    assert!(
        !RateLimitCoverage::of(census.is_rate_limited("0")).confirms_free(),
        "RULE denominator: an unnamed pane's FREE verdict is unconfirmed on the rate-limit axis"
    );

    let src = include_str!("../src/main.rs");
    let uses = src
        .lines()
        .filter(|line| line.contains("total_panes"))
        .filter(|line| !line.trim_start().starts_with("//") && !line.trim_start().starts_with("///"))
        .count();
    assert_eq!(
        uses, 0,
        "RULE denominator: `total_panes` is the AGENT count and must never reach executable code"
    );
}

// ── THE THREE-VALUED MATRIX, WHICH IS THE COVERAGE THAT DID NOT EXIST ──────────────────────
#[test]
fn rule_coverage_matrix_keeps_three_answers_three() {
    let census = census_of(HEALTHY_THREE);
    let matrix = [
        ("2", RateLimitCoverage::MeasuredLimited, true, false),
        ("1", RateLimitCoverage::MeasuredFree, false, true),
        ("0", RateLimitCoverage::Unmeasured, false, false),
    ];
    assert!(
        !matrix.is_empty(),
        "RULE coverage_non_vacuous: an empty matrix must never read as a pass"
    );
    for (pane, expected, refuses, confirms) in matrix {
        let got = RateLimitCoverage::of(census.is_rate_limited(pane));
        assert_eq!(
            got, expected,
            "RULE coverage_three_valued: pane {pane} is {expected:?}; collapsing any two of the \
             three answers is the defect this replaces"
        );
        assert_eq!(
            got.refusal_input(),
            refuses,
            "RULE coverage_refusal_input: only a MEASURED limit may drive the refusal, pane {pane}"
        );
        assert_eq!(
            got.confirms_free(),
            confirms,
            "RULE coverage_confirms_free: only a MEASURED free confirms the axis, pane {pane}"
        );
    }
}

// ── ANTI-VACUITY: KNOWN-ABOUT-NOBODY IS NOT UNKNOWN ────────────────────────────────────────
#[test]
fn rule_empty_scan_set_is_distinct_from_no_scan() {
    let answered_about_nobody = RateLimitCensus::Known(Default::default());
    let refused = RateLimitCensus::Unknown("agent-health REFUSED".to_owned());

    for census in [&answered_about_nobody, &refused] {
        assert_eq!(
            RateLimitCoverage::of(census.is_rate_limited("1")),
            RateLimitCoverage::Unmeasured,
            "RULE empty_scan_withheld: an empty scan set is UNMEASURED, never a pass"
        );
    }
    assert_ne!(
        answered_about_nobody.bound(),
        refused.bound(),
        "RULE bound_distinguishes: KNOWN-about-nobody and UNKNOWN must not render the same bound, \
         or the row loses the only surviving distinction between them"
    );
    assert!(
        answered_about_nobody.bound().contains("KNOWN panes=0"),
        "RULE bound_distinguishes: a zero-pane KNOWN census discloses as KNOWN, got {}",
        answered_about_nobody.bound()
    );
}

// ── THE DISCLOSURE MUST REACH THE OUTPUT, NOT JUST THE TYPE ────────────────────────────────
#[test]
fn rule_every_row_carries_its_coverage_and_bound() {
    let src = include_str!("../src/main.rs");
    assert!(
        src.contains(r#""rate_limit": coverage.as_str()"#),
        "RULE disclosure_on_row: every JSON row must name which census answer produced it"
    );
    assert!(
        src.contains(r#""rate_limit_bound": census.bound()"#),
        "RULE disclosure_on_row: every JSON row must name the coverage the census reported under"
    );
    assert!(
        src.contains("coverage.as_str()") && src.contains("{:<16}"),
        "RULE disclosure_on_row: the human table must disclose coverage too, not the JSON alone"
    );
    // And the flattening call this unit deleted must not come back — in EXECUTABLE CODE. The
    // prose above `rate_limit_census` names `.unwrap_or(false)` on purpose, because a deleted
    // defect that nobody can read about is a defect waiting to be reintroduced.
    let reflattened = src
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("unwrap_or(false)"))
        .count();
    assert_eq!(
        reflattened, 0,
        "RULE no_reflattening: `.unwrap_or(false)` on a census answer is the defect itself"
    );
}
