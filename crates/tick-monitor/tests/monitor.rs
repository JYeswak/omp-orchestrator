//! WIRING, not decoration.
//!
//! The binary's `--selftest` has 41 legs, and `cargo test` cannot reach a single one of
//! them -- a `main.rs` subcommand is invoked only by a human who remembers. That is the
//! BUILT != WIRED failure this repo names in AGENTS.md, so the library-level invariants
//! are asserted HERE, where `cargo test` (and therefore CI, and therefore any pane
//! running the suite) executes them on every run.
//!
//! These are the legs whose failure would make the monitors silently wrong rather than
//! loudly broken. Each one corresponds to a defect measured on 2026-08-31.

use std::time::Duration;
use tick_monitor::*;

/// The four status lines below are VERBATIM captures from live panes on 2026-08-31, not
/// hand-written approximations. That distinction is the whole lesson of
/// `omp-orchestrator-pane-truth-omp-v18-blind-lre`: pane-truth's fixtures were the Claude
/// Code format (`Working (2s - esc to interrupt)`), so its green selftest AND its mutation
/// leg were both vacuous against the payload it actually runs on (fh C38).
mod live {
    pub const GLM_WORKING: &str = " \u{2819} 12m  \u{b7} \u{25c9} GLM 5.3 \u{b7} \u{1f4c1} ~/Developer/omp-orchestrator \u{b7} \u{2442} main *1 ?6 \u{b7} \u{25eb} 13.0%/1.3M \u{27f2} \u{b7} $1.30";
    pub const CODEX_WORKING: &str = " \u{280f} 6m  > \u{25d5} GPT-5.6-Luna > \u{1f4c1} ~/Developer/omp-orchestrator > \u{2442} main *1 ?6 > S0.13 \u{25b6}";
    pub const CODEX_IDLE: &str = " \u{3c0}  > \u{25d5} GPT-5.6-Luna > \u{1f4c1} ~/Developer/omp-orchestrator > \u{2442} main *1 ?1 > S0.25 \u{25b6}";
    pub const GLM_IDLE: &str = " \u{3c0}  \u{b7} \u{25c9} GLM 5.3 \u{b7} \u{1f4c1} ~/Developer/omp-orchestrator \u{b7} \u{2442} main *1 ?1 \u{b7} \u{25eb} 17.4%/1.3M \u{27f2} \u{b7} $4.62";
}

#[test]
fn v18_working_lines_are_recognised() {
    assert_eq!(
        classify(live::GLM_WORKING),
        PaneState::Working { timer_secs: 720 }
    );
    assert_eq!(
        classify(live::CODEX_WORKING),
        PaneState::Working { timer_secs: 360 }
    );
}

#[test]
fn v18_idle_lines_are_recognised() {
    assert_eq!(classify(live::CODEX_IDLE), PaneState::Idle);
    assert_eq!(classify(live::GLM_IDLE), PaneState::Idle);
}

#[test]
fn token_budgets_and_spend_counters_are_not_elapsed_timers() {
    // Both appear on EVERY live v18 status line. If either parsed as a timer, an idle
    // pane would read as working forever.
    assert!(parse_timer("13.0%/1.3M").is_none(), "1.3M is a token budget");
    assert!(parse_timer("S0.25").is_none(), "S0.25 is a spend counter");
    assert!(parse_timer("5M").is_none(), "uppercase M is not minutes");
    assert_eq!(parse_timer("12m"), Some(720));
    assert_eq!(parse_timer("48s"), Some(48));
}

#[test]
fn a_spinner_in_scrollback_prose_does_not_make_a_pane_working() {
    // The over-broad fix this guards against: matching any braille glyph anywhere would
    // classify a pane that merely QUOTED a status line as working. Same class as
    // arc-keepalive's "quoted error text is not pane state".
    let prose = format!(
        "the agent printed \"{}\" a while ago\n{}",
        live::GLM_WORKING,
        live::GLM_IDLE
    );
    assert_eq!(classify(&prose), PaneState::Idle);
}

#[test]
fn animation_alone_does_not_change_the_stable_hash() {
    // THE SPINNER TRAP: a hash over the raw frame changes every animation step, so a dead
    // pane produces a changing hash forever and a busy-detector built on it can never
    // report idle.
    let a = format!("unchanged body\n \u{280b} 5m  \u{b7} tail");
    let b = format!("unchanged body\n \u{2819} 5m  \u{b7} tail");
    assert_eq!(stable_hash(&a), stable_hash(&b));
    let c = "CHANGED body\n \u{280b} 5m  \u{b7} tail".to_owned();
    assert_ne!(stable_hash(&a), stable_hash(&c));
}

