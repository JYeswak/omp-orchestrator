//! `ompo models` — a typed projection of OMP's native `get_available_models` command.
//!
//! The transport and its failure taxonomy are shared with `ompo state`, `stats`, and
//! `messages`. This module owns only the model-catalog projection. Measured against
//! installed `omp/18.1.19`: `data.models` is an array and members carry `id`, `name`,
//! `provider`, `contextWindow`, `maxTokens`, `reasoning`, and capability fields.

use crate::omp_state::{
    read_projection, render_counts, OmpReadOutcome, OmpReadPayload, OutcomeCodes,
    ProjectionMethod, EXIT_OK, EXIT_REFUSED,
};
use crate::umbrella;
use omp_rpc_session::{RpcRequest, NO_CLAIM_BOUNDARY};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The one OMP method reported by this projection.
pub const ADOPTED_METHOD: &str = "get_available_models";

/// What one `ompo models` invocation established.
pub type ModelsOutcome = OmpReadOutcome<OmpModels>;
/// One model entry. Optional fields remain absent rather than acquiring invented defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSummary {
    pub id: Option<String>,
    pub name: Option<String>,
    pub provider: Option<String>,
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
    pub reasoning: Option<bool>,
    pub supports_computer_use: Option<bool>,
}

/// Typed summary plus the complete forward-compatible payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmpModels {
    pub count: usize,
    pub providers: Vec<(String, usize)>,
    pub models: Vec<ModelSummary>,
    pub lifecycle: String,
    pub protocol_negotiated: u32,
    pub raw: Value,
}

impl OmpReadPayload for OmpModels {
    const CODES: OutcomeCodes = OutcomeCodes::new([
        "OMP_MODELS_OK",
        "OMP_MODELS_REFUSED",
        "OMP_MODELS_NO_PAYLOAD",
        "OMP_MODELS_ABSENT",
        "OMP_MODELS_TIMEOUT_UNMEASURED",
        "OMP_MODELS_TRANSPORT_FAILED",
    ]);
    const NO_PAYLOAD_DETAIL: &'static str =
        "OMP answered successfully without a models array; an empty array is a valid answer";

