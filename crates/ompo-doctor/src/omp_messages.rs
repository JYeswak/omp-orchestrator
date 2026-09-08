//! `ompo messages` — the projection of OMP's native `get_messages` command.
//!
//! # KERNEL-ONLY: this is a projection, not a transport
//!
//! [`omp_rpc_session::run_session`] already issues all four commands of OMP's `--mode=rpc`
//! vocabulary in ONE bounded session and hands back `report.selected.messages`. This module
//! adds a typed READING of that payload and nothing else; the taxonomy, the exit dictionary
//! and the `RpcError` classifier are reused from [`crate::omp_state`] rather than written a
//! second time, so a spawn failure cannot mean one thing under `ompo state` and another under
//! `ompo messages`.
//!
//! # The MEASURED payload, typed from the wire and not from a guess
//!
//! Measured 2026-09-08 against `omp/18.1.14`, the `get_messages` response `data` is:
//!
//! ```text
//! messages   array of objects; observed member keys: role (string),
//!            customType (string), content (string)
//! ```
//!
//! Longer sessions carry other roles and other member keys. None of them are assumed here:
//! every projected field is `Option`, and the whole `data` object is retained in
//! [`OmpMessages::raw`] so an OMP extension is not lost by passing through this type.
//!
//! The precedent for that discipline is the correction recorded in [`crate::omp_state`]: OMP's
//! `model` field is an OBJECT, a first pass read it with `as_str()`, and the verb reported
//! `model=absent` for a field that WAS present. A fixture-only suite passed against a payload
//! OMP never sends. So the fixtures below carry the measured member keys verbatim.
//!
//! # THE SEMANTIC LEG THIS VERB EXISTS TO GET RIGHT
//!
//! An EMPTY `messages` array is a SUCCESS with `count=0` at exit 0. A MISSING or non-array
//! `messages` key is `OMP_MESSAGES_NO_PAYLOAD` at exit 1. **A verb that uses a non-zero exit
//! to mean "ran fine, no results" is the failure being prevented**: it teaches every caller
//! to ignore its exit code, and once the exit code is ignored a real refusal is silent. "OMP
//! answered and this session has no messages" and "OMP answered and did not send the messages
//! field at all" have different remedies, so they are never one arm.

use crate::omp_state::{self, StateOutcome, EXIT_OK, EXIT_REFUSED, EXIT_UNMEASURED};
use crate::umbrella;
use omp_rpc_session::{run_session, OmpCommand, RpcError, RpcSessionConfig, NO_CLAIM_BOUNDARY};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// OMP's own native command this verb adopts. One method, named, so the report cannot claim
/// broader protocol coverage than it issues.
pub const ADOPTED_METHOD: &str = "get_messages";

/// What one `ompo messages` run established. The same six arms as
/// [`crate::omp_state::StateOutcome`] — one per cause — with reason codes of its own so a
/// reader can tell WHICH surface reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagesOutcome {
    /// OMP answered and the messages array is present. An EMPTY array lands here, with
    /// `count == 0`: it is an answer about the session, not a failure to answer.
    Answered(Box<OmpMessages>),
    /// OMP answered and refused. **"OMP said no" and "OMP did not answer" have opposite
    /// remedies**, so they are never one arm.
    Refused { detail: String },
    /// OMP answered successfully and carried no `messages` array — the key is missing, or is
    /// present with a non-array type. A protocol surprise, NOT an empty session.
    NoPayload,
    /// The binary is absent. UNMEASURED — this says nothing about OMP.
    Absent { detail: String },
    /// A deadline expired. A timeout is a restrictive terminal, never a pass.
    TimedOut { phase: String },
    /// Spawn, io or protocol failure. Distinct from a refusal by construction.
    TransportFailed { detail: String },
}

