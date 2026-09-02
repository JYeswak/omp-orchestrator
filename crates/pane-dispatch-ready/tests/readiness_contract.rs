//! INVARIANT SUITE for docs/contracts/pane_readiness_contract.md — laws PR-L1 … PR-L5.
//!
//! # Current enforcement state
//!
//! PR-L1 and PR-L3 were originally pinned as unenforced laws. The current source now enforces
//! the named wedge distinction through the existing tick-monitor authority and enforces the
//! canonical 75-second two-capture floor through PaneObservation. The suite retains the
//! behavior, authority, and state-registry legs so those fixes cannot regress silently.
//!
//! # No live `ntm` dependency
//!
//! `PR-L2` and `PR-L4` are about a foreign classifier's output. A test that shells `ntm`
//! would be a flake and would measure whatever the fleet happens to be doing. The snapshot
//! below is a VERBATIM `ntm --robot-activity` payload, dated and cited, so the leg tests the
//! RULE against a fixed observation instead of testing the fleet.

use omp_types::{CaptureSnapshot, PaneLiveness};
use pane_dispatch_ready::{
    classify, confirm_free, PaneDispatchReadyRules, PaneDispatchReadyState, TWO_CAPTURE_MIN_SECS,
};
use std::path::{Path, PathBuf};

fn rules() -> PaneDispatchReadyRules {
    PaneDispatchReadyRules::default()
}
fn snapshot(at_secs: u64, timer: &str, hash: &str) -> CaptureSnapshot {
    CaptureSnapshot::new(
        at_secs,
        PaneLiveness::Idle,
        Some(timer.to_owned()),
        hash.to_owned(),
    )
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live beneath the workspace root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} must be readable: {e}", p.display()))
}

/// An OMP v18 pane that is idle by every surface this crate can see: an agent is rendering,
/// no busy marker, and the `π` idle prompt on the last status line.
const IDLE_CLAUDE: &str = "\
some earlier output
tool call finished
 π  > claude-opus-5 > 📁 ~/Developer/omp-orchestrator > ⑂ main
";

/// The SAME pane, wedged: a packet arrived, was parked unsubmitted, and the composer footer
/// says so. Nothing else about the capture changed — which is the whole problem.
const WEDGED_CLAUDE: &str = "\
some earlier output
tool call finished
Press up to edit queued messages
 π  > claude-opus-5 > 📁 ~/Developer/omp-orchestrator > ⑂ main
";

/// A bare login shell. No agent marker anywhere, and a perfectly good prompt.
const BARE_SHELL: &str = "\
$ ls -1
Cargo.toml
crates
$ ";

/// VERBATIM `ntm --robot-activity=omp-orchestrator` payload, captured 2026-09-02T03:28:05Z.
/// Five panes, one snapshot, one `captured_at`. Trimmed to the fields under test; the
/// `agents[]` rows are byte-for-byte as emitted.
const NTM_SNAPSHOT: &str = r#"{
  "success": true,
  "session": "omp-orchestrator",
  "captured_at": "2026-09-02T03:28:05Z",
  "agents": [
    {"pane":"1","agent_type":"claude","state":"THINKING","confidence":0.8,
     "observation_state":"idle","observation_confidence":0.95,"safe_to_dispatch":true},
    {"pane":"2","agent_type":"codex","state":"ERROR","confidence":0.95,
     "observation_state":"working","observation_confidence":0.95,"safe_to_dispatch":false},
    {"pane":"3","agent_type":"codex","state":"UNKNOWN","confidence":0.5,
     "observation_state":"working","observation_confidence":0.95,"safe_to_dispatch":false},
    {"pane":"4","agent_type":"omp-glm","state":"THINKING","confidence":0.8,
     "observation_state":"working","observation_confidence":0.95,"safe_to_dispatch":false},
    {"pane":"5","agent_type":"omp-glm","state":"THINKING","confidence":0.8,
     "observation_state":"working","observation_confidence":0.95,"safe_to_dispatch":false}
  ]
}"#;

/// One `agents[]` row, parsed without a serde dependency: this crate ships `serde_json` as a
/// normal dep, so the test crate cannot see it. Hand-parsing five flat rows is cheaper than
/// adding a dev-dep, and the shape is fixed by the fixture above.
struct Row {
    pane: String,
    agent_type: String,
    state: String,
    confidence: f64,
    observation_state: String,
    observation_confidence: f64,
    safe_to_dispatch: bool,
}

