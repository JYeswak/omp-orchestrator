#![forbid(unsafe_code)]

//! L4-SPAWN: `am robot agents` must list every spawned pane id before tick_zero.
//! A missing registration fails. An empty roster is an error, never a pass.

use ompo_start::spawn_mail::{
    parse_roster, require_registered_before_tick_zero, spawn_requires_mail_registration,
    MailRegError, ROSTER_SOURCE,
};

/// Shape measured against the installed `am robot agents` on 2026-09-11:
/// a `_meta` object, a `count`, and `agents[]` rows keyed
/// last_active/model/msg_count/name/program/status.
fn roster_json(rows: &[(&str, &str)]) -> String {
    let agents = rows
        .iter()
        .map(|(name, task)| {
            format!(
                r#"{{"name":"{name}","program":"ntm","model":"coordinator","msg_count":0,"status":"active","last_active":"1m ago","task":"{task}"}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"_meta":{{"command":"robot agents","format":"json","version":"1.0"}},"count":{},"agents":[{agents}]}}"#,
        rows.len()
    )
}

fn panes(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn spawned_panes_are_listed_by_am_robot_agents() {
    let roster = roster_json(&[
        ("WaveScout", "pane=%1408 tick_zero pending"),
        ("WaveFalcon", "pane=%1409 tick_zero pending"),
    ]);
    let receipts = spawn_requires_mail_registration(&roster, &panes(&["%1408", "%1409"]))
        .expect("every spawned pane is registered before tick_zero");
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].pane_id, "%1408");
    assert_eq!(receipts[0].agent_name, "WaveScout");
    assert_eq!(receipts[1].pane_id, "%1409");
    assert_eq!(receipts[1].agent_name, "WaveFalcon");
    println!(
        "registered source={ROSTER_SOURCE} {} {}",
        receipts[0].pane_id, receipts[1].pane_id
    );
}

#[test]
fn missing_register_fails() {
    // Pane id in the NAME, so this leg is independent of the task-field scan:
    // a mutation to either field's scan reddens its own leg, not both.
    let roster = roster_json(&[("pane-%1408-worker", "tick_zero pending")]);
    match spawn_requires_mail_registration(&roster, &panes(&["%1408", "%1409"])) {
        Err(MailRegError::PaneNotRegistered {
            reason_code,
            pane_id,
            source,
            agent_count,
        }) => {
            assert_eq!(reason_code, "L4_MAIL_PANE_NOT_REGISTERED");
            assert_eq!(pane_id, "%1409");
            assert_eq!(source, ROSTER_SOURCE);
            assert_eq!(agent_count, 1);
            println!("missing register refused pane={pane_id}");
        }
        other => panic!("a pane absent from the roster must fail, got {other:?}"),
    }
}

#[test]
fn registration_name_alone_satisfies_the_readback() {
    // Registration may carry the pane id in --name instead of --task.
    let roster = roster_json(&[("pane-%1408-worker", "no pane token here")]);
    let receipts = spawn_requires_mail_registration(&roster, &panes(&["%1408"]))
        .expect("pane id carried in the agent name is a registration");
    assert_eq!(receipts[0].pane_id, "%1408");
}

#[test]
fn empty_roster_is_an_error_not_a_pass() {
    for stdout in [
        r#"{"_meta":{"command":"robot agents"},"count":0,"agents":[]}"#,
        r#"{"_meta":{"command":"robot agents"},"count":0}"#,
    ] {
        match parse_roster(stdout) {
            Err(MailRegError::RosterEmpty { reason_code }) => {
                assert_eq!(reason_code, "L4_MAIL_ROSTER_EMPTY");
            }
            other => panic!("an empty roster must refuse, got {other:?}"),
        }
    }
    // And the whole spawn seam must refuse, not silently report zero failures.
    let err = spawn_requires_mail_registration(
        r#"{"agents":[]}"#,
        &panes(&["%1408"]),
    )
    .expect_err("empty roster cannot witness a registered pane");
    assert_eq!(err.reason_code(), "L4_MAIL_ROSTER_EMPTY");
}

#[test]
fn empty_spawn_wave_is_an_error_not_a_pass() {
    let roster = parse_roster(&roster_json(&[("WaveScout", "pane=%1408")])).expect("roster");
    let err = require_registered_before_tick_zero(&[], &roster)
        .expect_err("an empty wave proves nothing and must not pass");
    assert_eq!(err.reason_code(), "L4_MAIL_EMPTY_WAVE");
}

#[test]
fn unparsable_roster_is_an_error() {
    let err = parse_roster("not json at all").expect_err("unparsable roster must refuse");
    assert_eq!(err.reason_code(), "L4_MAIL_ROSTER_UNPARSABLE");
}

#[test]
fn pane_id_must_match_as_a_whole_token() {
    // %140 must not satisfy %1408, and %14080 must not satisfy %1408.
    let roster = roster_json(&[("WaveScout", "pane=%140 pane=%14080")]);
    let err = spawn_requires_mail_registration(&roster, &panes(&["%1408"]))
        .expect_err("a prefix or extension of the pane id is not that pane");
    assert_eq!(err.reason_code(), "L4_MAIL_PANE_NOT_REGISTERED");
}