/// The typed projection of OMP's `get_messages` payload.
///
/// `count` and the histogram are counts OF AN ARRAY THAT WAS PRESENT; the absence of the
/// array is [`MessagesOutcome::NoPayload`] and never a zero here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmpMessages {
    /// Length of the `messages` array. Zero is a real answer.
    pub count: usize,
    /// Role histogram, sorted by role name ASCENDING so two runs over one payload print the
    /// same line. A `BTreeMap` is the sort; there is no second ordering rule to drift.
    pub roles: Vec<(String, usize)>,
    /// `role` of the first member, `None` when the array is empty or the member omits it.
    pub first_role: Option<String>,
    /// `role` of the last member, on the same terms.
    pub last_role: Option<String>,
    pub lifecycle: String,
    pub protocol_negotiated: u32,
    pub raw: Value,
}

/// One message, projected. Every field is `Option`: a member that omits `customType` must
/// read as absent rather than as an empty string, because an empty string is a VALUE that OMP
/// could legitimately send and the two must stay distinguishable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSummary {
    pub role: Option<String>,
    pub custom_type: Option<String>,
    /// Byte length of `content`. The bytes are NOT retained: a summary is a shape, and
    /// message bodies are the caller's business, available whole in [`OmpMessages::raw`].
    pub content_bytes: Option<usize>,
}

impl MessagesOutcome {
    #[must_use]
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Answered(_) => "OMP_MESSAGES_OK",
            Self::Refused { .. } => "OMP_MESSAGES_REFUSED",
            Self::NoPayload => "OMP_MESSAGES_NO_PAYLOAD",
            Self::Absent { .. } => "OMP_MESSAGES_ABSENT",
            Self::TimedOut { .. } => "OMP_MESSAGES_TIMEOUT_UNMEASURED",
            Self::TransportFailed { .. } => "OMP_MESSAGES_TRANSPORT_FAILED",
        }
    }

    /// The exit code, from [`crate::omp_state`]'s dictionary rather than a second one.
    ///
    /// `Answered` is `EXIT_OK` **including when `count == 0`**. Spending a non-zero code on
    /// "ran fine, no results" would make the code unreadable: a caller cannot then tell an
    /// empty session from a refusal without parsing prose.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Answered(_) => EXIT_OK,
            Self::Refused { .. } | Self::NoPayload => EXIT_REFUSED,
            Self::Absent { .. } | Self::TimedOut { .. } | Self::TransportFailed { .. } => {
                EXIT_UNMEASURED
            }
        }
    }

    /// The envelope status. An unreachable OMP is `UNKNOWN`, never `DOWN`: we did not measure
    /// OMP, we failed to reach it.
    #[must_use]
    pub fn envelope_status(&self) -> &'static str {
        match self {
            Self::Answered(_) => "OK",
            Self::Refused { .. } | Self::NoPayload => "DEGRADED",
            Self::Absent { .. } | Self::TimedOut { .. } | Self::TransportFailed { .. } => "UNKNOWN",
        }
    }

    /// True when nothing about OMP was established.
    #[must_use]
    pub fn is_unmeasured(&self) -> bool {
        matches!(
            self,
            Self::Absent { .. } | Self::TimedOut { .. } | Self::TransportFailed { .. }
        )
    }

    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Answered(messages) => format!(
                "count={} first_role={} last_role={}",
                messages.count,
                messages.first_role.as_deref().unwrap_or("absent"),
                messages.last_role.as_deref().unwrap_or("absent")
            ),
            Self::Refused { detail } | Self::TransportFailed { detail } => detail.clone(),
            Self::Absent { detail } => detail.clone(),
            Self::NoPayload => {
                "OMP answered successfully and carried no messages array; an EMPTY array is a \
                 success and is NOT reported here"
                    .to_owned()
            }
            Self::TimedOut { phase } => format!("deadline expired in phase={phase}"),
        }
    }
}