fn field<'a>(row: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":");
    let i = row
        .find(&needle)
        .unwrap_or_else(|| panic!("fixture row is missing {key}: {row}"));
    let rest = row[i + needle.len()..].trim_start();
    if let Some(q) = rest.strip_prefix('"') {
        let end = q.find('"').expect("unterminated string in fixture");
        &q[..end]
    } else {
        let end = rest
            .find([',', '}', '\n'])
            .expect("unterminated scalar in fixture");
        rest[..end].trim()
    }
}

fn snapshot_rows() -> Vec<Row> {
    let start = NTM_SNAPSHOT.find("\"agents\"").expect("agents key");
    let body = &NTM_SNAPSHOT[start..];
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(open) = rest.find("{\"pane\"") {
        let tail = &rest[open..];
        let close = tail.find('}').expect("unterminated fixture row");
        let row = &tail[..=close];
        out.push(Row {
            pane: field(row, "pane").to_owned(),
            agent_type: field(row, "agent_type").to_owned(),
            state: field(row, "state").to_owned(),
            confidence: field(row, "confidence").parse().expect("confidence"),
            observation_state: field(row, "observation_state").to_owned(),
            observation_confidence: field(row, "observation_confidence")
                .parse()
                .expect("observation_confidence"),
            safe_to_dispatch: field(row, "safe_to_dispatch") == "true",
        });
        rest = &tail[close..];
    }
    // ANTI-VACUITY: a parser that silently returns nothing reports identically to a snapshot
    // in which every law holds.
    assert_eq!(
        out.len(),
        5,
        "the fixture must parse to 5 rows; an empty or short parse is an ERROR, not a pass"
    );
    out
}

// ---------------------------------------------------------------------------------------
// PR-L1 — safe_to_dispatch is not liveness
// ---------------------------------------------------------------------------------------

#[test]
fn l1_a_wedged_pane_is_distinguishable_from_an_idle_one() {
    // WAS a pinned defect (bead omp-orchestrator-readiness-l1-wedge-blind-46y7): a wedged
    // pane and an idle pane produced a BYTE-IDENTICAL verdict line, so a caller that
    // trusted readiness dispatched into a pane that had accepted a packet and would never
    // submit it. Now the two are separable, and the leg asserts the separation rather than
    // the blindness.
    let idle = classify(IDLE_CLAUDE, false, &rules());
    let wedged = classify(WEDGED_CLAUDE, false, &rules());
    assert_eq!(
        wedged.state,
        PaneDispatchReadyState::Wedged,
        "a parked packet must not read as dispatchable, got {:?}",
        wedged.state
    );
    // KNOWN-GOOD ARM, and the more important half: a detector that pins every pane non-free
    // is a worse defect than the blindness it replaces.
    assert_eq!(
        idle.state,
        PaneDispatchReadyState::Free,
        "control: an idle pane must still be FREE, got {:?}",
        idle.state
    );
    assert_ne!(
        idle.pipe_line(),
        wedged.pipe_line(),
        "the two verdicts must now differ on the wire, not only in the enum"
    );
    // The reason must name the OPERATOR ACTION. `Busy` says "come back later"; this
    // condition never clears without a human, so a caller reading only the reason string
    // still learns the difference.
    assert!(
        wedged.reason.contains("submit or clear"),
        "the wedged reason must name the operator action, got {:?}",
        wedged.reason
    );
}

