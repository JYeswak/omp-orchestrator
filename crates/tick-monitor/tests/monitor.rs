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
    assert!(
        parse_timer("13.0%/1.3M").is_none(),
        "1.3M is a token budget"
    );
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
fn idle_and_working_status_glyphs_do_not_change_stable_content_hash() {
    let idle = "unchanged body\nπ . GPT-5.6 . /tmp/receiver";
    let working = "unchanged body\n⠙ 1s . GPT-5.6 . /tmp/receiver";
    assert_eq!(stable_hash(idle), stable_hash(working));
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
    assert!(
        !working.is_dispatchable(),
        "a live pane must never be filled"
    );

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
    assert_eq!(
        liveness(Some(&mk(60, 7, 1000)), &mk(60, 9, 1100)),
        Liveness::Live
    );
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
    assert_eq!(
        validate(&no_artifact, "", 0),
        Err(Reject::NoEscalationArtifact)
    );

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
            assert!(
                stdout.len() >= 200_000,
                "stdout truncated: {}",
                stdout.len()
            );
            assert!(
                stderr.len() >= 200_000,
                "stderr truncated: {}",
                stderr.len()
            );
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
    assert!(
        !v.is_dispatchable(),
        "one idle capture is still one capture"
    );
    // But it MUST be visible as free capacity.
    assert!(
        v.is_free_capacity(),
        "a conductor has to see the freed worker"
    );

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
    assert!(
        !v.is_free_capacity(),
        "a working pane is never free capacity"
    );
}

// ---------------------------------------------------------------------------
// DIALOG -- three planted states, each asserting a SPECIFIC name.
//
// Bead: omp-orchestrator-dialog-reads-as-working. The operator's watcher scored %1413 GONE
// on an Ask dialog; MY defect was the opposite polarity, measured on %1372: on OMP v18 the
// dialog renders ABOVE the status line, the status line stays last with an advancing
// spinner+timer, so a pane blocked on a human answer read as WORKING/LIVE for 26 minutes.
// ---------------------------------------------------------------------------

/// Verbatim capture of the live arc-keepalive approval dialog on %1372, 2026-08-31.
const DIALOG_FIXTURE: &str = include_str!("fixtures/dialog_v18.txt");

#[test]
fn planted_dialog_is_dialog_not_working() {
    let st = tick_monitor::classify(DIALOG_FIXTURE);
    assert_eq!(
        st.label(),
        "DIALOG",
        "a pane awaiting an answer must not read as WORKING; got {st:?}"
    );
}

#[test]
fn planted_working_is_still_working_after_the_new_state() {
    // The new state must not swallow the old ones.
    let cap = " \u{2839} 27m  \u{b7} \u{25d2} GPT-5.6-Luna \u{b7} ~/Developer/control-plane";
    assert_eq!(tick_monitor::classify(cap).label(), "WORKING");
}

#[test]
fn planted_idle_is_still_idle_after_the_new_state() {
    let cap = " \u{3c0}  \u{b7} \u{25c9} GLM 5.3 \u{b7} ~/Developer/omp-orchestrator";
    assert_eq!(tick_monitor::classify(cap).label(), "IDLE");
}

#[test]
fn framed_marker_far_from_status_line_is_not_a_dialog() {
    // THE SELF-POLLUTION LEG. Measured: my own pane carries BOX-FRAMED lines containing
    // "Esc cancel" because OMP renders tool-call blocks inside frames and my commands
    // quoted the marker. `framed && contains` fired on my own scrollback. Only ADJACENCY
    // to the status line discriminates. Without this leg the detector reports every pane
    // that has ever discussed a dialog as blocked on one.
    let cap = "\u{2502} grep -n 'Esc cancel' src/lib.rs\n\
               \u{2502} Enter select \u{b7} n note\n\
               some output\n\
               more output\n\
               another line of output\n\
               \u{2839} 5m \u{b7} \u{25d2} Opus 5 \u{b7} ~/Developer/omp-orchestrator";
    assert!(
        !tick_monitor::dialog_open(cap),
        "framed marker in scrollback must not read as an open dialog"
    );
    assert_eq!(tick_monitor::classify(cap).label(), "WORKING");
}

#[test]
fn a_dialog_pane_is_neither_dispatchable_nor_free_capacity_but_needs_an_answer() {
    let l = tick_monitor::Liveness::Dialog { timer_secs: 1560 };
    assert!(
        !l.is_dispatchable(),
        "cannot accept a packet while prompting"
    );
    assert!(!l.is_free_capacity(), "it is occupied, not free");
    assert!(l.needs_answer(), "the conductor must see it");
}

