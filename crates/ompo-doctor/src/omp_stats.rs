//! `ompo stats` — the SECOND cell of axis 2: `ompo` reading OMP's own native surface.
//!
//! Sibling of [`crate::omp_state`], for a different payload. `get_state` answers *what is
//! this session doing right now*; `get_session_stats` answers *what has it consumed* —
//! message counts, tool traffic, token totals, context-window pressure and cost. Both are
//! read FROM THE PROTOCOL rather than scraped from a rendering, which is the defect
//! `AGENTS.md`'s fifth rule names.
//!
//! # KERNEL-ONLY: neither the transport nor the error taxonomy is re-implemented here
//!
//! [`omp_rpc_session::run_session`] already issues all four commands in ONE bounded session
//! and hands back `report.selected.session_stats`; this module is a PROJECTION over that
//! payload, not a second transport. The `RpcError` -> outcome mapping likewise lives once, in
//! [`crate::omp_state::classify_error`], and is delegated to rather than copied: two
//! independent mappings would be two places for a spawn failure to stop meaning ABSENT.
//!
//! # MEASURED SHAPE, 2026-09-08 against `omp/18.1.14`
//!
//! The `data` object of a `get_session_stats` response carries eleven keys, and TWO OF THEM
//! ARE NESTED OBJECTS:
//!
//! ```text
//! assistantMessages  int
//! contextUsage       object  { contextWindow, percent, tokens }
//! cost               int in a fresh session -- a live one may send a FLOAT
//! premiumRequests    int
//! sessionFile        string (an absolute path)
//! sessionId          string
//! tokens             object  { cacheRead, cacheWrite, input, output, reasoning, total }
//! toolCalls          int
//! toolResults        int
//! totalMessages      int
//! userMessages       int
//! ```
//!
//! This module is typed FROM that measurement, not from a guess, because the guess has
//! already cost this repo a verb: the first `omp_state` pass read OMP's `model` field with
//! `as_str()` when OMP sends an eighteen-key OBJECT, so the verb reported `model=absent` for
//! a field that was PRESENT — and its fixture-only tests passed against a payload OMP never
//! sends. Consequently `cost` is read with [`serde_json::Value::as_f64`], which accepts both
//! the integer OMP sends today and the float a busier session sends; `as_u64` alone would
//! report `cost=absent` the moment a session spent a fraction of a cent.

use crate::omp_state::{
    classify_error_for, read_projection, OmpReadOutcome, OmpReadPayload, OutcomeCodes,
    ProjectionMethod, EXIT_OK, EXIT_REFUSED, EXIT_UNMEASURED,
};
#[cfg(test)]
use crate::omp_state::StateOutcome;
use crate::umbrella;
use omp_rpc_session::{RpcRequest, NO_CLAIM_BOUNDARY};
use serde_json::{json, Value};

/// OMP's own native command this verb adopts. One method, named, so the report cannot claim
/// broader protocol coverage than it issues.
pub const ADOPTED_METHOD: &str = "get_session_stats";

/// What one `ompo stats` run established.
///
/// The same six arms as [`crate::omp_state::StateOutcome`], because callers discriminate
/// the same six causes — but with its OWN reason codes, so a log line naming
/// `OMP_STATS_REFUSED` cannot be mistaken for the state verb's refusal.
pub type StatsOutcome = OmpReadOutcome<OmpStats>;

/// OMP's `tokens` object, measured as optional counters.
///
/// A nested object gets a typed struct so callers never index an accidental shape.
/// `tokens.total` remains a named field rather than an untyped lookup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenCounts {
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    pub total: Option<u64>,
}

/// OMP's `contextUsage` object, measured: window size, percent consumed, token count.
///
/// `percent` is a `f64` because a percentage is fractional; reading it as an integer would
/// round 99.6% of a context window down to 99 and hide the cliff.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContextUsage {
    pub context_window: Option<u64>,
    pub percent: Option<f64>,
    pub tokens: Option<u64>,
}