#[test]
fn l1_the_wedge_authority_is_consulted_not_reimplemented() {
    // The detection already existed THREE times over when this crate was blind, so the fix
    // was a dependency edge, not a fourth regex. That makes the previous pin — a source-text
    // search for the marker string in this crate — unable to signal: delegation leaves the
    // string absent. This leg is keyed on BEHAVIOUR and on the dependency instead.
    const MARKER: &str = "Press up to edit queued messages";

    // 1. The authority still recognises it. If this fails the delegation is broken upstream,
    //    which is a different failure from this crate regressing.
    assert_eq!(
        tick_monitor::classify(WEDGED_CLAUDE),
        tick_monitor::PaneState::Wedged,
        "the consulted authority no longer recognises the parked-packet footer"
    );

    // 2. This crate AGREES with it, and does so without carrying its own copy of the marker.
    assert_eq!(
        classify(WEDGED_CLAUDE, false, &rules()).state,
        PaneDispatchReadyState::Wedged
    );
    let own = read("crates/pane-dispatch-ready/src/lib.rs");
    assert!(
        !own.contains(&format!("\"{MARKER}\"")),
        "a fourth copy of the marker landed here; consult the authority instead"
    );
    assert!(
        own.contains("tick_monitor::PaneState::Wedged"),
        "the delegation edge is gone — this crate must consult the authority, not guess"
    );

    // 3. The SECOND footer, which the single anchor in the bead would have missed. Reuse is
    //    strictly richer than the regex that was proposed: this string appears nowhere in
    //    this crate and is recognised anyway.
    const SECOND: &str = "Messages to be submitted after next tool call";
    let second = WEDGED_CLAUDE.replace(MARKER, SECOND);
    assert_eq!(
        classify(&second, false, &rules()).state,
        PaneDispatchReadyState::Wedged,
        "the authority recognises two parked-packet footers; delegation must inherit both"
    );

    // POSITIVE CONTROL on the same reader: a marker this crate DOES carry, so the negative
    // assertion above is not the reader silently failing.
    assert!(
        own.contains("esc to interrupt"),
        "POSITIVE CONTROL FAILED: the reader cannot see a busy marker that is present, so its \
         absence result above proves nothing"
    );
}

#[test]
fn l1_state_registry_round_trips_and_covers_every_state_the_classifier_emits() {
    // `ALL` is hand-listed and `as_str`/`parse` are matches, so a new variant is a compile
    // error in `as_str` but could silently miss `parse` or `ALL`. Both halves are checked:
    // the wire round-trip, and that every state the classifier can actually PRODUCE is
    // registered — derived from behaviour, not from a pinned count.
    for state in PaneDispatchReadyState::ALL {
        assert_eq!(
            PaneDispatchReadyState::parse(state.as_str()),
            Some(*state),
            "{} does not survive the wire round-trip",
            state.as_str()
        );
    }
    assert_eq!(PaneDispatchReadyState::parse("NOT_A_STATE"), None);

    let corpus = [
        ("", false),
        ("bare shell, no agent\n$ ", false),
        (IDLE_CLAUDE, false),
        (IDLE_CLAUDE, true),
        (WEDGED_CLAUDE, false),
        ("π claude-opus-5\nWorking (12s) esc to interrupt\n", false),
        ("π claude-opus-5\nWeekly limit left: 0%\n", false),
    ];
    // NO `assert!(!corpus.is_empty())` HERE. Clippy caught that exact line as
    // "this expression always evaluates to false": `corpus` is a fixed-size array literal,
    // so emptiness is decided at compile time and the guard can NEVER fire. An anti-vacuity
    // check that is itself vacuous is worse than none — it reads as protection. The real
    // guard is the distinct-state count at the end of this test, which is derived from
    // classifier BEHAVIOUR and does fire (proven by mutation M2, which collapsed every row
    // to WEDGED and turned this leg RED).
    for (text, changed) in corpus {
        let got = classify(text, changed, &rules()).state;
        assert!(
            PaneDispatchReadyState::ALL.contains(&got),
            "classify emitted {got:?}, which is absent from PaneDispatchReadyState::ALL — the \
             registry and the wire round-trip cannot see it"
        );
    }
    // And the corpus is not vacuous in the other direction: it must exercise more than one
    // state, or "every emitted state is registered" is satisfied by a single row.
    let distinct: std::collections::BTreeSet<&str> = corpus
        .iter()
        .map(|(t, c)| classify(t, *c, &rules()).state.as_str())
        .collect();
    assert!(
        distinct.len() >= 5,
        "the corpus must exercise most of the registry, saw {distinct:?}"
    );
}

// ---------------------------------------------------------------------------------------
// PR-L2 — UNKNOWN is not a busy claim, and safe_to_dispatch does not come from `state`
// ---------------------------------------------------------------------------------------

