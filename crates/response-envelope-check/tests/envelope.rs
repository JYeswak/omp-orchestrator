use response_envelope_check::{check, CheckError, CheckResult, NtmSendRequest};

const FAST_DISPATCH: &str =
    r#"{"targets":["%1408"],"successful":["%1408"],"failed":[],"blocked":false}"#;
const LOOP_TICK: &str =
    r#"{"targets":["%1409"],"successful":["%1409"],"failed":[],"blocked":false,"success":true}"#;
const TICK_DISPATCH: &str =
    r#"{"targets":["%1410"],"successful":["%1410"],"failed":[],"blocked":false}"#;

fn request(target: &str) -> NtmSendRequest {
    NtmSendRequest::from_targets([target])
}

#[test]
fn fast_dispatch_positive_control_is_typed_nonzero() {
    assert_eq!(
        check(&request("%1408"), true, FAST_DISPATCH),
        CheckResult::TypedNonzero
    );
}

#[test]
fn loop_tick_positive_control_is_typed_nonzero() {
    assert_eq!(
        check(&request("%1409"), true, LOOP_TICK),
        CheckResult::TypedNonzero
    );
}

#[test]
fn tick_dispatch_positive_control_is_typed_nonzero() {
    assert_eq!(
        check(&request("%1410"), true, TICK_DISPATCH),
        CheckResult::TypedNonzero
    );
}

#[test]
fn empty_request_is_the_only_healthy_noop() {
    let request = NtmSendRequest::new(Vec::new());
    let envelope = r#"{"targets":[],"successful":[],"failed":[],"blocked":false}"#;
    assert_eq!(check(&request, true, envelope), CheckResult::HealthyNoOp);
    assert_ne!(check(&request, false, envelope), CheckResult::HealthyNoOp);
}

#[test]
fn process_failure_is_not_healthy_even_for_a_noop() {
    let request = NtmSendRequest::new(Vec::new());
    let envelope = r#"{"targets":[],"successful":[],"failed":[],"blocked":false}"#;
    assert_eq!(check(&request, false, envelope), CheckResult::TypedNonzero);
}

#[test]
fn success_only_envelope_is_error() {
    let result = check(&request("%1408"), true, r#"{"success":true}"#);
    let error = result.error();
    assert!(matches!(error, Some(CheckError::Envelope(_))));
}

#[test]
fn deleting_each_required_field_is_an_error() {
    for field in ["targets", "successful", "failed", "blocked"] {
        let mut value: serde_json::Value = serde_json::from_str(FAST_DISPATCH).unwrap();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(
                check(&request("%1408"), true, value.to_string()),
                CheckResult::Error(CheckError::Envelope(_))
            ),
            "missing {field} must be an error"
        );
    }
}

#[test]
fn changing_each_required_field_is_rejected() {
    let mutations = [
        ("targets", serde_json::json!(["%other"])),
        ("successful", serde_json::json!(["%other"])),
        ("failed", serde_json::json!(["%1408"])),
        ("blocked", serde_json::json!(true)),
    ];
    for (field, replacement) in mutations {
        let mut value: serde_json::Value = serde_json::from_str(FAST_DISPATCH).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert(field.to_owned(), replacement);
        assert!(
            matches!(
                check(&request("%1408"), true, value.to_string()),
                CheckResult::Error(_)
            ),
            "changed {field} must be an error"
        );
    }
}

#[test]
fn wrong_types_for_each_required_field_are_rejected() {
    let mutations = [
        ("targets", serde_json::json!(true)),
        ("successful", serde_json::json!("%1408")),
        ("failed", serde_json::json!(null)),
        ("blocked", serde_json::json!([])),
    ];
    for (field, replacement) in mutations {
        let mut value: serde_json::Value = serde_json::from_str(FAST_DISPATCH).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert(field.to_owned(), replacement);
        assert!(
            matches!(
                check(&request("%1408"), true, value.to_string()),
                CheckResult::Error(CheckError::Envelope(_))
            ),
            "wrong type for {field} must be an error"
        );
    }
}

#[test]
fn duplicate_targets_are_not_a_set_match() {
    let mut value: serde_json::Value = serde_json::from_str(FAST_DISPATCH).unwrap();
    value["targets"] = serde_json::json!(["%1408", "%1408"]);
    assert!(matches!(
        check(&request("%1408"), true, value.to_string()),
        CheckResult::Error(_)
    ));
}