/// The typed projection of OMP's `get_session_stats` payload.
///
/// Every field is `Option` because the payload is OMP's, not ours: a field OMP stops sending
/// must read as absent rather than as a default. Defaulting `tool_calls` to 0 would report a
/// busy session as idle — a silent WRONG ANSWER rather than a gap, and the two have opposite
/// remedies. `raw` is retained whole because OMP may extend the object without changing this
/// contract.
#[derive(Debug, Clone, PartialEq)]
pub struct OmpStats {
    pub session_id: Option<String>,
    pub total_messages: Option<u64>,
    pub user_messages: Option<u64>,
    pub assistant_messages: Option<u64>,
    pub tool_calls: Option<u64>,
    pub tool_results: Option<u64>,
    pub premium_requests: Option<u64>,
    /// Read with `as_f64`: OMP sends an integer in a fresh session and may send a float in a
    /// live one, and both must parse.
    pub cost: Option<f64>,
    pub tokens: Option<TokenCounts>,
    pub context: Option<ContextUsage>,
    pub lifecycle: String,
    pub protocol_negotiated: u32,
    pub raw: Value,
}

impl OmpReadPayload for OmpStats {
    const CODES: OutcomeCodes = OutcomeCodes::new([
        "OMP_STATS_OK",
        "OMP_STATS_REFUSED",
        "OMP_STATS_NO_PAYLOAD",
        "OMP_STATS_ABSENT",
        "OMP_STATS_TIMEOUT_UNMEASURED",
        "OMP_STATS_TRANSPORT_FAILED",
    ]);
    const NO_PAYLOAD_DETAIL: &'static str =
        "OMP answered successfully and carried no session-stats object";

    fn success_detail(&self) -> String {
        format!(
            "session_id={} messages={} tokens_total={}",
            self.session_id.as_deref().unwrap_or("absent"),
            absent_or(self.total_messages),
            absent_or(self.tokens.as_ref().and_then(|tokens| tokens.total)),
        )
    }
}

fn u64_field(object: &Value, key: &str) -> Option<u64> {
    object.get(key).and_then(Value::as_u64)
}

/// Project OMP's `tokens` object. Absent OBJECT and absent COUNTER are different absences:
/// the first yields `None` here, the second yields a struct whose fields are `None`.
fn token_counts(data: &Value) -> Option<TokenCounts> {
    let tokens = data.get("tokens")?;
    if !tokens.is_object() {
        return None;
    }
    Some(TokenCounts {
        cache_read: u64_field(tokens, "cacheRead"),
        cache_write: u64_field(tokens, "cacheWrite"),
        input: u64_field(tokens, "input"),
        output: u64_field(tokens, "output"),
        reasoning: u64_field(tokens, "reasoning"),
        total: u64_field(tokens, "total"),
    })
}

/// Project OMP's `contextUsage` object.
fn context_usage(data: &Value) -> Option<ContextUsage> {
    let context = data.get("contextUsage")?;
    if !context.is_object() {
        return None;
    }
    Some(ContextUsage {
        context_window: u64_field(context, "contextWindow"),
        percent: context.get("percent").and_then(Value::as_f64),
        tokens: u64_field(context, "tokens"),
    })
}

/// Project OMP's raw `get_session_stats` data into the typed shape.
#[must_use]
pub fn project(data: &Value, lifecycle: &str, negotiated: u32) -> OmpStats {
    OmpStats {
        session_id: data
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        total_messages: u64_field(data, "totalMessages"),
        user_messages: u64_field(data, "userMessages"),
        assistant_messages: u64_field(data, "assistantMessages"),
        tool_calls: u64_field(data, "toolCalls"),
        tool_results: u64_field(data, "toolResults"),
        premium_requests: u64_field(data, "premiumRequests"),
        // `as_f64` accepts both the integer OMP sends in a fresh session and the float a
        // live one sends. `as_u64` alone would read a real cost of 0.42 as absent.
        cost: data.get("cost").and_then(Value::as_f64),
        tokens: token_counts(data),
        context: context_usage(data),
        lifecycle: lifecycle.to_owned(),
        protocol_negotiated: negotiated,
        raw: data.clone(),
    }
}
/// The exact request set this verb issues: the handshake plus its own method.
///
/// `negotiate_protocol` is not optional -- `RpcSessionReport::ok()` requires
/// `negotiated == ProtocolVersion::V2`, so it is the precondition for any answer rather
/// than a method this verb reports on. That is why `ADOPTED_METHOD` names ONE method
/// while the set carries two.
#[must_use]
pub fn request_set() -> [RpcRequest; 2] {
    [RpcRequest::NegotiateProtocol, RpcRequest::GetSessionStats]
}

fn classify_stats_error(error: &omp_rpc_session::RpcError) -> StatsOutcome {
    classify_error_for(error)
}


