#![forbid(unsafe_code)]

use state_wildcard_lint::{find_findings_in_source, FindingKind};

#[test]
fn known_bad_state_wildcard_is_flagged() {
    let source = r#"
enum PaneState { Working, Idle }
fn check(input: PaneState) {
    let state: PaneState = input;
    match state {
        PaneState::Working => (),
        _ => (),
    }
}
"#;
    let findings = find_findings_in_source(source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, 5);
    assert_eq!(findings[0].1, 7);
    assert_eq!(findings[0].3.as_deref(), Some("PaneState"));
    assert_eq!(findings[0].4, FindingKind::WildcardState);
}

#[test]
fn wildcard_on_integer_and_string_passes() {
    let source = r#"
fn integer(value: i32) {
    let number: i32 = value;
    match number { 0 => (), _ => () }
}
fn text(value: &str) {
    let text: &str = value;
    match text { "yes" => (), _ => () }
}
"#;
    assert!(find_findings_in_source(source).is_empty());
}

#[test]
fn wildcard_on_non_state_enum_passes() {
    let source = r#"
enum ErrorKind { Io, Parse }
fn check(input: ErrorKind) {
    let error: ErrorKind = input;
    match error { ErrorKind::Io => (), _ => () }
}
"#;
    assert!(find_findings_in_source(source).is_empty());
}

#[test]
fn unresolved_state_like_type_is_not_reported_clean() {
    let source = r#"
fn check(input: RemoteState) {
    let state: RemoteState = input;
    match state { _ => () }
}
"#;
    let findings = find_findings_in_source(source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].4, FindingKind::UnresolvedStateType);
    assert_eq!(findings[0].3.as_deref(), Some("RemoteState"));
}

#[test]
fn mutation_removing_state_wildcard_is_green() {
    let source = r#"
enum PaneState { Working, Idle }
fn check(state: PaneState) {
    match state {
        PaneState::Working => (),
        _ => (),
    }
}
"#;
    assert_eq!(find_findings_in_source(source).len(), 1);
    let repaired = source.replace("_ => ()", "PaneState::Idle => ()");
    assert!(find_findings_in_source(&repaired).is_empty());
    assert!(source.contains("_ => ()"), "mutation must not alter the original");
}

#[test]
fn empty_or_unreadable_workspace_is_an_error() {
    let root = std::env::temp_dir().join(format!(
        "state-wildcard-lint-empty-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create empty root");
    let report = state_wildcard_lint::lint_workspace(&root);
    assert!(report.scanned.is_empty());
    assert!(report.findings.is_empty());
    assert!(report.error.is_some());
    std::fs::remove_dir_all(&root).expect("remove empty root");
}

#[test]
fn lint_is_wired_into_blocking_ci() {
    let workflow = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/gate.yml"
    ))
    .expect("gate workflow must be readable");
    assert!(
        workflow
            .lines()
            .any(|line| line.contains("cargo run --quiet -p state-wildcard-lint -- .")),
        "the state wildcard lint needs a blocking CI caller"
    );
    assert!(
        !workflow
            .lines()
            .any(|line| line.contains("cargo test -p unrelated-state-wildcard-lint")),
        "positive control: unrelated commands must not count as wiring"
    );
}
