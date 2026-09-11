#![forbid(unsafe_code)]

//! L5 portal-row contract legs for van0 (sources), cqwo (alerts), qcev (one next action).
//!
//! Every law here has a KNOWN-BAD that names the defect and a KNOWN-GOOD that keeps the
//! predicate from being merely restrictive. The reality control is
//! [`live_sources`] / [`live_alerts`] / [`live_next_action`]: the VERBATIM payload read
//! back from `ompo portal --json` on 2026-09-11, so a predicate that refuses the shipped
//! emitter fails here rather than in grading.

use ompo_start::portal_contract::{
    alerts_are_complete, degraded_sources, source_is_healthy, validate_one_next_action,
    validate_sources, AlertDefect, NextActionDefect, SourceDefect, ALERT_FIELDS, EXPECTED_SOURCES,
    HUMAN_HALT, NEXT_ACTION_FIELDS,
};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Reality controls. Measured, not invented:
//   ompo portal --json | jq -c '.data.sources'
//   ompo portal --json | jq -c '.data._alerts'
//   ompo portal --json | jq -c '.data.one_next_action'
// Note the `.data.` prefix: the top-level envelope keys are
// ["command","data","schema_version","status"], so the bead text's bare `.sources`
// reads null. That is a wrong-path null, not an unwired field.
// ---------------------------------------------------------------------------

fn live_sources() -> Value {
    json!({
        "agent-mail": {
            "age_ms": 12, "available": true, "fresh": true,
            "panes": [], "reason_code": "L4_MAIL_SOURCE"
        },
        "ntm": {
            "age_ms": null, "available": false, "fresh": false, "panes": [],
            "reason_code": "L4_NTM_UNAVAILABLE detail=ntm timed out after 10s"
        },
        "tick-monitor": {
            "age_ms": 5_035_000u64, "available": true, "fresh": false,
            "panes": ["%19", "%20"], "reason_code": "L4_TICK_SOURCE"
        },
    })
}

/// Every expected source, fully healthy. The baseline a "nothing is wrong" row must take.
fn healthy_sources() -> Value {
    let mut map = serde_json::Map::new();
    for name in EXPECTED_SOURCES {
        map.insert(
            name.to_owned(),
            json!({"age_ms": 5u64, "available": true, "fresh": true, "panes": [],
                   "reason_code": "L4_OK"}),
        );
    }
    Value::Object(map)
}

fn sources_with(name: &str, row: Value) -> Value {
    let mut base = healthy_sources();
    base.as_object_mut()
        .expect("object")
        .insert(name.to_owned(), row);
    base
}

// ===========================================================================
// van0 -- L5-SOURCE: per-source available/fresh/reason_code/age_ms
// ===========================================================================

/// KNOWN-GOOD, and the one that matters most: the predicate accepts the payload the
/// SHIPPED binary actually emits. A contract the emitter cannot satisfy is not a
/// contract, it is a wish.
#[test]
fn live_portal_sources_satisfy_the_contract() {
    validate_sources(&live_sources()).expect("the shipped emitter must satisfy its own contract");
    let measured = live_sources();
    let mut keys: Vec<&str> = measured
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["agent-mail", "ntm", "tick-monitor"],
        "the measured key set is the contract's expected set"
    );
}

/// KNOWN-GOOD: a fully healthy population passes. Without this the predicate could be
/// unconditionally refusing and every known-bad below would still be green.
#[test]
fn a_healthy_population_passes() {
    validate_sources(&healthy_sources()).expect("all three healthy must pass");
    assert!(degraded_sources(&healthy_sources()).is_empty());
}

/// ⛔ KNOWN-BAD, THE HEADLINE: an ABSENT source must not read as available. Drop `ntm`
/// and leave the remaining two perfectly healthy -- a predicate that only iterates the
/// rows it was handed sees two healthy sources and reports health.
#[test]
fn an_absent_source_refuses_instead_of_reading_as_healthy() {
    let mut shrunk = healthy_sources();
    shrunk.as_object_mut().expect("object").remove("ntm");
    assert_eq!(
        shrunk.as_object().expect("object").len(),
        2,
        "the fixture must actually be missing one source"
    );
    assert!(
        shrunk
            .as_object()
            .expect("object")
            .values()
            .all(source_is_healthy),
        "every REMAINING row is healthy -- absence is the only defect"
    );
    assert_eq!(
        validate_sources(&shrunk),
        Err(SourceDefect::MissingSource { name: "ntm" })
    );
}

