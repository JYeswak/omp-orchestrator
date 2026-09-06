//! Canonical `STEPS` for `ompo start`. Path named by docs/contracts/s1_l3_walkthrough.md.

/// Predicate that may change `status` but must never remove the row from `STEPS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    Always,
    /// Persona A never spawns. Unmet => `Skipped` / `NotApplicable`, still in the array.
    PersonaA,
    /// Spawn is L4. Unmet => `Skipped`, still in the array.
    NotLive,
    /// HD-0009 halt. Unmet => `Blocked`, still in the array.
    Hd0009Decided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Ready,
    Passed,
    Failed,
    Blocked,
    Skipped,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: &'static str,
    pub title: &'static str,
    pub status: StepStatus,
    pub reason_code: Option<&'static str>,
    pub next_command: Option<&'static str>,
    pub predicate: Predicate,
}

/// Persona / not-live / HD set `status`. They do not delete the step.
pub fn apply_predicates(steps: &mut [Step], live: bool, persona_a: bool, hd0009_decided: bool) {
    for step in steps {
        match step.predicate {
            Predicate::Always => {}
            Predicate::PersonaA if !persona_a => {
                step.status = StepStatus::NotApplicable;
            }
            Predicate::NotLive if !live => {
                step.status = StepStatus::Skipped;
            }
            Predicate::Hd0009Decided if !hd0009_decided => {
                step.status = StepStatus::Blocked;
                step.reason_code = Some("HD-0009");
            }
            _ => {}
        }
    }
}

/// Identity. Returning `&[Step]` makes a filter unrepresentable without changing the type.
///
/// A `Vec` filter here is the extra break besides the scout guess: persona-gated
/// omission of `NotApplicable` while the array still contains the row.
pub fn view(steps: &[Step]) -> &[Step] {
    steps
}

pub fn ordered_ids(steps: &[Step]) -> Vec<&'static str> {
    steps.iter().map(|step| step.id).collect()
}

pub fn tui_ordered_ids(steps: &[Step]) -> Vec<&'static str> {
    ordered_ids(view(steps))
}

pub fn json_ordered_ids(steps: &[Step]) -> Vec<&'static str> {
    ordered_ids(view(steps))
}

fn elapsed(status: StepStatus) -> bool {
    matches!(status, StepStatus::Passed | StepStatus::Skipped)
}

/// First non-Passed, non-Skipped step in array order.
/// Ready-first sorting keeps set equality and disagrees here (LAW-L3-ORDERED-IDS trap).
pub fn next_step(steps: &[Step]) -> Option<&Step> {
    view(steps).iter().find(|step| !elapsed(step.status))
}

pub fn next_command(steps: &[Step]) -> Option<&'static str> {
    next_step(steps).and_then(|step| step.next_command)
}

pub fn tui_next_command(steps: &[Step]) -> Option<&'static str> {
    next_command(view(steps))
}

pub fn json_next_command(steps: &[Step]) -> Option<&'static str> {
    next_command(view(steps))
}

/// Contract fixture: skipped spawn stays in the array (s1_l3_walkthrough.md Validation).
pub fn fixture_steps() -> Vec<Step> {
    vec![
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
            status: StepStatus::Ready,
            reason_code: None,
            next_command: Some("ask Joshua: slash vs launchd vs hand"),
            predicate: Predicate::Hd0009Decided,
        },
        Step {
            id: "L3-S2-SPAWN",
            title: "spawn",
            status: StepStatus::Ready,
            reason_code: None,
            next_command: None,
            predicate: Predicate::NotLive,
        },
        Step {
            id: "L3-S3-PORTAL",
            title: "portal",
            status: StepStatus::Blocked,
            reason_code: None,
            next_command: None,
            predicate: Predicate::Always,
        },
        Step {
            id: "L3-S4-PERSONA-SPAWN",
            title: "persona-A spawn",
            status: StepStatus::Ready,
            reason_code: None,
            next_command: None,
            predicate: Predicate::PersonaA,
        },
    ]
}