#[test]
fn l2_a_codex_pane_can_be_unclassifiable_in_the_state_field() {
    let rows = snapshot_rows();
    let unknown: Vec<&Row> = rows.iter().filter(|r| r.state == "UNKNOWN").collect();
    assert_eq!(
        unknown.len(),
        1,
        "the snapshot carries exactly one UNKNOWN row"
    );
    let r = unknown[0];
    assert_eq!(
        r.agent_type, "codex",
        "the unclassifiable pane is a codex pane"
    );
    assert_eq!(r.pane, "3");
    assert!(
        (r.confidence - 0.5).abs() < f64::EPSILON,
        "an UNKNOWN state carries confidence 0.5 — a coin flip, not a claim; got {}",
        r.confidence
    );
    // Both codex panes disagree with each other in `state` while agreeing in
    // `observation_state`: pane 2 reads ERROR at 0.95, pane 3 UNKNOWN at 0.5, both
    // observation_state=working at 0.95. `state` is the unreliable channel.
    let codex: Vec<&Row> = rows.iter().filter(|r| r.agent_type == "codex").collect();
    assert_eq!(codex.len(), 2);
    assert_ne!(
        codex[0].state, codex[1].state,
        "the two codex panes must differ in `state` — that is the field's unreliability"
    );
    assert_eq!(
        codex[0].observation_state, codex[1].observation_state,
        "…while agreeing in `observation_state`, the field a caller must gate on"
    );
}

#[test]
fn l2_safe_to_dispatch_tracks_observation_state_not_state() {
    // A CORRECTION recorded as a test. The standing account says ntm "DERIVES
    // safe_to_dispatch:false from that non-answer". Measured, it does not: pane 1 reads
    // state=THINKING — a busy-sounding word — with observation_state=idle at 0.95, and
    // safe_to_dispatch=TRUE. The derivation follows `observation_state`, never `state`.
    for r in snapshot_rows() {
        let expected = r.observation_state == "idle";
        assert_eq!(
            r.safe_to_dispatch, expected,
            "pane {}: safe_to_dispatch={} but observation_state={} — the derivation is not \
             what this law claims; re-measure before editing the contract",
            r.pane, r.safe_to_dispatch, r.observation_state
        );
        assert!(
            (r.observation_confidence - 0.95).abs() < f64::EPSILON,
            "pane {}: the observation channel must be the CONFIDENT one; got {}",
            r.pane,
            r.observation_confidence
        );
    }
    // The counter-example, named explicitly so a reader cannot miss it.
    let rows = snapshot_rows();
    let p1 = rows.iter().find(|r| r.pane == "1").expect("pane 1");
    assert_eq!(p1.state, "THINKING");
    assert!(
        p1.safe_to_dispatch,
        "pane 1 is the counter-example: a busy-sounding `state` with a dispatchable verdict"
    );
}

/// The two `robot-tail` status lines for pane 1, verbatim, ~95 seconds apart. Both captures
/// carry a braille spinner and an elapsed timer — the v18 WORKING signature — and the timer
/// ADVANCED between them, which is the strongest evidence grade `pane_observation_contract`
/// `PO-L1` defines. `ntm` reported `observation_state: "idle"` at `observation_confidence:
/// 0.95` for this pane at BOTH timestamps.
const PANE1_CAPTURE_A: &str =
    " ⠏ 26m  · ◕ Opus 5 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *8 ?5 · ◫ 77.4%/1M ⟲ · S1060.49";
const PANE1_CAPTURE_B: &str =
    " ⠼ 28m  · ◕ Opus 5 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *9 ?5 · ◫ 77.9%/1M ⟲ · S1";
/// What `ntm` said about that same pane, at both captures.
const PANE1_NTM_OBSERVATION_STATE: &str = "idle";
const PANE1_NTM_OBSERVATION_CONFIDENCE: f64 = 0.95;

