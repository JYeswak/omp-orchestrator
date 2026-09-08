use omp_rpc_session::{
    MalformedReason, OmpCommand, ProtocolVersion, RpcFrame, RpcRequest, RpcSessionConfig,
    RpcSessionReport, parse_frame,
};

mod fixture {
    pub const READY: &str =
        r#"{"type":"ready","protocolVersion":1,"supportedProtocolVersions":[1,2]}"#;
    pub const NEGOTIATED: &str = r#"{"id":"negotiate-2","type":"response","command":"negotiate_protocol","success":true,"data":{"protocolVersion":2}}"#;
    pub const STATE: &str = r#"{"id":"state","type":"response","command":"get_state","success":true,"data":{"model":{"id":"m"},"thinkingLevel":"medium"}}"#;
    pub const STATS: &str = r#"{"id":"stats","type":"response","command":"get_session_stats","success":true,"data":{"turns":0}}"#;
    pub const MESSAGES: &str = r#"{"id":"messages","type":"response","command":"get_messages","success":true,"data":{"messages":[]}}"#;
    pub const UNKNOWN: &str = r#"{"type":"future_frame","payload":42}"#;
    pub const REJECTED: &str = r#"{"id":"state","type":"response","command":"get_state","success":false,"error":"unavailable"}"#;
}

#[test]
fn fixture_frames_parse_into_selected_types() {
    assert!(matches!(parse_frame(1, fixture::READY), RpcFrame::Ready(_)));
    assert!(matches!(
        parse_frame(2, fixture::NEGOTIATED),
        RpcFrame::Response(_)
    ));
    assert!(matches!(
        parse_frame(3, fixture::STATE),
        RpcFrame::Response(_)
    ));
    assert!(matches!(
        parse_frame(4, fixture::STATS),
        RpcFrame::Response(_)
    ));
    assert!(matches!(
        parse_frame(5, fixture::MESSAGES),
        RpcFrame::Response(_)
    ));
}

#[test]
fn request_wire_contract_is_exact_and_bounded() {
    let sequence = RpcRequest::sequence();
    assert_eq!(sequence.len(), 4);
    assert_eq!(sequence[0].id(), "negotiate-2");
    assert_eq!(sequence[0].command(), "negotiate_protocol");
    assert_eq!(sequence[1].command(), "get_state");
    assert_eq!(sequence[2].command(), "get_session_stats");
    assert_eq!(sequence[3].command(), "get_messages");
    assert!(
        sequence
            .iter()
            .all(|request| request.to_frame().ends_with('\n'))
    );
    assert!(
        sequence
            .iter()
            .all(|request| request.to_frame().len() < 256)
    );
}
#[test]
fn report_names_only_the_native_omp_methods_it_adopts() {
    assert_eq!(
        RpcSessionReport::adopted_methods(),
        [
            "negotiate_protocol",
            "get_state",
            "get_session_stats",
            "get_messages",
        ]
    );
}

#[test]
fn unknown_malformed_and_rejected_frames_are_not_dropped() {
    assert!(matches!(
        parse_frame(6, fixture::UNKNOWN),
        RpcFrame::Unknown(_)
    ));
    let malformed = parse_frame(7, "not-json");
    assert!(matches!(
        malformed,
        RpcFrame::Malformed(frame) if matches!(frame.reason, MalformedReason::InvalidJson(_))
    ));
    let rejected = parse_frame(8, fixture::REJECTED);
    assert!(matches!(
        rejected,
        RpcFrame::Response(response)
            if !response.success && response.error.as_deref() == Some("unavailable")
    ));
}

#[test]
fn protocol_v2_is_the_only_negotiated_version() {
    assert_eq!(ProtocolVersion::V2.0, 2);
}

#[test]
fn existing_session_attach_adds_only_the_resume_selector() {
    let command = OmpCommand::new("omp")
        .resume("01a06796-2e83-70b6-9adb-79fa276b0f31")
        .expect("non-empty session id");
    let args: Vec<String> = command
        .args()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, ["--mode=rpc", "--resume=01a06796-2e83-70b6-9adb-79fa276b0f31"]);

    let config = RpcSessionConfig::for_existing_session("omp", "session-123")
        .expect("configures existing session");
    assert_eq!(config.command.args().last().unwrap().to_string_lossy(), "--resume=session-123");

    let error = OmpCommand::new("omp").resume("  ").expect_err("empty selector must refuse");
    assert!(error.to_string().contains("existing-session selector"));
}

#[test]
fn state_sequence_is_explicit_and_does_not_narrow_the_default() {
    assert_eq!(RpcRequest::sequence().len(), 4);
    assert_eq!(
        RpcRequest::state_sequence().map(RpcRequest::command),
        ["negotiate_protocol", "get_state"]
    );
}

#[test]
fn config_defaults_to_full_sequence_and_accepts_narrow_selector() {
    let full = RpcSessionConfig::with_command(OmpCommand::new("omp"));
    assert_eq!(full.requests, RpcRequest::sequence().to_vec());

    let state = full.clone().with_requests(RpcRequest::state_sequence());
    assert_eq!(state.requests, RpcRequest::state_sequence().to_vec());
    assert_ne!(state.requests, full.requests);
}