    fn success_detail(&self) -> String {
        format!(
            "count={} providers={}",
            self.count,
            render_counts(&self.providers)
        )
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Summarize one raw model without dropping malformed members.
#[must_use]
pub fn summarize(value: &Value) -> ModelSummary {
    ModelSummary {
        id: string_field(value, "id"),
        name: string_field(value, "name"),
        provider: string_field(value, "provider"),
        context_window: value.get("contextWindow").and_then(Value::as_u64),
        max_tokens: value.get("maxTokens").and_then(Value::as_u64),
        reasoning: value.get("reasoning").and_then(Value::as_bool),
        supports_computer_use: value.get("supportsComputerUse").and_then(Value::as_bool),
    }
}

/// Project a successful `get_available_models` response.
///
/// Missing or non-array `models` is `None`. An empty array is `Some(count=0)`: OMP
/// answered and the configured catalog is empty.
#[must_use]
pub fn project(data: &Value, lifecycle: &str, negotiated: u32) -> Option<OmpModels> {
    let raw_models = data.get("models")?.as_array()?;
    let models: Vec<ModelSummary> = raw_models.iter().map(summarize).collect();
    let mut providers = BTreeMap::new();
    for model in &models {
        if let Some(provider) = &model.provider {
            *providers.entry(provider.clone()).or_insert(0usize) += 1;
        }
    }
    Some(OmpModels {
        count: raw_models.len(),
        providers: providers.into_iter().collect(),
        models,
        lifecycle: lifecycle.to_owned(),
        protocol_negotiated: negotiated,
        raw: data.clone(),
    })
}
/// Handshake plus one catalog request. No unrelated method can consume this verb's deadline.
#[must_use]
pub fn request_set() -> [RpcRequest; 2] {
    [
        RpcRequest::NegotiateProtocol,
        RpcRequest::GetAvailableModels,
    ]
}

/// Drive one bounded OMP RPC session and project its model catalog.
pub async fn read_models(cx: &asupersync::Cx, binary: &str) -> ModelsOutcome {
    read_projection(cx, binary, ProjectionMethod::AvailableModels, project).await
}

/// Render the successful model catalog summary for a human operator.
#[must_use]
pub fn render(models: &OmpModels) -> String {
    format!(
        "OMPO_OMP_MODELS count={} providers={} lifecycle={} protocol={} adopted_method={ADOPTED_METHOD}",
        models.count,
        render_counts(&models.providers),
        models.lifecycle,
        models.protocol_negotiated,
    )
}

#[must_use]
pub fn envelope(outcome: &ModelsOutcome) -> Value {
    let data = match outcome {
        ModelsOutcome::Answered(models) => json!({
            "adopted_method": ADOPTED_METHOD,
            "lifecycle": models.lifecycle,
            "protocol_negotiated": models.protocol_negotiated,
            "count": models.count,
            "providers": models.providers.iter().map(|(provider, count)| {
                json!({"provider": provider, "count": count})
            }).collect::<Vec<_>>(),
            "models": models.models.iter().map(|model| json!({
                "id": model.id,
                "name": model.name,
                "provider": model.provider,
                "context_window": model.context_window,
                "max_tokens": model.max_tokens,
                "reasoning": model.reasoning,
                "supports_computer_use": model.supports_computer_use,
            })).collect::<Vec<_>>(),
            "reason_code": outcome.reason_code(),
            "raw": models.raw,
            "no_claim": NO_CLAIM_BOUNDARY,
        }),
        ModelsOutcome::Refused { .. }
        | ModelsOutcome::NoPayload
        | ModelsOutcome::Absent { .. }
        | ModelsOutcome::TimedOut { .. }
        | ModelsOutcome::TransportFailed { .. } => json!({
            "adopted_method": ADOPTED_METHOD,
            "reason_code": outcome.reason_code(),
            "detail": outcome.detail(),
            "unmeasured": outcome.is_unmeasured(),
            "no_claim": NO_CLAIM_BOUNDARY,
        }),
    };
    umbrella::envelope("models", outcome.envelope_status(), data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> Value {
        json!({
            "models": [
                {
                    "id": "nomic-embed-text:latest",
                    "name": "nomic-embed-text:latest",
                    "provider": "ollama",
                    "contextWindow": 8192,
                    "maxTokens": 2048,
                    "reasoning": false,
                    "supportsComputerUse": false
                },
                {
                    "id": "gpt-5.6-sol",
                    "name": "GPT-5.6 Sol",
                    "provider": "openai-codex",
                    "contextWindow": 1_000_000,
                    "reasoning": true,
                    "supportsComputerUse": true
                }
            ]
        })
    }

    #[test]
    fn measured_catalog_projects_identity_capabilities_and_provider_order() {
        let models = project(&payload(), "stopped", 2).expect("models array is present");
        assert_eq!(models.count, 2);
        assert_eq!(models.models[0].id.as_deref(), Some("nomic-embed-text:latest"));
        assert_eq!(models.models[0].context_window, Some(8192));
        assert_eq!(models.models[1].provider.as_deref(), Some("openai-codex"));
        assert_eq!(models.models[1].supports_computer_use, Some(true));
        assert_eq!(
            models.providers,
            vec![("ollama".to_owned(), 1), ("openai-codex".to_owned(), 1)]
        );
        assert!(render(&models).contains("providers=ollama:1,openai-codex:1"));
    }

    #[test]
    fn empty_catalog_is_an_answer_but_missing_catalog_is_not() {
        let empty =
            project(&json!({"models": []}), "stopped", 2).expect("an empty array is present");
        let answered = ModelsOutcome::Answered(Box::new(empty));
        assert_eq!(answered.exit_code(), EXIT_OK);
        assert_eq!(envelope(&answered)["data"]["count"], 0);
        assert_eq!(project(&json!({}), "stopped", 2), None);
        assert_eq!(project(&json!({"models": null}), "stopped", 2), None);
        assert_eq!(ModelsOutcome::NoPayload.exit_code(), EXIT_REFUSED);
    }

    #[test]
    fn malformed_members_are_retained_in_the_denominator() {
        let models = project(&json!({"models": ["unexpected"]}), "stopped", 2)
            .expect("models array is present");
        assert_eq!(models.count, 1);
        assert_eq!(models.models.len(), 1);
        assert_eq!(models.models[0].id, None);
        assert_eq!(models.models[0].provider, None);
    }

    #[test]
    fn request_set_is_narrow_and_names_the_reported_method() {
        let set = request_set();
        assert_eq!(set.len(), 2);
        assert_eq!(set[0], RpcRequest::NegotiateProtocol);
        assert_eq!(set[1], RpcRequest::GetAvailableModels);
        assert_eq!(set[1].command(), ADOPTED_METHOD);
        for forbidden in [
            RpcRequest::GetState,
            RpcRequest::GetSessionStats,
            RpcRequest::GetMessages,
        ] {
            assert!(!set.contains(&forbidden));
        }
    }

    #[test]
    fn refusal_envelope_never_contains_success_shaped_catalog_fields() {
        let envelope = envelope(&ModelsOutcome::NoPayload);
        assert_eq!(envelope["status"], "DEGRADED");
        assert_eq!(envelope["data"]["reason_code"], "OMP_MODELS_NO_PAYLOAD");
        assert!(envelope["data"].get("models").is_none());
        assert!(envelope["data"].get("count").is_none());
    }
}
