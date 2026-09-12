//! LAW-L3-SKIP-STAYS: Skipped / NotApplicable stay in STEPS; view() does not drop them.

use ompo_start::{
    apply_predicates, check_id_parity, fixture_steps, json_next_command, json_ordered_ids,
    next_command, next_step, ordered_ids, tui_next_command, tui_ordered_ids, view, Predicate,
    Step, StepStatus,
};


use serde::Deserialize;

use serde_json::Value;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct StepWire {
    id: String,
    title: String,
    status: StepStatus,
    reason_code: Value,
    next_command: Option<String>,
    predicate: Predicate,
}

#[test]
fn step_fields_round_trip() {
    let step = Step {
        id: "L3-STEP",
        title: "step carrier",
        status: StepStatus::Pending,
        reason_code: Some("L3-PENDING"),
        next_command: Some("ompo start --json"),
        predicate: Predicate::Always,
    };

    let encoded = serde_json::to_value(&step).expect("Step must serialize");
    let object = encoded.as_object().expect("Step wire value must be an object");
    let keys: std::collections::BTreeSet<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "id",
            "title",
            "status",
            "reason_code",
            "next_command",
            "predicate",
        ]
        .into_iter()
        .collect()
    );

    let decoded: StepWire = serde_json::from_value(encoded.clone()).expect("Step must round-trip");
    assert_eq!(
        decoded,
        StepWire {
            id: "L3-STEP".to_owned(),
            title: "step carrier".to_owned(),
            status: StepStatus::Pending,
            reason_code: serde_json::json!("L3-PENDING"),
            next_command: Some("ompo start --json".to_owned()),
            predicate: Predicate::Always,
        }
    );

    let mut missing_reason = encoded;
    missing_reason
        .as_object_mut()
        .expect("Step wire value must be an object")
        .remove("reason_code");
    let error = serde_json::from_value::<StepWire>(missing_reason)
        .expect_err("omitting reason_code must fail");
    assert!(error.to_string().contains("reason_code"), "{error}");
}
#[test]
fn skipped_steps_remain_in_both_renders() {
    let mut steps = fixture_steps();
    let before = steps.len();
    apply_predicates(&mut steps, false, false, false);

    assert_eq!(
        steps.len(),
        before,
        "predicates set status; they must not shrink STEPS"
    );
    let spawn = steps
        .iter()
        .find(|step| step.id == "L3-S2-SPAWN")
        .expect("spawn step remains in STEPS");
    assert_eq!(spawn.status, StepStatus::Skipped);
    let persona = steps
        .iter()
        .find(|step| step.id == "L3-S4-PERSONA-SPAWN")
        .expect("persona-gated spawn remains in STEPS");
    assert_eq!(persona.status, StepStatus::NotApplicable);

    let viewed = view(&steps);
    assert_eq!(
        viewed.len(),
        steps.len(),
        "view() is identity; a filter would drop Skipped/NotApplicable"
    );
    assert!(std::ptr::eq(viewed, steps.as_slice()));

    let source = ordered_ids(&steps);
    let tui = tui_ordered_ids(&steps);
    let json = json_ordered_ids(&steps);
    assert_eq!(tui, source, "TUI must emit STEPS order including Skipped");
    assert_eq!(json, source, "JSON must emit the same list as TUI and STEPS");
    assert_eq!(tui, json);
    assert!(
        tui.iter().any(|id| *id == "L3-S2-SPAWN"),
        "TUI must still emit the skipped spawn id"
    );
    assert!(
        tui.iter().any(|id| *id == "L3-S4-PERSONA-SPAWN"),
        "TUI must still emit the NotApplicable persona spawn id"
    );

    let dropped: Vec<&'static str> = steps
        .iter()
        .filter(|step| {
            step.status != StepStatus::Skipped && step.status != StepStatus::NotApplicable
        })
        .map(|step| step.id)
        .collect();
    assert_ne!(
        dropped, source,
        "a view() that dropped Skipped/NotApplicable must fail ordered-ID equality"
    );
    println!("STEPS {source:?}");
    println!("tui {tui:?}");
    println!("json {json:?}");
    println!("view_len {}", viewed.len());
}

