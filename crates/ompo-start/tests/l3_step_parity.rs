//! LAW-L3-SKIP-STAYS: Skipped / NotApplicable stay in STEPS; view() does not drop them.

use ompo_start::{
    apply_predicates, fixture_steps, json_ordered_ids, ordered_ids, tui_ordered_ids, view, StepStatus,
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