#[test]
fn idle_is_never_reported_from_a_single_capture() {
    let o = Observation {
        pane_id: "%1".to_owned(),
        state: PaneState::Idle,
        hash: 1,
        at: 1000,
    };
    let v = liveness(None, &o);
    assert!(matches!(v, Liveness::Unproven { .. }), "got {v:?}");
    assert!(!v.is_dispatchable());
}

#[test]
fn a_gap_shorter_than_the_floor_is_unproven() {
    let mk = |at| Observation {
        pane_id: "%1".to_owned(),
        state: PaneState::Idle,
        hash: 1,
        at,
    };
    // 30s was measured as TOO SHORT: a lane inside a long tool call has a static timer,
    // and a 30s window called two live panes frozen.
    assert!(!liveness(Some(&mk(1000)), &mk(1000 + MIN_GAP_SECS - 1)).is_dispatchable());
    assert!(liveness(Some(&mk(1000)), &mk(1000 + MIN_GAP_SECS + 1)).is_dispatchable());
}

#[test]
fn working_and_unproven_panes_are_never_dispatchable() {
    let mk = |st: PaneState, h, at| Observation {
        pane_id: "%1".to_owned(),
        state: st,
        hash: h,
        at,
    };
    let working = liveness(
        Some(&mk(PaneState::Working { timer_secs: 60 }, 1, 1000)),
        &mk(PaneState::Working { timer_secs: 120 }, 2, 1100),
    );
    assert_eq!(working, Liveness::Live);
    assert!(!working.is_dispatchable(), "a live pane must never be filled");

    let unproven = liveness(None, &mk(PaneState::Unproven, 0, 1000));
    assert!(!unproven.is_dispatchable());
}

#[test]
fn static_timer_with_changed_content_is_live_not_frozen() {
    // The false-freeze direction is the IRREVERSIBLE one: a missed freeze costs idle
    // minutes, a false freeze destroys work in flight.
    let mk = |t, h, at| Observation {
        pane_id: "%1".to_owned(),
        state: PaneState::Working { timer_secs: t },
        hash: h,
        at,
    };
    assert_eq!(liveness(Some(&mk(60, 7, 1000)), &mk(60, 9, 1100)), Liveness::Live);
    assert_eq!(
        liveness(Some(&mk(60, 7, 1000)), &mk(60, 7, 1100)),
        Liveness::Frozen
    );
}

#[test]
fn a_fabricated_mode_string_hard_rejects() {
    // The 2026-04-19 stall: 60 ticks over 9 hours, each logged with a fabricated mode
    // like "47th_HOLD_silent". The rules were correct and enforced by no one.
    let t = Tick {
        mode: "47th_HOLD_silent".to_owned(),
        verdict: "GREEN".to_owned(),
        ..Default::default()
    };
    assert_eq!(
        validate(&t, "", 0),
        Err(Reject::UnknownMode("47th_HOLD_silent".to_owned()))
    );
}

#[test]
fn forbidden_phrases_reject_even_inside_an_escalation_field() {
    // BLOCKED must not reopen the idling hole: honest blocking is expressible only as
    // STRUCTURE, never as prose.
    let t = Tick {
        mode: "BLOCKED".to_owned(),
        verdict: "BLOCKED".to_owned(),
        external_blocker: Some("infrastructure:rch".to_owned()),
        escalation_action: Some("standing by until it clears".to_owned()),
        ..Default::default()
    };
    assert_eq!(validate(&t, "", 0).map_err(|e| e.code()), Err(5));
}

#[test]
fn blocked_requires_a_typed_blocker_and_an_escalation_artifact() {
    let bare = Tick {
        mode: "BLOCKED".to_owned(),
        verdict: "BLOCKED".to_owned(),
        ..Default::default()
    };
    assert_eq!(validate(&bare, "", 0), Err(Reject::MissingBlocker));

    let no_artifact = Tick {
        external_blocker: Some("infrastructure:rch-exit-103".to_owned()),
        ..bare.clone()
    };
    assert_eq!(validate(&no_artifact, "", 0), Err(Reject::NoEscalationArtifact));

    let ok = Tick {
        escalation_action: Some("bead comment with 3 new acceptance clauses".to_owned()),
        ..no_artifact.clone()
    };
    assert!(validate(&ok, "", 0).is_ok());
}

#[test]
fn an_unnamed_human_gate_is_blocked_on_josh_in_a_new_hat() {
    let t = Tick {
        mode: "BLOCKED".to_owned(),
        verdict: "BLOCKED".to_owned(),
        external_blocker: Some("joshua-decision:pricing".to_owned()),
        escalation_action: Some("asked in channel".to_owned()),
        ..Default::default()
    };
    assert_eq!(validate(&t, "", 0), Err(Reject::JoshuaDecisionNeedsBead));

    let named = Tick {
        external_blocker: Some("joshua-decision:omp-orchestrator-2z2".to_owned()),
        ..t.clone()
    };
    assert!(validate(&named, "", 0).is_ok());
}