#[test]
fn next_command_agrees() {
    let halt = "ask Joshua: slash vs launchd vs hand";
    let spawn = "ompo spawn";
    let steps = vec![
        Step {
            id: "L3-S0-INTRO",
            title: "intro",
            status: StepStatus::Passed,
            reason_code: None,
            next_command: None,
            predicate: Predicate::Always,
        },
        Step {
            id: "L3-HD0009",
            title: "HD-0009 halt",
            status: StepStatus::Blocked,
            reason_code: Some("HD-0009"),
            next_command: Some(halt),
            predicate: Predicate::Hd0009Decided,
        },
        Step {
            id: "L3-S2-SPAWN",
            title: "spawn",
            status: StepStatus::Ready,
            reason_code: None,
            next_command: Some(spawn),
            predicate: Predicate::NotLive,
        },
    ];

    let cursor = next_step(&steps).expect("Blocked HD-0009 is eligible");
    assert_eq!(cursor.id, "L3-HD0009");
    assert_eq!(next_command(&steps), Some(halt));
    assert_eq!(tui_next_command(&steps), json_next_command(&steps));
    assert_eq!(tui_next_command(&steps), Some(halt));

    let ready_first = steps
        .iter()
        .find(|step| step.status == StepStatus::Ready)
        .expect("later Ready exists so the sort trap is live");
    assert_eq!(ready_first.id, "L3-S2-SPAWN");
    assert_ne!(
        ready_first.next_command,
        tui_next_command(&steps),
        "Ready-first sorting keeps set equality and disagrees on next"
    );

    let mut skipped_head = fixture_steps();
    apply_predicates(&mut skipped_head, false, false, false);
    assert_eq!(
        tui_next_command(&skipped_head),
        json_next_command(&skipped_head)
    );
    assert_eq!(
        next_step(&skipped_head).map(|step| step.id),
        Some("L3-HD0009"),
        "Skipped spawn is not the cursor; first Blocked in array order is"
    );
    println!("tui_next {:?}", tui_next_command(&steps));
    println!("json_next {:?}", json_next_command(&steps));
    println!("ready_first {:?}", ready_first.next_command);
}

#[test]
fn ordered_ids_tui_json_steps() {
    let mut steps = fixture_steps();
    apply_predicates(&mut steps, false, false, false);
    let source = ordered_ids(&steps);
    let tui = tui_ordered_ids(&steps);
    let json = json_ordered_ids(&steps);
    assert_eq!(tui, source, "TUI ordered ids must equal STEPS");
    assert_eq!(json, source, "JSON ordered ids must equal STEPS");
    assert_eq!(tui, json, "TUI and JSON must emit the same ordered ids");

    let tui_hides_skipped: Vec<&'static str> = steps
        .iter()
        .filter(|step| step.status != StepStatus::Skipped)
        .map(|step| step.id)
        .collect();
    let json_set: std::collections::HashSet<_> = json.iter().copied().collect();
    let buggy_set: std::collections::HashSet<_> = tui_hides_skipped.iter().copied().collect();
    let set_equality_would_pass_buggy = json_set.is_superset(&buggy_set);
    let ordered_equality_buggy = json == tui_hides_skipped;

    println!("STEPS {source:?}");
    println!("json {json:?}");
    println!("tui_honest {tui:?}");
    println!("tui_hides_skipped {tui_hides_skipped:?}");
    println!("honest_parity {}", tui == json && json == source);
    println!("set_equality_would_pass_buggy {set_equality_would_pass_buggy}");
    println!("ordered_equality_buggy {ordered_equality_buggy}");

    assert!(
        tui == json && json == source,
        "honest renderers must match STEPS order"
    );
    assert!(
        set_equality_would_pass_buggy,
        "set compare is the trap that would ship the buggy TUI"
    );
    assert!(
        !ordered_equality_buggy,
        "TUI that drops Skipped must FAIL the ordered compare"
    );
}

#[test]
fn both_renderers_borrow_the_same_array() {
    let steps = fixture_steps();
    let tui = view(&steps);
    let json = view(&steps);
    assert!(
        std::ptr::eq(tui, steps.as_slice()),
        "TUI renderer must borrow STEPS, not a clone"
    );
    assert!(
        std::ptr::eq(json, steps.as_slice()),
        "JSON renderer must borrow STEPS, not a clone"
    );
    assert!(std::ptr::eq(tui, json), "both renderers borrow one array");

    let mut cloned = steps.clone();
    cloned.pop();
    assert!(
        !std::ptr::eq(cloned.as_slice(), steps.as_slice()),
        "a cloned TUI vec is a fork; identity must RED"
    );
    assert_ne!(
        ordered_ids(&cloned),
        ordered_ids(&steps),
        "known-bad: cloned TUI that drops last id diverges"
    );
}