/// KNOWN-BAD: zero sources. An empty scan set is an ERROR, never a pass.
#[test]
fn an_empty_source_map_is_an_error_not_a_pass() {
    assert_eq!(validate_sources(&json!({})), Err(SourceDefect::EmptyScan));
}

/// KNOWN-BAD: `reason_code` must be populated exactly when it is needed. An unavailable
/// source with a blank reason reports a failure and withholds why.
#[test]
fn an_unavailable_source_without_a_reason_refuses() {
    for blank in ["", "   "] {
        let bad = sources_with(
            "ntm",
            json!({"age_ms": null, "available": false, "fresh": false, "panes": [],
                   "reason_code": blank}),
        );
        assert_eq!(
            validate_sources(&bad),
            Err(SourceDefect::UnavailableWithoutReason {
                source: "ntm".to_owned()
            }),
            "blank reason {blank:?} must refuse"
        );
    }
}

/// KNOWN-BAD: the other direction of missing-collapsing-to-healthy. A source cannot
/// report itself unavailable and fresh at the same time.
#[test]
fn an_unavailable_source_cannot_claim_freshness() {
    let bad = sources_with(
        "ntm",
        json!({"age_ms": null, "available": false, "fresh": true, "panes": [],
               "reason_code": "L4_NTM_UNAVAILABLE"}),
    );
    assert_eq!(
        validate_sources(&bad),
        Err(SourceDefect::UnavailableButFresh {
            source: "ntm".to_owned()
        })
    );
}

/// KNOWN-BAD: each required field is individually load-bearing. Dropping any one of the
/// four names that field, so a row cannot lose a column silently.
#[test]
fn every_required_source_field_is_individually_required() {
    for field in ["available", "fresh", "reason_code", "age_ms"] {
        let mut row = json!({"age_ms": 1u64, "available": true, "fresh": true, "panes": [],
                             "reason_code": "L4_OK"});
        row.as_object_mut().expect("object").remove(field);
        assert_eq!(
            validate_sources(&sources_with("ntm", row)),
            Err(SourceDefect::MissingField {
                source: "ntm".to_owned(),
                field
            }),
            "dropping {field} must be named"
        );
    }
}

/// KNOWN-BAD: a string "false" is not a boolean false, and a shape defect must not be
/// silently coerced into a value.
#[test]
fn source_flags_must_be_booleans_and_the_reason_a_string() {
    let not_bool = sources_with(
        "ntm",
        json!({"age_ms": 1u64, "available": "false", "fresh": true, "panes": [],
               "reason_code": "L4_OK"}),
    );
    assert_eq!(
        validate_sources(&not_bool),
        Err(SourceDefect::NotABool {
            source: "ntm".to_owned(),
            field: "available"
        })
    );
    let not_string = sources_with(
        "ntm",
        json!({"age_ms": 1u64, "available": true, "fresh": true, "panes": [],
               "reason_code": 42}),
    );
    assert_eq!(
        validate_sources(&not_string),
        Err(SourceDefect::ReasonNotAString {
            source: "ntm".to_owned()
        })
    );
}

/// KNOWN-BAD: the population is set EQUALITY. A source nobody expects means the map
/// drifted, and drift is reported rather than absorbed.
#[test]
fn an_unexpected_source_refuses() {
    let drifted = sources_with("ghost", json!({"age_ms": 1u64, "available": true,
                                               "fresh": true, "panes": [], "reason_code": "X"}));
    assert_eq!(
        validate_sources(&drifted),
        Err(SourceDefect::UnknownSource {
            name: "ghost".to_owned()
        })
    );
}