#[test]
fn the_third_tick_on_one_blocker_demands_escalation() {
    // A repeated block is a bead, not a rhythm. You cannot BLOCKED your way through a
    // night.
    let t = Tick {
        mode: "DISPATCH".to_owned(),
        verdict: "RED".to_owned(),
        external_blocker: Some("infrastructure:rch".to_owned()),
        ..Default::default()
    };
    assert_eq!(
        validate(&t, "infrastructure:rch", 2).map_err(|e| e.code()),
        Err(7)
    );

    let remediating = Tick {
        mode: "L1_REMEDIATION".to_owned(),
        ..t.clone()
    };
    assert!(validate(&remediating, "infrastructure:rch", 2).is_ok());
}

#[test]
fn a_timeout_is_not_a_verdict() {
    // AGENTS.md: a restrictive terminal must never be read as success, and an empty
    // buffer from a killed child must not map to the token a failing subject produces.
    let out = run(&["/bin/sleep", "30"], Duration::from_millis(500));
    match &out {
        Outcome::TimedOut { group_killed, .. } => assert!(*group_killed),
        other => panic!("expected TimedOut, got {}", other.kind()),
    }
    assert!(
        out.stdout_if_completed().is_none(),
        "a timeout must not be readable as output"
    );
}

#[test]
fn both_pipes_are_drained_past_the_deadlock_threshold() {
    // Undrained stdout+stderr deadlocks past ~64 KiB; the tell is 0% CPU with no
    // children, and widening the timeout only hides it longer.
    let out = run(
        &[
            "/bin/sh",
            "-c",
            "yes 0123456789abcdef | head -c 200000; yes fedcba9876543210 | head -c 200000 1>&2",
        ],
        Duration::from_secs(30),
    );
    match out {
        Outcome::Completed { stdout, stderr, .. } => {
            assert!(stdout.len() >= 200_000, "stdout truncated: {}", stdout.len());
            assert!(stderr.len() >= 200_000, "stderr truncated: {}", stderr.len());
        }
        other => panic!("200KB on both pipes deadlocked or failed: {}", other.kind()),
    }
}

#[test]
fn a_missing_binary_is_typed_not_a_panic() {
    assert!(matches!(
        run(&["/nonexistent/xyz"], Duration::from_secs(2)),
        Outcome::SpawnFailed { .. }
    ));
}

#[test]
fn a_just_finished_pane_is_newly_idle_not_live() {
    // BEAD -oco, found by the operator, not by this suite. The old code had a
    // `_ => Liveness::Live` catch-all, so a WORKING -> IDLE transition scored LIVE:
    // technically true ("it moved") and useless to a dispatcher, which then passed over a
    // freed worker. This assertion FAILS against that code and passes now.
    let prev = Observation {
        pane_id: "%1408".to_owned(),
        state: PaneState::Working { timer_secs: 120 },
        hash: 11,
        at: 1000,
    };
    let now = Observation {
        pane_id: "%1408".to_owned(),
        state: PaneState::Idle,
        hash: 22,
        at: 1000 + MIN_GAP_SECS + 5,
    };
    let v = liveness(Some(&prev), &now);
    assert_eq!(v, Liveness::NewlyIdle, "a just-finished pane is NEWLY_IDLE");
    assert_ne!(v, Liveness::Live, "the defect: it used to read LIVE");

    // Still not fillable on one idle capture -- visibility must not buy a slot.
    assert!(!v.is_dispatchable(), "one idle capture is still one capture");
    // But it MUST be visible as free capacity.
    assert!(v.is_free_capacity(), "a conductor has to see the freed worker");

    // The next tick confirms it and it becomes dispatchable.
    let later = Observation {
        pane_id: "%1408".to_owned(),
        state: PaneState::Idle,
        hash: 22,
        at: now.at + MIN_GAP_SECS + 5,
    };
    let confirmed = liveness(Some(&now), &later);
    assert_eq!(confirmed, Liveness::ConfirmedIdle);
    assert!(confirmed.is_dispatchable());
}

#[test]
fn a_pane_picking_work_up_is_live_and_not_free_capacity() {
    let prev = Observation {
        pane_id: "%1".to_owned(),
        state: PaneState::Idle,
        hash: 1,
        at: 1000,
    };
    let now = Observation {
        pane_id: "%1".to_owned(),
        state: PaneState::Working { timer_secs: 30 },
        hash: 2,
        at: 1000 + MIN_GAP_SECS + 5,
    };
    let v = liveness(Some(&prev), &now);
    assert_eq!(v, Liveness::Live);
    assert!(!v.is_free_capacity(), "a working pane is never free capacity");
}