/// Drive one bounded OMP `--mode=rpc` session and read its session stats.
///
/// `&Cx` first, per the asupersync contract; cancellation belongs to the caller.
pub async fn read_stats(cx: &asupersync::Cx, binary: &str) -> StatsOutcome {
    read_projection(cx, binary, ProjectionMethod::SessionStats, |data, lifecycle, negotiated| {
        Some(project(data, lifecycle, negotiated))
    })
    .await
}

fn absent_or<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "absent".to_owned(), |v| v.to_string())
}

/// The human line. Success only; a refusal prints its reason code on stderr instead.
///
/// An absent field renders the literal `absent` rather than `0`, so a reader can tell "OMP
/// did not send this" from "OMP sent zero".
#[must_use]
pub fn render(stats: &OmpStats) -> String {
    format!(
        "OMPO_OMP_STATS session_id={} messages={} tool_calls={} cost={} tokens_total={} \
         context_percent={} lifecycle={} protocol={} adopted_method={ADOPTED_METHOD}",
        stats.session_id.as_deref().unwrap_or("absent"),
        absent_or(stats.total_messages),
        absent_or(stats.tool_calls),
        absent_or(stats.cost),
        absent_or(stats.tokens.as_ref().and_then(|t| t.total)),
        absent_or(stats.context.as_ref().and_then(|c| c.percent)),
        stats.lifecycle,
        stats.protocol_negotiated,
    )
}

fn tokens_json(tokens: Option<&TokenCounts>) -> Value {
    match tokens {
        Some(tokens) => json!({
            "cache_read": tokens.cache_read,
            "cache_write": tokens.cache_write,
            "input": tokens.input,
            "output": tokens.output,
            "reasoning": tokens.reasoning,
            "total": tokens.total,
        }),
        None => Value::Null,
    }
}

fn context_json(context: Option<&ContextUsage>) -> Value {
    match context {
        Some(context) => json!({
            "context_window": context.context_window,
            "percent": context.percent,
            "tokens": context.tokens,
        }),
        None => Value::Null,
    }
}

