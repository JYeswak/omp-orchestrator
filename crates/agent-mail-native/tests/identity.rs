use agent_mail_native::identity::{
    assert_readback_fields, cleanup_pane_identities_arguments, format_sender_header,
    parse_pane_identity, tmux_identity_argv, validate_register_fields, BindingStatus,
    IdentityError, PaneIdentity,
};
use agent_mail_native::journey::{AgentName, ProjectKey};
use serde_json::{json, Map, Value};

fn fields(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

#[test]
fn register_rejects_agent_name_instead_of_minting_an_identity() {
    let request = fields([
        ("agent_name", json!("WindyWren")),
        ("program", json!("omp")),
    ]);

    assert!(matches!(
        validate_register_fields(&request),
        Err(IdentityError::UnknownRegisterField { field }) if field == "agent_name"
    ));
}

#[test]
fn register_accepts_the_protocol_name_field() {
    let request = fields([
        ("name", json!("GreenFrog")),
        ("program", json!("codex-cli")),
        ("model", json!("openai-codex/gpt-5.6-luna")),
    ]);

    validate_register_fields(&request).expect("protocol name is the accepted identity field");
}

#[test]
fn readback_requires_every_settable_field() {
    let request = fields([
        ("name", json!("GreenFrog")),
        ("program", json!("codex-cli")),
        ("model", json!("openai-codex/gpt-5.6-luna")),
        ("task_description", json!("FROM and REPLY VIA")),
        ("pane_id", json!("%1413")),
    ]);
    let response = json!({
        "name": "GreenFrog",
        "program": "codex-cli",
        "model": "openai-codex/gpt-5.6-luna",
        "task_description": "FROM and REPLY VIA"
    });

    assert!(matches!(
        assert_readback_fields(&request, &response),
        Err(IdentityError::MissingReadbackField { field }) if field == "pane_id"
    ));
}

#[test]
fn complete_readback_is_accepted() {
    let request = fields([
        ("name", json!("GreenFrog")),
        ("program", json!("codex-cli")),
        ("model", json!("openai-codex/gpt-5.6-luna")),
        ("task_description", json!("FROM and REPLY VIA")),
    ]);
    let response = json!({
        "name": "GreenFrog",
        "program": "codex-cli",
        "model": "openai-codex/gpt-5.6-luna",
        "task_description": "FROM and REPLY VIA"
    });

    assert_readback_fields(&request, &response).expect("all requested fields were echoed");
}

#[test]
fn pane_binding_requires_verified_live_status() {
    let verified = parse_pane_identity(&json!({
        "pane_id": "%1413",
        "binding": "verified-live"
    }))
    .expect("verified binding");
    assert_eq!(verified.binding, BindingStatus::VerifiedLive);

    assert!(matches!(
        parse_pane_identity(&json!({
            "pane_id": "%1397",
            "binding": "legacy-unverified"
        })),
        Err(IdentityError::UnverifiedPaneBinding { binding }) if binding == "legacy-unverified"
    ));
}

#[test]
fn pane_binding_rejects_unknown_status() {
    assert!(matches!(
        parse_pane_identity(&json!({
            "pane_id": "%1413",
            "binding": "maybe-live"
        })),
        Err(IdentityError::UnknownPaneBinding { binding }) if binding == "maybe-live"
    ));
}

#[test]
fn tmux_identity_targets_the_calling_pane() {
    let active_pane = "%1396";
    let calling_pane = "%1413";
    let args = tmux_identity_argv(calling_pane);
    assert_eq!(
        args,
        [
            "display-message",
            "-t",
            calling_pane,
            "-p",
            "#{pane_id} #{session_name}:#{window_index}.#{pane_index}"
        ]
    );
    assert_ne!(
        args[2], active_pane,
        "the query must not follow the focused pane"
    );
}

#[test]
fn sender_header_names_the_verified_agent_and_both_reply_routes() {
    let identity = PaneIdentity {
        pane_id: "%1408".to_owned(),
        binding: BindingStatus::VerifiedLive,
        agent_name: Some(AgentName::new("AmberGate")),
        session: Some("omp-orchestrator".to_owned()),
        pane_index: Some(4),
    };
    let header = format_sender_header(
        &identity,
        "omp-orchestrator",
        &ProjectKey::new(concat!("/Users", "/josh/Developer/omp-orchestrator")),
    )
    .expect("verified identity formats");
    assert_eq!(
        header,
        concat!(
            "FROM: AmberGate pane_index=4 pane_id=%1408 binding=verified-live\nREPLY-VIA: ",
            "ntm --robot-send=omp-orchestrator --panes=4 --msg-file <path>; Agent Mail to ",
            "AmberGate project=/Users", "/josh/Developer/omp-orchestrator\n"
        )
    );
}

#[test]
fn sender_header_refuses_missing_pane_index() {
    let identity = PaneIdentity {
        pane_id: "%1408".to_owned(),
        binding: BindingStatus::VerifiedLive,
        agent_name: Some(AgentName::new("AmberGate")),
        session: None,
        pane_index: None,
    };
    assert!(matches!(
        format_sender_header(&identity, "omp-orchestrator", &ProjectKey::new("project")),
        Err(IdentityError::MissingPaneIdentityField {
            field: "pane_index"
        })
    ));
}

#[test]
fn roster_cleanup_delegates_to_existing_kernel_tool() {
    assert_eq!(
        cleanup_pane_identities_arguments(concat!("/Users", "/josh/Developer/omp-orchestrator")),
        json!({"project_key": concat!("/Users", "/josh/Developer/omp-orchestrator")})
    );
}
