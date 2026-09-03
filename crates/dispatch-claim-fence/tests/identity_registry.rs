use dispatch_claim_fence::{
    authorize_with_identities, AssigneeIdentityError, BeadSnapshot, ClaimFenceError,
    DispatchIntent, DispatchPermit, IdentityNamespace, IdentityRecord, IdentityRegistries,
};

fn registries() -> IdentityRegistries {
    IdentityRegistries::new(
        vec![IdentityRecord::agent_mail("AmberGate", "pane:4")],
        vec![
            IdentityRecord::subagent("MailMining", "subagent:mail-mining"),
            IdentityRecord::subagent("ExtractTwo", "subagent:extract-two"),
        ],
    )
}

fn claimed_bead(assignee: &str) -> BeadSnapshot {
    BeadSnapshot::new(
        "u21m-test",
        "identity test",
        "verify assignee identity",
        "in_progress",
        Some(assignee),
    )
}

#[test]
fn unknown_assignee_refuses_and_names_both_registries() {
    let error = authorize_with_identities(
        &DispatchIntent::bead("u21m-test", "WildcardFix"),
        Some(&claimed_bead("WildcardFix")),
        &registries(),
    )
    .expect_err("an assignee absent from both registries must refuse");

    let ClaimFenceError::AssigneeIdentity(AssigneeIdentityError::UnknownAssignee {
        assignee,
        agent_mail,
        subagents,
    }) = &error
    else {
        panic!("expected typed unknown-assignee refusal, got {error:?}");
    };
    assert_eq!(assignee, "WildcardFix");
    assert_eq!(agent_mail, &["AmberGate"]);
    assert_eq!(subagents, &["ExtractTwo", "MailMining"]);
    let rendered = error.to_string();
    assert!(
        rendered.contains("WildcardFix"),
        "missing assignee: {rendered}"
    );
    assert!(
        rendered.contains("agent_mail"),
        "missing Agent Mail registry: {rendered}"
    );
    assert!(
        rendered.contains("subagents"),
        "missing subagent registry: {rendered}"
    );
}

#[test]
fn live_subagent_is_accepted_by_dispatch_claim() {
    let identities = registries();
    let resolved = identities
        .resolve("MailMining")
        .expect("namespace-two assignee must resolve");
    assert_eq!(resolved.assignee(), "MailMining");
    assert_eq!(resolved.actor_id(), "subagent:mail-mining");
    assert_eq!(resolved.namespaces(), &[IdentityNamespace::Subagent]);

    let permit = authorize_with_identities(
        &DispatchIntent::bead("u21m-test", "MailMining"),
        Some(&claimed_bead("MailMining")),
        &identities,
    )
    .expect("a live subagent in namespace two must be accepted");

    assert!(matches!(
        permit,
        DispatchPermit::Bead { bead_id, receiver_agent }
            if bead_id == "u21m-test" && receiver_agent == "MailMining"
    ));
}

#[test]
fn known_agent_mail_assignee_is_accepted() {
    let permit = authorize_with_identities(
        &DispatchIntent::bead("u21m-test", "AmberGate"),
        Some(&claimed_bead("AmberGate")),
        &registries(),
    )
    .expect("a registered Agent Mail assignee must pass");

    assert!(matches!(
        permit,
        DispatchPermit::Bead { receiver_agent, .. } if receiver_agent == "AmberGate"
    ));
}

#[test]
fn empty_agent_mail_registry_is_error_not_acceptance() {
    let identities = IdentityRegistries::new(
        Vec::<IdentityRecord>::new(),
        vec![IdentityRecord::subagent(
            "MailMining",
            "subagent:mail-mining",
        )],
    );
    let error = authorize_with_identities(
        &DispatchIntent::bead("u21m-test", "MailMining"),
        Some(&claimed_bead("MailMining")),
        &identities,
    )
    .expect_err("an empty Agent Mail registry must be an operational error");

    assert!(matches!(
        error,
        ClaimFenceError::AssigneeIdentity(AssigneeIdentityError::RegistryUnavailable { ref missing })
            if missing == &[IdentityNamespace::AgentMail]
    ));
    let rendered = error.to_string();
    assert!(
        rendered.contains("agent_mail"),
        "missing empty registry name: {rendered}"
    );
    assert!(
        rendered.contains("subagents"),
        "missing checked registry name: {rendered}"
    );
}

#[test]
fn empty_subagent_registry_is_error_not_acceptance() {
    let identities = IdentityRegistries::new(
        vec![IdentityRecord::agent_mail("AmberGate", "pane:4")],
        Vec::<IdentityRecord>::new(),
    );
    let error = authorize_with_identities(
        &DispatchIntent::bead("u21m-test", "AmberGate"),
        Some(&claimed_bead("AmberGate")),
        &identities,
    )
    .expect_err("an empty subagent registry must be an operational error");

    assert!(matches!(
        error,
        ClaimFenceError::AssigneeIdentity(AssigneeIdentityError::RegistryUnavailable { ref missing })
            if missing == &[IdentityNamespace::Subagent]
    ));
    let rendered = error.to_string();
    assert!(
        rendered.contains("agent_mail"),
        "missing checked registry name: {rendered}"
    );
    assert!(
        rendered.contains("subagents"),
        "missing empty registry name: {rendered}"
    );
}

#[test]
fn duplicate_aliases_are_reported_as_data() {
    let identities = IdentityRegistries::new(
        vec![
            IdentityRecord::agent_mail("Orchestrator", "pane:1"),
            IdentityRecord::agent_mail("omp-claude", "pane:1"),
            IdentityRecord::agent_mail("josh", "pane:1"),
            IdentityRecord::agent_mail("SnowyCanyon", "pane:1"),
        ],
        vec![IdentityRecord::subagent(
            "MailMining",
            "subagent:mail-mining",
        )],
    );
    let aliases = identities.duplicate_aliases();

    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].actor_id(), "pane:1");
    assert_eq!(
        aliases[0].names(),
        &["Orchestrator", "SnowyCanyon", "josh", "omp-claude",]
    );
}

#[test]
fn duplicate_same_name_across_namespaces_is_not_a_duplicate_alias() {
    let identities = IdentityRegistries::new(
        vec![IdentityRecord::agent_mail("AmberGate", "pane:4")],
        vec![IdentityRecord::subagent("AmberGate", "subagent:amber")],
    );

    assert!(identities.duplicate_aliases().is_empty());
    let error = identities
        .resolve("AmberGate")
        .expect_err("the same name with two actors is ambiguous");
    assert!(matches!(
        error,
        AssigneeIdentityError::AmbiguousAssignee { .. }
    ));
}