/// Summarize the members of an already-extracted `messages` array.
///
/// Members that are not objects still get a row: dropping them would make `summarize` disagree
/// with [`OmpMessages::count`], and a summary shorter than the count it accompanies reads as a
/// smaller session rather than as an unexpected member type.
#[must_use]
pub fn summarize(messages: &[Value]) -> Vec<MessageSummary> {
    messages
        .iter()
        .map(|message| MessageSummary {
            role: message
                .get("role")
                .and_then(Value::as_str)
                .map(str::to_owned),
            custom_type: message
                .get("customType")
                .and_then(Value::as_str)
                .map(str::to_owned),
            content_bytes: message
                .get("content")
                .and_then(Value::as_str)
                .map(str::len),
        })
        .collect()
}

/// Project OMP's raw `get_messages` data into the typed shape.
///
/// `None` when the `messages` key is ABSENT or is present with a non-array type. That is the
/// only `None`: an EMPTY array projects to `Some` with `count == 0`, because "this session has
/// no messages" is an answer and "OMP did not send the field" is not.
#[must_use]
pub fn project(data: &Value, lifecycle: &str, negotiated: u32) -> Option<OmpMessages> {
    let messages = data.get("messages")?.as_array()?;
    let mut histogram: BTreeMap<String, usize> = BTreeMap::new();
    for message in messages {
        if let Some(role) = message.get("role").and_then(Value::as_str) {
            *histogram.entry(role.to_owned()).or_insert(0) += 1;
        }
    }
    let role_of = |member: Option<&Value>| -> Option<String> {
        member?
            .get("role")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    Some(OmpMessages {
        count: messages.len(),
        roles: histogram.into_iter().collect(),
        first_role: role_of(messages.first()),
        last_role: role_of(messages.last()),
        lifecycle: lifecycle.to_owned(),
        protocol_negotiated: negotiated,
        raw: data.clone(),
    })
}

/// Convert the shared classifier's verdict onto this verb's arms.
///
/// [`crate::omp_state::classify_error`] owns the `RpcError` mapping; duplicating it would let
/// the two verbs disagree about what a spawn failure is. This function is the ONLY translation
/// layer, and it is total: the classifier's payload-bearing arms cannot arise from an error, so
/// they land on `TransportFailed` with a detail that says so rather than on a silent default.
#[must_use]
pub fn from_state_outcome(outcome: &StateOutcome) -> MessagesOutcome {
    match outcome {
        StateOutcome::Refused { detail } => MessagesOutcome::Refused {
            detail: detail.clone(),
        },
        StateOutcome::Absent { detail } => MessagesOutcome::Absent {
            detail: detail.clone(),
        },
        StateOutcome::TimedOut { phase } => MessagesOutcome::TimedOut {
            phase: phase.clone(),
        },
        StateOutcome::TransportFailed { detail } => MessagesOutcome::TransportFailed {
            detail: detail.clone(),
        },
        StateOutcome::NoPayload | StateOutcome::Answered(_) => MessagesOutcome::TransportFailed {
            detail: "classifier returned a payload arm for a transport error".to_owned(),
        },
    }
}

/// Map a transport error onto this verb's taxonomy, via the shared classifier.
#[must_use]
pub fn classify_error(error: &RpcError) -> MessagesOutcome {
    from_state_outcome(&omp_state::classify_error(error))
}

/// Drive one bounded OMP `--mode=rpc` session and read its message list.
///
/// `&Cx` first, per the asupersync contract; cancellation belongs to the caller.
pub async fn read_messages(cx: &asupersync::Cx, binary: &str) -> MessagesOutcome {
    let config = RpcSessionConfig::with_command(OmpCommand::new(binary));
    match run_session(cx, &config).await {
        Ok(report) => {
            let negotiated = report.negotiated.0;
            let lifecycle = report.lifecycle.as_str();
            if let Some(refusal) = report
                .responses
                .iter()
                .find(|response| !response.success && response.command.as_str() == ADOPTED_METHOD)
            {
                return MessagesOutcome::Refused {
                    detail: refusal.error.clone().unwrap_or_else(|| {
                        "omp refused get_messages without an error string".to_owned()
                    }),
                };
            }
            report
                .selected
                .messages
                .as_ref()
                .and_then(|data| project(data, lifecycle, negotiated))
                .map_or(MessagesOutcome::NoPayload, |messages| {
                    MessagesOutcome::Answered(Box::new(messages))
                })
        }
        Err(error) => classify_error(&error),
    }
}

/// Render the role histogram deterministically: `role:count` pairs, ascending by role.
fn render_roles(roles: &[(String, usize)]) -> String {
    if roles.is_empty() {
        // Not a hidden zero: `count=` on the same line says whether the session was empty or
        // whether every member omitted its `role` key.
        return "absent".to_owned();
    }
    roles
        .iter()
        .map(|(role, count)| format!("{role}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The human line. Success only; a refusal prints its reason code on stderr instead.
#[must_use]
pub fn render(messages: &OmpMessages) -> String {
    format!(
        "OMPO_OMP_MESSAGES count={} roles={} first_role={} last_role={} lifecycle={} \
         protocol={} adopted_method={ADOPTED_METHOD}",
        messages.count,
        render_roles(&messages.roles),
        messages.first_role.as_deref().unwrap_or("absent"),
        messages.last_role.as_deref().unwrap_or("absent"),
        messages.lifecycle,
        messages.protocol_negotiated,
    )
}

/// The `--json` envelope. The payload keys appear on the SUCCESS path only: a refusal that
/// prints a success-shaped envelope is indistinguishable from a success.
#[must_use]
pub fn envelope(outcome: &MessagesOutcome) -> Value {
    let data = match outcome {
        MessagesOutcome::Answered(messages) => {
            let summaries: Vec<Value> = messages
                .raw
                .get("messages")
                .and_then(Value::as_array)
                .map(|members| {
                    summarize(members)
                        .into_iter()
                        .map(|summary| {
                            json!({
                                "role": summary.role,
                                "custom_type": summary.custom_type,
                                "content_bytes": summary.content_bytes,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "adopted_method": ADOPTED_METHOD,
                "lifecycle": messages.lifecycle,
                "protocol_negotiated": messages.protocol_negotiated,
                "count": messages.count,
                "roles": messages
                    .roles
                    .iter()
                    .map(|(role, count)| json!({"role": role, "count": count}))
                    .collect::<Vec<_>>(),
                "first_role": messages.first_role,
                "last_role": messages.last_role,
                "messages": summaries,
                "reason_code": outcome.reason_code(),
                "raw": messages.raw,
                "no_claim": NO_CLAIM_BOUNDARY,
            })
        }
        other => json!({
            "adopted_method": ADOPTED_METHOD,
            "reason_code": other.reason_code(),
            "detail": other.detail(),
            "unmeasured": other.is_unmeasured(),
            "no_claim": NO_CLAIM_BOUNDARY,
        }),
    };
    umbrella::envelope("messages", outcome.envelope_status(), data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omp_rpc_session::TimeoutPhase;

    /// The MEASURED member keys, verbatim: `role`, `customType`, `content`. A fixture invented
    /// from a field list is how a suite passes against a payload OMP never sends.
    fn payload() -> Value {
        json!({
            "messages": [
                {"role": "user", "customType": "text", "content": "hello"},
                {"role": "assistant", "customType": "text", "content": "hi there"},
                {"role": "user", "customType": "tool_result", "content": "ok"},
            ]
        })
    }

    #[test]
    fn the_projection_reads_omps_field_names_not_ours() {
        let messages = project(&payload(), "stopped", 2).expect("the array is present");
        assert_eq!(messages.count, 3);
        assert_eq!(messages.first_role.as_deref(), Some("user"));
        assert_eq!(messages.last_role.as_deref(), Some("user"));
        assert_eq!(messages.lifecycle, "stopped");
        assert_eq!(messages.protocol_negotiated, 2);
    }

    #[test]
    fn the_role_histogram_is_sorted_by_role_name_so_the_line_is_deterministic() {
        let messages = project(&payload(), "stopped", 2).expect("the array is present");
        assert_eq!(
            messages.roles,
            vec![("assistant".to_owned(), 1), ("user".to_owned(), 2)]
        );
        assert!(render(&messages).contains("roles=assistant:1,user:2"));
    }

    #[test]
    fn an_empty_messages_array_is_a_success_with_a_zero_count() {
        // THE LEG. A verb that uses a non-zero exit to mean "ran fine, no results" is the
        // failure being prevented: it trains every caller to ignore the exit code, and a
        // refusal is then indistinguishable from an empty session.
        let messages =
            project(&json!({"messages": []}), "stopped", 2).expect("an empty array is present");
        assert_eq!(messages.count, 0);
        assert!(messages.roles.is_empty());
        assert_eq!(messages.first_role, None);
        let outcome = MessagesOutcome::Answered(Box::new(messages));
        assert_eq!(outcome.exit_code(), EXIT_OK);
        assert_eq!(outcome.reason_code(), "OMP_MESSAGES_OK");
        assert_eq!(outcome.envelope_status(), "OK");
        assert!(!outcome.is_unmeasured());
        assert_eq!(envelope(&outcome)["data"]["count"], 0);
    }

    #[test]
    fn a_missing_or_non_array_messages_key_is_no_payload_and_not_an_empty_session() {
        // The other direction of the same leg. An absent array and an EMPTY array must not
        // collapse: "OMP did not send the field" and "this session has no messages" send a
        // reader to different places.
        assert_eq!(project(&json!({}), "stopped", 2), None);
        assert_eq!(project(&json!({"messages": Value::Null}), "stopped", 2), None);
        assert_eq!(project(&json!({"messages": 7}), "stopped", 2), None);
        assert_eq!(project(&json!({"messages": {"0": {}}}), "stopped", 2), None);
        assert_eq!(MessagesOutcome::NoPayload.exit_code(), EXIT_REFUSED);
        assert_ne!(MessagesOutcome::NoPayload.exit_code(), EXIT_OK);
        assert_eq!(
            MessagesOutcome::NoPayload.reason_code(),
            "OMP_MESSAGES_NO_PAYLOAD"
        );
    }

    #[test]
    fn a_field_omp_stops_sending_reads_as_absent_rather_than_as_a_default() {
        // Defaulting `role` to "user" would attribute a message to the wrong speaker -- a
        // silent wrong answer rather than a gap.
        let messages = project(&json!({"messages": [{"content": "x"}]}), "stopped", 2)
            .expect("the array is present");
        assert_eq!(messages.first_role, None);
        assert_eq!(messages.last_role, None);
        assert!(messages.roles.is_empty());
        assert!(render(&messages).contains("first_role=absent"));
        assert!(render(&messages).contains("roles=absent"));
        // The count still reports, so "empty session" and "member without a role" stay
        // distinguishable on the same line.
        assert!(render(&messages).contains("count=1"));
    }

    #[test]
    fn a_summary_field_omp_stops_sending_reads_as_absent_rather_than_as_an_empty_string() {
        let summaries = summarize(&[json!({"role": "user"})]);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].role.as_deref(), Some("user"));
        assert_eq!(summaries[0].custom_type, None);
        assert_eq!(summaries[0].content_bytes, None);
    }

    #[test]
    fn the_summary_reads_the_measured_member_keys_and_counts_content_bytes() {
        let members = payload()["messages"].as_array().expect("fixture array").clone();
        let summaries = summarize(&members);
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].role.as_deref(), Some("user"));
        assert_eq!(summaries[0].custom_type.as_deref(), Some("text"));
        assert_eq!(summaries[0].content_bytes, Some(5));
        assert_eq!(summaries[2].custom_type.as_deref(), Some("tool_result"));
    }

    #[test]
    fn a_non_object_member_still_gets_a_row_so_the_summary_matches_the_count() {
        let summaries = summarize(&[json!("bare string"), json!({"role": "user"})]);
        assert_eq!(summaries.len(), 2, "a dropped member reads as a smaller session");
        assert_eq!(summaries[0].role, None);
    }

    #[test]
    fn the_raw_payload_is_retained_whole_so_an_omp_extension_is_not_lost() {
        let data = json!({
            "messages": [{"role": "user", "customType": "text", "content": "hello"}],
            "someFutureField": {"nested": 42},
        });
        let messages = project(&data, "stopped", 2).expect("the array is present");
        assert_eq!(messages.raw, data);
        assert_eq!(messages.raw["someFutureField"]["nested"], 42);
        assert_eq!(messages.raw["messages"][0]["content"], "hello");
    }

    #[test]
    fn omp_said_no_and_omp_did_not_answer_are_different_outcomes() {
        let refused = MessagesOutcome::Refused {
            detail: "no session".to_owned(),
        };
        let absent = MessagesOutcome::Absent {
            detail: "no such file".to_owned(),
        };
        assert_ne!(refused.reason_code(), absent.reason_code());
        assert_ne!(refused.exit_code(), absent.exit_code());
        assert!(!refused.is_unmeasured(), "a refusal IS a measurement of OMP");
        assert!(absent.is_unmeasured(), "an absent binary measures nothing");
        assert_eq!(refused.envelope_status(), "DEGRADED");
        assert_eq!(absent.envelope_status(), "UNKNOWN");
    }

    #[test]
    fn a_timeout_is_unmeasured_and_never_a_pass() {
        let timed_out = MessagesOutcome::TimedOut {
            phase: "startup".to_owned(),
        };
        assert_eq!(timed_out.reason_code(), "OMP_MESSAGES_TIMEOUT_UNMEASURED");
        assert_eq!(timed_out.exit_code(), EXIT_UNMEASURED);
        assert_ne!(timed_out.exit_code(), EXIT_OK);
        assert!(timed_out.is_unmeasured());
    }

    #[test]
    fn every_outcome_has_a_distinct_reason_code() {
        let codes = [
            MessagesOutcome::Answered(Box::new(
                project(&payload(), "stopped", 2).expect("the array is present"),
            ))
            .reason_code(),
            MessagesOutcome::Refused {
                detail: String::new(),
            }
            .reason_code(),
            MessagesOutcome::NoPayload.reason_code(),
            MessagesOutcome::Absent {
                detail: String::new(),
            }
            .reason_code(),
            MessagesOutcome::TimedOut {
                phase: String::new(),
            }
            .reason_code(),
            MessagesOutcome::TransportFailed {
                detail: String::new(),
            }
            .reason_code(),
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            codes.len(),
            "two causes share one token: {codes:?}"
        );
        for code in codes {
            assert!(
                code.starts_with("OMP_MESSAGES_"),
                "a reader must be able to tell WHICH surface reported; got {code}"
            );
        }
    }

    #[test]
    fn exit_vocabulary_is_pairwise_distinct() {
        let codes = [
            EXIT_OK,
            EXIT_REFUSED,
            omp_state::EXIT_BAD_INVOCATION,
            omp_state::EXIT_INSTRUMENT,
            EXIT_UNMEASURED,
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            codes.len(),
            "two causes share one exit code: {codes:?}"
        );
    }

    #[test]
    fn the_shared_classifier_is_reused_so_a_spawn_failure_means_one_thing_in_both_verbs() {
        let spawn = classify_error(&RpcError::Process {
            operation: "spawn".to_owned(),
            detail: "No such file or directory".to_owned(),
        });
        assert_eq!(spawn.reason_code(), "OMP_MESSAGES_ABSENT");
        assert_eq!(spawn.exit_code(), EXIT_UNMEASURED);
        let other = classify_error(&RpcError::Process {
            operation: "kill".to_owned(),
            detail: "boom".to_owned(),
        });
        assert_eq!(other.reason_code(), "OMP_MESSAGES_TRANSPORT_FAILED");
    }

    #[test]
    fn every_rpc_error_arm_lands_on_a_named_arm_of_this_verbs_taxonomy() {
        let errors = [
            RpcError::Process {
                operation: "spawn".to_owned(),
                detail: "no such file".to_owned(),
            },
            RpcError::ProcessExited { code: Some(1) },
            RpcError::Cancelled {
                detail: "parent cancelled".to_owned(),
            },
            RpcError::Timeout {
                phase: TimeoutPhase::Request,
            },
            RpcError::Io {
                stream: "stdout",
                detail: "broken pipe".to_owned(),
            },
        ];
        for error in &errors {
            let outcome = classify_error(error);
            assert_ne!(
                outcome.exit_code(),
                EXIT_OK,
                "a transport error must never read as a success: {error:?}"
            );
            assert!(
                !outcome.detail().is_empty(),
                "a non-zero path must name its cause: {error:?}"
            );
        }
    }

    #[test]
    fn the_timeout_phase_survives_the_translation_rather_than_becoming_a_placeholder() {
        for phase in [
            TimeoutPhase::Startup,
            TimeoutPhase::Request,
            TimeoutPhase::Shutdown,
        ] {
            let outcome = classify_error(&RpcError::Timeout { phase });
            let MessagesOutcome::TimedOut { phase: name } = &outcome else {
                panic!("a timeout must classify as TimedOut, got {outcome:?}");
            };
            assert!(!name.is_empty(), "the phase must be named");
            assert_ne!(
                name, "unknown",
                "a placeholder phase hides which deadline expired"
            );
        }
    }

    #[test]
    fn the_success_envelope_carries_the_adopted_method_and_the_no_claim() {
        let outcome = MessagesOutcome::Answered(Box::new(
            project(&payload(), "stopped", 2).expect("the array is present"),
        ));
        let value = envelope(&outcome);
        assert_eq!(value["command"], "messages");
        assert_eq!(value["status"], "OK");
        assert_eq!(value["schema_version"], umbrella::SCHEMA_VERSION);
        assert_eq!(value["data"]["adopted_method"], ADOPTED_METHOD);
        assert_eq!(value["data"]["adopted_method"], "get_messages");
        assert_eq!(value["data"]["count"], 3);
        assert_eq!(value["data"]["protocol_negotiated"], 2);
        assert_eq!(value["data"]["roles"][0]["role"], "assistant");
        assert_eq!(value["data"]["messages"][0]["content_bytes"], 5);
        assert_eq!(value["data"]["raw"]["messages"][1]["role"], "assistant");
        assert!(
            value["data"]["no_claim"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "the transport's own boundary must travel with the report"
        );
    }

    #[test]
    fn a_refusal_envelope_carries_no_message_fields_at_all() {
        // A refusal that prints a success-shaped envelope is indistinguishable from a success.
        for outcome in [
            MessagesOutcome::Absent {
                detail: "no such file".to_owned(),
            },
            MessagesOutcome::NoPayload,
            MessagesOutcome::Refused {
                detail: "no session".to_owned(),
            },
        ] {
            let value = envelope(&outcome);
            assert_eq!(value["command"], "messages");
            assert_eq!(value["data"]["reason_code"], outcome.reason_code());
            assert_eq!(value["data"]["unmeasured"], outcome.is_unmeasured());
            for withheld in ["count", "roles", "first_role", "last_role", "messages", "raw"] {
                assert!(
                    value["data"][withheld].is_null(),
                    "a non-success envelope must imply no message data; `{withheld}` leaked in \
                     {value:?}"
                );
            }
        }
    }

    #[test]
    fn the_rendered_line_is_stable_and_carries_every_documented_token() {
        let messages = project(&payload(), "stopped", 2).expect("the array is present");
        let line = render(&messages);
        assert!(line.starts_with("OMPO_OMP_MESSAGES "));
        for token in [
            "count=3",
            "roles=assistant:1,user:2",
            "first_role=user",
            "last_role=user",
            "lifecycle=stopped",
            "protocol=2",
            "adopted_method=get_messages",
        ] {
            assert!(line.contains(token), "the line must carry `{token}`; got {line:?}");
        }
        assert_eq!(line, render(&messages), "one payload must render one line");
    }
}