#[test]
fn dialog_survives_the_state_file_round_trip() {
    // A writer that emits DIALOG and a reader that parses it as UNPROVEN loses the fact
    // one tick after it is established.
    let obs = tick_monitor::Observation {
        pane_id: "%1372".into(),
        state: tick_monitor::PaneState::Dialog { timer_secs: 1560 },
        hash: 42,
        at: 1788170000,
    };
    let dir = std::env::temp_dir().join(format!("tmdlg{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("state.tsv");
    let st = tick_monitor::State {
        panes: vec![obs],
        ..Default::default()
    };
    tick_monitor::save(&p, &st).unwrap();
    let back = tick_monitor::load(&p);
    assert_eq!(
        back.panes[0].state,
        tick_monitor::PaneState::Dialog { timer_secs: 1560 },
        "DIALOG must round-trip, not silently downgrade to UNPROVEN"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_absent_pane_id_is_dead_and_a_dialog_pane_is_not() {
    // THE DEAD LEG. Death and DIALOG demand opposite responses; asserting both in one
    // test is what proves they are actually distinguished rather than merged.
    let prior = vec![
        tick_monitor::Observation {
            pane_id: "%1413".into(),
            state: tick_monitor::PaneState::Dialog { timer_secs: 60 },
            hash: 1,
            at: 100,
        },
        tick_monitor::Observation {
            pane_id: "%9999".into(),
            state: tick_monitor::PaneState::Working { timer_secs: 60 },
            hash: 2,
            at: 100,
        },
    ];
    let live = vec!["%1413".to_string(), "%1397".to_string()];
    let dead = tick_monitor::vanished(&prior, &live);
    assert_eq!(
        dead,
        vec!["%9999".to_string()],
        "only the absent id is dead"
    );
    assert!(
        !dead.contains(&"%1413".to_string()),
        "a pane with a dialog open is PRESENT and must never be called dead"
    );
}

#[test]
fn an_empty_pane_list_declares_nobody_dead() {
    // ANTI-VACUITY. A failed `tmux list-panes` must not produce a fleet-wide obituary.
    // Without this leg the death detector's worst failure looks identical to a quiet tick.
    let prior = vec![tick_monitor::Observation {
        pane_id: "%1413".into(),
        state: tick_monitor::PaneState::Working { timer_secs: 1 },
        hash: 1,
        at: 100,
    }];
    assert!(
        tick_monitor::vanished(&prior, &[]).is_empty(),
        "an empty scan is a failed scan, not a dead fleet"
    );
}

// ---------------------------------------------------------------------------
// OBSCURED -- the capture shape I previously said I could not plant.
//
// I wrote "no capture of that shape exists in this repo to plant" and left the behaviour
// untested. That was a real gap, not a formality: the operator's own watcher scored %1413
// GONE on a covered status line at 09:15Z, and after their fix produced a FALSE DIALOG on
// %1414 at 09:38Z when a box-drawing region briefly covered a pane mid-work at 26/26.
// So the shape is synthesised here: a box-drawing region occupying the last lines, with NO
// dialog footer. It must read OBSCURED -- neither dropped, nor promoted to DIALOG.
// ---------------------------------------------------------------------------

fn obs_at(id: &str, st: tick_monitor::PaneState, at: u64) -> tick_monitor::Observation {
    tick_monitor::Observation {
        pane_id: id.into(),
        state: st,
        hash: 7,
        at,
    }
}

/// A frame drawn OVER the status line: no model name, and no dialog footer either.
const COVERED: &str = "\u{2502} building target/debug/foo\n\
                       \u{2502} 26/26 crates\n\
                       \u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}";

#[test]
fn a_covered_status_line_is_not_a_dialog() {
    // THE FALSE-POSITIVE LEG, aimed at the exact %1414 09:38Z misread.
    assert!(
        !tick_monitor::dialog_open(COVERED),
        "a box-drawing region with no footer must never read as an open dialog"
    );
    assert_eq!(tick_monitor::classify(COVERED).label(), "UNPROVEN");
}

#[test]
fn a_covered_pane_that_was_working_is_obscured_not_dropped() {
    // The pane carried a model line last tick, so it IS an agent pane and IS alive.
    let prev = obs_at(
        "%1414",
        tick_monitor::PaneState::Working { timer_secs: 60 },
        1000,
    );
    let now = obs_at("%1414", tick_monitor::classify(COVERED), 1200);
    let l = tick_monitor::liveness(Some(&prev), &now);
    assert_eq!(
        l.label(),
        "OBSCURED",
        "a covered live pane must not vanish into UNPROVEN"
    );
    assert!(l.needs_attention(), "the conductor must be told to LOOK");
    assert!(
        !l.is_dispatchable() && !l.is_free_capacity(),
        "unreadable is not free"
    );
    assert!(
        !l.needs_answer(),
        "OBSCURED needs a deeper capture, not an answer"
    );
}

#[test]
fn a_shell_pane_stays_unproven_and_is_never_obscured() {
    // The discriminator is the PRIOR observation, not the shape of this capture. Without
    // this leg, "no model line -> alive and obscured" would relabel every shell pane as a
    // live agent needing attention, and the attention list would be permanently noisy.
    let prev = obs_at("%1396", tick_monitor::PaneState::Unproven, 1000);
    let now = obs_at("%1396", tick_monitor::PaneState::Unproven, 1200);
    assert_eq!(
        tick_monitor::liveness(Some(&prev), &now).label(),
        "UNPROVEN"
    );
}

#[test]
fn a_covered_pane_with_no_prior_is_unproven_not_obscured() {
    // First sighting: we cannot know it was ever an agent. Claiming OBSCURED here would be
    // a liveness claim from one capture, which is the rule this crate exists to enforce.
    let now = obs_at("%1414", tick_monitor::classify(COVERED), 1200);
    assert_eq!(tick_monitor::liveness(None, &now).label(), "UNPROVEN");
}

#[test]
fn a_dialog_still_beats_a_covered_frame_when_the_footer_is_present() {
    // OBSCURED must not swallow DIALOG: same framed shape, footer adjacent to a status line.
    let st = tick_monitor::classify(DIALOG_FIXTURE);
    assert_eq!(st.label(), "DIALOG");
}

// ---------------------------------------------------------------------------
// WIRED, not merely BUILT. This repo's UNWIRED_LANE_ALLOWANCE is empty, which means a lane
// with no production caller is a defect rather than a TODO. I shipped `vanished()` with
// unit tests and ZERO callers and caught it myself; this leg is what stops it recurring.
//
// Honest about its own class: this is a SOURCE-level check, the same mechanism
// no-shell-gate's wired_lanes uses. It proves a call site exists in the production binary.
// It does NOT prove that call site executes on a live fleet -- that is the live run
// recorded in the bead, and it is a separate kind of evidence.
// ---------------------------------------------------------------------------

#[test]
fn vanished_has_a_production_caller_in_the_binary() {
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("tick_monitor::vanished(&prior.panes, &ids)"),
        "vanished() lost its production call site in observe -- an unwired lane"
    );
    // POSITIVE CONTROL: the pattern must be capable of failing. A grep that can never miss
    // is not evidence. `fh N043` is this repo failing exactly this way: a full battery of
    // verification rituals that never fired once.
    assert!(
        !main_rs.contains("tick_monitor::a_function_that_does_not_exist("),
        "positive control: the scan must be able to report absence"
    );
    // And the result must be RENDERED. A value computed and never printed is still unwired.
    assert!(
        main_rs.contains("\\\"dead_panes\\\""),
        "dead panes computed but never emitted is not wired"
    );
    assert!(
        main_rs.contains("\\\"attention\\\""),
        "attention computed but never emitted is not wired"
    );
}

#[test]
fn needs_attention_has_a_production_caller_too() {
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("live.needs_attention()"),
        "needs_attention() is unwired -- the OBSCURED state would be computed and discarded"
    );
}

#[test]
fn capacity_alarm_fires_once_after_three_free_ticks() {
    let mut alarm = CapacityAlarm::new(3);
    assert_eq!(
        alarm.observe(true),
        CapacityAlarmEvent::None {
            consecutive_ticks: 1
        }
    );
    assert_eq!(
        alarm.observe(true),
        CapacityAlarmEvent::None {
            consecutive_ticks: 2
        }
    );
    assert_eq!(
        alarm.observe(true),
        CapacityAlarmEvent::Fire {
            consecutive_ticks: 3
        }
    );
    assert_eq!(
        alarm.observe(true),
        CapacityAlarmEvent::None {
            consecutive_ticks: 4
        }
    );
}

#[test]
fn fully_occupied_fleet_never_fires_capacity_alarm() {
    let mut alarm = CapacityAlarm::new(3);
    for _ in 0..100 {
        assert_eq!(
            alarm.observe(false),
            CapacityAlarmEvent::None {
                consecutive_ticks: 0
            }
        );
    }
    assert_eq!(alarm.consecutive_free_ticks(), 0);
}

#[test]
fn capacity_alarm_mutation_leg_rejects_inverted_predicate() {
    // A mutant that changes the free-capacity predicate to `!free_capacity` fires here.
    // This is the deciding negative: an occupied fleet must never notify.
    let mut alarm = CapacityAlarm::new(2);
    assert_eq!(
        alarm.observe(false),
        CapacityAlarmEvent::None {
            consecutive_ticks: 0
        }
    );
    assert_eq!(
        alarm.observe(false),
        CapacityAlarmEvent::None {
            consecutive_ticks: 0
        }
    );
}

#[test]
fn capacity_escalation_writes_urgent_and_observes_notification() {
    let dir = std::env::temp_dir().join(format!(
        "tick-monitor-capacity-alarm-{}-{}",
        std::process::id(),
        line!()
    ));
    std::fs::create_dir_all(&dir).expect("create alarm test directory");
    let urgent = dir.join("URGENT_JOSH.md");
    let receipt = escalate_idle_capacity_with_notifier(
        &urgent,
        17,
        3,
        "free_capacity=[%1413]",
        std::path::Path::new("/bin/echo"),
    )
    .expect("test notifier should complete");
    let urgent_text = std::fs::read_to_string(&urgent).expect("urgent artifact exists");
    assert!(receipt.notification_observed);
    assert_eq!(receipt.consecutive_ticks, 3);
    assert!(urgent_text.contains("URGENT: persistent idle capacity"));
    assert!(urgent_text.contains("consecutive_ticks: 3"));
    assert!(urgent_text.contains("tick: 17"));
    std::fs::remove_dir_all(&dir).expect("remove owned test directory");
}

#[test]
fn capacity_alarm_is_wired_to_watch_escalation() {
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("CapacityAlarm::new(capacity_alarm_after)"),
        "watch lost the persistent capacity alarm state"
    );
    assert!(
        main_rs.contains("escalate_idle_capacity(&urgent, ticks, consecutive_ticks, &line)"),
        "watch computes capacity but does not reach the escalation mechanism"
    );
    assert!(
        main_rs.contains("URGENT_JOSH.md"),
        "watch escalation lost its durable urgent artifact"
    );
}

// ── LEDGER OWNERSHIP ────────────────────────────────────────────────────────
// Measured 2026-08-31: two watchers on one ledger decayed the observation gap
// 15s per tick (75 -> 66 -> 51 -> 36 -> 22 -> 6) and disabled the two-capture
// liveness rule on 82% of ticks, reporting `gap_too_short` and never an error.

#[test]
fn an_unowned_ledger_is_claimable() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("s.tsv");
    assert!(tick_monitor::check_ownership(&p, 4242).is_ok(),
        "a ledger that does not exist yet must be claimable");
}