#[test]
fn undecided_hd0009_blocks_tail() {
    let mut steps = fixture_steps();
    apply_predicates(&mut steps, true, true, false);
    let hd = steps
        .iter()
        .find(|step| step.id == "L3-HD0009")
        .expect("HD-0009 stays in STEPS");
    assert_eq!(hd.status, StepStatus::Blocked);
    assert_eq!(hd.reason_code, Some("HD-0009"));
    assert_eq!(
        next_command(&steps),
        Some("ask Joshua: slash vs launchd vs hand")
    );
    let hd_idx = steps
        .iter()
        .position(|step| step.id == "L3-HD0009")
        .expect("HD-0009 index");
    assert!(
        steps[hd_idx + 1..]
            .iter()
            .all(|step| step.status != StepStatus::Ready),
        "undecided HD-0009 must not leave a later Ready: {:?}",
        steps[hd_idx + 1..]
            .iter()
            .map(|step| (step.id, step.status))
            .collect::<Vec<_>>()
    );
}

#[test]
fn second_start_same_ids() {
    let mut first = fixture_steps();
    apply_predicates(&mut first, false, false, false);
    let first_ids = ordered_ids(&first);
    // Second start over the same fixture: fresh state, same passes.
    let mut second = fixture_steps();
    apply_predicates(&mut second, false, false, false);
    let second_ids = ordered_ids(&second);
    println!("first_start {first_ids:?}");
    println!("second_start {second_ids:?}");
    assert_eq!(
        second_ids, first_ids,
        "second ompo start must report the same step ids"
    );

    // Known-bad, both directions: a second start that appends or drops must RED.
    let mut appended = second.clone();
    appended.push(first[0].clone());
    assert_ne!(
        ordered_ids(&appended),
        first_ids,
        "known-bad: second start that appends a step diverges"
    );
    let mut dropped = second.clone();
    dropped.pop();
    assert_ne!(
        ordered_ids(&dropped),
        first_ids,
        "known-bad: second start that drops a step diverges"
    );
}

#[test]
fn divergent_ids_floor_zero() {
    let mut steps = fixture_steps();
    apply_predicates(&mut steps, false, false, false);
    let tui = tui_ordered_ids(&steps);
    let json = json_ordered_ids(&steps);
    assert!(!tui.is_empty(), "vacuous input is refused, never floor-clean");
    // Ordered-position count, not set/length-only: a swap is set-identical
    // and length-identical yet divergent at two positions.
    let divergent: Vec<usize> = tui
        .iter()
        .zip(json.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    let count = divergent.len() + tui.len().abs_diff(json.len());
    println!("divergent_positions {divergent:?}");
    println!("divergent_count {count}");
    assert_eq!(count, 0, "TUI/JSON ID positions diverged at {divergent:?}");
    assert!(
        check_id_parity(&tui, &json).is_ok(),
        "production parity gate must agree with the floor"
    );
}

/// L3-ARTIFACT (a5kd): the STEPS array itself round-trips through an
/// artifact file. After the write lands, the file exists and readback of
/// the renderer keys (`id`, `status`) succeeds in order. A zero-byte write
/// and a missing file both FAIL readback — neither is an empty success.
#[test]
fn steps_artifact_write_readback() {
    use ompo_start::{read_steps_artifact, write_steps_artifact};

    // KNOWN-GOOD control: post-predicate fixture steps write and read back
    // with ids and statuses intact and in order.
    let mut steps = fixture_steps();
    apply_predicates(&mut steps, false, false, false);
    let dir = tempfile::tempdir().expect("scratch");
    let path = write_steps_artifact(dir.path(), &steps).expect("artifact writes");
    assert!(path.exists(), "the artifact must exist after the write");
    let rows = read_steps_artifact(&path).expect("artifact reads back");
    let ids: Vec<&str> = rows
        .iter()
        .map(|row| row.get("id").and_then(Value::as_str).expect("id reads"))
        .collect();
    assert_eq!(
        ids,
        ordered_ids(&steps),
        "the artifact array must be the steps array, in order"
    );
    println!("L3_STEPS_OK rows={} first={}", ids.len(), ids[0]);

    // ZERO-BYTE write FAILS readback: empty bytes are not an empty success.
    let zero = dir.path().join("zero-steps.json");
    std::fs::write(&zero, b"").expect("zero bytes land");
    let error = read_steps_artifact(&zero).expect_err(
        "a zero-byte artifact must FAIL readback, not succeed",
    );
    println!("L3_STEPS_ZERO refusal={error}");

    // MISSING file FAILS readback: absence is not a row either.
    let missing = dir.path().join("never-written.json");
    assert!(!missing.exists());
    let error = read_steps_artifact(&missing).expect_err(
        "a missing artifact must FAIL readback, not succeed",
    );
    println!("L3_STEPS_MISSING refusal={error}");
    assert!(
        error.contains("never-written.json"),
        "the refusal must name the missing path, got {error}"
    );
}