/// KNOWN-BAD: the field itself must be an object, not an array of rows.
#[test]
fn a_non_object_source_field_refuses() {
    assert_eq!(validate_sources(&json!([])), Err(SourceDefect::NotAnObject));
    assert_eq!(
        validate_sources(&Value::Null),
        Err(SourceDefect::NotAnObject)
    );
}

// ===========================================================================
// cqwo -- L5-ALERT: _alerts[] severity + summary + action
// ===========================================================================

/// The VERBATIM `.data._alerts` array read back from the installed binary on
/// 2026-09-11:
///   ompo portal --json | jq -c '.data._alerts'
/// Two entries, because `ntm` was unavailable (severity error) and
/// `tick-monitor` was available but stale (severity warn). Pairs with
/// [`live_sources`], which is the same run's `.data.sources`.
fn live_alerts() -> Value {
    json!([
        {
            "action": "ompo portal --json --session omp-orchestrator",
            "severity": "error",
            "summary": "ntm source is not fresh and available"
        },
        {
            "action": "ompo portal --json --session omp-orchestrator",
            "severity": "warn",
            "summary": "tick-monitor source is not fresh and available"
        }
    ])
}

/// One complete alert naming the source it is about.
fn alert_for(source: &str, severity: &str) -> Value {
    json!({
        "severity": severity,
        "summary": format!("{source} source is not fresh and available"),
        "action": "ompo portal --json --session omp-orchestrator",
    })
}

/// The alerts a fully degraded [`live_sources`] population owes, with ONE key
/// overridden -- the vehicle for every per-field known-bad below.
fn live_alerts_with(index: usize, field: &str, value: Value) -> Value {
    let mut alerts = live_alerts();
    let row = alerts.as_array_mut().expect("array")[index]
        .as_object_mut()
        .expect("object");
    if value.is_null() {
        row.remove(field);
    } else {
        row.insert(field.to_owned(), value);
    }
    alerts
}

/// KNOWN-GOOD, the reality control: the alerts the SHIPPED binary emits satisfy
/// the predicate, against that same run's sources. A contract the emitter
/// cannot satisfy is a wish.
#[test]
fn live_portal_alerts_satisfy_the_contract() {
    assert_eq!(
        alerts_are_complete(&live_alerts(), &live_sources()),
        Ok(()),
        "the shipped emitter must satisfy its own contract"
    );
}

/// KNOWN-GOOD, and the reason the cross-check is not merely restrictive: an
/// EMPTY array is correct when nothing is degraded. The predicate must not
/// demand alerts on a healthy system.
#[test]
fn an_empty_alert_array_passes_on_a_healthy_population() {
    assert_eq!(alerts_are_complete(&json!([]), &healthy_sources()), Ok(()));
    assert!(
        degraded_sources(&healthy_sources()).is_empty(),
        "the control is only meaningful if nothing is degraded"
    );
}

/// ⛔ KNOWN-BAD, THE HEADLINE (cqwo): an alert with an EMPTY `action` announces
/// a problem and withholds the remedy. Everything else about the entry is
/// perfect, so nothing but the action clause can be refusing it.
#[test]
fn an_alert_with_an_empty_action_refuses() {
    assert_eq!(
        alerts_are_complete(&live_alerts_with(0, "action", json!("")), &live_sources()),
        Err(AlertDefect::Empty {
            index: 0,
            field: "action"
        })
    );
    // Whitespace is not an action either.
    assert_eq!(
        alerts_are_complete(&live_alerts_with(1, "action", json!("   ")), &live_sources()),
        Err(AlertDefect::Empty {
            index: 1,
            field: "action"
        })
    );
}

/// KNOWN-BAD: each of the three keys is individually load-bearing, so an entry
/// cannot lose a column silently. Dropping any one names that one.
#[test]
fn every_alert_field_is_individually_required() {
    for field in ALERT_FIELDS {
        // `null` is the remove sentinel in `live_alerts_with`.
        let defect = alerts_are_complete(&live_alerts_with(0, field, Value::Null), &live_sources());
        assert_eq!(
            defect,
            Err(AlertDefect::MissingField { index: 0, field }),
            "dropping {field} must name {field}"
        );
        let empty = alerts_are_complete(&live_alerts_with(0, field, json!("")), &live_sources());
        assert_eq!(
            empty,
            Err(AlertDefect::Empty { index: 0, field }),
            "blanking {field} must name {field}"
        );
    }
}