#[test]
fn l2_the_observation_channel_was_confidently_wrong_about_a_working_pane() {
    // MEASURED 2026-09-02T03:42-03:44Z, six minutes AFTER this contract landed, while
    // reporting dispatch results to pane 1. It refutes the conclusion the contract had just
    // published — "gate on observation_state" — and is recorded rather than buried.
    //
    // Bead omp-orchestrator-observation-state-false-idle-riqd.
    //
    // A spinner plus an elapsed timer is the v18 WORKING signature (AGENTS.md). Both captures
    // carry one, the timer advanced 26m -> 28m across ~95s, and the dirty-file count moved
    // 8 -> 9. That is two-capture motion above the 75-second floor: WORKING is PROVEN, not
    // inferred. `ntm` said idle at 0.95 on both.
    fn has_spinner(line: &str) -> bool {
        line.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
    }
    fn minutes(line: &str) -> u64 {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i].is_ascii_digit() {
                let s = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'm' {
                    return line[s..i].parse().expect("elapsed minutes");
                }
            } else {
                i += 1;
            }
        }
        panic!("no elapsed timer in {line}");
    }

    for (label, line) in [("A", PANE1_CAPTURE_A), ("B", PANE1_CAPTURE_B)] {
        assert!(
            has_spinner(line),
            "capture {label} must carry the braille spinner that makes WORKING readable"
        );
    }
    let (a, b) = (minutes(PANE1_CAPTURE_A), minutes(PANE1_CAPTURE_B));
    assert!(
        b > a,
        "the elapsed timer must ADVANCE between captures for motion to be proven: {a}m -> {b}m"
    );
    assert_ne!(
        PANE1_CAPTURE_A, PANE1_CAPTURE_B,
        "the spinner-stripped content must differ, per PO-L1's second clause"
    );

    // The contradiction, asserted rather than described.
    assert_eq!(
        PANE1_NTM_OBSERVATION_STATE, "idle",
        "the fixture records what ntm actually said; do not soften it"
    );
    assert!(
        (PANE1_NTM_OBSERVATION_CONFIDENCE - 0.95).abs() < f64::EPSILON,
        "and it said so at 0.95 — a confident wrong answer, not a hedge"
    );

    // POSITIVE CONTROL on the same readers: an idle v18 status line has NO spinner and no
    // elapsed timer, so `has_spinner` and `minutes` are not answering true for everything.
    let idle_line = " π  > claude-opus-5 > 📁 ~/Developer/omp-orchestrator > ⑂ main";
    assert!(
        !has_spinner(idle_line),
        "POSITIVE CONTROL FAILED: the spinner reader fires on an idle line, so its result \
         above proves nothing"
    );
}

/// SECOND REPRODUCTION, 2026-09-02T23:38:21Z / 23:41:59Z — a 218-second gap on the same
/// pane, twenty hours after the first. Recorded because the bead's acceptance 1 asked
/// whether the defect re-triggers: it does, on the first attempt.
const REPRO_CAPTURE_A: &str =
    " ⠋ 17m  · ◕ Fable 5.1 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *144 ?12 · ◫ 83.0%/1M";
const REPRO_CAPTURE_B: &str =
    " ⠼ 20m  · ◕ Fable 5.1 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *133 ?11 · ◫ 83.2%/1M";
const REPRO_GAP_SECS: u64 = 218;
/// `capture_provenance` and `capture_collected_at` from the SAME activity payload that
/// said idle, beside the tail's own `captured_at`. Equal to the second.
const REPRO_PROVENANCE: &str = "live";
const REPRO_COLLECTED_AT: &str = "2026-09-02T23:41:59Z";
const REPRO_TAIL_CAPTURED_AT: &str = "2026-09-02T23:41:59Z";
/// `detected_patterns` for the false-idle row, verbatim. This is the decisive field.
const REPRO_DETECTED_PATTERNS: &[&str] = &["failed_text", "claude_unicode_prompt", "braille_spinner"];

