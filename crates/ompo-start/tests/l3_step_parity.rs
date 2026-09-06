//! LAW-L3-SKIP-STAYS: Skipped / NotApplicable stay in STEPS; view() does not drop them.

use ompo_start::{
    apply_predicates, fixture_steps, json_next_command, json_ordered_ids, next_command, next_step,
    ordered_ids, tui_next_command, tui_ordered_ids, view, Predicate, Step, StepStatus,
};

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
