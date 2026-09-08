//! `ompo state` — the FIRST cell of axis 2: `ompo` reading OMP's own native surface.
//!
//! Bead: `omp-orchestrator-jplf.7.2`. Joshua's sequencing, verbatim: *"mapping at least one
//! surface and getting it proven end to end then ported to rust asupersync to prove it works
//! via a cli command, then we can expand."* This is the one cell, not the map.
//!
//! # It answers the question `AGENTS.md`'s fifth rule says we SCRAPE
//!
//! That rule names the defect exactly: *"pane state — a braille-spinner regex over
//! `capture-pane` … we are parsing paint."* OMP's `get_state` returns `isStreaming`,
//! `queuedMessageCount`, `messageCount`, `model`, `contextUsage`, `sessionId` and
//! `todoPhases` **from the protocol**. A spinner is a RENDERING of that state; this is the
//! state.
//!
//! # KERNEL-ONLY: the transport already existed and is not re-implemented here
//!
//! [`omp_rpc_session::run_session`] is a 1,383-line asupersync-backed bounded transport at
//! HEAD that already models `ready`/`response`/`unknown`/`malformed` frames, negotiates
//! protocol v2, and drives OMP's real `get_state` command. This module adds a VERB, not a
//! second transport, and the `Cx` acquisition below copies `omp-orchestrator/src/main.rs`
//! rather than inventing a runtime discipline.
//!
//! # MEASURED CORRECTION to the premise this unit was dispatched under
//!
//! The dispatch proposed `session/list` from a fourteen-method `omp/*` list. Measured
//! 2026-09-08 against `omp/18.1.14`: `--mode=rpc` does NOT speak `{"method": …}` JSON-RPC.
//! It speaks `{"id": …, "type": …}` frames, and a `{"method": …}` frame is answered
//! `{"type":"response","success":false,"error":"Unknown command: undefined"}` — it looked for
//! a `command`/`type` field and found none. The `omp/*` names are the MUX socket protocol, a
//! different channel. `session/list` is not in this vocabulary; `get_state` is, and it is the
//! surface that replaces a rendering.
//!
//! All four commands the transport already issues were verified to ANSWER on this host, with a
//! negative control that discriminates (`{"type":"zzz_cannot_exist"}` gets NO response frame at
//! all, so a reply is evidence of a real command rather than of a permissive server).

use crate::umbrella;
use omp_rpc_session::{
    run_session, OmpCommand, RpcError, RpcSessionConfig, TimeoutPhase, NO_CLAIM_BOUNDARY,
};
use serde_json::{json, Value};

/// OMP's own native command this verb adopts. One method, named, so the report cannot claim
/// broader protocol coverage than it issues.
pub const ADOPTED_METHOD: &str = "get_state";

/// Exit codes — a DOCUMENTED DICTIONARY, each naming a different cause. Mirrors `main.rs`;
/// `exit_vocabulary_is_pairwise_distinct` guards the pair against drifting together.
pub const EXIT_OK: u8 = 0;
/// OMP answered and said no, or answered with nothing. **The subject failed.**
pub const EXIT_REFUSED: u8 = 1;
/// The caller's mistake.
pub const EXIT_BAD_INVOCATION: u8 = 2;
/// This binary could not build its own runtime.
pub const EXIT_INSTRUMENT: u8 = 3;
/// **UNMEASURED**, never a verdict about OMP: absent binary, or a deadline.
pub const EXIT_UNMEASURED: u8 = 4;

/// What one `ompo state` run established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateOutcome {
    /// OMP answered and the state payload is present.
    Answered(Box<OmpState>),
    /// OMP answered and refused. **"OMP said no" and "OMP did not answer" have opposite
    /// remedies**, so they are never one arm.
    Refused { detail: String },
    /// OMP answered successfully and carried no state object. A protocol surprise, not a
    /// success: a verb that reports nothing must not report identically to one that reported.
    NoPayload,
    /// The binary is absent. UNMEASURED — this says nothing about OMP.
    Absent { detail: String },
    /// A deadline expired. `subprocess_contract::BoundedOutcome::TimedOut -> "UNMEASURED"` is
    /// the precedent at `crate::run_doctor`'s probe loop and it is reused rather than
    /// reinvented: **a timeout is a restrictive terminal, never a pass.**
    TimedOut { phase: String },
    /// Spawn, io or protocol failure. Distinct from a refusal by construction.
    TransportFailed { detail: String },
}

