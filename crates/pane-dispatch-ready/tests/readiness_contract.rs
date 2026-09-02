//! INVARIANT SUITE for `docs/contracts/pane_readiness_contract.md` — laws `PR-L1` … `PR-L5`.
//!
//! # Two legs assert a law is NOT enforced, on purpose
//!
//! `PR-L1` and `PR-L3` are stated in the contract and unenforced *by this crate*. Those legs
//! are **pinned defects**: they assert today's behaviour and name the bead that will change
//! it, so the fix cannot land silently — the leg goes RED and forces the contract to be
//! updated in the same commit.
//!
//! # No live `ntm` dependency
//!
//! `PR-L2` and `PR-L4` are about a foreign classifier's output. A test that shells `ntm`
//! would be a flake and would measure whatever the fleet happens to be doing. The snapshot
//! below is a VERBATIM `ntm --robot-activity` payload, dated and cited, so the leg tests the
//! RULE against a fixed observation instead of testing the fleet.

use pane_dispatch_ready::{
    classify, confirm_free, PaneDispatchReadyRules, PaneDispatchReadyState, DEFAULT_MOTION_SECS,
};
use std::path::{Path, PathBuf};

fn rules() -> PaneDispatchReadyRules {
    PaneDispatchReadyRules::default()
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
fn l1_a_wedged_pane_still_classifies_free_in_this_crate() {
    // PINNED DEFECT, bead omp-orchestrator-readiness-l1-wedge-blind-46y7.
    //
    // A wedged pane ACCEPTED a packet and parked it at "Press up to edit queued messages".
    // Every surface this crate reads still says free: an agent is rendering, no busy marker
    // is in the 6-line tail, the buffer is unchanged, and the `π` prompt is present. So
    // `classify` returns FREE and a caller dispatches into a pane that will never submit.
    let idle = classify(IDLE_CLAUDE, false, &rules());
    let wedged = classify(WEDGED_CLAUDE, false, &rules());
    assert_eq!(idle.state, PaneDispatchReadyState::Free, "control: idle is FREE");
    assert_eq!(
        wedged.state,
        PaneDispatchReadyState::Free,
        "PR-L1 may now be enforced: the wedge marker changed the verdict to {:?}. Update the \
         contract's PR-L1 row and close the bead in the same commit.",
        wedged.state
    );
    // And the two verdicts are INDISTINGUISHABLE, which is the sharper statement.
    assert_eq!(
        idle.pipe_line(),
        wedged.pipe_line(),
        "a wedged pane and an idle pane must currently produce the same line"
    );
}

#[test]
fn l1_the_wedge_marker_is_detected_by_three_other_crates() {
    // The detection EXISTS; it just does not live in the readiness authority. Named here so
    // the finding is "the classifier is blind", not "the fleet is blind" — a wrong scope
    // would send someone to build a detector that already ships three times over.
    const MARKER: &str = "Press up to edit queued messages";
    let consumers = [
        "crates/fast-dispatch/src/lib.rs",
        "crates/fleet-monitor/src/lib.rs",
        "crates/tick-monitor/src/lib.rs",
    ];
    for rel in consumers {
        assert!(
            read(rel).contains(MARKER),
            "{rel} must carry the wedge marker; if this fails the contract's PR-L1 evidence \
             is stale, not satisfied"
        );
    }
    // The readiness authority itself does NOT.
    let own = read("crates/pane-dispatch-ready/src/lib.rs");
    assert!(
        !own.contains(MARKER),
        "PR-L1 may now be enforced in the readiness crate. Update the contract and close the \
         bead in the same commit."
    );
    // POSITIVE CONTROL on the same reader: a marker this crate DOES carry.
    assert!(
        own.contains("esc to interrupt"),
        "POSITIVE CONTROL FAILED: the reader cannot see a busy marker that is present, so its \
         absence result above proves nothing"
    );
}

// ---------------------------------------------------------------------------------------
// PR-L2 — UNKNOWN is not a busy claim, and safe_to_dispatch does not come from `state`
// ---------------------------------------------------------------------------------------

#[test]
fn l2_a_codex_pane_can_be_unclassifiable_in_the_state_field() {
    let rows = snapshot_rows();
    let unknown: Vec<&Row> = rows.iter().filter(|r| r.state == "UNKNOWN").collect();
    assert_eq!(unknown.len(), 1, "the snapshot carries exactly one UNKNOWN row");
    let r = unknown[0];
    assert_eq!(r.agent_type, "codex", "the unclassifiable pane is a codex pane");
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

// ---------------------------------------------------------------------------------------
// PR-L3 — readiness needs two captures >=75s apart  (PINNED DEFECT)
// ---------------------------------------------------------------------------------------

#[test]
fn l3_this_crates_motion_window_is_below_the_75_second_floor() {
    // PINNED DEFECT, bead omp-orchestrator-readiness-l3-motion-window-7523.
    //
    // `pane_observation_contract` PO-L1 requires two captures at least 75 seconds apart.
    // `pane-truth` and `tick-monitor` both encode 75. This crate sleeps DEFAULT_MOTION_SECS
    // between captures, and an UNCHANGED buffer over that window is what lets a provisional
    // FREE stand.
    assert_eq!(
        DEFAULT_MOTION_SECS, 10,
        "PR-L3 may now be satisfied: the motion window is {DEFAULT_MOTION_SECS}s. Update the \
         contract's PR-L3 row and close the bead in the same commit."
    );
    // The two authorities that DO carry the floor, read from source so no dev-dep is added.
    let pane_truth = read("crates/pane-truth/src/lib.rs");
    let tick_monitor = read("crates/tick-monitor/src/lib.rs");
    assert!(
        pane_truth.contains("TWO_CAPTURE_MIN_SECS: i64 = 75"),
        "pane-truth must still encode the 75s floor; if not, PR-L3's evidence is stale"
    );
    assert!(
        tick_monitor.contains("MIN_GAP_SECS: u64 = 75"),
        "tick-monitor must still encode the 75s floor"
    );
    // POSITIVE CONTROL on the same reader: a constant this crate genuinely declares.
    assert!(
        read("crates/pane-dispatch-ready/src/lib.rs")
            .contains("DEFAULT_MOTION_SECS: u64 = 10"),
        "POSITIVE CONTROL FAILED: the reader cannot see this crate's own constant"
    );
    assert!(
        DEFAULT_MOTION_SECS < 75,
        "the window must be below the floor for this leg to be the pinned defect it claims"
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
    let confirmed = confirm_free(first, IDLE_CLAUDE, "sha-one", "sha-two", &rules());
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
    let confirmed = confirm_free(first, "", "sha-one", "sha-one", &rules());
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
    // And an unchanged second capture leaves the free read standing — the known-GOOD arm,
    // without which this suite would be over-strict and get routed around.
    let again = classify(IDLE_CLAUDE, false, &rules());
    let held = confirm_free(again, IDLE_CLAUDE, "same", "same", &rules());
    assert_eq!(
        held.state,
        PaneDispatchReadyState::Free,
        "an unchanged second capture must not invent motion"
    );
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
    let stale = format!(
        "esc to interrupt\n{}\n{IDLE_CLAUDE}",
        "filler\n".repeat(20)
    );
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