/// ⛔ KNOWN-BAD, the clause that defeats a hard-wired `[]`: a DEGRADED source
/// with no alert is a silent failure. Every per-entry rule above is vacuously
/// satisfied by an emitter that never emits; only the cross-check against the
/// sources catches it.
#[test]
fn a_degraded_source_with_no_alert_is_silent_degradation() {
    let degraded = degraded_sources(&live_sources());
    assert_eq!(
        degraded,
        vec!["ntm".to_owned(), "tick-monitor".to_owned()],
        "the fixture must actually be degraded or this leg proves nothing"
    );
    assert_eq!(
        alerts_are_complete(&json!([]), &live_sources()),
        Err(AlertDefect::SilentDegradation {
            source: "ntm".to_owned()
        }),
        "an emitter hard-wired to [] must refuse against a degraded population"
    );
    // And a PARTIAL alert set is caught too: alerting on one degradation and
    // not the other is the same defect, one source later.
    assert_eq!(
        alerts_are_complete(&json!([alert_for("ntm", "error")]), &live_sources()),
        Err(AlertDefect::SilentDegradation {
            source: "tick-monitor".to_owned()
        })
    );
}

/// KNOWN-BAD: a shape defect must not be coerced into a value. A numeric
/// action is not an action, and `severity: false` is not a severity.
#[test]
fn alert_fields_must_be_strings() {
    assert_eq!(
        alerts_are_complete(&live_alerts_with(0, "action", json!(0)), &live_sources()),
        Err(AlertDefect::NotAString {
            index: 0,
            field: "action"
        })
    );
    assert_eq!(
        alerts_are_complete(&live_alerts_with(0, "severity", json!(false)), &live_sources()),
        Err(AlertDefect::NotAString {
            index: 0,
            field: "severity"
        })
    );
}

/// KNOWN-BAD: severity is a closed vocabulary. A free-text severity reads as
/// triaged while being unsortable.
#[test]
fn an_unknown_severity_refuses() {
    assert_eq!(
        alerts_are_complete(&live_alerts_with(0, "severity", json!("kinda bad")), &live_sources()),
        Err(AlertDefect::UnknownSeverity {
            index: 0,
            severity: "kinda bad".to_owned()
        })
    );
    // KNOWN-GOOD counterpart: both emitted severities are accepted, so the
    // vocabulary check is not simply rejecting the second one.
    assert_eq!(
        alerts_are_complete(
            &json!([alert_for("ntm", "error"), alert_for("tick-monitor", "warn")]),
            &live_sources()
        ),
        Ok(())
    );
}

/// KNOWN-BAD: the field itself must be an array of objects.
#[test]
fn a_non_array_alerts_field_refuses() {
    assert_eq!(
        alerts_are_complete(&json!({}), &healthy_sources()),
        Err(AlertDefect::NotAnArray)
    );
    assert_eq!(
        alerts_are_complete(&Value::Null, &healthy_sources()),
        Err(AlertDefect::NotAnArray)
    );
    assert_eq!(
        alerts_are_complete(&json!(["ntm is stale"]), &healthy_sources()),
        Err(AlertDefect::NotAnObject { index: 0 })
    );
}

// ===========================================================================
// qcev -- L5-ONE-NEXT: exactly one {command, reason_code} or HUMAN HALT
// ===========================================================================

/// The `.data.one_next_action` read back on 2026-09-11:
///   ompo portal --json | jq -c '.data.one_next_action'
///
/// ONE SUBSTITUTION, declared: the measured `--repo` argument was the author
/// machine's absolute checkout path, which `path-literal-guard` refuses in a
/// committed source file. It is replaced by `/repo` here. Nothing the
/// predicate examines depends on the value -- `command` is checked for being
/// a non-empty string, not for its contents -- so the substitution cannot
/// make a failing cursor pass. Everything else, including the reason_code,
/// is verbatim.
fn live_next_action() -> Value {
    json!({
        "command": "ompo start --repo /repo --session omp-orchestrator --json",
        "reason_code": "L4_SILENT_SOURCE source=ntm"
    })
}