/// The typed projection of OMP's `get_state` payload.
///
/// Every field is `Option` because the payload is OMP's, not ours: a field OMP stops sending
/// must read as absent rather than as a default. `raw` is retained whole because OMP may
/// extend the object without changing this contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmpState {
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub is_streaming: Option<bool>,
    pub queued_message_count: Option<u64>,
    pub message_count: Option<u64>,
    pub lifecycle: String,
    pub protocol_negotiated: u32,
    pub raw: Value,
}

impl StateOutcome {
    #[must_use]
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Answered(_) => "OMP_STATE_OK",
            Self::Refused { .. } => "OMP_STATE_REFUSED",
            Self::NoPayload => "OMP_STATE_NO_PAYLOAD",
            Self::Absent { .. } => "OMP_STATE_ABSENT",
            Self::TimedOut { .. } => "OMP_STATE_TIMEOUT_UNMEASURED",
            Self::TransportFailed { .. } => "OMP_STATE_TRANSPORT_FAILED",
        }
    }

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
            Self::Answered(state) => format!(
                "session_id={} model={}",
                state.session_id.as_deref().unwrap_or("absent"),
                state.model.as_deref().unwrap_or("absent")
            ),
            Self::Refused { detail } | Self::TransportFailed { detail } => detail.clone(),
            Self::Absent { detail } => detail.clone(),
            Self::NoPayload => {
                "OMP answered successfully and carried no state object".to_owned()
            }
            Self::TimedOut { phase } => format!("deadline expired in phase={phase}"),
        }
    }
}

/// OMP's `model` is an OBJECT, not a string — measured against `omp/18.1.14`: eighteen keys
/// including `id`, `name` and `provider`. A first pass read it with `as_str()` and reported
/// `model=absent` for a field that WAS present, which is the honest failure direction but
/// still the wrong answer: "OMP stopped sending this" and "we read it with the wrong type"
/// have different remedies. `id` is the stable identifier (`"claude-opus-5"`); `name` is the
/// display string and is not used as an identity.
fn model_id(data: &Value) -> Option<String> {
    let model = data.get("model")?;
    if let Some(text) = model.as_str() {
        return Some(text.to_owned());
    }
    model
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Project OMP's raw `get_state` data into the typed shape.
#[must_use]
pub fn project(data: &Value, lifecycle: &str, negotiated: u32) -> OmpState {
    OmpState {
        session_id: data
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        model: model_id(data),
        is_streaming: data.get("isStreaming").and_then(Value::as_bool),
        queued_message_count: data.get("queuedMessageCount").and_then(Value::as_u64),
        message_count: data.get("messageCount").and_then(Value::as_u64),
        lifecycle: lifecycle.to_owned(),
        protocol_negotiated: negotiated,
        raw: data.clone(),
    }
}

/// Map a transport error onto the outcome taxonomy. **Each `RpcError` arm lands on exactly one
/// outcome**, and the mapping is the whole point: a spawn failure and a refusal must not share
/// a code.
#[must_use]
pub fn classify_error(error: &RpcError) -> StateOutcome {
    match error {
        RpcError::Process { operation, detail } => {
            // A missing binary surfaces here as a spawn failure. It is ABSENT, not a
            // transport defect, because nothing was ever reached.
            if operation == "spawn" {
                StateOutcome::Absent {
                    detail: detail.clone(),
                }
            } else {
                StateOutcome::TransportFailed {
                    detail: format!("{operation}: {detail}"),
                }
            }
        }
        RpcError::ProcessExited { code } => StateOutcome::TransportFailed {
            detail: format!(
                "omp exited before answering, code={}",
                code.map_or_else(|| "none".to_owned(), |c| c.to_string())
            ),
        },
        RpcError::Cancelled { detail } => StateOutcome::TimedOut {
            phase: format!("cancelled: {detail}"),
        },
        RpcError::Timeout { phase } => StateOutcome::TimedOut {
            phase: timeout_phase(*phase).to_owned(),
        },
        RpcError::Protocol(protocol) => StateOutcome::TransportFailed {
            detail: format!("protocol: {protocol}"),
        },
        RpcError::Io { stream, detail } => StateOutcome::TransportFailed {
            detail: format!("io on {stream}: {detail}"),
        },
        RpcError::Cleanup { primary, detail } => StateOutcome::TransportFailed {
            detail: format!("cleanup after {}: {detail}", primary_label(primary)),
        },
    }
}

fn timeout_phase(phase: TimeoutPhase) -> &'static str {
    match phase {
        TimeoutPhase::Startup => "startup",
        TimeoutPhase::Request => "request",
        TimeoutPhase::Shutdown => "shutdown",
    }
}

