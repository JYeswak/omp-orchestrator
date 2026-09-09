use std::process::ExitCode;

#[test]
fn missing_repository_is_not_a_successful_supervisor_run() {
    let code = omp_orchestrator::resident::run(vec![
        "--once".to_owned(),
        "--repo".to_owned(),
        "/definitely/missing/omp-repository".to_owned(),
    ]);
    assert_eq!(
        code,
        ExitCode::from(2),
        "a missing repository must not take the no-op success path"
    );
}

#[test]
fn conductor_invokes_silence_watch_after_dispatch() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("dispatch-silence-watch"),
        "conductor manifest must depend on the silence-watch lane"
    );

    let source = include_str!("../src/resident.rs");
    let dispatch = source
        .find("SupervisorDecision::Dispatch")
        .expect("dispatch branch must exist");
    let after_dispatch = &source[dispatch..];
    assert!(
        after_dispatch.contains("dispatch-silence-watch")
            || after_dispatch.contains("silence_watch"),
        "the canonical dispatch lane must invoke dispatch-silence-watch"
    );
}