/// KNOWN-GOOD, the reality control: the cursor the SHIPPED binary emits passes.
#[test]
fn live_portal_one_next_action_satisfies_the_contract() {
    assert_eq!(validate_one_next_action(&live_next_action()), Ok(()));
}

/// KNOWN-GOOD: a HUMAN HALT is still ONE object with a populated command, so
/// the halt path does not degrade into an absent cursor.
#[test]
fn a_human_halt_is_still_one_object() {
    assert_eq!(
        validate_one_next_action(&json!({
            "command": "HUMAN HALT: L5 decision owed, escalate to S9",
            "reason_code": HUMAN_HALT
        })),
        Ok(())
    );
}

/// ⛔ CARDINALITY IN ALL THREE DIRECTIONS. "Exactly one" is a claim about zero
/// and two as much as about one; a predicate that only asks "is there at least
/// one" satisfies the title and not the contract.
#[test]
fn cardinality_refuses_zero_and_two_and_accepts_one() {
    // ZERO: absent or null.
    assert_eq!(
        validate_one_next_action(&Value::Null),
        Err(NextActionDefect::Absent)
    );
    // TWO.
    assert_eq!(
        validate_one_next_action(&json!([live_next_action(), live_next_action()])),
        Err(NextActionDefect::NotAnObject { len: 2 })
    );
    // ONE.
    assert_eq!(validate_one_next_action(&live_next_action()), Ok(()));
}

/// ⛔ KNOWN-BAD, the one a count-only predicate lets through: a SINGLETON ARRAY.
/// The count is one, and it still refuses, because "a list is a protocol bug"
/// is a statement about the CARRIER. An empty array is the same refusal from
/// the other end.
#[test]
fn a_singleton_array_refuses_because_the_carrier_is_a_list() {
    assert_eq!(
        validate_one_next_action(&json!([live_next_action()])),
        Err(NextActionDefect::NotAnObject { len: 1 })
    );
    assert_eq!(
        validate_one_next_action(&json!([])),
        Err(NextActionDefect::NotAnObject { len: 0 })
    );
}

/// KNOWN-BAD: both keys are individually load-bearing. A command with no
/// reason_code is an instruction with no justification; a reason_code with no
/// command is a diagnosis with no remedy.
#[test]
fn every_next_action_field_is_individually_required() {
    for field in NEXT_ACTION_FIELDS {
        let mut row = live_next_action();
        row.as_object_mut().expect("object").remove(field);
        assert_eq!(
            validate_one_next_action(&row),
            Err(NextActionDefect::MissingField { field }),
            "dropping {field} must name {field}"
        );

        let mut blank = live_next_action();
        blank[field] = json!("  ");
        assert_eq!(
            validate_one_next_action(&blank),
            Err(NextActionDefect::Empty { field }),
            "blanking {field} must name {field}"
        );

        let mut typed = live_next_action();
        typed[field] = json!(7);
        assert_eq!(
            validate_one_next_action(&typed),
            Err(NextActionDefect::NotAString { field }),
            "a non-string {field} is a shape defect, not a value"
        );
    }
}

/// KNOWN-BAD: a bare string cursor. Not an object, not an array, not null.
#[test]
fn a_scalar_next_action_refuses() {
    assert_eq!(
        validate_one_next_action(&json!("ompo start")),
        Err(NextActionDefect::NotAnObjectScalar)
    );
}

