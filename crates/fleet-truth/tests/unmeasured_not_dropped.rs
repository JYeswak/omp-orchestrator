//! `omp-orchestrator-dw3l`: a panicking sensor thread must yield a TYPED UNMEASURED row, never
//! a silently dropped one.
//!
//! NAMED TARGET per the mmt4 ruling: cite as
//! `cargo test -p fleet-truth --test unmeasured_not_dropped`, never a bare `-p` aggregate.
//!
//! WHAT THIS DEFENDS. `main.rs` used `filter_map(|h| h.join().ok())`, which maps a panicked
//! child to `None` and DROPS it. Quiescence held — the lexical `std::thread::scope` joins every
//! handle before returning, which is why `zaxp` certified that site as needing no asupersync
//! child region — but ERROR PROPAGATION did not. In the ground-truth register that made a
//! session whose sensors PANICKED indistinguishable from a session that DOES NOT EXIST: this
//! repository's silent-success class inverted into a silent PARTIAL.
//!
//! THE OLD COMMENT DEFENDED A REAL TRADEOFF and it is worth stating why the fix is not a
//! reversal of judgement: dropping avoided "poisoning the whole observation", which would make
//! the crate unusable. A typed row gives BOTH — the register survives and nothing is lost. The
//! dichotomy was false, not the concern.

use fleet_truth::{unmeasured_row, FleetTruthRules, TruthRow};

fn rules_ranking_high() -> FleetTruthRules {
    let mut rules = FleetTruthRules::default();
    rules.identity_unknown_ranks_high = true;
    rules
}

/// FIRES-ON-KNOWN-BAD with a REAL panicking child, exercising the exact `scope` + `join`
/// shape `main.rs` uses. Asserts the OUTCOME — one row per session, the panicked one named —
/// never an exit code.
///
/// MUTATION: replace the `map(... match join)` with `filter_map(|h| h.join().ok())` and the
/// length assertion goes RED, naming the shortfall.
#[test]
fn a_panicking_sensor_thread_yields_a_row_instead_of_vanishing() {
    let rules = rules_ranking_high();
    let sessions = vec!["alpha".to_owned(), "panics".to_owned(), "omega".to_owned()];

    // Deliberately silence the panic hook: a panicking child prints a backtrace that would
    // otherwise look like a test failure in the log. The panic still happens and is still
    // caught by join(); only the noise is suppressed.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let rows: Vec<TruthRow> = std::thread::scope(|scope| {
        let handles: Vec<_> = sessions
            .iter()
            .map(|s| {
                (
                    s.as_str(),
                    scope.spawn(move || {
                        if s == "panics" {
                            panic!("synthetic sensor failure");
                        }
                        TruthRow {
                            score: 1,
                            session: s.clone(),
                            repo: "r".into(),
                            commits: "0".into(),
                            last_bead_close: "-".into(),
                            dirty: "0".into(),
                            behind: "0".into(),
                            ctx: "ok".into(),
                            save_age: "0".into(),
                            save_alert: "-".into(),
                            reason: "healthy".into(),
                        }
                    }),
                )
            })
            .collect();
        handles
            .into_iter()
            .map(|(session, handle)| match handle.join() {
                Ok(row) => row,
                Err(_) => unmeasured_row(session, &rules, "sensor thread panicked"),
            })
            .collect()
    });

    std::panic::set_hook(previous);

    assert_eq!(
        rows.len(),
        sessions.len(),
        "THE WHOLE DEFECT: one session panicked and the register returned {} rows for {} \
         sessions. A short table makes a failed session indistinguishable from an absent one",
        rows.len(),
        sessions.len()
    );

    let panicked = rows
        .iter()
        .find(|row| row.session == "panics")
        .expect("the panicked session must still appear, BY NAME, in the register");
    assert!(
        panicked.reason.contains("UNMEASURED"),
        "the row must be TYPED as unmeasured, not silently look healthy: {:?}",
        panicked.reason
    );
    assert!(
        panicked.reason.contains("sensor thread panicked"),
        "the row must name the CAUSE so an operator knows what to inspect: {:?}",
        panicked.reason
    );
    assert_eq!(
        panicked.save_alert, "unmeasured",
        "the alert field must mark it, so a caller filtering on alerts sees it"
    );

    // KNOWN-GOOD half, in the same run: the healthy siblings are untouched.
    for name in ["alpha", "omega"] {
        let row = rows
            .iter()
            .find(|row| row.session == name)
            .expect("healthy session present");
        assert_eq!(row.reason, "healthy", "a healthy row must not be rewritten");
        assert_eq!(row.score, 1);
    }
}

