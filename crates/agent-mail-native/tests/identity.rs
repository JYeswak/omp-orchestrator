use agent_mail_native::identity::{
    assert_readback_fields, cleanup_pane_identities_arguments, parse_pane_identity,
    tmux_identity_argv, validate_register_fields, BindingStatus, IdentityError,
};
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
    assert_eq!(
        tmux_identity_argv("%1413"),
        [
            "display-message",
            "-t",
            "%1413",
            "-p",
            "#{pane_id} #{session_name}:#{window_index}.#{pane_index}"
        ]
    );
}

#[test]
fn roster_cleanup_delegates_to_existing_kernel_tool() {
    assert_eq!(
        cleanup_pane_identities_arguments("/Users/josh/Developer/omp-orchestrator"),
        json!({"project_key": "/Users/josh/Developer/omp-orchestrator"})
    );
}
