//! Typed K0 namespace and identity primitives.
//!
//! This module owns only identity policy and request-shape validation. The
//! authenticated Agent Mail transport remains in [`crate::client::MailClient`].
//! It deliberately does not infer work from registration recency, and it delegates
//! roster cleanup to the existing `cleanup_pane_identities` kernel tool.

use crate::client::MailClient;
use crate::error::MailError;
use crate::journey::{AgentName, ProjectKey};
use asupersync::Cx;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fmt;

/// The Agent Mail tool used for identity registration.
pub const REGISTER_AGENT_TOOL: &str = "register_agent";
/// The Agent Mail tool used for reading file-backed pane identity.
pub const RESOLVE_PANE_IDENTITY_TOOL: &str = "resolve_pane_identity";
/// The existing Agent Mail tool used for stale pane identity cleanup.
pub const CLEANUP_PANE_IDENTITIES_TOOL: &str = "cleanup_pane_identities";

const REGISTER_FIELDS: &[&str] = &[
    "name",
    "project_key",
    "program",
    "model",
    "task_description",
    "pane_id",
];

/// A K0 identity or request-shape failure.
#[derive(Debug)]
pub enum IdentityError {
    /// The caller used a field that this typed boundary does not understand.
    UnknownRegisterField { field: String },
    /// The register request was not a JSON object.
    RegisterArgumentsNotObject,
    /// The register response was not a JSON object.
    ReadbackNotObject,
    /// A field sent by the caller was omitted from the daemon response.
    MissingReadbackField { field: String },
    /// A field was echoed with a different value.
    MismatchedReadbackField { field: String },
    /// The pane identity response was not an object.
    PaneIdentityNotObject,
    /// The pane identity response omitted a required field.
    MissingPaneIdentityField { field: &'static str },
    /// The daemon returned a binding state outside the known vocabulary.
    UnknownPaneBinding { binding: String },
    /// Only a live binding establishes identity for dispatch.
    UnverifiedPaneBinding { binding: String },
    /// The caller's `TMUX_PANE` environment value was not available.
    MissingTmuxPane,
    /// An Agent Mail transport or protocol failure.
    Mail(MailError),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRegisterField { field } => {
                write!(formatter, "register_agent unknown field `{field}`")
            }
            Self::RegisterArgumentsNotObject => {
                formatter.write_str("register_agent arguments must be an object")
            }
            Self::ReadbackNotObject => {
                formatter.write_str("register_agent read-back must be an object")
            }
            Self::MissingReadbackField { field } => {
                write!(formatter, "register_agent read-back omitted `{field}`")
            }
            Self::MismatchedReadbackField { field } => {
                write!(formatter, "register_agent read-back mismatched `{field}`")
            }
            Self::PaneIdentityNotObject => {
                formatter.write_str("resolve_pane_identity response must be an object")
            }
            Self::MissingPaneIdentityField { field } => {
                write!(
                    formatter,
                    "resolve_pane_identity response omitted `{field}`"
                )
            }
            Self::UnknownPaneBinding { binding } => {
                write!(
                    formatter,
                    "resolve_pane_identity unknown binding `{binding}`"
                )
            }
            Self::UnverifiedPaneBinding { binding } => {
                write!(
                    formatter,
                    "resolve_pane_identity binding `{binding}` is not verified-live"
                )
            }
            Self::MissingTmuxPane => formatter.write_str("TMUX_PANE is required for self-identity"),
            Self::Mail(error) => write!(formatter, "Agent Mail: {error:?}"),
        }
    }
}

impl std::error::Error for IdentityError {}

impl From<MailError> for IdentityError {
    fn from(error: MailError) -> Self {
        Self::Mail(error)
    }
}

fn checkpoint(cx: &Cx, operation: &str) -> Result<(), IdentityError> {
    cx.checkpoint()
        .map_err(|_| IdentityError::from(MailError::from_cancelled(cx, operation)))
}

/// Validate the exact fields accepted by the typed registration boundary.
///
/// `agent_name` is intentionally not accepted. Without this guard, the daemon
/// can treat it as an unknown input, mint a different random identity, and
/// return a success-shaped response that leaves the caller unreachable.
pub fn validate_register_fields(arguments: &Map<String, Value>) -> Result<(), IdentityError> {
    for field in arguments.keys() {
        if !REGISTER_FIELDS.contains(&field.as_str()) {
            return Err(IdentityError::UnknownRegisterField {
                field: field.clone(),
            });
        }
    }
    Ok(())
}

/// Assert that every field sent by a write operation was echoed unchanged.
///
/// Presence and equality are checked separately from transport success. This
/// catches accepted-but-discarded fields such as `pane_id` and prevents a
/// response envelope from laundering silent state loss into success.
pub fn assert_readback_fields(
    request: &Map<String, Value>,
    response: &Value,
) -> Result<(), IdentityError> {
    let Some(response) = response.as_object() else {
        return Err(IdentityError::ReadbackNotObject);
    };
    for (field, expected) in request {
        let Some(actual) = response.get(field) else {
            return Err(IdentityError::MissingReadbackField {
                field: field.clone(),
            });
        };
        if actual != expected {
            return Err(IdentityError::MismatchedReadbackField {
                field: field.clone(),
            });
        }
    }
    Ok(())
}