#[test]
fn l2_the_false_idle_is_a_precedence_defect_upstream_not_staleness_here() {
    // ACCEPTANCE 2 of omp-orchestrator-observation-state-false-idle-riqd asked which side is
    // broken: (a) ntm misreads the v18 spinner+timer footer, or (b) it derives from a stale
    // or differently-scoped capture. The payload answers, and it is neither exactly.
    //
    // NOT (b): the observation row's own `capture_provenance` is "live" and its
    // `capture_collected_at` equals the tail's `captured_at` to the second. Same capture,
    // same instant, no staleness to blame.
    assert_eq!(REPRO_PROVENANCE, "live", "staleness is refuted by the payload itself");
    assert_eq!(
        REPRO_COLLECTED_AT, REPRO_TAIL_CAPTURED_AT,
        "the observation and the tail read the same capture at the same second"
    );

    // NOT quite (a) either — and this is the sharper result. `braille_spinner` IS in
    // `detected_patterns` for the very row that reads idle. The spinner was DETECTED and an
    // idle-side signal outranked it. That makes it a PRECEDENCE defect, not a missed read,
    // which is a different upstream report.
    assert!(
        REPRO_DETECTED_PATTERNS.contains(&"braille_spinner"),
        "the decisive evidence: the spinner was detected on the row that reads idle"
    );

    // And the pane really was working, at the same grade as the first measurement.
    //
    // Clippy flagged the runtime form as "this assertion has a constant value", which is
    // correct and is an argument for STRENGTHENING rather than silencing: both sides are
    // consts, so the check belongs at build time. If someone edits the recorded gap below
    // the floor, or raises the floor above the recorded gap, the evidence and the law now
    // contradict each other as a COMPILE ERROR instead of a test failure nobody may run.
    const _: () = assert!(REPRO_GAP_SECS >= TWO_CAPTURE_MIN_SECS);
    let mins = |line: &str| -> u64 {
        let s = line.split('m').next().expect("timer");
        s.trim_start_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .expect("elapsed minutes")
    };
    assert!(
        mins(REPRO_CAPTURE_B) > mins(REPRO_CAPTURE_A),
        "the timer must advance: {}m -> {}m",
        mins(REPRO_CAPTURE_A),
        mins(REPRO_CAPTURE_B)
    );
    assert_ne!(REPRO_CAPTURE_A, REPRO_CAPTURE_B, "and the content must differ");

    // POSITIVE CONTROL on the pattern reader: it must not answer true for everything, or
    // the assertion above is satisfied by a broken `contains`.
    assert!(
        !REPRO_DETECTED_PATTERNS.contains(&"codex_chevron_prompt"),
        "POSITIVE CONTROL FAILED: the pattern reader matches a pattern that is absent"
    );
}

/// The BOUND on the defect, from a 22-row sample across five live sessions at
/// 2026-09-02T23:39Z. `braille_spinner` present implied `observation_state == "working"` on
/// **12 of 13** rows carrying it; `omp-orchestrator` pane 1 was the only violation. Three
/// candidate triggers were refuted by rows in the same sample, and the surviving candidate
/// has n=1 and is NOT claimed. Each row is `(session, agent_type, observation_state,
/// detected_patterns)`.
const WIDE_SAMPLE: &[(&str, &str, &str, &[&str])] = &[
    ("omp-orchestrator", "claude", "idle", &["failed_text", "claude_unicode_prompt", "braille_spinner"]),
    ("omp-orchestrator", "codex", "working", &["braille_spinner"]),
    ("omp-orchestrator", "omp-glm", "working", &["braille_spinner"]),
    ("clutterfreespaces", "claude", "idle", &["claude_spinner_past"]),
    ("clutterfreespaces", "omp", "working", &["braille_spinner"]),
    ("clutterfreespaces", "omp", "working", &["api_error", "failed_text", "braille_spinner"]),
    ("control-plane", "omp-claude", "idle", &["sigkill"]),
    ("control-plane", "omp-claude", "working", &["braille_spinner"]),
    ("franken-harvest", "codex", "idle", &["failed_text", "codex_chevron_prompt"]),
    ("franken-harvest", "grok", "idle", &["grok_composer_prompt"]),
    ("zeststream-cast", "claude", "working", &["claude_spinner_timing", "claude_unicode_prompt", "claude_spinner_past"]),
    ("zeststream-cast", "codex", "idle", &["generic_angle"]),
    ("zeststream-cast", "codex", "working", &["braille_spinner"]),
];