/// The predicate's own no-op guard: `source_is_healthy` is the SAME condition
/// the emitter uses to decide whether to raise an alert. If that ever drifts,
/// the cross-check in `alerts_are_complete` stops meaning anything, so it is
/// pinned here rather than trusted.
#[test]
fn the_alert_cross_check_uses_the_emitters_own_health_condition() {
    let row = |available: bool, fresh: bool, age: Value| {
        json!({"available": available, "fresh": fresh, "age_ms": age, "panes": [],
               "reason_code": "L4_OK"})
    };
    assert!(source_is_healthy(&row(true, true, json!(5))));
    assert!(!source_is_healthy(&row(false, true, json!(5))));
    assert!(!source_is_healthy(&row(true, false, json!(5))));
    assert!(
        !source_is_healthy(&row(true, true, Value::Null)),
        "available and fresh with NO age is not health -- a claim with no reading behind it"
    );
}

// ===========================================================================
// j4ert, site 6 -- THE PIN THAT A COLLAPSE CANNOT GIVE.
//
// `source_is_healthy` is the silence law over an EMITTED JSON ROW;
// `liveness::is_silent` is the same law over a `SourceVerdict`. Two
// representations of one law, which is legitimate -- the contract validates
// the WIRE shape and cannot take a typed verdict -- but only if something
// holds them together. Nothing did.
//
// ⛔ WHY THIS MATTERS TO cqwo SPECIFICALLY, and it is a finding against my own
// graded work: `alerts_are_complete` refuses a degraded source that no alert
// names. The EMITTER decided degradation with its own inline copy and this
// PREDICATE decides it with `source_is_healthy`. The cross-check compared two
// independent copies of the law, and every leg above stays green if they
// drift, because they all feed BOTH sides fixtures built by hand. The emitter
// side is now collapsed onto `is_silent`; this pins the remaining pair.
// ===========================================================================

/// Every combination of the three inputs the law reads. Exhaustive rather than
/// sampled: eight rows is the whole domain, so a drift cannot hide in the
/// combination nobody thought to write down.
#[test]
fn the_wire_predicate_and_the_typed_predicate_agree_on_every_input() {
    let mut checked = 0;
    for available in [true, false] {
        for fresh in [true, false] {
            for age_ms in [Some(7_u64), None] {
                let verdict = ompo_start::liveness::SourceVerdict {
                    name: "ntm".to_owned(),
                    available,
                    fresh,
                    reason_code: "L4_PIN".to_owned(),
                    age_ms,
                    panes: Vec::new(),
                };
                let row = ompo_start::liveness::source_json(&verdict);
                let typed_silent = ompo_start::liveness::is_silent(&verdict);

                assert_eq!(
                    source_is_healthy(&row),
                    !typed_silent,
                    "the wire predicate and the typed predicate disagree on \
                     available={available} fresh={fresh} age_ms={age_ms:?}: row={row}"
                );
                // And the row's OWN `silent` flag is the third encoding a
                // reader might trust. Pinned to the same answer, so a consumer
                // reading the flag and a consumer running the predicate cannot
                // reach different conclusions about one row.
                assert_eq!(
                    row["silent"], typed_silent,
                    "the emitted silent flag must equal the predicate that produced it"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 8, "the domain is three booleans-worth of input");
}

/// KNOWN-GOOD anchor for the pin above: the two predicates are not agreeing
/// vacuously by both being constant. Exactly ONE of the eight rows is healthy,
/// and it is the all-true one.
#[test]
fn the_pin_is_not_vacuous_exactly_one_input_is_healthy() {
    let healthy = ompo_start::liveness::SourceVerdict {
        name: "ntm".to_owned(),
        available: true,
        fresh: true,
        reason_code: "L4_OK".to_owned(),
        age_ms: Some(7),
        panes: Vec::new(),
    };
    assert!(source_is_healthy(&ompo_start::liveness::source_json(&healthy)));
    assert!(!ompo_start::liveness::is_silent(&healthy));

    for (available, fresh, age_ms) in [
        (false, true, Some(7_u64)),
        (true, false, Some(7)),
        (true, true, None),
    ] {
        let degraded = ompo_start::liveness::SourceVerdict {
            available,
            fresh,
            age_ms,
            ..healthy.clone()
        };
        assert!(
            !source_is_healthy(&ompo_start::liveness::source_json(&degraded)),
            "each of the three clauses alone must make a row unhealthy"
        );
        assert!(ompo_start::liveness::is_silent(&degraded));
    }
}