/// Convert the existing typed journey request into the wire fields K0 checks.
#[must_use]
pub fn register_arguments(request: &crate::journey::RegisterRequest) -> Map<String, Value> {
    let mut arguments = Map::new();
    arguments.insert("project_key".to_owned(), json!(request.project.as_str()));
    arguments.insert("program".to_owned(), json!(request.program));
    arguments.insert("model".to_owned(), json!(request.model));
    if let Some(description) = &request.task_description {
        arguments.insert("task_description".to_owned(), json!(description));
    }
    arguments
}

/// The three binding states returned by the file-backed pane resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingStatus {
    /// The file binding and the current live pane agree.
    VerifiedLive,
    /// The file names a pane that no longer exists.
    AdoptedDead,
    /// The file exists but has not been verified against the live pane.
    LegacyUnverified,
}

/// A pane identity that passed the K0 verified-live gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneIdentity {
    /// The tmux pane ID, e.g. `%1413`.
    pub pane_id: String,
    /// The binding state read from the resolver.
    pub binding: BindingStatus,
    /// The Agent Mail identity, when the resolver returned one.
    pub agent_name: Option<AgentName>,
    /// The session name, when the resolver returned one.
    pub session: Option<String>,
    /// The pane index, when the resolver returned one.
    pub pane_index: Option<u32>,
}

/// Parse a pane resolver response and refuse every non-live binding.
pub fn parse_pane_identity(response: &Value) -> Result<PaneIdentity, IdentityError> {
    let Some(object) = response.as_object() else {
        return Err(IdentityError::PaneIdentityNotObject);
    };
    let pane_id = object
        .get("pane_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(IdentityError::MissingPaneIdentityField { field: "pane_id" })?;
    let binding = object
        .get("binding")
        .and_then(Value::as_str)
        .ok_or(IdentityError::MissingPaneIdentityField { field: "binding" })?;
    let binding = match binding {
        "verified-live" => BindingStatus::VerifiedLive,
        "adopted-dead" => BindingStatus::AdoptedDead,
        "legacy-unverified" => BindingStatus::LegacyUnverified,
        other => {
            return Err(IdentityError::UnknownPaneBinding {
                binding: other.to_owned(),
            });
        }
    };
    if binding != BindingStatus::VerifiedLive {
        return Err(IdentityError::UnverifiedPaneBinding {
            binding: binding_string(binding),
        });
    }

    Ok(PaneIdentity {
        pane_id: pane_id.to_owned(),
        binding,
        agent_name: object
            .get("agent_name")
            .and_then(Value::as_str)
            .map(AgentName::new),
        session: object
            .get("session")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        pane_index: object
            .get("pane_index")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
    })
}

fn binding_string(binding: BindingStatus) -> String {
    match binding {
        BindingStatus::VerifiedLive => "verified-live".to_owned(),
        BindingStatus::AdoptedDead => "adopted-dead".to_owned(),
        BindingStatus::LegacyUnverified => "legacy-unverified".to_owned(),
    }
}

/// Resolve one file-backed pane identity through the existing kernel tool.
pub async fn resolve_pane_identity(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    pane_id: &str,
) -> Result<PaneIdentity, IdentityError> {
    checkpoint(cx, RESOLVE_PANE_IDENTITY_TOOL)?;
    let payload = client
        .call_tool(
            cx,
            RESOLVE_PANE_IDENTITY_TOOL,
            json!({
                "project_key": project.as_str(),
                "pane_id": pane_id,
            }),
        )
        .await?;
    checkpoint(cx, RESOLVE_PANE_IDENTITY_TOOL)?;
    parse_pane_identity(&payload)
}

/// Build the existing roster-cleanup tool request; no local reaper is created.
#[must_use]
pub fn cleanup_pane_identities_arguments(project: &str) -> Value {
    json!({"project_key": project})
}

/// Delegate stale pane identity cleanup to Agent Mail's existing tool.
pub async fn cleanup_pane_identities(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
) -> Result<Value, IdentityError> {
    checkpoint(cx, CLEANUP_PANE_IDENTITIES_TOOL)?;
    let payload = client
        .call_tool(
            cx,
            CLEANUP_PANE_IDENTITIES_TOOL,
            cleanup_pane_identities_arguments(project.as_str()),
        )
        .await?;
    checkpoint(cx, CLEANUP_PANE_IDENTITIES_TOOL)?;
    Ok(payload)
}

/// Construct a tmux identity query targeted at the caller's pane.
///
/// The caller must obtain `pane_id` from its own `TMUX_PANE` environment value
/// and pass it here. The `-t` argument is mandatory; an unqualified
/// `display-message` answers for whichever pane is focused on the host.
#[must_use]
pub fn tmux_identity_argv(pane_id: &str) -> [&str; 5] {
    [
        "display-message",
        "-t",
        pane_id,
        "-p",
        "#{pane_id} #{session_name}:#{window_index}.#{pane_index}",
    ]
}

/// Read the caller's TMUX_PANE and build a target-qualified identity query.
pub fn tmux_identity_argv_from_env() -> Result<[String; 5], IdentityError> {
    let pane_id = std::env::var("TMUX_PANE").map_err(|_| IdentityError::MissingTmuxPane)?;
    Ok([
        "display-message".to_owned(),
        "-t".to_owned(),
        pane_id,
        "-p".to_owned(),
        "#{pane_id} #{session_name}:#{window_index}.#{pane_index}".to_owned(),
    ])
}