#[test]
fn the_owner_may_rewrite_its_own_ledger() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("s.tsv");
    let me = std::process::id();
    let st = tick_monitor::State { owner_pid: me, ..Default::default() };
    tick_monitor::save(&p, &st).unwrap();
    assert!(tick_monitor::check_ownership(&p, me).is_ok(),
        "the owning process must not lock itself out");
}

#[test]
fn a_second_live_writer_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("s.tsv");
    // Our own pid is unambiguously live; claim as it, then approach as someone else.
    let owner = std::process::id();
    tick_monitor::save(&p, &tick_monitor::State { owner_pid: owner, ..Default::default() }).unwrap();

    let err = tick_monitor::check_ownership(&p, owner + 1)
        .expect_err("a different LIVE owner must be refused");
    assert!(err.contains("LEDGER CONTENDED"), "refusal must name the condition: {err}");
    assert!(err.contains(&owner.to_string()), "refusal must name the owner: {err}");
}

#[test]
fn a_dead_owner_does_not_hold_the_ledger_forever() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("s.tsv");
    // pid 0 is never a live user process, and the loader treats an absent field as 0;
    // a reaped watcher must not wedge the next one out.
    tick_monitor::save(&p, &tick_monitor::State { owner_pid: 0, ..Default::default() }).unwrap();
    assert!(tick_monitor::check_ownership(&p, 4242).is_ok(),
        "a stale owner must be reclaimable, or one crash disables monitoring permanently");
}

#[test]
fn owner_pid_survives_a_save_load_round_trip() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("s.tsv");
    tick_monitor::save(&p, &tick_monitor::State { owner_pid: 31337, ..Default::default() }).unwrap();
    assert_eq!(tick_monitor::load(&p).owner_pid, 31337,
        "an owner that does not round-trip is no owner at all");
}
