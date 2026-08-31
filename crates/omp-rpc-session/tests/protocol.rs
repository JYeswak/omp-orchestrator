use omp_rpc_session::{MalformedReason, ProtocolVersion, RpcFrame, RpcRequest, parse_frame};

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
