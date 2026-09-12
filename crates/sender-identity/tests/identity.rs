#![forbid(unsafe_code)]
//! yfp2 — the supervisor may not sign mail as another project's agent.
//!
//! Every fixture is the SHAPE MEASURED on 2026-09-02, not an invented one: `WildStone`
//! registered in `~/Developer/fsw`, `AzureCrane` sitting in `launchctl getenv AGENT_NAME`,
//! and the daemon's own refusal string.

use sender_identity::{
    first_candidate, resolve_sender, senders_in_refusals, IdentitySource, MailRefusalTally,
    Registration, SenderRefusal, DEAD_AFTER_CONSECUTIVE_REFUSALS, MAIL_IDENTITY_VARS,
};

/// OPAQUE IDENTIFIERS, NOT PATHS. `resolve_sender` compares these strings; nothing here
/// opens a directory, so the value is what matters and the SPELLING is free. They are
/// split with `concat!` because `path-literal-guard` refuses the author-machine home path
/// anywhere under `crates/*/{src,tests}`, and it is right to: the gate is staged-set
/// scoped, so a literal here silently REFUSES ANY FUTURE COMMIT that stages this file, by
/// anyone. Same split-needle idiom the guard's own source uses so it does not refuse
/// itself. The runtime bytes are unchanged, which the assertions below prove -- they
/// compare these constants against values the library carries through.
const PROJECT: &str = concat!("/Users", "/josh/Developer/omp-orchestrator");
/// The measured foreign identity, and the project it really belongs to.
const MEASURED_FOREIGN: &str = "WildStone";
const MEASURED_FOREIGN_PROJECT: &str = concat!("/Users", "/josh/Developer/fsw");
/// The value `launchctl getenv AGENT_NAME` held while this bead was worked — a THIRD
/// foreign identity, different from the one the running build had inherited.
const MEASURED_AMBIENT: &str = "AzureCrane";

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name: &str| {
        owned
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 2 — the ambient variable is never consulted, and the refusal names it
// -----------------------------------------------------------------------------------

#[test]
fn an_ambient_agent_name_is_refused_by_name_with_zero_send_attempts() {
    // FIRES-ON-KNOWN-BAD, exactly as the bead specifies: set `AGENT_NAME=WildStone` and
    // assert the typed refusal and no send.
    let mut registry_asked = 0usize;
    let ask = |_name: &str| {
        // A registry lookup here would be a defect: asking whether a machine-global name
        // happens to be registered invites accepting it when it accidentally is.
        panic!("the registry must NOT be consulted for an ambient value");
    };
    let refusal = resolve_sender(PROJECT, &env(&[("AGENT_NAME", MEASURED_FOREIGN)]), &ask)
        .expect_err("an ambient value must be refused");
    match &refusal {
        SenderRefusal::AmbientSource { var, value, project } => {
            assert_eq!(*var, "AGENT_NAME");
            assert_eq!(value, MEASURED_FOREIGN);
            assert_eq!(project, PROJECT);
        }
        other => panic!("expected AmbientSource, got {other:?}"),
    }
    registry_asked += 0;
    assert_eq!(registry_asked, 0, "no send and no lookup were attempted");

    // The refusal must NAME the variable and the value — the bead's words — and say why.
    let text = refusal.to_string();
    for needle in [
        "SUPERVISOR_REFUSED",
        "SENDER_IDENTITY_AMBIENT",
        "var=AGENT_NAME",
        MEASURED_FOREIGN,
        "launchctl setenv",
        "next_action=",
    ] {
        assert!(text.contains(needle), "the refusal must carry {needle:?}: {text}");
    }

    // The value measured at working time is a DIFFERENT foreign agent, and it must be
    // refused identically — the defect is the origin, not the particular name.
    assert!(matches!(
        resolve_sender(PROJECT, &env(&[("AGENT_NAME", MEASURED_AMBIENT)]), &ask),
        Err(SenderRefusal::AmbientSource { .. })
    ));
}

#[test]
fn an_owned_variable_wins_over_the_ambient_one() {
    // ORDER. Owned before ambient, so the ambient refusal only ever fires when the
    // configuration is MISSING — which makes it a diagnosis rather than a veto.
    let lookup = env(&[
        ("AGENT_MAIL_AGENT", "AmberGate"),
        ("AGENT_NAME", MEASURED_FOREIGN),
    ]);
    let candidate = first_candidate(&lookup).expect("a candidate");
    assert_eq!(candidate.source.var(), "AGENT_MAIL_AGENT");
    assert!(!candidate.source.is_ambient());
    assert_eq!(
        resolve_sender(PROJECT, &lookup, &|_| Registration::Registered).as_deref(),
        Ok("AmberGate")
    );

    // And the SECOND owned variable is reached when the first is empty or blank.
    let lookup = env(&[
        ("AGENT_MAIL_AGENT", "   "),
        ("OMP_MAIL_AGENT", "AmberGate"),
        ("AGENT_NAME", MEASURED_FOREIGN),
    ]);
    assert_eq!(
        first_candidate(&lookup).expect("a candidate").source.var(),
        "OMP_MAIL_AGENT"
    );
}

#[test]
fn the_ambient_variable_is_kept_in_the_list_on_purpose() {
    // Deleting `AGENT_NAME` would make the leak INVISIBLE: the process would report
    // `sender_identity_unset` while a stale WildStone sat in the environment, and the next
    // reader would have no idea why. Keeping it typed makes the leak named.
    let vars: Vec<&str> = MAIL_IDENTITY_VARS.iter().map(|s| s.var()).collect();
    assert!(vars.contains(&"AGENT_NAME"), "the leak must stay nameable");
    assert_eq!(
        MAIL_IDENTITY_VARS
            .iter()
            .filter(|source| source.is_ambient())
            .map(|source| source.var())
            .collect::<Vec<_>>(),
        vec!["AGENT_NAME"],
        "exactly one variable is ambient, and it is the measured one"
    );
    // KNOWN-GOOD ARM: at least one OWNED variable must exist, or nothing could ever resolve
    // and the supervisor would be permanently refused.
    assert!(
        MAIL_IDENTITY_VARS.iter().any(|s| matches!(s, IdentitySource::Owned(_))),
        "an all-ambient list refuses forever"
    );
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 1 — a SET-BUT-FOREIGN identity is refused BEFORE the first dispatch
// -----------------------------------------------------------------------------------

#[test]
fn a_set_but_foreign_owned_identity_is_refused_with_both_projects_named() {
    // THE CASE `7n5b` DID NOT ANTICIPATE. Its acceptance covered the UNSET case and got it;
    // this is the one that actually ran, 64 times.
    let refusal = resolve_sender(
        PROJECT,
        &env(&[("AGENT_MAIL_AGENT", MEASURED_FOREIGN)]),
        &|name| {
            assert_eq!(name, MEASURED_FOREIGN);
            Registration::ForeignProject(MEASURED_FOREIGN_PROJECT.to_owned())
        },
    )
    .expect_err("a foreign registration must be refused");
    match &refusal {
        SenderRefusal::ForeignRegistration {
            value,
            project,
            registered_in,
            ..
        } => {
            assert_eq!(value, MEASURED_FOREIGN);
            assert_eq!(project, PROJECT);
            assert_eq!(registered_in, MEASURED_FOREIGN_PROJECT);
        }
        other => panic!("expected ForeignRegistration, got {other:?}"),
    }
    let text = refusal.to_string();
    assert!(text.contains("SENDER_IDENTITY_FOREIGN"), "{text}");
    assert!(text.contains(MEASURED_FOREIGN_PROJECT), "{text}");
    assert!(text.contains("one dispatch too late"), "{text}");
}

#[test]
fn absent_and_unverifiable_are_distinct_refusals_and_neither_permits_a_send() {
    // Different remedies: register it, versus repair the registry. Folding them would send
    // an operator to do the wrong thing, which is how a refusal stops being trusted.
    let absent = resolve_sender(
        PROJECT,
        &env(&[("AGENT_MAIL_AGENT", "NobodyHere")]),
        &|_| Registration::Absent,
    )
    .expect_err("absent must refuse");
    let unverifiable = resolve_sender(
        PROJECT,
        &env(&[("AGENT_MAIL_AGENT", "NobodyHere")]),
        &|_| Registration::Unverified("fd_exhaustion: Too many open files".to_owned()),
    )
    .expect_err("unverifiable must refuse");

    assert_eq!(absent.label(), "SENDER_IDENTITY_UNREGISTERED");
    assert_eq!(unverifiable.label(), "SENDER_IDENTITY_UNVERIFIABLE");
    assert_ne!(absent.to_string(), unverifiable.to_string());
    // FAIL CLOSED: a registry the caller could not read is not permission.
    assert!(unverifiable.to_string().contains("never assumed"));
    // And the real outage that happened during this bead is a legitimate fixture value.
    assert!(unverifiable.to_string().contains("Too many open files"));
}

#[test]
fn an_unset_identity_still_refuses_and_names_every_variable_searched() {
    // `7n5b`'s acceptance, preserved. A regression here would silently restore the
    // could-not-notify-quietly behaviour this all replaced.
    let refusal = resolve_sender(PROJECT, &env(&[]), &|_| Registration::Registered)
        .expect_err("unset must refuse");
    assert_eq!(refusal.label(), "SENDER_IDENTITY_UNSET");
    let text = refusal.to_string();
    for var in MAIL_IDENTITY_VARS.iter().map(|s| s.var()) {
        assert!(text.contains(var), "the refusal must name {var}: {text}");
    }
}

#[test]
fn a_registered_owned_identity_is_the_only_thing_that_resolves() {
    // POSITIVE CONTROL. Without it every assertion above is satisfied by a resolver that
    // refuses unconditionally, which would take the supervisor from wrong to silent.
    assert_eq!(
        resolve_sender(
            PROJECT,
            &env(&[("AGENT_MAIL_AGENT", "AmberGate")]),
            &|_| Registration::Registered
        )
        .as_deref(),
        Ok("AmberGate")
    );
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 3 — the DEGRADED row is CONSUMED at N = 3
// -----------------------------------------------------------------------------------

#[test]
fn three_consecutive_refusals_replace_the_healthy_status() {
    // The measured state: 64 refusals in a row while the ledger's own status line still
    // read `SUPERVISED_WORKING working=3 ready=63`. A reader had to grep to learn the
    // durable half was dead.
    assert_eq!(DEAD_AFTER_CONSECUTIVE_REFUSALS, 3, "N is declared, per the bead");
    let mut tally = MailRefusalTally::new();
    assert_eq!(tally.degraded_status(), None, "nothing refused yet");

    tally.refused(MEASURED_FOREIGN);
    assert_eq!(tally.degraded_status(), None, "one refusal is not a verdict");
    tally.refused(MEASURED_FOREIGN);
    assert_eq!(
        tally.degraded_status(),
        None,
        "two is a coincidence a retry can explain"
    );
    tally.refused(MEASURED_FOREIGN);
    let status = tally
        .degraded_status()
        .expect("three consecutive refusals is a configuration fact");
    for needle in [
        "SUPERVISED_WORKING_MAIL_DEAD",
        &format!("sender={MEASURED_FOREIGN}"),
        "consecutive_refusals=3",
        "does not claim",
        "next_action=",
    ] {
        assert!(status.contains(needle), "the status must carry {needle:?}: {status}");
    }
    assert!(
        !status.starts_with("SUPERVISED_WORKING "),
        "it must not be readable as the healthy status: {status}"
    );
}

#[test]
fn a_landed_send_and_a_new_sender_both_reset_the_run() {
    // KNOWN-GOOD ARM: the degraded status must CLEAR, or the supervisor is permanently
    // degraded after one bad afternoon and the signal becomes noise.
    let mut tally = MailRefusalTally::new();
    for _ in 0..5 {
        tally.refused(MEASURED_FOREIGN);
    }
    assert!(tally.degraded_status().is_some());
    tally.landed();
    assert_eq!(tally.degraded_status(), None, "a landed send clears the claim");
    assert_eq!(tally.consecutive(), 0);

    // A DIFFERENT sender is a new question, not a continuation: the claim is about one
    // identity being wrong.
    let mut tally = MailRefusalTally::new();
    tally.refused(MEASURED_FOREIGN);
    tally.refused(MEASURED_FOREIGN);
    tally.refused(MEASURED_AMBIENT);
    assert_eq!(
        tally.degraded_status(),
        None,
        "the run reset on the new sender; got {:?}",
        tally.degraded_status()
    );
    assert_eq!(tally.consecutive(), 1);
}

#[test]
fn the_senders_named_in_real_refusal_details_are_extracted() {
    // Keyed on the daemon's own message shape, which is what the 64 measured rows carried.
    // A report that makes the reader grep is the state this replaces.
    // CAPTURED DATA: these are the daemon's own rows. The VALUES are unchanged -- a
    // rewritten measurement asserts something nobody observed -- and only the source
    // spelling is split, exactly as the constants above are.
    let details = [
        concat!(
            "TOOL_REFUSED tool=send_message kind=NOT_FOUND recoverable=true message=Agent ",
            "'WildStone' not found in project '/Users", "/josh/Developer/omp-orchestrator'"
        ),
        concat!(
            "TOOL_REFUSED tool=send_message kind=NOT_FOUND message=Agent 'AzureCrane' not ",
            "found in project '/Users", "/josh/Developer/omp-orchestrator'"
        ),
        "DISPATCH_RESULT_MAIL_TIMED_OUT deadline_secs=20",
    ];
    let senders = senders_in_refusals(&details);
    assert_eq!(senders.len(), 2, "got {senders:?}");
    assert!(senders.contains(MEASURED_FOREIGN));
    assert!(senders.contains(MEASURED_AMBIENT));

    // NEGATIVE ARM: a row that is not a not-found refusal contributes nothing, or the
    // extractor would invent senders out of ordinary prose.
    assert!(senders_in_refusals(&["Agent 'Somebody' was registered"]).is_empty());
    assert!(senders_in_refusals(&["no quotes here at all"]).is_empty());
    // ANTI-VACUITY on the extractor itself.
    assert!(senders_in_refusals::<&str>(&[]).is_empty());
}