fn primary_label(primary: &RpcError) -> &'static str {
    match primary {
        RpcError::Process { .. } => "process",
        RpcError::ProcessExited { .. } => "process-exited",
        RpcError::Cancelled { .. } => "cancelled",
        RpcError::Timeout { .. } => "timeout",
        RpcError::Protocol(_) => "protocol",
        RpcError::Io { .. } => "io",
        RpcError::Cleanup { .. } => "cleanup",
    }
}

/// Drive one bounded OMP `--mode=rpc` session and read its state.
///
/// `&Cx` first, per the asupersync contract; cancellation belongs to the caller.
pub async fn read_state(cx: &asupersync::Cx, binary: &str) -> StateOutcome {
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
                return StateOutcome::Refused {
                    detail: refusal
                        .error
                        .clone()
                        .unwrap_or_else(|| "omp refused get_state without an error string".to_owned()),
                };
            }
            match report.selected.state.as_ref() {
                Some(data) => {
                    StateOutcome::Answered(Box::new(project(data, lifecycle, negotiated)))
                }
                None => StateOutcome::NoPayload,
            }
        }
        Err(error) => classify_error(&error),
    }
}

/// The human line. Success only; a refusal prints its reason code on stderr instead.
#[must_use]
pub fn render(state: &OmpState) -> String {
    format!(
        "OMPO_OMP_STATE session_id={} model={} is_streaming={} queued={} messages={} \
         lifecycle={} protocol={} adopted_method={ADOPTED_METHOD}",
        state.session_id.as_deref().unwrap_or("absent"),
        state.model.as_deref().unwrap_or("absent"),
        state
            .is_streaming
            .map_or_else(|| "absent".to_owned(), |v| v.to_string()),
        state
            .queued_message_count
            .map_or_else(|| "absent".to_owned(), |v| v.to_string()),
        state
            .message_count
            .map_or_else(|| "absent".to_owned(), |v| v.to_string()),
        state.lifecycle,
        state.protocol_negotiated,
    )
}