#[test]
fn l2_three_candidate_triggers_for_the_false_idle_are_refuted_by_the_wider_sample() {
    // The bead's step 1 asked for the conditions under which the defect does NOT occur. Four
    // features were perfectly correlated with the single idle row in the five-pane session
    // sample, so none could be blamed from it. Widening to five sessions refutes three.
    fn rows_with(pat: &'static str) -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str, &'static [&'static str])> {
        WIDE_SAMPLE.iter().filter(move |r| r.3.contains(&pat))
    }

    // REFUTED: "agent_type claude implies idle" — zeststream-cast pane 1 is claude, working.
    assert!(
        WIDE_SAMPLE.iter().any(|r| r.1.contains("claude") && r.2 == "working"),
        "no claude row reads working, so agent_type remains a live candidate"
    );
    // REFUTED: "claude_unicode_prompt implies idle" — the same row carries it and works.
    assert!(
        rows_with("claude_unicode_prompt").any(|r| r.2 == "working"),
        "claude_unicode_prompt remains a live candidate"
    );
    // REFUTED: "failed_text implies idle" — clutterfreespaces carries it and works.
    assert!(
        rows_with("failed_text").any(|r| r.2 == "working"),
        "failed_text remains a live candidate"
    );

    // WHAT SURVIVES, stated as a bound rather than a cause. Every row carrying a bare
    // `braille_spinner` reads working EXCEPT the one violation, so the defect is rare and
    // conditional — n=1, and this leg does not name its trigger.
    let spinner: Vec<_> = WIDE_SAMPLE.iter().filter(|r| r.3.contains(&"braille_spinner")).collect();
    let violations: Vec<_> = spinner.iter().filter(|r| r.2 == "idle").collect();
    assert!(!spinner.is_empty(), "ANTI-VACUITY: no spinner rows means no bound");
    assert_eq!(
        violations.len(),
        1,
        "the bound is one violation in {} spinner rows; a change here is new evidence and \
         belongs in the bead, not a silent edit: {violations:?}",
        spinner.len()
    );
    assert_eq!(violations[0].0, "omp-orchestrator");
}

// ---------------------------------------------------------------------------------------
// PR-L3 — readiness needs two captures >=75s apart
// ---------------------------------------------------------------------------------------

#[test]
fn l3_this_crates_motion_window_meets_the_75_second_floor() {
    assert_eq!(
        TWO_CAPTURE_MIN_SECS, 75,
        "the readiness gate must use the canonical K0 two-capture floor"
    );
    assert!(
        read("crates/pane-dispatch-ready/src/lib.rs").contains("TWO_CAPTURE_MIN_SECS"),
        "the readiness implementation must retain the canonical interval"
    );
}

// ---------------------------------------------------------------------------------------
// PR-L4 — a CONFIDENT busy claim beats a stale free read; an UNKNOWN one does not
// ---------------------------------------------------------------------------------------

#[test]
fn l4_a_changed_second_capture_overturns_a_provisional_free() {
    // The CONFIDENT busy arm: motion between captures is positive evidence of work, and it
    // beats the first capture's free read.
    let first = classify(IDLE_CLAUDE, false, &rules());
    assert_eq!(first.state, PaneDispatchReadyState::Free);
    let confirmed = confirm_free(
        first,
        IDLE_CLAUDE,
        snapshot(0, "27s", "sha-one"),
        snapshot(75, "27s", "sha-two"),
        &rules(),
    );
    assert_eq!(
        confirmed.state,
        PaneDispatchReadyState::Busy,
        "a changed hash between captures must overturn FREE"
    );
    assert!(
        confirmed.reason.contains("buffer changed"),
        "the reason must name the motion, not merely assert BUSY: {}",
        confirmed.reason
    );
}

#[test]
fn l4_an_unreadable_second_capture_does_not_become_free_or_a_busy_claim() {
    // The UNKNOWN arm. A non-answer must not be laundered into either verdict: it is
    // UNREADABLE, which is fail-closed for dispatch and is NOT a claim that the pane is
    // working. Conflating "I could not look" with "it is busy" is the same defect as an
    // empty scan set reporting as a pass.
    let first = classify(IDLE_CLAUDE, false, &rules());
    assert_eq!(first.state, PaneDispatchReadyState::Free);
    let confirmed = confirm_free(
        first,
        "",
        snapshot(0, "27s", "sha-one"),
        snapshot(75, "27s", "sha-one"),
        &rules(),
    );
    assert_eq!(
        confirmed.state,
        PaneDispatchReadyState::Unreadable,
        "an empty second capture must be UNREADABLE, never FREE and never BUSY"
    );
    assert!(
        confirmed.reason.contains("fail closed"),
        "the reason must say the verdict is fail-closed: {}",
        confirmed.reason
    );
    // An unchanged second capture does not establish motion. The K0 evidence
    // type refuses to call the pane idle when both timer and spinner-stripped
    // content hash are unchanged.
    let again = classify(IDLE_CLAUDE, false, &rules());
    let held = confirm_free(
        again,
        IDLE_CLAUDE,
        snapshot(0, "27s", "same"),
        snapshot(75, "27s", "same"),
        &rules(),
    );
    assert_eq!(held.state, PaneDispatchReadyState::Unreadable);
    assert!(held.reason.contains("TWO_CAPTURE_UNPROVEN"));
}

// ---------------------------------------------------------------------------------------
// PR-L5 — NO_AGENT is a bare shell and never dispatchable
// ---------------------------------------------------------------------------------------

#[test]
fn l5_a_bare_shell_is_no_agent_even_with_a_perfect_prompt() {
    // The prompt marker is PRESENT and would satisfy the free-prompt check, so this leg
    // proves the agent test runs FIRST and is not overridden by a good-looking prompt.
    let v = classify(BARE_SHELL, false, &rules());
    assert_eq!(
        v.state,
        PaneDispatchReadyState::NoAgent,
        "a bare shell must be NO_AGENT, not FREE"
    );
    assert_ne!(
        v.state,
        PaneDispatchReadyState::Free,
        "NO_AGENT must never collapse into FREE — there is no agent to receive a packet"
    );
    assert!(
        v.reason.contains("bare shell"),
        "the reason must name the condition: {}",
        v.reason
    );
    // Ordering proof: the same text with an agent marker added is no longer NO_AGENT, so the
    // NO_AGENT verdict above is attributable to the missing agent and nothing else.
    let with_agent = format!("{BARE_SHELL}\n claude-opus-5 ready\n π  > x\n");
    assert_ne!(
        classify(&with_agent, false, &rules()).state,
        PaneDispatchReadyState::NoAgent,
        "adding an agent marker must change the verdict, or NO_AGENT is not attributable"
    );
}

#[test]
fn l5_an_empty_capture_is_unreadable_not_no_agent() {
    // Two different absences, two different states. An empty capture is a failed READ; a
    // populated capture with no agent is a bare shell. Collapsing them would make a
    // capture-pane failure look like an operator's idle terminal.
    let v = classify("", false, &rules());
    assert_eq!(v.state, PaneDispatchReadyState::Unreadable);
    assert!(v.reason.contains("empty capture"), "reason: {}", v.reason);
}

// ---------------------------------------------------------------------------------------
// Fires-on-known-bad and known-good, on the states themselves
// ---------------------------------------------------------------------------------------

#[test]
fn a_planted_busy_marker_is_caught_in_the_tail_and_ignored_above_it() {
    // KNOWN-BAD: a busy marker inside the 6-line tail must produce BUSY.
    let busy = format!("{IDLE_CLAUDE}\nesc to interrupt\n");
    assert_eq!(
        classify(&busy, false, &rules()).state,
        PaneDispatchReadyState::Busy,
        "a busy marker in the tail must be caught"
    );
    // KNOWN-GOOD: the same marker far above the tail must NOT, or a stale spinner in
    // scrollback pins every pane BUSY forever.
    let stale = format!("esc to interrupt\n{}\n{IDLE_CLAUDE}", "filler\n".repeat(20));
    assert_eq!(
        classify(&stale, false, &rules()).state,
        PaneDispatchReadyState::Free,
        "a stale busy marker above the tail must not pin the pane BUSY"
    );
}

#[test]
fn a_quota_exhausted_pane_is_neither_busy_nor_free() {
    // QUOTA_BLOCKED is its own state for the same reason UNREADABLE is: the operator action
    // is spend, not a retry and not a dispatch.
    let quota = format!("{IDLE_CLAUDE}\nYou've hit your usage limit\n");
    let v = classify(&quota, false, &rules());
    assert_eq!(v.state, PaneDispatchReadyState::QuotaBlocked);
    assert!(
        v.reason.contains("not busy, not free"),
        "the reason must refuse both readings: {}",
        v.reason
    );
}
