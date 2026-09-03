#![forbid(unsafe_code)]
//! gfm6 — every acceptance, as a leg.
//!
//! The fixtures are the SHAPES MEASURED in the live tracker, not invented ones: dead
//! `supervisor:<pid>`, `pane3` labels, the git/`$USER` default author, and the two illegal
//! states the orchestrator's own repair created.

use bead_holder::{
    audit, classify_state, derive_bindings, pane_map, parse_ack, resolve, roster_from,
    AgentIdentity, AuditError, BeadRow, HolderVerdict, IdentityRefusal, Liveness, StateFinding,
    LIVENESS_SOURCE,
};
use std::collections::BTreeSet;

fn defaults() -> BTreeSet<String> {
    ["josh".to_owned()].into_iter().collect()
}

fn roster() -> BTreeSet<String> {
    ["AmberGate", "SilverWolf", "GreenFrog", "BlueLantern"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn bead(id: &str, status: &str, assignee: &str, comments: &[(&str, &str)]) -> BeadRow {
    BeadRow {
        id: id.to_owned(),
        status: status.to_owned(),
        assignee: assignee.to_owned(),
        comments: comments
            .iter()
            .map(|(a, t)| ((*a).to_owned(), (*t).to_owned()))
            .collect(),
    }
}

fn live(pane: &str) -> Option<Liveness> {
    (pane == "%1408").then(|| Liveness {
        state: "WORKING".to_owned(),
        source: LIVENESS_SOURCE.to_owned(),
    })
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 1 — an assignee is a TYPED identity, not free text
// -----------------------------------------------------------------------------------

#[test]
fn every_measured_non_agent_shape_is_refused_with_its_own_reason() {
    // MEASURED shapes. All seven `supervisor:<pid>` values in the live tracker named
    // processes verified dead with `ps -p`.
    let cases: &[(&str, IdentityRefusal)] = &[
        ("", IdentityRefusal::Empty),
        ("   ", IdentityRefusal::Empty),
        (
            "supervisor:26985",
            IdentityRefusal::DeadPidShape {
                raw: "supervisor:26985".to_owned(),
            },
        ),
        (
            "pane3",
            IdentityRefusal::PaneLabelShape {
                raw: "pane3".to_owned(),
            },
        ),
        (
            "%1408",
            IdentityRefusal::PaneIdShape {
                raw: "%1408".to_owned(),
            },
        ),
        (
            "josh",
            IdentityRefusal::DefaultAuthor {
                raw: "josh".to_owned(),
            },
        ),
        (
            "Josh",
            IdentityRefusal::DefaultAuthor {
                raw: "Josh".to_owned(),
            },
        ),
        (
            "JourneySpine",
            IdentityRefusal::Unregistered {
                raw: "JourneySpine".to_owned(),
            },
        ),
    ];
    assert!(!cases.is_empty(), "ANTI-VACUITY: no cases, no evidence");
    for (raw, expected) in cases {
        assert_eq!(
            AgentIdentity::parse(raw, &roster(), &defaults()),
            Err(expected.clone()),
            "{raw:?} must be refused as {expected:?}"
        );
    }

    // `Josh` vs `josh` is not a nicety. `git config user.name` answered `Josh` while the
    // comment author was `josh`; a case-sensitive exclusion silently failed and reported
    // EVERY pane as ambiguous.
    assert_ne!(
        AgentIdentity::parse("Josh", &roster(), &defaults()),
        Ok(AgentIdentity::parse("AmberGate", &roster(), &defaults()).unwrap())
    );

    // KNOWN-GOOD ARM: a real agent passes, or the classifier refuses everything and is
    // worse than the free text it replaces.
    let ok = AgentIdentity::parse("AmberGate", &roster(), &defaults()).expect("a real agent");
    assert_eq!(ok.as_str(), "AmberGate");

    // Every refusal must NAME its remedy. A refusal an agent cannot act on gets deleted.
    for (raw, _) in cases {
        let text = AgentIdentity::parse(raw, &roster(), &defaults())
            .expect_err("refused")
            .to_string();
        assert!(text.contains("ASSIGNEE_REFUSED"), "{text}");
        assert!(text.contains("next_action="), "{raw:?}: {text}");
    }
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 2 — the binding is DERIVED from evidence, and the default author is excluded
// -----------------------------------------------------------------------------------

#[test]
fn the_loose_ack_grammar_is_used_because_the_strict_one_misses_38_percent() {
    // MEASURED over the live tracker: `ack-stage`'s `ACK <token> on <pane> -- ` matches 32
    // of 52 ACK rows and MISSES 20 — including this crate's own ACKs, which carry no
    // trailing clause. A derivation inheriting the strict grammar would see a third of the
    // evidence and call the rest absent.
    assert_eq!(
        parse_ack("ACK gfm6 on %1408"),
        Some(("gfm6".to_owned(), "%1408".to_owned())),
        "the no-clause form is the majority form in the wild"
    );
    assert_eq!(
        parse_ack("ACK gfm6 on %1408 -- claimed and reading"),
        Some(("gfm6".to_owned(), "%1408".to_owned()))
    );
    // NEGATIVE ARM: not every comment is an ACK, or the derivation would bind everything.
    for text in [
        "ORCHESTRATOR: the evidence source I cited",
        "ACK gfm6 on pane4",
        "ACK on %1408",
        "ACK  on %1408",
        "DONE gfm6 on %1408",
        "",
    ] {
        assert_eq!(parse_ack(text), None, "{text:?} must not parse as an ACK");
    }
}

#[test]
fn the_default_author_is_excluded_or_every_pane_reads_ambiguous() {
    // THE CORRECTION THIS CRATE IS BUILT ON, as a leg. With the default author counted, the
    // live data reports all four panes ambiguous; without it, each pane resolves to one
    // agent.
    let comments = [
        ("AmberGate".to_owned(), "ACK a on %1408".to_owned()),
        ("josh".to_owned(), "ACK a on %1408".to_owned()),
        ("josh".to_owned(), "ACK a on %1409".to_owned()),
    ];
    let bindings = derive_bindings("b", &comments, &defaults());
    let map = pane_map(&bindings);
    assert_eq!(
        map.len(),
        1,
        "only the actored ACK may bind; got {map:?}"
    );
    assert_eq!(
        map["%1408"],
        ["AmberGate".to_owned()].into_iter().collect::<BTreeSet<_>>()
    );

    // POSITIVE CONTROL on the same reader: with an EMPTY default set the same input does
    // produce the ambiguity, so the exclusion is what removed it and not a broken parser.
    let none = BTreeSet::new();
    let contaminated = pane_map(&derive_bindings("b", &comments, &none));
    assert_eq!(contaminated.len(), 2);
    assert_eq!(contaminated["%1408"].len(), 2);

    // The roster is DERIVED from the bindings. There is no setter, by design.
    assert_eq!(
        roster_from(&bindings),
        ["AmberGate".to_owned()].into_iter().collect::<BTreeSet<_>>()
    );
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 3 + 7 — liveness from tick-monitor only, and one bead resolves end to end
// -----------------------------------------------------------------------------------

#[test]
fn a_correctly_bound_bead_resolves_end_to_end() {
    // POSITIVE CONTROL. Without this the checker could report every bead unanswered and be
    // indistinguishable from a tracker that is entirely broken.
    let row = bead(
        "omp-orchestrator-gfm6",
        "in_progress",
        "AmberGate",
        &[("AmberGate", "ACK gfm6 on %1408")],
    );
    match resolve(&row, &roster(), &defaults(), &live) {
        HolderVerdict::Bound {
            agent,
            pane_id,
            liveness,
            ..
        } => {
            assert_eq!(agent, "AmberGate");
            assert_eq!(pane_id, "%1408");
            assert_eq!(liveness.state, "WORKING");
            assert_eq!(liveness.source, LIVENESS_SOURCE);
        }
        other => panic!("expected Bound, got {other:?}"),
    }
    let text = resolve(&row, &roster(), &defaults(), &live).to_string();
    for needle in ["HOLDER ", "agent=AmberGate", "pane=%1408", "source=tick-monitor"] {
        assert!(text.contains(needle), "{text}");
    }
}

#[test]
fn a_roster_last_active_can_never_produce_a_bound_verdict() {
    // ACCEPTANCE 3's teeth. Agent Mail's roster read `last_active = 15h ago` for an agent
    // that had committed 20 minutes earlier, and `2d` for two agents actively working
    // beads. Any provenance other than tick-monitor is REFUSED into LivenessUnavailable —
    // mechanically, not by convention.
    let row = bead(
        "b",
        "in_progress",
        "AmberGate",
        &[("AmberGate", "ACK b on %1408")],
    );
    let roster_source = |_pane: &str| {
        Some(Liveness {
            state: "ACTIVE".to_owned(),
            source: "agent-mail last_active".to_owned(),
        })
    };
    let verdict = resolve(&row, &roster(), &defaults(), &roster_source);
    assert!(!verdict.is_answered(), "a roster answer must not count");
    match &verdict {
        HolderVerdict::LivenessUnavailable { detail, .. } => {
            assert!(detail.contains("15h wrong"), "{detail}");
        }
        other => panic!("expected LivenessUnavailable, got {other:?}"),
    }

    // And a pane tick-monitor says nothing about is also unavailable, not alive.
    let unknown = bead(
        "b",
        "in_progress",
        "SilverWolf",
        &[("SilverWolf", "ACK b on %1409")],
    );
    assert!(matches!(
        resolve(&unknown, &roster(), &defaults(), &live),
        HolderVerdict::LivenessUnavailable { .. }
    ));
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 4 — both illegal states, fires-on-known-bad for EACH
// -----------------------------------------------------------------------------------

#[test]
fn both_illegal_states_are_found_and_a_legal_bead_produces_nothing() {
    // FIRES-ON-KNOWN-BAD, one fixture per state.
    assert_eq!(
        classify_state("b1", "open", "AmberGate"),
        Some(StateFinding::HalfClaim {
            bead_id: "b1".to_owned(),
            assignee: "AmberGate".to_owned()
        })
    );
    assert_eq!(
        classify_state("b2", "in_progress", ""),
        Some(StateFinding::OrphanClaim {
            bead_id: "b2".to_owned()
        })
    );
    assert_eq!(
        classify_state("b3", "in_progress", "   "),
        Some(StateFinding::OrphanClaim {
            bead_id: "b3".to_owned()
        }),
        "whitespace is unassigned; a space is not an owner"
    );

    // KNOWN-GOOD: both legal pairs produce NO finding. A checker that flags legal states
    // is routed around within a day.
    assert_eq!(classify_state("b4", "in_progress", "AmberGate"), None);
    assert_eq!(classify_state("b5", "open", ""), None);
    assert_eq!(classify_state("b6", "closed", "AmberGate"), None);

    // The two findings are DISTINCT and each names its own repair, because the repairs are
    // opposite: one adds a status, the other removes an assignee.
    let half = classify_state("b1", "open", "AmberGate").unwrap().to_string();
    let orphan = classify_state("b2", "in_progress", "").unwrap().to_string();
    assert!(half.contains("HALF_CLAIM") && half.contains("next_action="), "{half}");
    assert!(orphan.contains("ORPHAN_CLAIM") && orphan.contains("next_action="), "{orphan}");
    assert_ne!(half, orphan);

    // And an illegal state pre-empts the holder question: a bead in an illegal state has no
    // well-defined holder, so reporting one would be a guess.
    let row = bead("b1", "open", "AmberGate", &[("AmberGate", "ACK b1 on %1408")]);
    assert!(matches!(
        resolve(&row, &roster(), &defaults(), &live),
        HolderVerdict::Illegal(StateFinding::HalfClaim { .. })
    ));
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 5 — ambiguity is NAMED, never guessed
// -----------------------------------------------------------------------------------

#[test]
fn two_agents_on_one_bead_produce_a_typed_ambiguous_with_both_candidates() {
    // MEASURED: two live `in_progress` beads carry ACKs from two different panes by two
    // different agents. A silent pick is what produced two agent names on one pane.
    let row = bead(
        "omp-orchestrator-eg0m",
        "in_progress",
        "AmberGate",
        &[
            ("AmberGate", "ACK eg0m on %1408"),
            ("BlueLantern", "ACK eg0m on %1414"),
        ],
    );
    match resolve(&row, &roster(), &defaults(), &live) {
        HolderVerdict::Ambiguous { candidates, .. } => {
            assert_eq!(candidates.len(), 2, "both candidates must be carried");
            let text = resolve(&row, &roster(), &defaults(), &live).to_string();
            assert!(text.contains("HOLDER_AMBIGUOUS"), "{text}");
            assert!(text.contains("AmberGate@%1408"), "{text}");
            assert!(text.contains("BlueLantern@%1414"), "{text}");
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }

    // The assignee field disagreeing with the ACK is the same shape and gets the same
    // treatment: two claims on one bead, neither picked.
    let disagree = bead(
        "b",
        "in_progress",
        "SilverWolf",
        &[("AmberGate", "ACK b on %1408")],
    );
    assert!(matches!(
        resolve(&disagree, &roster(), &defaults(), &live),
        HolderVerdict::Ambiguous { .. }
    ));
}

#[test]
fn a_bead_with_no_ack_is_named_unanswered_and_not_assumed_alive() {
    // The ACK protocol went live 2026-09-02 and most in-flight beads predate it: measured,
    // 66 of 85 have no ACK evidence. That is a NAMED gap, not an error and not a holder.
    let row = bead("b", "in_progress", "AmberGate", &[]);
    match resolve(&row, &roster(), &defaults(), &live) {
        HolderVerdict::NoPaneEvidence { assignee, .. } => assert_eq!(assignee, "AmberGate"),
        other => panic!("expected NoPaneEvidence, got {other:?}"),
    }
    assert!(!resolve(&row, &roster(), &defaults(), &live).is_answered());
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 6 — anti-vacuity
// -----------------------------------------------------------------------------------

#[test]
fn an_empty_scan_is_an_error_and_never_a_clean_bill() {
    // Zero in-flight beads is indistinguishable from a broken query, so it must not pass.
    assert_eq!(
        audit(&[], &roster(), &defaults(), &live).err(),
        Some(AuditError::EmptyScan)
    );
    // A tracker with only closed beads is the same case: nothing in flight to check.
    let closed = [bead("b", "closed", "AmberGate", &[])];
    assert_eq!(
        audit(&closed, &roster(), &defaults(), &live).err(),
        Some(AuditError::EmptyScan)
    );
    assert!(audit(&[], &roster(), &defaults(), &live)
        .expect_err("an empty scan must be an error")
        .to_string()
        .contains("EMPTY_SCAN"));

    // POSITIVE CONTROL: one in-flight bead DOES produce an audit, so the error above is
    // about emptiness and not about `audit` never succeeding.
    let one = [bead(
        "b",
        "in_progress",
        "AmberGate",
        &[("AmberGate", "ACK b on %1408")],
    )];
    let report = audit(&one, &roster(), &defaults(), &live).expect("one bead audits");
    assert_eq!(report.verdicts.len(), 1);
    assert_eq!(report.answered(), 1);

    // An `open`+assigned bead is IN SCOPE even though it is not in_progress — it is one of
    // the two illegal states, and a scan keyed only on in_progress could never see it.
    let half = [bead("b1", "open", "AmberGate", &[])];
    let report = audit(&half, &roster(), &defaults(), &live).expect("half-claims are in scope");
    assert_eq!(report.by_label().get("illegal_state").copied(), Some(1));
    assert_eq!(report.answered(), 0);
}