/// The `--json` envelope. Emitted for the SUCCESS path only: a refusal that prints a
/// success-shaped envelope is indistinguishable from a success.
#[must_use]
pub fn envelope(outcome: &StatsOutcome) -> Value {
    let data = match outcome {
        StatsOutcome::Answered(stats) => json!({
            "adopted_method": ADOPTED_METHOD,
            "lifecycle": stats.lifecycle,
            "protocol_negotiated": stats.protocol_negotiated,
            "session_id": stats.session_id,
            "total_messages": stats.total_messages,
            "user_messages": stats.user_messages,
            "assistant_messages": stats.assistant_messages,
            "tool_calls": stats.tool_calls,
            "tool_results": stats.tool_results,
            "premium_requests": stats.premium_requests,
            "cost": stats.cost,
            "tokens": tokens_json(stats.tokens.as_ref()),
            "context_usage": context_json(stats.context.as_ref()),
            "reason_code": outcome.reason_code(),
            "raw": stats.raw,
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
    umbrella::envelope("stats", outcome.envelope_status(), data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omp_state::{EXIT_BAD_INVOCATION, EXIT_INSTRUMENT};
    use omp_rpc_session::{RpcError, TimeoutPhase};

    /// The MEASURED payload, 2026-09-08 against `omp/18.1.14`, nested objects included.
    ///
    /// `sessionFile` carries a NEUTRAL path: the measured value is an absolute path under a
    /// real account home, and this repository's `path-literal-guard` pre-commit gate refuses
    /// such a literal. The field's SHAPE (a string absolute path) is what the fixture must
    /// preserve, and it does.
    fn payload() -> Value {
        json!({
            "assistantMessages": 20,
            "contextUsage": {"contextWindow": 200_000, "percent": 12.5, "tokens": 25_000},
            "cost": 0,
            "premiumRequests": 3,
            "sessionFile": "/tmp/fixture/session.jsonl",
            "sessionId": "abc-123",
            "tokens": {
                "cacheRead": 900,
                "cacheWrite": 800,
                "input": 100,
                "output": 200,
                "reasoning": 50,
                "total": 2_050
            },
            "toolCalls": 7,
            "toolResults": 6,
            "totalMessages": 41,
            "userMessages": 21,
        })
    }

    fn answered() -> StatsOutcome {
        StatsOutcome::Answered(Box::new(project(&payload(), "stopped", 2)))
    }

    #[test]
    fn the_projection_reads_omps_field_names_not_ours() {
        let stats = project(&payload(), "stopped", 2);
        assert_eq!(stats.session_id.as_deref(), Some("abc-123"));
        assert_eq!(stats.total_messages, Some(41));
        assert_eq!(stats.user_messages, Some(21));
        assert_eq!(stats.assistant_messages, Some(20));
        assert_eq!(stats.tool_calls, Some(7));
        assert_eq!(stats.tool_results, Some(6));
        assert_eq!(stats.premium_requests, Some(3));
        assert_eq!(stats.lifecycle, "stopped");
        assert_eq!(stats.protocol_negotiated, 2);
    }

    #[test]
    fn the_nested_token_object_is_typed_not_read_as_a_scalar() {
        // MEASURED: `tokens` is an OBJECT with six counters. The sibling verb's first pass
        // read an object field with `as_str()` and reported it absent while it was PRESENT;
        // typing the nested shape is what stops that recurring here.
        let stats = project(&payload(), "stopped", 2);
        let tokens = stats.tokens.expect("the measured payload carries `tokens`");
        assert_eq!(tokens.cache_read, Some(900));
        assert_eq!(tokens.cache_write, Some(800));
        assert_eq!(tokens.input, Some(100));
        assert_eq!(tokens.output, Some(200));
        assert_eq!(tokens.reasoning, Some(50));
        assert_eq!(tokens.total, Some(2_050));
    }

    #[test]
    fn the_nested_context_usage_object_is_typed_and_its_percent_stays_fractional() {
        let stats = project(&payload(), "stopped", 2);
        let context = stats
            .context
            .expect("the measured payload carries `contextUsage`");
        assert_eq!(context.context_window, Some(200_000));
        assert_eq!(context.tokens, Some(25_000));
        // Reading a percentage as an integer would round a 99.6% window down to 99 and hide
        // the cliff, so the fraction must survive.
        assert_eq!(context.percent, Some(12.5));
    }

    #[test]
    fn cost_parses_as_both_the_integer_omp_sends_and_the_float_it_may_send() {
        // OMP sends 0 in a fresh session; a live session sends a fraction of a cent. Reading
        // with `as_u64` alone would report a real cost of 0.42 as absent.
        assert_eq!(project(&payload(), "stopped", 2).cost, Some(0.0));
        assert_eq!(
            project(&json!({"cost": 0.42}), "stopped", 2).cost,
            Some(0.42)
        );
        assert_eq!(project(&json!({"cost": 7}), "stopped", 2).cost, Some(7.0));
    }

    #[test]
    fn a_field_omp_stops_sending_reads_as_absent_rather_than_as_a_default() {
        // Defaulting `toolCalls` to 0 would report a BUSY session as idle -- a silent wrong
        // answer rather than a gap, and the two have opposite remedies.
        let stats = project(&json!({"sessionId": "x"}), "stopped", 2);
        assert_eq!(stats.tool_calls, None);
        assert_eq!(stats.total_messages, None);
        assert_eq!(stats.cost, None);
        assert_eq!(stats.tokens, None);
        assert_eq!(stats.context, None);
        let line = render(&stats);
        assert!(line.contains("tool_calls=absent"), "got {line:?}");
        assert!(line.contains("messages=absent"), "got {line:?}");
        assert!(line.contains("cost=absent"), "got {line:?}");
        assert!(line.contains("tokens_total=absent"), "got {line:?}");
        assert!(line.contains("context_percent=absent"), "got {line:?}");
        assert!(!line.contains("=0"), "no field may default to zero: {line:?}");
    }

    #[test]
    fn an_absent_nested_counter_is_not_the_same_absence_as_an_absent_nested_object() {
        // OMP sending `tokens: {}` means "the object exists and this counter did not come";
        // OMP omitting `tokens` means "no object at all". Collapsing them would report a
        // protocol change as a zero count.
        let present_but_empty = project(&json!({"tokens": {}}), "stopped", 2);
        let tokens = present_but_empty
            .tokens
            .expect("an empty object is still an object");
        assert_eq!(tokens.total, None);
        assert_eq!(project(&json!({}), "stopped", 2).tokens, None);
    }

    #[test]
    fn the_raw_payload_is_retained_whole_so_an_omp_extension_is_not_lost() {
        let stats = project(&payload(), "stopped", 2);
        // `sessionFile` is projected into no typed field, and it must still survive.
        assert_eq!(stats.raw["sessionFile"], "/tmp/fixture/session.jsonl");
        assert_eq!(stats.raw["tokens"]["cacheRead"], 900);
        assert_eq!(stats.raw["contextUsage"]["percent"], 12.5);
        assert_eq!(stats.raw, payload());
    }

    #[test]
    fn omp_said_no_and_omp_did_not_answer_are_different_outcomes() {
        let refused = StatsOutcome::Refused {
            detail: "no session".to_owned(),
        };
        let absent = StatsOutcome::Absent {
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
        let timed_out = StatsOutcome::TimedOut {
            phase: "startup".to_owned(),
        };
        assert_eq!(timed_out.reason_code(), "OMP_STATS_TIMEOUT_UNMEASURED");
        assert_eq!(timed_out.exit_code(), EXIT_UNMEASURED);
        assert_ne!(timed_out.exit_code(), EXIT_OK);
        assert!(timed_out.is_unmeasured());
    }

    #[test]
    fn success_with_no_payload_is_not_a_success() {
        assert_eq!(StatsOutcome::NoPayload.exit_code(), EXIT_REFUSED);
        assert_ne!(StatsOutcome::NoPayload.exit_code(), EXIT_OK);
        assert_eq!(StatsOutcome::NoPayload.reason_code(), "OMP_STATS_NO_PAYLOAD");
        assert_eq!(StatsOutcome::NoPayload.envelope_status(), "DEGRADED");
    }

    #[test]
    fn the_request_set_is_narrow_and_names_the_method_this_verb_reports() {
        // The selector ruling, pinned. A WIDENING is what this leg exists to catch: requesting
        // `get_session_stats` plus anything else means the verb pays a deadline for a method it does not
        // report, and %8 measured the four-request sequence TIMING OUT at `get_messages` on a
        // resumed session. Two entries, not three, and not four.
        let set = request_set();
        assert_eq!(set.len(), 2, "a wider set makes this verb wait on methods it never reports");
        assert_eq!(set[0], RpcRequest::NegotiateProtocol, "the handshake is the precondition");
        assert_eq!(set[1], RpcRequest::GetSessionStats);
        assert_eq!(
            set[1].command(),
            ADOPTED_METHOD,
            "ADOPTED_METHOD must name the method actually issued -- a report naming a method the \
             session did not issue is the overclaim this pins shut"
        );
    }

    #[test]
    fn the_request_set_does_not_carry_the_other_verbs_methods() {
        // Anti-vacuity for the leg above: asserting a length says nothing if the CONTENTS drift.
        let set = request_set();
        for forbidden in [RpcRequest::GetState, RpcRequest::GetMessages] {
            assert!(
                !set.contains(&forbidden),
                "{forbidden:?} belongs to another verb and its deadline is not this verb's to pay"
            );
        }
    }

    #[test]
    fn every_outcome_has_a_distinct_reason_code() {
        let codes = [
            answered().reason_code(),
            StatsOutcome::Refused {
                detail: String::new(),
            }
            .reason_code(),
            StatsOutcome::NoPayload.reason_code(),
            StatsOutcome::Absent {
                detail: String::new(),
            }
            .reason_code(),
            StatsOutcome::TimedOut {
                phase: String::new(),
            }
            .reason_code(),
            StatsOutcome::TransportFailed {
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
                code.starts_with("OMP_STATS_"),
                "every reason must name THIS verb's family, not the sibling's: {code:?}"
            );
        }
    }

    #[test]
    fn this_verbs_reason_codes_never_collide_with_the_state_verbs() {
        // Two verbs sharing a token means a log line cannot say which surface refused.
        let mine = [
            "OMP_STATS_OK",
            "OMP_STATS_REFUSED",
            "OMP_STATS_NO_PAYLOAD",
            "OMP_STATS_ABSENT",
            "OMP_STATS_TIMEOUT_UNMEASURED",
            "OMP_STATS_TRANSPORT_FAILED",
        ];
        let theirs = [
            StateOutcome::Refused {
                detail: String::new(),
            }
            .reason_code(),
            StateOutcome::NoPayload.reason_code(),
            StateOutcome::Absent {
                detail: String::new(),
            }
            .reason_code(),
            StateOutcome::TimedOut {
                phase: String::new(),
            }
            .reason_code(),
            StateOutcome::TransportFailed {
                detail: String::new(),
            }
            .reason_code(),
        ];
        for code in theirs {
            assert!(
                !mine.contains(&code),
                "`{code}` belongs to two verbs at once"
            );
        }
        assert_eq!(answered().reason_code(), "OMP_STATS_OK");
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
        assert_eq!(
            unique.len(),
            codes.len(),
            "two causes share one exit code: {codes:?}"
        );
    }

    #[test]
    fn the_shared_transport_classifier_is_delegated_to_arm_for_arm() {
        // The mapping lives ONCE, in `omp_state::classify_error`. A second copy is a second
        // place for a spawn failure to stop meaning ABSENT.
        let spawn = classify_stats_error(&RpcError::Process {
            operation: "spawn".to_owned(),
            detail: "No such file or directory".to_owned(),
        });
        assert_eq!(spawn.reason_code(), "OMP_STATS_ABSENT");
        assert!(spawn.is_unmeasured());

        let other = classify_stats_error(&RpcError::Process {
            operation: "kill".to_owned(),
            detail: "boom".to_owned(),
        });
        assert_eq!(other.reason_code(), "OMP_STATS_TRANSPORT_FAILED");

        for phase in [
            TimeoutPhase::Startup,
            TimeoutPhase::Request,
            TimeoutPhase::Shutdown,
        ] {
            let outcome = classify_stats_error(&RpcError::Timeout { phase });
            let StatsOutcome::TimedOut { phase: name } = &outcome else {
                panic!("a timeout must classify as TimedOut, got {outcome:?}");
            };
            assert!(!name.is_empty(), "the phase must be named");
            assert_ne!(
                name, "unknown",
                "a placeholder phase hides which deadline expired"
            );
        }

        let exited = classify_stats_error(&RpcError::ProcessExited { code: Some(1) });
        assert_eq!(exited.reason_code(), "OMP_STATS_TRANSPORT_FAILED");
    }

    #[test]
    fn the_success_envelope_carries_the_adopted_method_and_the_no_claim() {
        let value = envelope(&answered());
        assert_eq!(value["command"], "stats");
        assert_eq!(value["status"], "OK");
        assert_eq!(value["schema_version"], umbrella::SCHEMA_VERSION);
        assert_eq!(value["data"]["adopted_method"], ADOPTED_METHOD);
        assert_eq!(value["data"]["adopted_method"], "get_session_stats");
        assert_eq!(value["data"]["session_id"], "abc-123");
        assert_eq!(value["data"]["total_messages"], 41);
        assert_eq!(value["data"]["tokens"]["total"], 2_050);
        assert_eq!(value["data"]["context_usage"]["percent"], 12.5);
        assert_eq!(value["data"]["protocol_negotiated"], 2);
        assert_eq!(
            value["data"]["no_claim"], NO_CLAIM_BOUNDARY,
            "the transport's own boundary must travel with the report"
        );
        assert!(
            value["data"]["no_claim"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "the boundary must be a non-empty string"
        );
    }

    #[test]
    fn a_refusal_envelope_carries_no_stats_fields_at_all() {
        // A refusal that prints a success-shaped envelope is indistinguishable from a
        // success to any consumer that parses stdout and ignores the exit code.
        let value = envelope(&StatsOutcome::Absent {
            detail: "no such file".to_owned(),
        });
        assert_eq!(value["status"], "UNKNOWN");
        assert_eq!(value["data"]["reason_code"], "OMP_STATS_ABSENT");
        assert_eq!(value["data"]["unmeasured"], true);
        for withheld in [
            "session_id",
            "total_messages",
            "tool_calls",
            "cost",
            "tokens",
            "context_usage",
            "raw",
        ] {
            assert!(
                value["data"][withheld].is_null(),
                "a refusal must imply no `{withheld}`; got {value:?}"
            );
        }
    }

    #[test]
    fn a_refused_outcome_is_measured_and_still_withholds_the_payload() {
        let value = envelope(&StatsOutcome::Refused {
            detail: "no active session".to_owned(),
        });
        assert_eq!(value["status"], "DEGRADED");
        assert_eq!(value["data"]["reason_code"], "OMP_STATS_REFUSED");
        assert_eq!(value["data"]["unmeasured"], false);
        assert_eq!(value["data"]["detail"], "no active session");
        assert!(value["data"]["tokens"].is_null());
    }

    #[test]
    fn the_rendered_line_names_the_verb_and_every_documented_field() {
        let stats = project(&payload(), "stopped", 2);
        let line = render(&stats);
        assert!(line.starts_with("OMPO_OMP_STATS "), "got {line:?}");
        for field in [
            "session_id=abc-123",
            "messages=41",
            "tool_calls=7",
            "cost=0",
            "tokens_total=2050",
            "context_percent=12.5",
            "lifecycle=stopped",
            "protocol=2",
            "adopted_method=get_session_stats",
        ] {
            assert!(line.contains(field), "`{field}` missing from {line:?}");
        }
    }
}
