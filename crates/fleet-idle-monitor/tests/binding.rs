#![forbid(unsafe_code)]

//! The legs for `omp-orchestrator-47g0`.
//!
//! The measured event these reconstruct, from the lane's own log on 2026-09-07:
//!
//! ```text
//! IDLE_PROVEN    session=omp-orchestrator pane=%20 age=600s timer=21d
//! NUDGE_VERIFIED session=omp-orchestrator pane=%20 bead=uds-snq transition=omp_working_marker
//! ```
//!
//! A foreign repository's bead delivered into this session's pane, receipted as a success.
//! Leg 1 is that exact pair of inputs and asserts the MESSAGE; leg 2 is the upstream
//! configuration that produced it; leg 3 is the known-good that keeps the conductor useful.

use fleet_idle_monitor::{
    bind_exit_code, decide, exit_code, parse_issue_prefix, parse_ready_queue, session_repo,
    BindRefusal, Decision, QueueUnreadable, TrackerBinding,
};
use std::path::{Path, PathBuf};

/// A synthetic checkout root. Never this machine's home: a hardcoded checkout compiles and
/// then reads the wrong repository, which is the failure family this whole crate is about.
fn developer_root() -> PathBuf {
    PathBuf::from("/srv/fleet/Developer")
}

fn bound(session: &str) -> TrackerBinding {
    let repo = session_repo(&developer_root(), session);
    TrackerBinding::reconcile(session, &repo, Some(session)).expect("known-good binding")
}

/// LEG 1 — THE REPRODUCTION, asserting the message and not a bare nonzero.
///
/// Session `omp-orchestrator`, a queue carrying `uds-snq`. Before the fix this pair produced
/// a dispatch and a `NUDGE_VERIFIED` receipt. It must now refuse, and the refusal must name
/// BOTH the bead and the tracker it was reconciled against — a refusal naming only one side
/// leaves the operator unable to tell which of the two bindings was wrong.
#[test]
fn a_bead_from_another_tracker_is_refused_naming_bead_and_tracker() {
    let binding = bound("omp-orchestrator");
    let payload = r#"[{"id":"uds-snq","status":"open"}]"#;

    let decision = decide(&binding, parse_ready_queue(payload));

    let Decision::Refused(refusal) = &decision else {
        panic!("a foreign bead must be REFUSED, not dispatched: {decision}");
    };
    let message = refusal.to_string();
    assert!(
        message.contains("DISPATCH_REFUSED_FOREIGN_TRACKER"),
        "the refusal must be named, not inferred from an exit code: {message}"
    );
    assert!(
        message.contains("bead=uds-snq"),
        "the refusal must name the bead it refused: {message}"
    );
    assert!(
        message.contains("tracker=omp-orchestrator"),
        "the refusal must name the tracker it reconciled against: {message}"
    );
    assert!(
        message.contains("population=1"),
        "the refusal must carry the population it observed: {message}"
    );
    assert_eq!(
        exit_code(&decision),
        3,
        "a foreign tracker is XC-003 TRACKER_ERROR"
    );
}

/// LEG 2 — THE UPSTREAM CONFIGURATION ITSELF.
///
/// `control-plane/crates/fleet-monitor/src/bin/fleet-idle-monitor.rs:22,117,118` pairs
/// `FLEET_SESSION` with a queue repository that defaults to an absolute path naming a third
/// checkout. That pairing must not be constructible here at all — the misroute is refused one
/// layer earlier than the queue, so it cannot depend on what the queue happens to contain.
#[test]
fn a_session_cannot_be_bound_to_another_projects_checkout() {
    let foreign = developer_root().join("uds");

    let refusal = TrackerBinding::reconcile("omp-orchestrator", &foreign, Some("uds"))
        .expect_err("session omp-orchestrator against the uds checkout must REFUSE");

    assert_eq!(
        refusal,
        BindRefusal::SessionQueueMismatch {
            session: "omp-orchestrator".to_owned(),
            repo: foreign.clone(),
            repo_name: "uds".to_owned(),
        }
    );
    let message = refusal.to_string();
    assert!(
        message.contains("BIND_REFUSED_SESSION_QUEUE_MISMATCH"),
        "{message}"
    );
    assert!(message.contains("session=omp-orchestrator"), "{message}");
    assert!(message.contains("repo_name=uds"), "{message}");
    assert_eq!(bind_exit_code(&refusal), 78, "a wrong binding is XC-078 EX_CONFIG");
}

/// LEG 3 — KNOWN-GOOD, mandatory. An attack-only suite ships an over-strict conductor, and an
/// over-strict conductor gets disarmed. A bead that DOES belong to the bound tracker must
/// still dispatch, and the proposal must carry the binding.
#[test]
fn a_bead_from_the_bound_tracker_still_dispatches() {
    let binding = bound("omp-orchestrator");
    let payload = r#"[{"id":"omp-orchestrator-815","status":"open"},
                      {"id":"omp-orchestrator-47g0","status":"open"}]"#;

    let decision = decide(&binding, parse_ready_queue(payload));

    let Decision::Dispatch(dispatch) = &decision else {
        panic!("an in-tracker bead must dispatch: {decision}");
    };
    assert_eq!(dispatch.bead(), "omp-orchestrator-815");
    assert_eq!(dispatch.population(), 2);
    let proposal = dispatch.proposal();
    assert!(proposal.contains("DISPATCH_BOUND"), "{proposal}");
    assert!(proposal.contains("tracker=omp-orchestrator"), "{proposal}");
    assert!(proposal.contains("population=2"), "{proposal}");
    assert_eq!(exit_code(&decision), 0);
}