/// The `--json` envelope. Emitted for the SUCCESS path only: a refusal that prints a
/// success-shaped envelope is indistinguishable from a success.
#[must_use]
pub fn envelope(outcome: &StateOutcome) -> Value {
    let data = match outcome {
        StateOutcome::Answered(state) => json!({
            "adopted_method": ADOPTED_METHOD,
            "lifecycle": state.lifecycle,
            "protocol_negotiated": state.protocol_negotiated,
            "session_id": state.session_id,
            "model": state.model,
            "is_streaming": state.is_streaming,
            "queued_message_count": state.queued_message_count,
            "message_count": state.message_count,
            "reason_code": outcome.reason_code(),
            "raw": state.raw,
            "no_claim": NO_CLAIM_BOUNDARY,
        }),
        other => json!({
            "adopted_method": ADOPTED_METHOD,
            "reason_code": other.reason_code(),
            "detail": other.detail(),
            "unmeasured": other.is_unmeasured(),
            "no_claim": NO_CLAIM_BOUNDARY,
        }),
    };
    umbrella::envelope("state", outcome.envelope_status(), data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> Value {
        json!({
            "sessionId": "abc-123",
            "model": {"id": "claude-opus-5", "name": "Claude Opus 5", "provider": "anthropic"},
            "isStreaming": false,
            "queuedMessageCount": 0,
            "messageCount": 41,
            "contextUsage": {"used": 1},
        })
    }

    #[test]
    fn the_projection_reads_omps_field_names_not_ours() {
        let state = project(&payload(), "stopped", 2);
        assert_eq!(state.session_id.as_deref(), Some("abc-123"));
        assert_eq!(state.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(state.is_streaming, Some(false));
        assert_eq!(state.queued_message_count, Some(0));
        assert_eq!(state.message_count, Some(41));
        assert_eq!(state.protocol_negotiated, 2);
        assert_eq!(state.lifecycle, "stopped");
    }

    #[test]
    fn a_field_omp_stops_sending_reads_as_absent_rather_than_as_a_default() {
        // The payload is OMP's, not ours. Defaulting `isStreaming` to false would report a
        // streaming session as idle -- a silent wrong answer rather than a gap.
        let state = project(&json!({"sessionId": "x"}), "stopped", 2);
        assert_eq!(state.is_streaming, None);
        assert_eq!(state.message_count, None);
        assert_eq!(state.model, None);
        assert!(render(&state).contains("is_streaming=absent"));
        assert!(render(&state).contains("messages=absent"));
    }

    #[test]
    fn the_model_object_is_read_by_id_not_with_the_wrong_type() {
        // MEASURED: OMP sends `model` as an eighteen-key OBJECT, not a string. Reading it
        // with as_str() reported `model=absent` for a field that was present -- honest in
        // direction and still the wrong answer.
        let state = project(&payload(), "stopped", 2);
        assert_eq!(state.model.as_deref(), Some("claude-opus-5"));
    }

    #[test]
    fn a_string_model_still_reads_so_an_omp_shape_change_does_not_break_the_verb() {
        let state = project(&json!({"model": "some-future-string"}), "stopped", 2);
        assert_eq!(state.model.as_deref(), Some("some-future-string"));
    }

    #[test]
    fn a_model_object_without_an_id_reads_as_absent_rather_than_borrowing_the_display_name() {
        // `name` is a display string, not an identity. Falling back to it would report a
        // human label where a caller expects a stable id.
        let state = project(&json!({"model": {"name": "Claude Opus 5"}}), "stopped", 2);
        assert_eq!(state.model, None);
    }

    #[test]
    fn the_raw_payload_is_retained_whole_so_an_omp_extension_is_not_lost() {
        let state = project(&payload(), "stopped", 2);
        assert_eq!(state.raw["contextUsage"]["used"], 1);
    }

    #[test]
    fn omp_said_no_and_omp_did_not_answer_are_different_outcomes() {
        let refused = StateOutcome::Refused {
            detail: "no session".to_owned(),
        };
        let absent = StateOutcome::Absent {
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
        let timed_out = StateOutcome::TimedOut {
            phase: "startup".to_owned(),
        };
        assert_eq!(timed_out.reason_code(), "OMP_STATE_TIMEOUT_UNMEASURED");
        assert_eq!(timed_out.exit_code(), EXIT_UNMEASURED);
        assert_ne!(timed_out.exit_code(), EXIT_OK);
        assert!(timed_out.is_unmeasured());
    }

    #[test]
    fn success_with_no_payload_is_not_a_success() {
        // Anti-vacuity: a verb that reported nothing must not report identically to one that
        // reported. OMP answering `success:true` with no state object is a protocol surprise.
        assert_eq!(StateOutcome::NoPayload.exit_code(), EXIT_REFUSED);
        assert_ne!(StateOutcome::NoPayload.exit_code(), EXIT_OK);
        assert_eq!(StateOutcome::NoPayload.reason_code(), "OMP_STATE_NO_PAYLOAD");
    }

    #[test]
    fn every_outcome_has_a_distinct_reason_code() {
        let codes = [
            StateOutcome::Answered(Box::new(project(&payload(), "stopped", 2))).reason_code(),
            StateOutcome::Refused { detail: String::new() }.reason_code(),
            StateOutcome::NoPayload.reason_code(),
            StateOutcome::Absent { detail: String::new() }.reason_code(),
            StateOutcome::TimedOut { phase: String::new() }.reason_code(),
            StateOutcome::TransportFailed { detail: String::new() }.reason_code(),
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len(), "two causes share one token: {codes:?}");
    }

    #[test]
    fn exit_vocabulary_is_pairwise_distinct() {
        let codes = [
            EXIT_OK,
            EXIT_REFUSED,
            EXIT_BAD_INVOCATION,
            EXIT_INSTRUMENT,
            EXIT_UNMEASURED,
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len(), "two causes share one exit code: {codes:?}");
    }

    #[test]
    fn a_spawn_failure_is_absent_and_any_other_process_error_is_a_transport_failure() {
        // The discriminator matters: an absent binary is a LOCAL install gap and a mid-session
        // process error is a transport defect. One remedy is `install omp`, the other is not.
        let spawn = classify_error(&RpcError::Process {
            operation: "spawn".to_owned(),
            detail: "No such file or directory".to_owned(),
        });
        assert_eq!(spawn.reason_code(), "OMP_STATE_ABSENT");
        let other = classify_error(&RpcError::Process {
            operation: "kill".to_owned(),
            detail: "boom".to_owned(),
        });
        assert_eq!(other.reason_code(), "OMP_STATE_TRANSPORT_FAILED");
    }

    #[test]
    fn every_timeout_phase_maps_to_a_named_phase_not_to_a_placeholder() {
        // The three real phases, READ from the enum rather than inferred: my first pass
        // guessed `Ready` and `Total` from one visible line and the compiler refused both.
        for phase in [
            TimeoutPhase::Startup,
            TimeoutPhase::Request,
            TimeoutPhase::Shutdown,
        ] {
            let outcome = classify_error(&RpcError::Timeout { phase });
            let StateOutcome::TimedOut { phase: name } = &outcome else {
                panic!("a timeout must classify as TimedOut, got {outcome:?}");
            };
            assert!(!name.is_empty(), "the phase must be named");
            assert_ne!(name, "unknown", "a placeholder phase hides which deadline expired");
        }
    }

    #[test]
    fn the_success_envelope_carries_the_adopted_method_and_the_no_claim() {
        let outcome = StateOutcome::Answered(Box::new(project(&payload(), "stopped", 2)));
        let value = envelope(&outcome);
        assert_eq!(value["command"], "state");
        assert_eq!(value["status"], "OK");
        assert_eq!(value["schema_version"], umbrella::SCHEMA_VERSION);
        assert_eq!(value["data"]["adopted_method"], ADOPTED_METHOD);
        assert_eq!(value["data"]["session_id"], "abc-123");
        assert_eq!(value["data"]["protocol_negotiated"], 2);
        assert!(
            value["data"]["no_claim"].as_str().is_some_and(|s| !s.is_empty()),
            "the transport's own boundary must travel with the report"
        );
    }

    #[test]
    fn a_refusal_envelope_carries_no_state_fields_at_all() {
        // A refusal that prints a success-shaped envelope is indistinguishable from a success.
        let value = envelope(&StateOutcome::Absent {
            detail: "no such file".to_owned(),
        });
        assert_eq!(value["status"], "UNKNOWN");
        assert_eq!(value["data"]["reason_code"], "OMP_STATE_ABSENT");
        assert_eq!(value["data"]["unmeasured"], true);
        assert!(value["data"]["session_id"].is_null(), "no state may be implied");
        assert!(value["data"]["is_streaming"].is_null(), "no state may be implied");
    }
}