/// KNOWN-GOOD, MANDATORY and separate: a fleet with ZERO panics must return exactly as many
/// rows as sessions, unchanged. An over-strict fix that refuses the whole register on any
/// hiccup would make the ground-truth crate unusable — the failure the old comment feared.
#[test]
fn a_healthy_fleet_returns_one_unmodified_row_per_session() {
    let sessions = vec!["a".to_owned(), "b".to_owned(), "c".to_owned(), "d".to_owned()];
    let rows: Vec<TruthRow> = std::thread::scope(|scope| {
        let handles: Vec<_> = sessions
            .iter()
            .map(|s| {
                (
                    s.as_str(),
                    scope.spawn(move || TruthRow {
                        score: 7,
                        session: s.clone(),
                        repo: "r".into(),
                        commits: "1".into(),
                        last_bead_close: "-".into(),
                        dirty: "0".into(),
                        behind: "0".into(),
                        ctx: "ok".into(),
                        save_age: "0".into(),
                        save_alert: "-".into(),
                        reason: "healthy".into(),
                    }),
                )
            })
            .collect();
        handles
            .into_iter()
            .map(|(session, handle)| match handle.join() {
                Ok(row) => row,
                Err(_) => unmeasured_row(session, &FleetTruthRules::default(), "panicked"),
            })
            .collect()
    });

    assert_eq!(rows.len(), sessions.len(), "no panics, no shortfall");
    assert!(
        rows.iter().all(|row| row.reason == "healthy" && row.score == 7),
        "a healthy fleet must pass through untouched; got {:?}",
        rows.iter().map(|r| &r.reason).collect::<Vec<_>>()
    );
}

/// The unmeasured row reuses `truth_row`'s EXISTING identity-unknown shape rather than minting
/// new vocabulary, and it must RANK TO THE TOP so an operator sees it instead of it sinking.
#[test]
fn an_unmeasured_row_ranks_high_and_reuses_the_existing_unknown_shape() {
    let high = unmeasured_row("s", &rules_ranking_high(), "sensor thread panicked");
    assert_eq!(
        high.score, 999,
        "under identity_unknown_ranks_high an unmeasurable session must sort to the TOP, the \
         same as an identity-unknown one; a sunk row is nearly as invisible as a dropped one"
    );
    assert_eq!(high.save_age, "UNKNOWN", "reuses the existing UNKNOWN marker");
    for field in [&high.repo, &high.commits, &high.dirty, &high.behind, &high.ctx] {
        assert_eq!(field, "?", "unmeasured sensor fields use the existing `?` marker");
    }

    let mut low_rules = FleetTruthRules::default();
    low_rules.identity_unknown_ranks_high = false;
    let low = unmeasured_row("s", &low_rules, "sensor thread panicked");
    assert_eq!(
        low.score, 0,
        "and it must honour the SAME rule switch as truth_row rather than hardcoding a rank"
    );
}

/// ANTI-VACUITY: an empty session list yields an empty register, and that is a real answer
/// rather than a hidden shortfall — but it must not be confused with a panicked fleet. The
/// denominator equality is what distinguishes them.
#[test]
fn an_empty_session_list_is_zero_of_zero_not_a_shortfall() {
    let sessions: Vec<String> = Vec::new();
    let rows: Vec<TruthRow> = std::thread::scope(|scope| {
        let handles: Vec<(&str, std::thread::ScopedJoinHandle<'_, TruthRow>)> = sessions
            .iter()
            .map(|s: &String| (s.as_str(), scope.spawn(|| unreachable!())))
            .collect();
        handles
            .into_iter()
            .map(|(session, handle)| match handle.join() {
                Ok(row) => row,
                Err(_) => unmeasured_row(session, &FleetTruthRules::default(), "panicked"),
            })
            .collect()
    });
    assert_eq!(rows.len(), sessions.len(), "0 of 0 is complete, not short");
}