/// LEG 3b — a MIXED queue fails closed rather than quietly filtering. Dropping the foreign row
/// and dispatching the good one would hide the misroute instead of reporting it, and hiding it
/// is what made the original failure survive weeks of green receipts.
#[test]
fn a_queue_mixing_trackers_refuses_rather_than_filtering() {
    let binding = bound("omp-orchestrator");
    let payload = r#"[{"id":"omp-orchestrator-815"},{"id":"uds-snq"}]"#;

    let decision = decide(&binding, parse_ready_queue(payload));

    assert!(
        matches!(decision, Decision::Refused(_)),
        "a queue holding foreign work is not the bound tracker's queue: {decision}"
    );
    assert!(decision.to_string().contains("bead=uds-snq"));
}

/// LEG 4 — ANTI-VACUITY. An empty queue is an outcome that carries its population, and it does
/// NOT share an exit code with a tick that actually dispatched.
#[test]
fn an_empty_queue_is_nothing_to_dispatch_not_a_pass() {
    let binding = bound("omp-orchestrator");

    let decision = decide(&binding, parse_ready_queue("[]"));

    assert_eq!(decision, Decision::NothingToDispatch { population: 0 });
    assert_eq!(decision.to_string(), "NOTHING_TO_DISPATCH population=0");
    assert_eq!(
        exit_code(&decision),
        70,
        "an empty tick must not be indistinguishable from a dispatching one"
    );
    assert_ne!(
        exit_code(&decision),
        exit_code(&decide(
            &binding,
            parse_ready_queue(r#"[{"id":"omp-orchestrator-815"}]"#)
        ))
    );
}

/// LEG 5 — CANNOT-OBSERVE IS NOT ABSENT. All three payload shapes the tracker actually
/// produces are handled, and the two failure shapes are Unobservable rather than empty.
///
/// The bare array is the shape that bites a reader written against `{"issues":[…]}`: asking
/// for a field on a list yields nothing and reads exactly like a drained queue.
#[test]
fn every_real_queue_shape_is_read_and_no_failure_reads_as_empty() {
    let binding = bound("omp-orchestrator");

    // Shape 1: the bare array, as returned today.
    let bare = parse_ready_queue(r#"[{"id":"omp-orchestrator-815"}]"#).expect("bare array");
    assert_eq!(bare.len(), 1, "positive control: the bare array must parse");

    // Shape 2: the object projection other readers in this workspace parse.
    let wrapped =
        parse_ready_queue(r#"{"issues":[{"id":"omp-orchestrator-815"}]}"#).expect("issues object");
    assert_eq!(wrapped, bare, "both shapes must yield the same queue");

    // Shape 3: the tracker's own error envelope — a directory that is not a checkout.
    let not_a_checkout = r#"{"error":{"code":"NOT_INITIALIZED","message":"Beads not initialized"}}"#;
    let decision = decide(&binding, parse_ready_queue(not_a_checkout));
    assert_eq!(
        decision,
        Decision::Unobservable(QueueUnreadable::TrackerError {
            code: "NOT_INITIALIZED".to_owned(),
            message: "Beads not initialized".to_owned(),
        }),
        "a tracker error must never read as an empty queue"
    );
    assert_eq!(exit_code(&decision), 69);

    // And non-JSON, the silent-empty case a `print nothing on exception` reader produces.
    let decision = decide(&binding, parse_ready_queue(""));
    assert!(
        matches!(
            decision,
            Decision::Unobservable(QueueUnreadable::NotJson { .. })
        ),
        "an unreadable payload is UNOBSERVABLE, never NothingToDispatch: {decision}"
    );
    assert_ne!(exit_code(&decision), exit_code(&Decision::NothingToDispatch { population: 0 }));
}

/// LEG 6 — the receipt means something. `NUDGE_VERIFIED` now names the repository and tracker
/// it was earned under, so a reader can tell a correct delivery from a misrouted one. The
/// original marker carried session, pane, bead and transition — every one of which was TRUE
/// for the misroute.
#[test]
fn nudge_verified_names_the_binding_it_was_earned_under() {
    let binding = bound("omp-orchestrator");
    let decision = decide(
        &binding,
        parse_ready_queue(r#"[{"id":"omp-orchestrator-815"}]"#),
    );
    let Decision::Dispatch(dispatch) = decision else {
        panic!("known-good must dispatch");
    };

    let receipt = dispatch.verified("%20", "omp_working_marker").receipt();

    assert!(receipt.starts_with("NUDGE_VERIFIED "), "{receipt}");
    assert!(receipt.contains("session=omp-orchestrator"), "{receipt}");
    assert!(
        receipt.contains("repo=/srv/fleet/Developer/omp-orchestrator"),
        "the receipt must name the repository the work came from: {receipt}"
    );
    assert!(
        receipt.contains("tracker=omp-orchestrator"),
        "the receipt must name the tracker: {receipt}"
    );
    assert!(receipt.contains("pane=%20"), "{receipt}");
    assert!(receipt.contains("bead=omp-orchestrator-815"), "{receipt}");
}

/// LEG 7 — the tracker prefix comes from the repository, and its absence is a refusal rather
/// than a default. Includes the live file as a positive control: a parser that silently fails
/// on the real spelling is the false-zero instrument class this repo has measured repeatedly.
#[test]
fn tracker_prefix_is_read_from_the_repository_never_defaulted() {
    // The live file writes the field inside a comment. Read it, do not assume it.
    let live = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.beads/config.yaml");
    let text = std::fs::read_to_string(&live)
        .unwrap_or_else(|error| panic!("positive control unreadable at {}: {error}", live.display()));
    assert_eq!(
        parse_issue_prefix(&text).as_deref(),
        Some("omp-orchestrator"),
        "the parser must read the LIVE spelling, not a spelling we imagined: {text:?}"
    );

    // Uncommented form, for a repository that writes it plainly.
    assert_eq!(
        parse_issue_prefix("issue_prefix: cp\n").as_deref(),
        Some("cp")
    );
    // NEGATIVE CONTROL: absence is absence.
    assert_eq!(parse_issue_prefix("default_priority: 2\n"), None);

    // And an unresolved tracker refuses instead of guessing.
    let repo = session_repo(&developer_root(), "omp-orchestrator");
    let refusal = TrackerBinding::reconcile("omp-orchestrator", &repo, None)
        .expect_err("no tracker prefix must refuse");
    assert_eq!(refusal, BindRefusal::TrackerUnresolved { repo });
}

/// LEG 8 — the prefix test is a tracker test, not a string-prefix test. `omp` must not adopt
/// `omperator-1`, or the reconciliation would pass exactly the ids it exists to catch.
#[test]
fn a_tracker_does_not_adopt_a_bead_that_merely_starts_with_its_letters() {
    let repo = session_repo(&developer_root(), "omp");
    let binding = TrackerBinding::reconcile("omp", &repo, Some("omp")).expect("binding");

    assert!(binding.owns("omp-1"), "positive control");
    assert!(!binding.owns("omperator-1"));
    assert!(!binding.owns("omp"));
    assert!(!binding.owns("omp-"));
}

/// LEG 9 — an unnamed session is a refusal. Upstream defaulted it to a literal session name,
/// which is how a lane ran for weeks against a fleet nobody had chosen for it.
#[test]
fn an_unnamed_session_refuses_instead_of_defaulting() {
    let repo = session_repo(&developer_root(), "omp-orchestrator");
    assert_eq!(
        TrackerBinding::reconcile("   ", &repo, Some("omp-orchestrator")).unwrap_err(),
        BindRefusal::UnnamedSession
    );
}

/// LEG 10 — THE WHOLE TICK, over the LIVE checkout this crate sits in.
///
/// Everything above exercises one joint at a time. The measured defect lived in the GLUE —
/// a session resolved in one place, a queue resolved in another — so the glue gets its own
/// leg, driven through the same entry point the CLI calls, against the real repository's own
/// tracker configuration rather than a spelling we imagined.
#[test]
fn the_whole_tick_refuses_the_measured_misroute_against_this_live_checkout() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root must exist");
    let session = repo
        .file_name()
        .and_then(|name| name.to_str())
        .expect("the checkout must have a name")
        .to_owned();
    let config = std::fs::read_to_string(repo.join(".beads/config.yaml")).expect("live config");
    let tracker = parse_issue_prefix(&config).expect("positive control: the live config declares a prefix");

    // KNOWN-GOOD first: if this leg can only refuse, it proves nothing about the refusal.
    let good = format!(r#"[{{"id":"{tracker}-815","status":"open"}}]"#);
    let decision = fleet_idle_monitor::tick(&session, &repo, &config, &good).expect("binding");
    let Decision::Dispatch(dispatch) = &decision else {
        panic!("this checkout's own bead must dispatch: {decision}");
    };
    assert_eq!(dispatch.bead(), format!("{tracker}-815"));

    // KNOWN-BAD: the exact payload measured on 2026-09-07.
    let bad = r#"[{"id":"uds-snq","status":"open"}]"#;
    let decision = fleet_idle_monitor::tick(&session, &repo, &config, bad).expect("binding");
    assert!(
        matches!(decision, Decision::Refused(_)),
        "the measured misroute must be refused end to end: {decision}"
    );
    let message = decision.to_string();
    assert!(message.contains("bead=uds-snq"), "{message}");
    assert!(message.contains(&format!("tracker={tracker}")), "{message}");
    assert_eq!(exit_code(&decision), 3);
}
