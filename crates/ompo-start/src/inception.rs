#![forbid(unsafe_code)]

//! S1 inception manifest writer.
//!
//! The writer establishes the durable repository boundary consumed by later
//! stages. It records identity, control-file presence, host facts, required
//! tool names, and an explicit trust status, then reads the JSON back before
//! returning success.

use input_manifest::InputManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use lifecycle_event::{
    default_repo_journal, DurableJournal, EmitOutcome, Layer, LifecycleEvent, ReasonCode,
};
use serde_json::{Map, Value};
use lifecycle_monitor::verify_artifact;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use agent_mail_native::{
    resolve_pane_identity, BindingStatus, IdentityError, MailClient, MailError, PaneIdentity,
    ProjectKey,
};
use asupersync::{runtime::RuntimeBuilder, Cx};
use sender_identity::first_candidate;
use std::env;

// Reuse the hook's existing source-set, digest, manifest, and diff authority
// without adding a cycle from ompo-start -> no-shell-gate -> ompo-start.
#[path = "../../no-shell-gate/src/hook_digest.rs"]
mod hook_digest_authority;

pub const SCHEMA_VERSION: &str = "inception.v1";

const REQUIRED_CONTROL_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "Cargo.toml",
    "SCHEMAS.toml",
    "docs/decisions.jsonl",
];

/// The control files an initialised repository must carry, exposed read-only.
///
/// `health` needs the same list `initialize` refuses on, and a second copy would drift the
/// moment either side changed. Reused rather than re-derived.
#[must_use]
pub fn required_control_files() -> &'static [&'static str] {
    REQUIRED_CONTROL_FILES
}

/// Presence of each required control file under `repo`, WITHOUT writing anything.
///
/// This is the single computation behind both `initialize`'s
/// `INCEPTION_CONTROL_FILES_MISSING` refusal and `ompo health`'s read-only signal, so the
/// two can never disagree about what "complete" means.
#[must_use]
pub fn control_file_presence(repo: &Path) -> BTreeMap<String, bool> {
    REQUIRED_CONTROL_FILES
        .iter()
        .map(|relative| ((*relative).to_owned(), repo.join(relative).is_file()))
        .collect()
}

const REQUIRED_TOOLS: &[&str] = &["git", "cargo", "br", "bv", "ntm", "am", "jq"];
const IDENTITY_COMMAND_DEADLINE: Duration = Duration::from_secs(10);
const AGENT_MAIL_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const REQUIRED_KEYS: &[&str] = &[
    "schema_version",
    "project_id",
    "repo_identity",
    "control_files",
    "host_capabilities",
    "required_tools",
    "epistemic",
    "trust_status",
];
const OPTIONAL_KEYS: &[&str] = &["evidence", "status", "degradations", "template_identity"];
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoWorkspaceError {
    CargoMissing { program: String, detail: String },
    MetadataFailed { detail: String },
    MetadataMalformed { detail: String },
    Empty { field: &'static str },
    CurrentCrateAbsent { package: String, detail: String },
    PartialInput {
        bound_kind: String,
        bound_value: u64,
        source: String,
    },
    RefusedInput { reason: String },
}

impl CargoWorkspaceError {
    const fn reason_code(&self) -> &'static str {
        match self {
            Self::CargoMissing { .. } => "CARGO_WORKSPACE_CARGO_MISSING",
            Self::MetadataFailed { .. } => "CARGO_WORKSPACE_METADATA_FAILED",
            Self::MetadataMalformed { .. } => "CARGO_WORKSPACE_METADATA_MALFORMED",
            Self::Empty { .. } => "CARGO_WORKSPACE_EMPTY",
            Self::CurrentCrateAbsent { .. } => "CARGO_WORKSPACE_CURRENT_CRATE_ABSENT",
            Self::PartialInput { .. } => "CARGO_WORKSPACE_INPUT_PARTIAL",
            Self::RefusedInput { .. } => "CARGO_WORKSPACE_INPUT_REFUSED",
        }
    }

    const fn remediation(&self) -> &'static str {
        match self {
            Self::CargoMissing { .. } => "install Cargo or put the intended cargo executable on PATH",
            Self::MetadataFailed { .. } => "run cargo metadata --no-deps --format-version 1 --offline and repair the reported manifest or workspace error",
            Self::MetadataMalformed { .. } => "use Cargo JSON from cargo metadata --format-version 1; do not parse logs or prose",
            Self::Empty { .. } => "declare a nonempty package and workspace-member set",
            Self::CurrentCrateAbsent { .. } => "add the current crate to the workspace members or run from its owning workspace",
            Self::PartialInput { .. } => "rerun over the full metadata input",
            Self::RefusedInput { .. } => "resolve the input refusal before deriving workspace trust",
        }
    }
}

impl fmt::Display for CargoWorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "HUMAN_HALT {} ", self.reason_code())?;
        match self {
            Self::CargoMissing { program, detail } => {
                write!(formatter, "program={program} detail={detail}")?
            }
            Self::MetadataFailed { detail } | Self::MetadataMalformed { detail } => {
                write!(formatter, "detail={detail}")?
            }
            Self::Empty { field } => write!(formatter, "field={field}")?,
            Self::CurrentCrateAbsent { package, detail } => {
                write!(formatter, "package={package} detail={detail}")?
            }
            Self::PartialInput {
                bound_kind,
                bound_value,
                source,
            } => write!(
                formatter,
                "bound_kind={bound_kind} bound_value={bound_value} source={source}"
            )?,
            Self::RefusedInput { reason } => write!(formatter, "reason={reason}")?,
        }
        write!(formatter, " remedy={}", self.remediation())
    }
}

impl std::error::Error for CargoWorkspaceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentMailRegistrationError {
    ProjectMissing {
        project: String,
        detail: String,
    },
    AgentMissing {
        project: String,
        agent: Option<String>,
        detail: String,
    },
    ProjectMismatch {
        expected: String,
        actual: String,
    },
    PaneAgentMismatch {
        expected_pane: String,
        actual_pane: String,
        intended_agent: String,
        resolved_agent: Option<String>,
    },
    ServiceUnavailable {
        detail: String,
    },
    MalformedResponse {
        detail: String,
    },
    Unknown {
        detail: String,
    },
    PartialInput {
        bound_kind: String,
        bound_value: u64,
        source: String,
    },
    RefusedInput {
        reason: String,
    },
}

fn write_human_halt(
    formatter: &mut fmt::Formatter<'_>,
    code: &str,
    detail: fmt::Arguments<'_>,
    remedy: &str,
) -> fmt::Result {
    write!(formatter, "HUMAN_HALT {code} {detail} remedy={remedy}")
}

impl fmt::Display for AgentMailRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectMissing { project, detail } => write_human_halt(
                formatter,
                "AGENT_MAIL_PROJECT_MISSING",
                format_args!("project={project} detail={detail}"),
                "register the canonical repository project, then retry the read-only resource",
            ),
            Self::AgentMissing { project, agent, detail } => write_human_halt(
                formatter,
                "AGENT_MAIL_AGENT_MISSING",
                format_args!(
                    "project={project} agent={} detail={detail}",
                    agent.as_deref().unwrap_or("<unset>")
                ),
                "register the intended agent in the canonical project and bind it to this pane",
            ),
            Self::ProjectMismatch { expected, actual } => write_human_halt(
                formatter,
                "AGENT_MAIL_PROJECT_MISMATCH",
                format_args!("expected_project={expected} actual_project={actual}"),
                "use the exact canonical git root as the Agent Mail project key",
            ),
            Self::PaneAgentMismatch {
                expected_pane,
                actual_pane,
                intended_agent,
                resolved_agent,
            } => write_human_halt(
                formatter,
                "AGENT_MAIL_PANE_AGENT_MISMATCH",
                format_args!(
                    "expected_pane={expected_pane} actual_pane={actual_pane} intended_agent={intended_agent} resolved_agent={}",
                    resolved_agent.as_deref().unwrap_or("<missing>")
                ),
                "repair the canonical pane identity so it names this pane and intended agent",
            ),
            Self::ServiceUnavailable { detail } => write_human_halt(
                formatter,
                "AGENT_MAIL_SERVICE_UNAVAILABLE",
                format_args!("detail={detail}"),
                "restore the Agent Mail daemon and credential, then rerun registration readback",
            ),
            Self::MalformedResponse { detail } => write_human_halt(
                formatter,
                "AGENT_MAIL_RESPONSE_MALFORMED",
                format_args!("detail={detail}"),
                "repair or upgrade the Agent Mail service; do not infer registration from malformed data",
            ),
            Self::Unknown { detail } => write_human_halt(
                formatter,
                "AGENT_MAIL_REGISTRATION_UNKNOWN",
                format_args!("detail={detail}"),
                "resolve the unknown Agent Mail state before continuing",
            ),
            Self::PartialInput {
                bound_kind,
                bound_value,
                source,
            } => write_human_halt(
                formatter,
                "AGENT_MAIL_INPUT_PARTIAL",
                format_args!(
                    "bound_kind={bound_kind} bound_value={bound_value} source={source}"
                ),
                "rerun registration over the full project, roster, and pane identity inputs",
            ),
            Self::RefusedInput { reason } => write_human_halt(
                formatter,
                "AGENT_MAIL_INPUT_REFUSED",
                format_args!("reason={reason}"),
                "resolve the input refusal before deriving registration trust",
            ),
        }
    }
}
impl std::error::Error for AgentMailRegistrationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMailRegistrationReport {
    pub input: InputManifest,
    pub project: String,
    pub pane_id: String,
    pub agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentMailProjectRegistry {
    project: String,
    agents: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RchLaneState {
    Mapped { workers: BTreeSet<String> },
    Unknown { cause: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RchLaneReport {
    pub input: InputManifest,
    pub project_id: String,
    pub state: RchLaneState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RchLaneError {
    MissingRch { program: String, detail: String },
    Timeout { surface: &'static str },
    CommandFailed { surface: &'static str, detail: String },
    MalformedOutput { surface: &'static str, detail: String },
    ProjectExcluded { project_id: String, workers: BTreeSet<String> },
    ProjectRowAbsent { project_id: String },
    ContradictoryTopology { detail: String },
    PartialInput {
        bound_kind: String,
        bound_value: u64,
        source: String,
    },
    RefusedInput { reason: String },
}

struct RchLaneErrorSpec {
    code: &'static str,
    remediation: &'static str,
}

impl RchLaneError {
    const fn spec(&self) -> RchLaneErrorSpec {
        let (code, remediation) = match self {
            Self::MissingRch { .. } => (
                "RCH_LANE_RCH_MISSING",
                "install the approved rch client on PATH; do not build locally",
            ),
            Self::Timeout { .. } => (
                "RCH_LANE_TIMEOUT",
                "restore the local RCH daemon or retry the bounded read-only surface",
            ),
            Self::CommandFailed { .. } => (
                "RCH_LANE_COMMAND_FAILED",
                "repair the named RCH topology surface before continuing",
            ),
            Self::MalformedOutput { .. } => (
                "RCH_LANE_OUTPUT_MALFORMED",
                "upgrade or repair RCH; do not infer lane state from malformed output",
            ),
            Self::ProjectExcluded { .. } => (
                "RCH_LANE_PROJECT_EXCLUDED",
                "wait for project exclusion to clear or use the scheduler's unpinned retry path",
            ),
            Self::ProjectRowAbsent { .. } => (
                "RCH_LANE_PROJECT_ROW_ABSENT",
                "rerun convergence after RCH records this repository",
            ),
            Self::ContradictoryTopology { .. } => (
                "RCH_LANE_TOPOLOGY_CONTRADICTORY",
                "repair the topology source; conflicting rows cannot authorize dispatch",
            ),
            Self::PartialInput { .. } => (
                "RCH_LANE_INPUT_PARTIAL",
                "rerun the RCH lane report over all three read-only surfaces",
            ),
            Self::RefusedInput { .. } => (
                "RCH_LANE_INPUT_REFUSED",
                "resolve the input refusal before deriving RCH lane state",
            ),
        };
        RchLaneErrorSpec { code, remediation }
    }
}

impl fmt::Display for RchLaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spec = self.spec();
        write_human_halt(
            formatter,
            spec.code,
            format_args!("{self:?}"),
            spec.remediation,
        )
    }
}
impl std::error::Error for RchLaneError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedInitConsent {
    Absent,
    Explicit {
        decision_id: String,
        repository_scope: PathBuf,
        source_revision: String,
        policy_sha256: String,
        template_path: PathBuf,
        template_input: InputManifest,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedInitDecision {
    OwnedPolicy,
    ExplicitConsent {
        decision_id: String,
        repository_scope: PathBuf,
        source_revision: String,
        policy_sha256: String,
        template_identity: TemplateIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateIdentity {
    pub canonical_path: String,
    pub source_sha256: String,
    pub source_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateIdentityError {
    Missing { path: PathBuf },
    Unreadable { path: PathBuf, detail: String },
    AuthorityRootEscape { authority_root: PathBuf, template_path: PathBuf },
    Empty { path: PathBuf },
    Malformed { field: &'static str, detail: String },
    RevisionUnresolvable { path: PathBuf, detail: String },
    SourceChanged {
        path: PathBuf,
        expected_sha256: String,
        actual_sha256: String,
        expected_revision: String,
        actual_revision: String,
    },
    PartialInput { bound_kind: String, bound_value: u64, source: String },
    RefusedInput { reason: String },
}

impl fmt::Display for TemplateIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_MISSING",
                format_args!("path={}", path.display()),
                "restore the exact template source before trusted initialization",
            ),
            Self::Unreadable { path, detail } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_UNREADABLE",
                format_args!("path={} detail={detail}", path.display()),
                "restore read access to the template source",
            ),
            Self::AuthorityRootEscape { authority_root, template_path } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_AUTHORITY_ESCAPE",
                format_args!(
                    "authority_root={} template_path={}",
                    authority_root.display(),
                    template_path.display()
                ),
                "choose a template whose canonical path is inside the repository authority root",
            ),
            Self::Empty { path } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_EMPTY",
                format_args!("path={}", path.display()),
                "restore nonempty template source bytes",
            ),
            Self::Malformed { field, detail } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_MALFORMED",
                format_args!("field={field} detail={detail}"),
                "re-capture template identity from canonical source bytes and a live revision",
            ),
            Self::RevisionUnresolvable { path, detail } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_REVISION_UNRESOLVABLE",
                format_args!("path={} detail={detail}", path.display()),
                "commit the template source and restore a resolvable repository HEAD",
            ),
            Self::SourceChanged {
                path,
                expected_sha256,
                actual_sha256,
                expected_revision,
                actual_revision,
            } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_SOURCE_CHANGED",
                format_args!(
                    "path={} expected_sha256={expected_sha256} actual_sha256={actual_sha256} expected_revision={expected_revision} actual_revision={actual_revision}",
                    path.display()
                ),
                "re-capture consent after the template source and repository revision stop changing",
            ),
            Self::PartialInput { bound_kind, bound_value, source } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_INPUT_PARTIAL",
                format_args!("bound_kind={bound_kind} bound_value={bound_value} source={source}"),
                "re-run identity capture over the full template source",
            ),
            Self::RefusedInput { reason } => write_human_halt(
                formatter,
                "TEMPLATE_IDENTITY_INPUT_REFUSED",
                format_args!("reason={reason}"),
                "resolve the input refusal before trusted initialization",
            ),
        }
    }
}

impl std::error::Error for TemplateIdentityError {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpistemicValidationError {
    EmptyLedger,
    BlankField {
        category: &'static str,
        index: usize,
        field: &'static str,
    },
}

impl fmt::Display for EpistemicValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLedger => write!(formatter, "INCEPTION_EPISTEMIC_EMPTY"),
            Self::BlankField { category, index, field } => write!(
                formatter,
                "INCEPTION_EPISTEMIC_BLANK category={category} index={index} field={field}"
            ),
        }
    }
}

impl std::error::Error for EpistemicValidationError {}

#[derive(Debug)]
pub enum InceptionError {
    RepositoryUnreadable { path: PathBuf, detail: String },
    MissingControlFiles(Vec<String>),
    Write { path: PathBuf, detail: String },
    Readback { path: PathBuf, detail: String },
    IdentityUnavailable { field: &'static str, detail: String },
    /// Cargo workspace membership refused before trust initialization.
    CargoWorkspace(CargoWorkspaceError),
    /// Agent Mail project and pane-bound agent registration could not be proven.
    AgentMailRegistration(AgentMailRegistrationError),
    /// Repository-to-RCH lane mapping could not be read consistently.
    RchLane(RchLaneError),
    /// Template identity could not be established before trusted initialization.
    TemplateIdentity(TemplateIdentityError),
    /// A foreign AGENTS.md reached an explicit consent-taking entry with no consent.
    TrustedInitConsentMissing {
        path: PathBuf,
        policy_sha256: String,
    },
    /// A supplied consent record cannot represent a valid decision.
    TrustedInitConsentMalformed {
        field: &'static str,
        detail: String,
    },
    /// Consent was issued for a different canonical repository.
    TrustedInitConsentScopeMismatch {
        expected: PathBuf,
        provided: PathBuf,
    },
    /// Consent names an older repository revision or policy digest.
    TrustedInitConsentStaleOrReplayed {
        expected_source_revision: String,
        provided_source_revision: String,
        expected_policy_sha256: String,
        provided_policy_sha256: String,
    },
    /// The installed pre-commit hook cannot prove it was built from the
    /// repository's current source authority.
    HookIdentityRefused {
        path: PathBuf,
        status: HookIdentityStatus,
        detail: String,
    },
    /// Init refused over a rust-toolchain.toml whose pin report is not
    /// Ready at the gated entry: the L2 trust flow requires the declared
    /// toolchain identity to match the active toolchain before
    /// initialization can continue. No opt-in override exists on the
    /// gated entry by design -- declare the pin and install it.
    ToolchainPinRefused {
        path: PathBuf,
        status: ToolchainPinStatus,
        declared: Option<String>,
    },
    /// Init refused over a `.beads` tracker whose init report is not
    /// Ready at the gated entry: the L2 trust flow requires an
    /// initialized, readable, writable tracker before initialization or
    /// dispatch can continue. No opt-in override exists on the gated
    /// entry by design -- initialize the tracker (`br init`), restore
    /// its state, and ensure it is readable and writable.
    BeadsInitRefused {
        path: PathBuf,
        status: BeadsInitStatus,
    },
    /// Init refused over an AGENTS.md state that cannot be consented to.
    /// Nonempty foreign policy reaches the typed consent decision instead;
    /// missing, empty, or unreadable policy remains restrictive here.
    AgentsStampRefused {
        path: PathBuf,
        status: AgentsStampStatus,
    },
    /// Init refused over a CLAUDE.md whose stamp report is not Stamped:
    /// the L2 trust flow requires the stamped control file before any
    /// trust-dependent continuation. No opt-in override exists on the
    /// gated entry by design -- stamp the file.
    UntrustedClaudeMd {
        path: PathBuf,
        status: AgentsStampStatus,
    },
    /// A legacy no-consent entry observed foreign AGENTS.md policy.
    /// Explicit consent-taking entries use the distinct consent refusals above.
    UntrustedAgentsMd { path: PathBuf },
    /// AGENTS.md exists but is empty: a broken fixture, not a foreign repo.
    EmptyAgentsMd { path: PathBuf },
    /// The artifact is absent: anti-vacuity is a typed refusal, never a pass.
    ReadbackMissing { path: PathBuf },
    /// The artifact exists but is not parseable JSON.
    ReadbackMalformed { path: PathBuf, detail: String },
    /// A required or nested required field is absent.
    ReadbackMissingKey { path: PathBuf, key: String },
    /// The artifact contains evidence outside the current schema contract.
    ReadbackExtraKey { path: PathBuf, key: String },
    /// A field has a JSON type other than the contract declares.
    ReadbackWrongType {
        path: PathBuf,
        key: String,
        expected: &'static str,
        found: &'static str,
    },
    /// A required string or array item is present but empty.
    ReadbackEmpty { path: PathBuf, key: String },
    /// A typed field is present but violates its value contract.
    ReadbackInvalid {
        path: PathBuf,
        key: String,
        detail: String,
    },
    /// The persisted epistemic ledger is empty or contains a blank cell.
    Epistemic(EpistemicValidationError),
}

impl fmt::Display for InceptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepositoryUnreadable { path, detail } => write!(
                formatter,
                "INCEPTION_REPOSITORY_UNREADABLE path={} detail={detail}",
                path.display()
            ),
            Self::MissingControlFiles(paths) => write!(
                formatter,
                "INCEPTION_CONTROL_FILES_MISSING paths={}",
                paths.join(",")
            ),
            Self::Write { path, detail } => write!(
                formatter,
                "INCEPTION_WRITE_FAILED path={} detail={detail}",
                path.display()
            ),
            Self::ToolchainPinRefused {
                path,
                status,
                declared,
            } => write!(
                formatter,
                "HUMAN_HALT refusing init over {status:?} toolchain pin path={} declared={} (declare the channel in rust-toolchain.toml and install it with rustup)",
                path.display(),
                declared.as_deref().unwrap_or("none"),
            ),
            Self::BeadsInitRefused { path, status } => write!(
                formatter,
                "HUMAN_HALT refusing init over {status:?} tracker state path={} (initialize the tracker with `br init`, restore its state, and ensure it is readable and writable)",
                path.display()
            ),
            Self::Readback { path, detail } => write!(
                formatter,
                "INCEPTION_READBACK_FAILED path={} detail={detail}",
                path.display()
            ),
            Self::IdentityUnavailable { field, detail } => write!(
                formatter,
                "INCEPTION_IDENTITY_UNAVAILABLE field={field} detail={detail}"
            ),
            Self::CargoWorkspace(error) => write!(formatter, "{error}"),
            Self::AgentMailRegistration(error) => write!(formatter, "{error}"),
            Self::RchLane(error) => write!(formatter, "{error}"),
            Self::TemplateIdentity(error) => write!(formatter, "{error}"),
            Self::TrustedInitConsentMissing { path, policy_sha256 } => write_human_halt(
                formatter,
                "TRUSTED_INIT_CONSENT_MISSING",
                format_args!("foreign_policy={} policy_sha256={policy_sha256}", path.display()),
                "pass TrustedInitConsent::Explicit scoped to this repository, revision, and policy digest",
            ),
            Self::TrustedInitConsentMalformed { field, detail } => write_human_halt(
                formatter,
                "TRUSTED_INIT_CONSENT_MALFORMED",
                format_args!("field={field} detail={detail}"),
                "construct a nonempty typed consent record with canonical scope and lowercase digests",
            ),
            Self::TrustedInitConsentScopeMismatch { expected, provided } => write_human_halt(
                formatter,
                "TRUSTED_INIT_CONSENT_SCOPE_MISMATCH",
                format_args!("expected={} provided={}", expected.display(), provided.display()),
                "issue new consent for the exact canonical repository root",
            ),
            Self::TrustedInitConsentStaleOrReplayed {
                expected_source_revision,
                provided_source_revision,
                expected_policy_sha256,
                provided_policy_sha256,
            } => write_human_halt(
                formatter,
                "TRUSTED_INIT_CONSENT_STALE_OR_REPLAYED",
                format_args!(
                    "expected_revision={expected_source_revision} provided_revision={provided_source_revision} expected_policy_sha256={expected_policy_sha256} provided_policy_sha256={provided_policy_sha256}"
                ),
                "reconfirm the current repository revision and foreign-policy digest, then issue a new decision id",
            ),
            Self::HookIdentityRefused { path, status, detail } => write!(
                formatter,
                "HUMAN_HALT {} hook={} detail={} remedy={}",
                status.reason_code(),
                path.display(),
                detail,
                status.remediation(),
            ),
            Self::AgentsStampRefused { path, status } => write!(
                formatter,
                "HUMAN_HALT refusing init over {status:?} AGENTS.md path={} (stamp it with the project token to opt in; no flag bypasses this)",
                path.display()
            ),
            Self::UntrustedClaudeMd { path, status } => write!(
                formatter,
                "HUMAN_HALT refusing init over {status:?} CLAUDE.md path={} (stamp it with the project token to opt in; no flag bypasses this)",
                path.display()
            ),
            Self::UntrustedAgentsMd { path } => write_human_halt(
                formatter,
                "TRUSTED_INIT_FOREIGN_POLICY",
                format_args!("path={}", path.display()),
                "use an explicit consent-taking entry and pass TrustedInitConsent::Explicit",
            ),
            Self::EmptyAgentsMd { path } => write!(
                formatter,
                "INCEPTION_EMPTY_AGENTS_MD path={} — an empty AGENTS.md is a broken fixture, never a foreign repo",
                path.display()
            ),
            Self::ReadbackMissing { path } => {
                write!(formatter, "INCEPTION_READBACK_MISSING path={}", path.display())
            }
            Self::ReadbackMalformed { path, detail } => write!(
                formatter,
                "INCEPTION_READBACK_MALFORMED path={} detail={detail}",
                path.display()
            ),
            Self::ReadbackMissingKey { path, key } => write!(
                formatter,
                "INCEPTION_READBACK_MISSING_KEY path={} key={key}",
                path.display()
            ),
            Self::ReadbackExtraKey { path, key } => write!(
                formatter,
                "INCEPTION_READBACK_EXTRA_KEY path={} key={key}",
                path.display()
            ),
            Self::ReadbackWrongType { path, key, expected, found } => write!(
                formatter,
                "INCEPTION_READBACK_WRONG_TYPE path={} key={key} expected={expected} found={found}",
                path.display()
            ),
            Self::ReadbackEmpty { path, key } => write!(
                formatter,
                "INCEPTION_READBACK_EMPTY path={} key={key}",
                path.display()
            ),
            Self::ReadbackInvalid { path, key, detail } => write!(
                formatter,
                "INCEPTION_READBACK_INVALID path={} key={key} detail={detail}",
                path.display()
            ),
            Self::Epistemic(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for InceptionError {}
impl From<EpistemicValidationError> for InceptionError {
    fn from(error: EpistemicValidationError) -> Self {
        Self::Epistemic(error)
    }
}

impl From<TemplateIdentityError> for InceptionError {
    fn from(error: TemplateIdentityError) -> Self {
        Self::TemplateIdentity(error)
    }
}

impl InceptionError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::ReadbackMissing { .. } | Self::ReadbackMalformed { .. } => 2,
            Self::ReadbackMissingKey { .. }
            | Self::ReadbackExtraKey { .. }
            | Self::ReadbackWrongType { .. }
            | Self::ReadbackEmpty { .. }
            | Self::ReadbackInvalid { .. }
            | Self::Epistemic(_) => 3,
            _ => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InceptionManifest {
    pub schema_version: String,
    pub project_id: String,
    pub repo_identity: RepoIdentity,
    pub control_files: BTreeMap<String, bool>,
    pub host_capabilities: HostCapabilities,
    pub required_tools: Vec<String>,
    pub epistemic: EpistemicLedger,
    pub trust_status: TrustStatus,
    pub template_identity: Option<TemplateIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InceptionReadback {
    pub project_id: String,
    pub repo_identity: RepoIdentity,
    pub control_files_complete: bool,
    pub epistemic: EpistemicLedger,
    pub template_identity: Option<TemplateIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub manifest: InceptionManifest,
    /// Remote policy observed by the reachable L2 gate. Shared repair
    /// initialization does not evaluate persona policy and leaves this absent;
    /// [`initialize_gated`] always carries the explicit Persona A verdict.
    pub persona_remote: Option<PersonaRemote>,
    /// Exact read-only Agent Mail registration carried by the gated entry.
    /// Shared repair initialization does not query Agent Mail.
    pub agent_mail_registration: Option<AgentMailRegistrationReport>,
    /// Repo-scoped RCH topology report carried by the gated entry.
    /// Shared repair initialization does not query RCH.
    pub rch_lane: Option<RchLaneReport>,
    /// The policy decision consumed before this run's first mutation.
    pub trusted_init: TrustedInitDecision,
    pub actions: usize,
    pub backup: Option<PathBuf>,
    pub journal_rows: usize,
    pub monitor_rows: usize,
    /// Files whose CONTENT this run actually replaced or created. Equal to
    /// `actions` today, published separately because the 1:1 backup law is
    /// stated over mutations, not over "something happened".
    pub files_mutated: usize,
    /// Snapshots this run wrote. The law is NOT a flat 1:1 against
    /// `files_mutated`: a virgin repo mutates one file and has nothing to
    /// snapshot, so it is legitimately 0:1. The 1:1 obligation binds only when
    /// PRE-EXISTING CONTENT was superseded.
    pub backups_written: usize,
    /// Whether the artifact already existed before this run. This is what makes
    /// 0:1 and 1:1 distinguishable instead of one ratio with two meanings.
    pub preexisting: bool,
    /// The 1:1 verdict, always emitted: never inferred from the ratio by a
    /// reader who cannot see `preexisting`.
    pub backup_ratio_verdict: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoIdentity {
    pub canonical_path: String,
    pub git_marker: String,
    pub source_revision: String,
    pub host_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapabilities {
    pub os: String,
    pub arch: String,
    pub filesystem: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustStatus {
    pub status: String,
    pub reason_code: String,
    pub policy_sha256: String,
    pub control_files_complete: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicKnown {
    pub claim: String,
    pub evidence_source: String,
    pub evidence_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicUnknown {
    pub question: String,
    pub owner: String,
    pub resolving_experiment: String,
    pub cost: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicGap {
    pub missing_capability: String,
    pub owner: String,
    pub cost_if_left_open: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicLedger {
    pub known: Vec<EpistemicKnown>,
    pub unknown: Vec<EpistemicUnknown>,
    pub gaps: Vec<EpistemicGap>,
}

impl EpistemicLedger {
    #[must_use]
    pub fn inception_default() -> Self {
        Self {
            known: vec![EpistemicKnown {
                claim: "ompo init writes inception.json and reads it back".to_owned(),
                evidence_source: "ompo_start::inception::initialize".to_owned(),
                evidence_command: "ompo init".to_owned(),
            }],
            unknown: vec![EpistemicUnknown {
                question: "whether the host filesystem survives a crash after the inception write".to_owned(),
                owner: "S1-L2".to_owned(),
                resolving_experiment: "run a bounded crash-recovery write/readback fixture".to_owned(),
                cost: "one remote fixture run plus operator review".to_owned(),
            }],
            gaps: vec![EpistemicGap {
                missing_capability: "an S2 consumer enforcing epistemic completeness".to_owned(),
                owner: "S1-L2".to_owned(),
                cost_if_left_open: "S2 may start without a machine-readable epistemic ledger consumer".to_owned(),
            }],
        }
    }

    pub fn validate(&self) -> Result<(), EpistemicValidationError> {
        if self.known.is_empty() && self.unknown.is_empty() && self.gaps.is_empty() {
            return Err(EpistemicValidationError::EmptyLedger);
        }
        for (index, entry) in self.known.iter().enumerate() {
            for (field, value) in [
                ("claim", entry.claim.as_str()),
                ("evidence_source", entry.evidence_source.as_str()),
                ("evidence_command", entry.evidence_command.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(EpistemicValidationError::BlankField {
                        category: "known",
                        index,
                        field,
                    });
                }
            }
        }
        for (index, entry) in self.unknown.iter().enumerate() {
            for (field, value) in [
                ("question", entry.question.as_str()),
                ("owner", entry.owner.as_str()),
                ("resolving_experiment", entry.resolving_experiment.as_str()),
                ("cost", entry.cost.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(EpistemicValidationError::BlankField {
                        category: "unknown",
                        index,
                        field,
                    });
                }
            }
        }
        for (index, entry) in self.gaps.iter().enumerate() {
            for (field, value) in [
                ("missing_capability", entry.missing_capability.as_str()),
                ("owner", entry.owner.as_str()),
                ("cost_if_left_open", entry.cost_if_left_open.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(EpistemicValidationError::BlankField {
                        category: "gaps",
                        index,
                        field,
                    });
                }
            }
        }
        Ok(())
    }
}

fn project_id(path: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(path.as_bytes());
    let digest = digest.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    format!("omp-{hex}")[..20].to_owned()
}
fn identity_field(field: &'static str, value: &str) -> Result<String, InceptionError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(InceptionError::IdentityUnavailable {
            field,
            detail: "empty value".to_owned(),
        });
    }
    Ok(value.to_owned())
}

#[derive(Debug)]
enum BoundedCommandError {
    Exit { code: Option<i32>, stderr: String },
    TimedOut,
    Unspawned {
        kind: std::io::ErrorKind,
        detail: String,
    },
}

impl fmt::Display for BoundedCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exit { code, stderr } => {
                write!(formatter, "command exited {code:?}: {stderr}")
            }
            Self::TimedOut => write!(
                formatter,
                "command exceeded {}s",
                IDENTITY_COMMAND_DEADLINE.as_secs()
            ),
            Self::Unspawned { detail, .. } => {
                write!(formatter, "command could not start: {detail}")
            }
        }
    }
}

fn run_command_output_typed(
    command: &mut Command,
) -> Result<std::process::Output, BoundedCommandError> {
    match subprocess_contract::bounded_output(command, IDENTITY_COMMAND_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(output)
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(BoundedCommandError::Exit {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }),
        subprocess_contract::BoundedOutcome::TimedOut => Err(BoundedCommandError::TimedOut),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(BoundedCommandError::Unspawned {
                kind: error.kind(),
                detail: error.to_string(),
            })
        }
    }
}

fn run_command_output(command: &mut Command) -> Result<std::process::Output, String> {
    run_command_output_typed(command).map_err(|error| error.to_string())
}

fn run_identity_command(
    command: &mut Command,
    field: &'static str,
) -> Result<String, InceptionError> {
    let output = run_command_output(command)
        .map_err(|detail| InceptionError::IdentityUnavailable { field, detail })?;
    identity_field(field, &String::from_utf8_lossy(&output.stdout))
}

fn source_revision(repo_root: &Path) -> Result<String, InceptionError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(["rev-parse", "HEAD"]);
    run_identity_command(&mut command, "source_revision")
}

fn host_identity() -> Result<String, InceptionError> {
    let mut command = Command::new("hostname");
    run_identity_command(&mut command, "host_identity")
}
fn git_marker(repo_root: &Path) -> Result<String, InceptionError> {
    let marker = repo_root.join(".git");
    if marker.is_dir() {
        return Ok("directory".to_owned());
    }
    if marker.is_file() {
        let contents =
            fs::read_to_string(&marker).map_err(|error| InceptionError::RepositoryUnreadable {
                path: marker.clone(),
                detail: error.to_string(),
            })?;
        return Ok(contents.lines().next().unwrap_or("file").to_owned());
    }
    Ok("missing".to_owned())
}

/// L2-BUILD-GIT-REPO (contract s1_l2_ecosystem.md): canonical git
/// repository check via `show-toplevel`.
///
/// Healthy: exit 0 plus an existing directory is the canonical top-level
/// path. Halt: git's not-a-repository refusal, a missing binary, a kill,
/// or unreadable output is `IdentityUnavailable` with a remedy, never a
/// guessed path. The L1 git leg pins the same mapping test-locally; this
/// is the production function the L2 flow adopts.
///
/// WIRED (rule 9): sole production caller is [`initialize_gated`], the L2
/// operator entry. It is deliberately NOT wired into shared
/// `initialize`/`build_manifest`, which would re-route doctor repair too,
/// whose kyng-class legs run green on non-git fixtures on workers WITH a
/// `.git` upward and would newly refuse on workers WITHOUT one --
/// environment-divergent breakage for zero new capability.
pub fn git_repo_toplevel(repo: &Path) -> Result<PathBuf, InceptionError> {
    let mut command = Command::new("git");
    // Ceiling the upward search at the argument's parent keeps this
    // deterministic on every lane: without it a bare directory inside any
    // checkout resolves upward and reads as a repository (measured on a
    // worker whose scratch sits under a checkout). A real repository
    // carries its own `.git`, found before any ascent, so the ceiling
    // never consults -- it only stops the climb that manufactures repos.
    if let Some(parent) = repo.parent() {
        command.env("GIT_CEILING_DIRECTORIES", parent);
    }
    command
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--show-toplevel"]);
    let line = run_identity_command(&mut command, "git_toplevel").map_err(|error| match error {
        InceptionError::IdentityUnavailable { field, detail } => {
            InceptionError::IdentityUnavailable {
                field,
                detail: format!(
                    "{detail}; remedy: run git init here or point --repo at a git checkout"
                ),
            }
        }
        other => other,
    })?;
    let path = PathBuf::from(&line);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(InceptionError::IdentityUnavailable {
            field: "git_toplevel",
            detail: format!(
                "show-toplevel printed {line:?}, which is not a directory; remedy: run git init here or point --repo at a git checkout"
            ),
        })
    }
}


/// Package whose membership authorizes this L2 entry. The member population
/// is always derived from fresh Cargo metadata; no absolute count is stored.
pub const CURRENT_WORKSPACE_PACKAGE: &str = "ompo-start";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoWorkspaceMemberReport {
    pub input: InputManifest,
    pub package_ids: BTreeSet<String>,
    pub workspace_member_ids: BTreeSet<String>,
    pub current_package_id: String,
}

impl From<CargoWorkspaceError> for InceptionError {
    fn from(error: CargoWorkspaceError) -> Self {
        Self::CargoWorkspace(error)
    }
}

impl From<AgentMailRegistrationError> for InceptionError {
    fn from(error: AgentMailRegistrationError) -> Self {
        Self::AgentMailRegistration(error)
    }
}

impl From<RchLaneError> for InceptionError {
    fn from(error: RchLaneError) -> Self {
        Self::RchLane(error)
    }
}

fn require_full_cargo_input(input: &InputManifest) -> Result<(), CargoWorkspaceError> {
    match input {
        InputManifest::Full => Ok(()),
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => Err(CargoWorkspaceError::PartialInput {
            bound_kind: bound_kind.clone(),
            bound_value: *bound_value,
            source: source.clone(),
        }),
        InputManifest::Refused { reason } => Err(CargoWorkspaceError::RefusedInput {
            reason: reason.clone(),
        }),
    }
}

pub fn cargo_workspace_member_from_output(
    output: &[u8],
    input: &InputManifest,
) -> Result<CargoWorkspaceMemberReport, CargoWorkspaceError> {
    require_full_cargo_input(input)?;
    let value: Value = serde_json::from_slice(output).map_err(|error| {
        CargoWorkspaceError::MetadataMalformed {
            detail: error.to_string(),
        }
    })?;
    let packages = value
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoWorkspaceError::MetadataMalformed {
            detail: "packages is missing or not an array".to_owned(),
        })?;
    if packages.is_empty() {
        return Err(CargoWorkspaceError::Empty { field: "packages" });
    }
    let members = value
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoWorkspaceError::MetadataMalformed {
            detail: "workspace_members is missing or not an array".to_owned(),
        })?;
    if members.is_empty() {
        return Err(CargoWorkspaceError::Empty {
            field: "workspace_members",
        });
    }
    let mut package_ids = BTreeSet::new();
    let mut current_package_id = None;
    for (index, package) in packages.iter().enumerate() {
        let package = package
            .as_object()
            .ok_or_else(|| CargoWorkspaceError::MetadataMalformed {
                detail: format!("packages[{index}] is not an object"),
            })?;
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| CargoWorkspaceError::MetadataMalformed {
                detail: format!("packages[{index}].id is missing or empty"),
            })?;
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| CargoWorkspaceError::MetadataMalformed {
                detail: format!("packages[{index}].name is missing or empty"),
            })?;
        package_ids.insert(id.to_owned());
        if name == CURRENT_WORKSPACE_PACKAGE {
            current_package_id = Some(id.to_owned());
        }
    }
    let workspace_member_ids: BTreeSet<String> = members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            member
                .as_str()
                .filter(|member| !member.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| CargoWorkspaceError::MetadataMalformed {
                    detail: format!(
                        "workspace_members[{index}] is not a nonempty string"
                    ),
                })
        })
        .collect::<Result<_, _>>()?;
    let current_package_id = current_package_id.ok_or_else(|| {
        CargoWorkspaceError::CurrentCrateAbsent {
            package: CURRENT_WORKSPACE_PACKAGE.to_owned(),
            detail: "package is absent from metadata packages".to_owned(),
        }
    })?;
    if !workspace_member_ids.contains(&current_package_id) {
        return Err(CargoWorkspaceError::CurrentCrateAbsent {
            package: CURRENT_WORKSPACE_PACKAGE.to_owned(),
            detail: "package id is absent from workspace_members".to_owned(),
        });
    }
    Ok(CargoWorkspaceMemberReport {
        input: input.clone(),
        package_ids,
        workspace_member_ids,
        current_package_id,
    })
}

pub fn cargo_workspace_member_report_with_program(
    repo: &Path,
    input: &InputManifest,
    program: &Path,
) -> Result<CargoWorkspaceMemberReport, CargoWorkspaceError> {
    require_full_cargo_input(input)?;
    let mut command = Command::new(program);
    command.current_dir(repo).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--offline",
    ]);
    let output = run_command_output_typed(&mut command).map_err(|error| match error {
        BoundedCommandError::Unspawned { kind, detail }
            if kind == std::io::ErrorKind::NotFound => CargoWorkspaceError::CargoMissing {
                program: program.display().to_string(),
                detail,
            },
        other => CargoWorkspaceError::MetadataFailed {
            detail: other.to_string(),
        },
    })?;
    cargo_workspace_member_from_output(&output.stdout, input)
}

pub fn cargo_workspace_member_report(
    repo: &Path,
    input: &InputManifest,
) -> Result<CargoWorkspaceMemberReport, CargoWorkspaceError> {
    cargo_workspace_member_report_with_program(repo, input, Path::new("cargo"))
}
/// Parse the read-only project registry and require its exact canonical path.
fn parse_agent_mail_project_registry(
    value: &Value,
    expected_project: &str,
) -> Result<AgentMailProjectRegistry, AgentMailRegistrationError> {
    let object =
        value
            .as_object()
            .ok_or_else(|| AgentMailRegistrationError::MalformedResponse {
                detail: "agent registry resource is not an object".to_owned(),
            })?;
    let project =
        object
            .get("project")
            .ok_or_else(|| AgentMailRegistrationError::ProjectMissing {
                project: expected_project.to_owned(),
                detail: "agent registry omitted project readback".to_owned(),
            })?;
    let project =
        project
            .as_object()
            .ok_or_else(|| AgentMailRegistrationError::MalformedResponse {
                detail: "agent registry project is not an object".to_owned(),
            })?;
    let actual_project = project
        .get("human_key")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AgentMailRegistrationError::ProjectMissing {
            project: expected_project.to_owned(),
            detail: "agent registry omitted project.human_key".to_owned(),
        })?;
    if actual_project != expected_project {
        return Err(AgentMailRegistrationError::ProjectMismatch {
            expected: expected_project.to_owned(),
            actual: actual_project.to_owned(),
        });
    }

    let rows = object
        .get("agents")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentMailRegistrationError::MalformedResponse {
            detail: "agent registry omitted the agents array".to_owned(),
        })?;
    let agents = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            row.as_object()
                .and_then(|row| row.get("name"))
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| AgentMailRegistrationError::MalformedResponse {
                    detail: format!("agent registry agents[{index}].name is missing or empty"),
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(AgentMailProjectRegistry {
        project: actual_project.to_owned(),
        agents,
    })
}

fn require_full_agent_mail_input(input: &InputManifest) -> Result<(), AgentMailRegistrationError> {
    input.require_full().map_err(|_| match input {
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => AgentMailRegistrationError::PartialInput {
            bound_kind: bound_kind.clone(),
            bound_value: *bound_value,
            source: source.clone(),
        },
        InputManifest::Refused { reason } => AgentMailRegistrationError::RefusedInput {
            reason: reason.clone(),
        },
        InputManifest::Full => unreachable!("require_full accepted FULL"),
    })
}

fn agent_mail_registration_from_registry(
    registry: AgentMailProjectRegistry,
    expected_pane: &str,
    intended_agent: &str,
    pane_identity: &PaneIdentity,
    input: &InputManifest,
) -> Result<AgentMailRegistrationReport, AgentMailRegistrationError> {
    require_full_agent_mail_input(input)?;
    let intended_agent = intended_agent.trim();
    if intended_agent.is_empty() {
        return Err(AgentMailRegistrationError::AgentMissing {
            project: registry.project,
            agent: None,
            detail: "the canonical intended-agent surface was empty".to_owned(),
        });
    }
    if !registry.agents.contains(intended_agent) {
        return Err(AgentMailRegistrationError::AgentMissing {
            project: registry.project,
            agent: Some(intended_agent.to_owned()),
            detail: "the read-only project roster does not contain the intended agent".to_owned(),
        });
    }
    if pane_identity.binding != BindingStatus::VerifiedLive {
        return Err(AgentMailRegistrationError::Unknown {
            detail: format!(
                "pane {} has non-live binding {:?}",
                pane_identity.pane_id, pane_identity.binding
            ),
        });
    }
    let resolved_agent = pane_identity
        .agent_name
        .as_ref()
        .map(|agent| agent.as_str());
    if pane_identity.pane_id != expected_pane || resolved_agent != Some(intended_agent) {
        return Err(AgentMailRegistrationError::PaneAgentMismatch {
            expected_pane: expected_pane.to_owned(),
            actual_pane: pane_identity.pane_id.clone(),
            intended_agent: intended_agent.to_owned(),
            resolved_agent: resolved_agent.map(str::to_owned),
        });
    }
    Ok(AgentMailRegistrationReport {
        input: input.clone(),
        project: registry.project,
        pane_id: pane_identity.pane_id.clone(),
        agent: intended_agent.to_owned(),
    })
}

/// Validate read-only Agent Mail project and pane identity responses.
///
/// This pure boundary lets restrictive response shapes be tested without a
/// daemon. Production obtains both values through [`MailClient`], never from a
/// config file or a name-only assertion.
pub fn agent_mail_registration_from_readbacks(
    expected_project: &Path,
    expected_pane: &str,
    intended_agent: &str,
    registry_value: &Value,
    pane_identity: &PaneIdentity,
    input: &InputManifest,
) -> Result<AgentMailRegistrationReport, AgentMailRegistrationError> {
    require_full_agent_mail_input(input)?;
    let expected_project = expected_project.display().to_string();
    let registry = parse_agent_mail_project_registry(registry_value, &expected_project)?;
    agent_mail_registration_from_registry(
        registry,
        expected_pane,
        intended_agent,
        pane_identity,
        input,
    )
}

fn agent_mail_error(
    error: MailError,
    project: &str,
    agent: Option<&str>,
) -> AgentMailRegistrationError {
    match error {
        MailError::Rpc { code, message }
            if message.to_ascii_lowercase().contains("project not found") =>
        {
            AgentMailRegistrationError::ProjectMissing {
                project: project.to_owned(),
                detail: format!("rpc_code={code} message={message}"),
            }
        }
        MailError::ToolRefused { kind, message, .. }
            if kind == "IDENTITY_NOT_FOUND" || kind == "AGENT_NOT_FOUND" =>
        {
            AgentMailRegistrationError::AgentMissing {
                project: project.to_owned(),
                agent: agent.map(str::to_owned),
                detail: format!("kind={kind} message={message}"),
            }
        }
        MailError::Unreachable { .. }
        | MailError::Unauthorized { .. }
        | MailError::MissingCredential { .. }
        | MailError::TimedOut { .. }
        | MailError::Cancelled(_)
        | MailError::UnexpectedStatus { .. } => AgentMailRegistrationError::ServiceUnavailable {
            detail: error.to_string(),
        },
        MailError::Protocol { .. } | MailError::Codec { .. } => {
            AgentMailRegistrationError::MalformedResponse {
                detail: error.to_string(),
            }
        }
        other => AgentMailRegistrationError::Unknown {
            detail: other.to_string(),
        },
    }
}

fn agent_mail_identity_error(
    error: IdentityError,
    project: &str,
    agent: &str,
) -> AgentMailRegistrationError {
    match error {
        IdentityError::Mail(error) => agent_mail_error(error, project, Some(agent)),
        IdentityError::UnknownPaneBinding { binding } => AgentMailRegistrationError::Unknown {
            detail: format!("unknown pane binding {binding}"),
        },
        IdentityError::UnverifiedPaneBinding { binding } => AgentMailRegistrationError::Unknown {
            detail: format!("unverified pane binding {binding}"),
        },
        IdentityError::MissingTmuxPane => AgentMailRegistrationError::PaneAgentMismatch {
            expected_pane: "<missing>".to_owned(),
            actual_pane: "<missing>".to_owned(),
            intended_agent: agent.to_owned(),
            resolved_agent: None,
        },
        other => AgentMailRegistrationError::MalformedResponse {
            detail: other.to_string(),
        },
    }
}

/// Read back the canonical project roster and this pane's exact agent binding.
///
/// The project resource is deliberately used instead of the `list_agents`
/// tool: the resource only reads existing state, while an absolute-path tool
/// lookup may ensure a missing project. The intended agent comes from the
/// typed sender-identity surface; a machine-global `AGENT_NAME` remains an
/// explicit Unknown and is never accepted as project registration.
pub fn agent_mail_registration_report(
    repo: &Path,
    input: &InputManifest,
) -> Result<AgentMailRegistrationReport, AgentMailRegistrationError> {
    require_full_agent_mail_input(input)?;
    let project = repo.display().to_string();
    let pane_id = env::var("TMUX_PANE")
        .ok()
        .map(|pane| pane.trim().to_owned())
        .filter(|pane| !pane.is_empty())
        .ok_or_else(|| AgentMailRegistrationError::PaneAgentMismatch {
            expected_pane: "<missing>".to_owned(),
            actual_pane: "<missing>".to_owned(),
            intended_agent: "<unknown>".to_owned(),
            resolved_agent: None,
        })?;
    let candidate = first_candidate(&|name| env::var(name).ok()).ok_or_else(|| {
        AgentMailRegistrationError::AgentMissing {
            project: project.clone(),
            agent: None,
            detail: "none of the canonical sender identity variables is set".to_owned(),
        }
    })?;
    if candidate.source.is_ambient() {
        return Err(AgentMailRegistrationError::Unknown {
            detail: format!(
                "{}={} is machine-global and cannot establish the intended project agent",
                candidate.source.var(),
                candidate.value
            ),
        });
    }
    let intended_agent = candidate.value;
    let project_key = ProjectKey::new(project.clone());
    let uri = format!("resource://agents/{project}");
    let client = MailClient::discover().with_request_timeout(AGENT_MAIL_REQUEST_TIMEOUT);
    let runtime = RuntimeBuilder::current_thread().build().map_err(|error| {
        AgentMailRegistrationError::ServiceUnavailable {
            detail: format!("Agent Mail runtime build failed: {error}"),
        }
    })?;
    let (registry_value, pane_identity) = runtime.block_on(async {
        let cx = Cx::current().ok_or_else(|| AgentMailRegistrationError::Unknown {
            detail: "Agent Mail runtime supplied no Cx".to_owned(),
        })?;
        let registry_value = client
            .read_resource(&cx, &uri)
            .await
            .map_err(|error| agent_mail_error(error, &project, Some(&intended_agent)))?;
        let pane_identity = resolve_pane_identity(&cx, &client, &project_key, &pane_id)
            .await
            .map_err(|error| agent_mail_identity_error(error, &project, &intended_agent))?;
        Ok::<_, AgentMailRegistrationError>((registry_value, pane_identity))
    })?;
    let registry = parse_agent_mail_project_registry(&registry_value, &project)?;
    agent_mail_registration_from_registry(
        registry,
        &pane_id,
        &intended_agent,
        &pane_identity,
        input,
    )
}

fn require_full_rch_input(input: &InputManifest) -> Result<(), RchLaneError> {
    if let InputManifest::Partial {
        bound_kind,
        bound_value,
        source,
    } = input
    {
        return Err(RchLaneError::PartialInput {
            bound_kind: bound_kind.clone(),
            bound_value: *bound_value,
            source: source.clone(),
        });
    }
    if let InputManifest::Refused { reason } = input {
        return Err(RchLaneError::RefusedInput {
            reason: reason.clone(),
        });
    }
    Ok(())
}

fn rch_surface_data(
    bytes: &[u8],
    surface: &'static str,
    expected_command: &str,
) -> Result<Map<String, Value>, RchLaneError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        RchLaneError::MalformedOutput {
            surface,
            detail: format!("response is not JSON: {error}"),
        }
    })?;
    let object = value.as_object().ok_or_else(|| RchLaneError::MalformedOutput {
        surface,
        detail: "response envelope is not an object".to_owned(),
    })?;
    let command = object
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface,
            detail: "response omitted command".to_owned(),
        })?;
    if command != expected_command {
        return Err(RchLaneError::MalformedOutput {
            surface,
            detail: format!("expected command={expected_command}, found={command}"),
        });
    }
    match object.get("success").and_then(Value::as_bool) {
        Some(true) => {}
        Some(false) => {
            return Err(RchLaneError::CommandFailed {
                surface,
                detail: "RCH returned success=false".to_owned(),
            });
        }
        None => {
            return Err(RchLaneError::MalformedOutput {
                surface,
                detail: "response omitted boolean success".to_owned(),
            });
        }
    }
    object
        .get("data")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface,
            detail: "response omitted object data".to_owned(),
        })
}

fn rch_string_set(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<BTreeSet<String>, RchLaneError> {
    let rows = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "status",
            detail: format!("worker row omitted array {field}"),
        })?;
    rows.iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| RchLaneError::MalformedOutput {
                    surface: "status",
                    detail: format!("{field}[{index}] is not a nonempty string"),
                })
        })
        .collect()
}

fn rch_project_id(repo: &Path) -> Result<String, RchLaneError> {
    let name = repo
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| RchLaneError::ContradictoryTopology {
            detail: format!("repository root {} has no usable final component", repo.display()),
        })?;
    if name == "."
        || name == ".."
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name.starts_with('-')
    {
        return Ok("unknown".to_owned());
    }
    let is_safe = name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
        && !name.starts_with('.');
    if is_safe {
        return Ok(name.to_owned());
    }
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_start_matches('.');
    Ok(if sanitized.is_empty() {
        "unknown".to_owned()
    } else {
        sanitized.to_owned()
    })
}

/// Derive one repository's RCH lane state from the read-only doctor, status,
/// and dry-run diagnosis envelopes. A selected worker is deliberately ignored:
/// only convergence rows spanning the reported topology can establish MAPPED.
pub fn rch_lane_report_from_outputs(
    repo: &Path,
    input: &InputManifest,
    doctor_bytes: &[u8],
    status_bytes: &[u8],
    diagnose_bytes: &[u8],
) -> Result<RchLaneReport, RchLaneError> {
    require_full_rch_input(input)?;
    let project_id = rch_project_id(repo)?;
    let doctor = rch_surface_data(doctor_bytes, "doctor", "doctor.reliability")?;
    let status = rch_surface_data(status_bytes, "status", "status")?;
    let diagnose = rch_surface_data(diagnose_bytes, "diagnose", "diagnose")?;

    let scope = doctor
        .get("scope")
        .and_then(Value::as_array)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "doctor",
            detail: "doctor omitted scope".to_owned(),
        })?;
    for required in ["topology", "convergence"] {
        if !scope.iter().any(|value| value.as_str() == Some(required)) {
            return Err(RchLaneError::MalformedOutput {
                surface: "doctor",
                detail: format!("doctor scope omitted {required}"),
            });
        }
    }
    let diagnostics = doctor
        .get("diagnostics")
        .and_then(Value::as_array)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "doctor",
            detail: "doctor omitted diagnostics".to_owned(),
        })?;
    if !diagnostics.iter().any(|row| row.get("category").and_then(Value::as_str) == Some("topology")) {
        return Err(RchLaneError::MalformedOutput {
            surface: "doctor",
            detail: "doctor returned no topology diagnostic".to_owned(),
        });
    }
    let convergence_diagnostic = diagnostics
        .iter()
        .find(|row| row.get("check_name").and_then(Value::as_str) == Some("repo_convergence"))
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "doctor",
            detail: "doctor returned no repo_convergence diagnostic".to_owned(),
        })?;
    let doctor_code = convergence_diagnostic
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("<missing-code>");
    let doctor_message = convergence_diagnostic
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("<missing-message>");
    let doctor_details = convergence_diagnostic
        .get("details")
        .and_then(Value::as_str)
        .unwrap_or("<missing-details>");
    if doctor_code.starts_with("<missing") || doctor_message.starts_with("<missing") {
        return Err(RchLaneError::MalformedOutput {
            surface: "doctor",
            detail: "repo_convergence diagnostic omitted code or message".to_owned(),
        });
    }
    let topology_causes = diagnostics
        .iter()
        .filter(|row| row.get("category").and_then(Value::as_str) == Some("topology"))
        .filter(|row| row.get("severity").and_then(Value::as_str) != Some("pass"))
        .filter_map(|row| row.get("message").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("; ");

    let classification = diagnose
        .get("classification")
        .and_then(Value::as_object)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "diagnose",
            detail: "diagnose omitted classification".to_owned(),
        })?;
    if classification.get("is_compilation").and_then(Value::as_bool) != Some(true) {
        return Err(RchLaneError::ContradictoryTopology {
            detail: "repo-scoped cargo check was not classified as compilation".to_owned(),
        });
    }
    let decision = diagnose
        .get("decision")
        .and_then(Value::as_object)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "diagnose",
            detail: "diagnose omitted decision".to_owned(),
        })?;
    let would_intercept = decision
        .get("would_intercept")
        .and_then(Value::as_bool)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "diagnose",
            detail: "diagnose decision omitted would_intercept".to_owned(),
        })?;
    let diagnose_reason = decision
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("no diagnose reason");

    let mut excluded_workers = BTreeSet::new();
    if let Some(selection) = diagnose.get("worker_selection") {
        let selection = selection.as_object().ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "diagnose",
            detail: "worker_selection is not an object".to_owned(),
        })?;
        if selection
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason.contains("project_excluded"))
        {
            excluded_workers.insert("<selection>".to_owned());
        }
        if let Some(selection_diagnostics) = selection.get("diagnostics") {
            let selection_diagnostics = selection_diagnostics.as_object().ok_or_else(|| {
                RchLaneError::MalformedOutput {
                    surface: "diagnose",
                    detail: "worker_selection.diagnostics is not an object".to_owned(),
                }
            })?;
            let declared = selection_diagnostics
                .get("active_project_exclusion_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| RchLaneError::MalformedOutput {
                    surface: "diagnose",
                    detail: "selection diagnostics omitted exclusion count".to_owned(),
                })?;
            let rows = selection_diagnostics
                .get("workers")
                .and_then(Value::as_array)
                .ok_or_else(|| RchLaneError::MalformedOutput {
                    surface: "diagnose",
                    detail: "selection diagnostics omitted workers".to_owned(),
                })?;
            for row in rows {
                if row.get("active_project_excluded").and_then(Value::as_bool) == Some(true) {
                    let worker = row
                        .get("worker_id")
                        .and_then(Value::as_str)
                        .filter(|worker| !worker.is_empty())
                        .ok_or_else(|| RchLaneError::MalformedOutput {
                            surface: "diagnose",
                            detail: "excluded worker omitted worker_id".to_owned(),
                        })?;
                    excluded_workers.insert(worker.to_owned());
                }
            }
            if declared != u64::try_from(excluded_workers.len()).unwrap_or(u64::MAX) {
                return Err(RchLaneError::ContradictoryTopology {
                    detail: format!(
                        "diagnose exclusion count={declared} but named workers={}",
                        excluded_workers.len()
                    ),
                });
            }
        }
    }
    if !excluded_workers.is_empty() {
        return Err(RchLaneError::ProjectExcluded {
            project_id,
            workers: excluded_workers,
        });
    }

    let convergence = status
        .get("convergence")
        .and_then(Value::as_object)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "status",
            detail: "status omitted convergence".to_owned(),
        })?;
    let convergence_status = convergence
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "status",
            detail: "convergence omitted status".to_owned(),
        })?;
    let workers = convergence
        .get("workers")
        .and_then(Value::as_array)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "status",
            detail: "convergence omitted workers".to_owned(),
        })?;
    let summary = convergence
        .get("summary")
        .and_then(Value::as_object)
        .ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "status",
            detail: "convergence omitted summary".to_owned(),
        })?;
    let summary_value = |field: &'static str| {
        summary.get(field).and_then(Value::as_u64).ok_or_else(|| {
            RchLaneError::MalformedOutput {
                surface: "status",
                detail: format!("convergence summary omitted {field}"),
            }
        })
    };
    let total = summary_value("total_workers")?;
    let partition = ["ready", "drifting", "converging", "failed", "stale"]
        .into_iter()
        .try_fold(0_u64, |sum, field| {
            summary_value(field).and_then(|value| {
                sum.checked_add(value).ok_or_else(|| RchLaneError::ContradictoryTopology {
                    detail: "convergence summary overflowed".to_owned(),
                })
            })
        })?;
    if total != u64::try_from(workers.len()).unwrap_or(u64::MAX) || total != partition {
        return Err(RchLaneError::ContradictoryTopology {
            detail: format!(
                "convergence summary total={total} partition={partition} rows={}",
                workers.len()
            ),
        });
    }
    if convergence_status == "unknown" {
        if !workers.is_empty() || total != 0 || doctor_code != "RCH-R303" {
            return Err(RchLaneError::ContradictoryTopology {
                detail: format!(
                    "status=unknown rows={} total={total} doctor_code={doctor_code}",
                    workers.len()
                ),
            });
        }
        return Ok(RchLaneReport {
            input: input.clone(),
            project_id,
            state: RchLaneState::Unknown {
                cause: format!("{doctor_message}; {doctor_details}"),
            },
        });
    }
    if doctor_code == "RCH-R303" {
        return Err(RchLaneError::ContradictoryTopology {
            detail: format!(
                "doctor reports no convergence rows while status={convergence_status} rows={}",
                workers.len()
            ),
        });
    }
    if !matches!(convergence_status, "ready" | "drifting" | "converging" | "failed" | "stale") {
        return Ok(RchLaneReport {
            input: input.clone(),
            project_id,
            state: RchLaneState::Unknown {
                cause: format!("unrecognized convergence status={convergence_status}"),
            },
        });
    }

    let mut seen_workers = BTreeSet::new();
    let mut mapped_workers = BTreeSet::new();
    let mut relevant_rows = false;
    let mut unresolved = Vec::new();
    for row in workers {
        let row = row.as_object().ok_or_else(|| RchLaneError::MalformedOutput {
            surface: "status",
            detail: "convergence worker row is not an object".to_owned(),
        })?;
        let worker_id = row
            .get("worker_id")
            .and_then(Value::as_str)
            .filter(|worker| !worker.is_empty())
            .ok_or_else(|| RchLaneError::MalformedOutput {
                surface: "status",
                detail: "convergence worker omitted worker_id".to_owned(),
            })?;
        if !seen_workers.insert(worker_id.to_owned()) {
            return Err(RchLaneError::ContradictoryTopology {
                detail: format!("duplicate convergence worker_id={worker_id}"),
            });
        }
        let drift_state = row
            .get("drift_state")
            .and_then(Value::as_str)
            .ok_or_else(|| RchLaneError::MalformedOutput {
                surface: "status",
                detail: format!("worker {worker_id} omitted drift_state"),
            })?;
        let required = rch_string_set(row, "required_repos")?;
        let synced = rch_string_set(row, "synced_repos")?;
        let missing = rch_string_set(row, "missing_repos")?;
        let is_required = required.contains(&project_id);
        let is_synced = synced.contains(&project_id);
        let is_missing = missing.contains(&project_id);
        if !(is_required || is_synced || is_missing) {
            continue;
        }
        relevant_rows = true;
        if is_synced && is_missing {
            return Err(RchLaneError::ContradictoryTopology {
                detail: format!("worker {worker_id} lists {project_id} as synced and missing"),
            });
        }
        if is_synced && !is_required {
            return Err(RchLaneError::ContradictoryTopology {
                detail: format!("worker {worker_id} syncs unrequired project {project_id}"),
            });
        }
        if is_required && is_synced && !is_missing && drift_state == "ready" {
            mapped_workers.insert(worker_id.to_owned());
        } else {
            unresolved.push(format!(
                "worker={worker_id} state={drift_state} required={is_required} synced={is_synced} missing={is_missing}"
            ));
        }
    }
    if !relevant_rows {
        return Err(RchLaneError::ProjectRowAbsent { project_id });
    }
    if !topology_causes.is_empty() || !would_intercept || mapped_workers.is_empty() {
        let mut causes = unresolved;
        if !topology_causes.is_empty() {
            causes.push(format!("topology={topology_causes}"));
        }
        if !would_intercept {
            causes.push(format!("diagnose={diagnose_reason}"));
        }
        if causes.is_empty() {
            causes.push(format!("convergence={convergence_status}"));
        }
        return Ok(RchLaneReport {
            input: input.clone(),
            project_id,
            state: RchLaneState::Unknown {
                cause: causes.join("; "),
            },
        });
    }
    Ok(RchLaneReport {
        input: input.clone(),
        project_id,
        state: RchLaneState::Mapped {
            workers: mapped_workers,
        },
    })
}

fn run_rch_surface(
    program: &Path,
    repo: &Path,
    surface: &'static str,
    args: &[&str],
) -> Result<Vec<u8>, RchLaneError> {
    let mut command = Command::new(program);
    command.current_dir(repo).args(args);
    match run_command_output_typed(&mut command) {
        Ok(output) => Ok(output.stdout),
        Err(BoundedCommandError::Unspawned { kind, detail })
            if kind == std::io::ErrorKind::NotFound =>
        {
            Err(RchLaneError::MissingRch {
                program: program.display().to_string(),
                detail,
            })
        }
        Err(BoundedCommandError::TimedOut) => Err(RchLaneError::Timeout { surface }),
        Err(error) => Err(RchLaneError::CommandFailed {
            surface,
            detail: error.to_string(),
        }),
    }
}
pub fn rch_lane_report_with_program(
    repo: &Path,
    input: &InputManifest,
    program: &Path,
) -> Result<RchLaneReport, RchLaneError> {
    require_full_rch_input(input)?;
    let doctor = run_rch_surface(
        program,
        repo,
        "doctor",
        &[
            "--no-self-healing",
            "doctor",
            "--reliability",
            "--scope",
            "topology,convergence",
            "--json",
        ],
    )?;
    let status = run_rch_surface(
        program,
        repo,
        "status",
        &["--no-self-healing", "status", "--workers", "--json"],
    )?;
    let diagnose = run_rch_surface(
        program,
        repo,
        "diagnose",
        &[
            "--no-self-healing",
            "diagnose",
            "--dry-run",
            "--json",
            "cargo",
            "check",
            "-p",
            CURRENT_WORKSPACE_PACKAGE,
        ],
    )?;
    rch_lane_report_from_outputs(repo, input, &doctor, &status, &diagnose)
}

pub fn rch_lane_report(
    repo: &Path,
    input: &InputManifest,
) -> Result<RchLaneReport, RchLaneError> {
    rch_lane_report_with_program(repo, input, Path::new("rch"))
}

/// L2-BUILD-REMOTE-PERSONA-A (contract s1_l2_ecosystem.md): Persona A
/// local-only remote rule.
///
/// Persona A runs without remotes: a subject with no git remote is an
/// explicitly recorded allowance (`remote_optional=true`) with
/// continuation -- never a silent universal success, and never a halt.
/// Every other combination is restrictive (`remote_optional=false`): a
/// present remote needs no allowance, and a non-Persona-A subject must
/// treat a missing remote as required. (No Persona B/C variants exist
/// in this tree; non-Persona-A covers them, and this rule changes
/// nothing for that population.)
///
/// `remote -v` reads the subject's own config, so unlike `show-toplevel`
/// there is no upward search to ceiling. An unobservable subject (git
/// missing, killed) is a typed error, never an absence claim: absence
/// of evidence is not evidence of a remote.
///
/// WIRED (rule 9): [`initialize_gated`] consumes this verdict immediately
/// before initialization continues and returns the same record in
/// [`InitReport::persona_remote`]. Shared repair initialization remains
/// deliberately outside this policy boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonaRemote {
    pub remote_optional: bool,
    /// The observed remote presence behind the verdict: `true` when
    /// `git remote -v` listed at least one remote. Carried so the
    /// dispatch gate below needs no second probe and no bool-passing.
    pub remote_present: bool,
    pub reason_code: &'static str,
}

/// Persona A remote allowance over a live `git remote -v` observation.
pub fn persona_remote_policy(
    repo: &Path,
    persona_a: bool,
) -> Result<PersonaRemote, InceptionError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(["remote", "-v"]);
    let present = match subprocess_contract::bounded_output(
        &mut command,
        IDENTITY_COMMAND_DEADLINE,
    ) {
        subprocess_contract::BoundedOutcome::Completed(output) => output
            .stdout
            .split(|byte| *byte == b'\n')
            .any(|line| !line.iter().all(u8::is_ascii_whitespace)),
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(InceptionError::IdentityUnavailable {
                field: "git_remote",
                detail: "remote listing timed out; unobservable, never absent".to_owned(),
            })
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(InceptionError::IdentityUnavailable {
                field: "git_remote",
                detail: format!("remote listing could not start: {error}"),
            })
        }
    };
    if persona_a && !present {
        Ok(PersonaRemote {
            remote_optional: true,
            remote_present: present,
            reason_code: "PERSONA_A_LOCAL_ONLY",
        })
    } else {
        Ok(PersonaRemote {
            remote_optional: false,
            remote_present: present,
            reason_code: "REMOTE_REQUIRED",
        })
    }
}
/// L2-BUILD-REMOTE-PERSONA-BC (bead mxro): fleet required-remote gate.
///
/// A non-optional policy without an observed remote halts shared dispatch
/// with a named remediation; every other record passes through untouched.
/// Persona A allowance and present remotes both continue -- the halt
/// fires only for the restrictive branch with nothing behind it. No
/// Persona B/C variants exist in this tree; non-Persona-A records cover
/// that population without inventing new persona types.
///
/// WIRED (rule 9): [`remote_policy_for_shared_dispatch`] is the sole
/// production caller and applies this gate to the one observed
/// [`PersonaRemote`] before the `ompo start --spawn` path can dispatch.
pub fn require_remote_for_dispatch(
    policy: &PersonaRemote,
) -> Result<(), InceptionError> {
    if policy.remote_optional || policy.remote_present {
        return Ok(());
    }
    Err(InceptionError::IdentityUnavailable {
        field: "git_remote",
        detail: format!(
            "fleet dispatch requires a git remote ({}); remedy: configure a remote for this checkout, or run local-only as Persona A",
            policy.reason_code
        ),
    })
}

/// Reachable L2 boundary for remote-dependent shared dispatch.
///
/// The policy observation is made exactly once, then the existing
/// required-remote gate consumes that same record. The production
/// `ompo start --spawn` path calls this before liveness or spawn work;
/// non-dispatching start views do not need a remote.
pub fn remote_policy_for_shared_dispatch(
    repo: &Path,
    persona_a: bool,
) -> Result<(), InceptionError> {
    let policy = persona_remote_policy(repo, persona_a)?;
    require_remote_for_dispatch(&policy)
}

/// Read-only verdict for the installed pre-commit hook's source identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum HookIdentityStatus {
    ExactMatch,
    MissingHook,
    UnreadableHook,
    SourceCommitAbsent,
    SourceCommitUnresolvable,
    ContentMismatch,
    UnprovenArtifact,
}

const HOOK_IDENTITY_POLICIES: [(&str, &str); 7] = [
    ("HOOK_IDENTITY_EXACT", "none"),
    ("HOOK_IDENTITY_MISSING", "restore the repository's installed pre-commit hook"),
    ("HOOK_IDENTITY_UNREADABLE", "restore read access to the installed pre-commit hook"),
    ("HOOK_SOURCE_COMMIT_ABSENT", "restore the repository HEAD source authority"),
    ("HOOK_SOURCE_COMMIT_UNRESOLVABLE", "repair the HEAD reference or object database"),
    ("HOOK_IDENTITY_HEAD_MISMATCH", "stop and route hook refresh through the authorized hook path"),
    ("HOOK_IDENTITY_UNPROVEN_ARTIFACT", "install a hook carrying the canonical source manifest stamp"),
];

impl HookIdentityStatus {
    fn policy(self) -> (&'static str, &'static str) {
        HOOK_IDENTITY_POLICIES[self as usize]
    }

    #[must_use]
    pub fn reason_code(self) -> &'static str {
        self.policy().0
    }

    #[must_use]
    pub fn remediation(self) -> &'static str {
        self.policy().1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookIdentityReport {
    pub status: HookIdentityStatus,
    pub hook_path: PathBuf,
    pub source_commit: Option<String>,
    pub manifest_rows: usize,
    pub detail: String,
}

fn hook_identity_report(
    status: HookIdentityStatus,
    hook_path: PathBuf,
    source_commit: Option<String>,
    manifest_rows: usize,
    detail: impl Into<String>,
) -> HookIdentityReport {
    HookIdentityReport {
        status,
        hook_path,
        source_commit,
        manifest_rows,
        detail: detail.into(),
    }
}

fn manifest_path_shape(path: &str) -> bool {
    path.starts_with("crates/")
        && path.contains("/src/")
        && path.ends_with(".rs")
        && path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-')
        })
}

fn manifest_rows_in_string(line: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = line[cursor..].find("crates/") {
        let start = cursor + relative;
        let Some(space_relative) = line[start..].find(' ') else {
            break;
        };
        let space = start + space_relative;
        let path = &line[start..space];
        let digest_start = space + 1;
        let digest_end = digest_start + 64;
        let digest = line.get(digest_start..digest_end);
        if manifest_path_shape(path)
            && digest.is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            rows.push((path.to_owned(), digest.expect("checked").to_ascii_lowercase()));
            cursor = digest_end;
        } else {
            cursor = start + "crates/".len();
        }
    }
    rows
}

fn stamped_hook_manifest(strings_output: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(strings_output);
    let candidates: Vec<Vec<(String, String)>> = text
        .lines()
        .map(manifest_rows_in_string)
        .filter(|rows| !rows.is_empty())
        .collect();
    if candidates.len() != 1 {
        return Err(format!(
            "expected exactly one embedded hook source manifest, found {}",
            candidates.len()
        ));
    }
    let rows = &candidates[0];
    let unique: BTreeSet<_> = rows.iter().map(|(path, _)| path.as_str()).collect();
    if unique.len() != rows.len() {
        return Err("embedded hook source manifest contains duplicate paths".to_owned());
    }
    let mut manifest = String::new();
    for (path, digest) in rows {
        writeln!(manifest, "{path} {digest}").expect("writing to String cannot fail");
    }
    Ok(manifest)
}

fn head_hook_source_manifest(
    repo: &Path,
    source_commit: &str,
) -> Result<String, String> {
    let mut list = Command::new("git");
    list.arg("-C")
        .arg(repo)
        .args(["ls-tree", "-r", "--name-only", source_commit, "--"]);
    for crate_name in hook_digest_authority::HOOK_SOURCE_CRATES {
        list.arg(format!("crates/{crate_name}/src"));
    }
    let output = run_command_output(&mut list)?;
    let mut paths: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|path| path.ends_with(".rs"))
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths.dedup();

    let mut manifest = String::new();
    for path in paths {
        let mut show = Command::new("git");
        show.arg("-C")
            .arg(repo)
            .args(["show", &format!("{source_commit}:{path}")]);
        let bytes = run_command_output(&mut show)?.stdout;
        let mut digest = Sha256::new();
        digest.update(&bytes);
        writeln!(manifest, "{path} {}", hook_digest_authority::hex(&digest.finalize()))
            .expect("writing to String cannot fail");
    }
    Ok(manifest)
}

/// Compare the installed hook's embedded source manifest with the source set
/// at repository HEAD. No hook is executed and mtime is never consulted.
#[must_use]
pub fn hook_source_identity_report(repo: &Path) -> HookIdentityReport {
    let hook_path = repo.join(".git/hooks/pre-commit");
    let hook_bytes = match fs::read(&hook_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return hook_identity_report(
                HookIdentityStatus::MissingHook,
                hook_path,
                None,
                0,
                "installed hook is absent",
            );
        }
        Err(error) => {
            return hook_identity_report(
                HookIdentityStatus::UnreadableHook,
                hook_path,
                None,
                0,
                error.to_string(),
            );
        }
    };
    if hook_bytes.is_empty() {
        return hook_identity_report(
            HookIdentityStatus::UnprovenArtifact,
            hook_path,
            None,
            0,
            "installed hook is empty",
        );
    }

    let mut strings = Command::new("strings");
    strings.arg(&hook_path);
    let strings_output = match run_command_output(&mut strings) {
        Ok(output) => output,
        Err(detail) => {
            return hook_identity_report(
                HookIdentityStatus::UnprovenArtifact,
                hook_path,
                None,
                0,
                detail,
            );
        }
    };
    let stamped = match stamped_hook_manifest(&strings_output.stdout) {
        Ok(manifest) => manifest,
        Err(detail) => {
            return hook_identity_report(
                HookIdentityStatus::UnprovenArtifact,
                hook_path,
                None,
                0,
                detail,
            );
        }
    };
    let row_count = hook_digest_authority::manifest_rows(&stamped).len();

    if !repo.join(".git/HEAD").is_file() {
        return hook_identity_report(
            HookIdentityStatus::SourceCommitAbsent,
            hook_path,
            None,
            row_count,
            "repository HEAD source authority is absent",
        );
    }
    let mut head = Command::new("git");
    head.arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "HEAD^{commit}"]);
    let source_commit = match run_command_output(&mut head) {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        Err(detail) => {
            return hook_identity_report(
                HookIdentityStatus::SourceCommitUnresolvable,
                hook_path,
                None,
                row_count,
                detail,
            );
        }
    };
    if source_commit.is_empty() {
        return hook_identity_report(
            HookIdentityStatus::SourceCommitUnresolvable,
            hook_path,
            None,
            row_count,
            "git resolved an empty HEAD commit",
        );
    }

    let current = match head_hook_source_manifest(repo, &source_commit) {
        Ok(manifest) => manifest,
        Err(detail) => {
            return hook_identity_report(
                HookIdentityStatus::SourceCommitUnresolvable,
                hook_path,
                Some(source_commit),
                row_count,
                detail,
            );
        }
    };
    let difference = hook_digest_authority::diff_manifests(&stamped, &current);
    if !difference.is_empty() {
        return hook_identity_report(
            HookIdentityStatus::ContentMismatch,
            hook_path,
            Some(source_commit),
            row_count,
            difference.summary(),
        );
    }
    hook_identity_report(
        HookIdentityStatus::ExactMatch,
        hook_path,
        Some(source_commit),
        row_count,
        "installed hook source manifest matches repository HEAD",
    )
}

fn build_manifest(repo_root: &Path) -> Result<InceptionManifest, InceptionError> {
    let canonical =
        repo_root
            .canonicalize()
            .map_err(|error| InceptionError::RepositoryUnreadable {
                path: repo_root.to_owned(),
                detail: error.to_string(),
            })?;
    if !canonical.is_dir() {
        return Err(InceptionError::RepositoryUnreadable {
            path: canonical,
            detail: "repository path is not a directory".to_owned(),
        });
    }

    let control_files: BTreeMap<String, bool> = control_file_presence(&canonical);
    let missing: Vec<String> = control_files
        .iter()
        .filter_map(|(path, present)| (!present).then_some(path.clone()))
        .collect();
    if !missing.is_empty() {
        return Err(InceptionError::MissingControlFiles(missing));
    }
    let policy_path = canonical.join("AGENTS.md");
    let policy_bytes = fs::read(&policy_path).map_err(|error| InceptionError::RepositoryUnreadable {
        path: policy_path.clone(),
        detail: format!("AGENTS.md policy hash failed: {error}"),
    })?;
    let policy_sha256 = sha256_hex(&policy_bytes);

    let canonical_path = canonical.display().to_string();
    let source_revision = source_revision(&canonical)?;
    let host_identity = host_identity()?;
    let project_id = identity_field("project_id", &project_id(&canonical_path))?;
    Ok(InceptionManifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        project_id,
        repo_identity: RepoIdentity {
            canonical_path,
            git_marker: git_marker(&canonical)?,
            source_revision,
            host_identity,
        },
        control_files,
        host_capabilities: HostCapabilities {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            filesystem: "local".to_owned(),
        },
        required_tools: REQUIRED_TOOLS
            .iter()
            .map(|tool| (*tool).to_owned())
            .collect(),
        epistemic: EpistemicLedger::inception_default(),
        trust_status: TrustStatus {
            status: "unverified".to_owned(),
            reason_code: "TRUST_DECISION_REQUIRED".to_owned(),
            policy_sha256,
            control_files_complete: true,
        },
        template_identity: None,
    })
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                write!(escaped, "\\u{:04x}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn render_repo_identity(output: &mut String, identity: &RepoIdentity) {
    writeln!(output, "  \"repo_identity\": {{").expect("writing to String cannot fail");
    writeln!(output, "    \"canonical_path\": {},", json_string(&identity.canonical_path))
        .expect("writing to String cannot fail");
    writeln!(output, "    \"git_marker\": {},", json_string(&identity.git_marker))
        .expect("writing to String cannot fail");
    writeln!(output, "    \"source_revision\": {},", json_string(&identity.source_revision))
        .expect("writing to String cannot fail");
    writeln!(output, "    \"host_identity\": {}", json_string(&identity.host_identity))
        .expect("writing to String cannot fail");
    writeln!(output, "  }},").expect("writing to String cannot fail");
}


fn render_template_identity(output: &mut String, identity: &TemplateIdentity) {
    writeln!(output, "  \"template_identity\": {{").expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"canonical_path\": {},",
        json_string(&identity.canonical_path)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"source_sha256\": {},",
        json_string(&identity.source_sha256)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"source_revision\": {}",
        json_string(&identity.source_revision)
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  }},").expect("writing to String cannot fail");
}

fn render_manifest(manifest: &InceptionManifest) -> String {
    let mut output = String::from("{\n");
    writeln!(
        output,
        "  \"schema_version\": {},",
        json_string(&manifest.schema_version)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "  \"project_id\": {},",
        json_string(&manifest.project_id)
    )
    .expect("writing to String cannot fail");
    render_repo_identity(&mut output, &manifest.repo_identity);
    if let Some(identity) = &manifest.template_identity {
        render_template_identity(&mut output, identity);
    }

    writeln!(output, "  \"control_files\": {{").expect("writing to String cannot fail");
    for (index, (path, present)) in manifest.control_files.iter().enumerate() {
        let comma = if index + 1 == manifest.control_files.len() {
            ""
        } else {
            ","
        };
        writeln!(output, "    {}: {present}{comma}", json_string(path))
            .expect("writing to String cannot fail");
    }
    writeln!(output, "  }},").expect("writing to String cannot fail");

    writeln!(output, "  \"host_capabilities\": {{").expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"os\": {},",
        json_string(&manifest.host_capabilities.os)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"arch\": {},",
        json_string(&manifest.host_capabilities.arch)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"filesystem\": {}",
        json_string(&manifest.host_capabilities.filesystem)
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  }},").expect("writing to String cannot fail");

    writeln!(output, "  \"required_tools\": [").expect("writing to String cannot fail");
    for (index, tool) in manifest.required_tools.iter().enumerate() {
        let comma = if index + 1 == manifest.required_tools.len() {
            ""
        } else {
            ","
        };
        writeln!(output, "    {}{comma}", json_string(tool))
            .expect("writing to String cannot fail");
    }
    writeln!(output, "  ],").expect("writing to String cannot fail");
    writeln!(
        output,
        "  \"epistemic\": {},",
        serde_json::to_string(&manifest.epistemic).expect("epistemic ledger is serializable")
    )
    .expect("writing to String cannot fail");

    writeln!(output, "  \"trust_status\": {{").expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"status\": {},",
        json_string(&manifest.trust_status.status)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"reason_code\": {},",
        json_string(&manifest.trust_status.reason_code)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"policy_sha256\": {},",
        json_string(&manifest.trust_status.policy_sha256)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"control_files_complete\": {}",
        manifest.trust_status.control_files_complete
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  }}").expect("writing to String cannot fail");
    output.push_str("}\n");
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    hex
}

fn snapshot_existing(path: &Path) -> Result<Option<PathBuf>, InceptionError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| InceptionError::Write {
        path: path.to_owned(),
        detail: format!("backup read failed: {error}"),
    })?;
    let parent = path.parent().ok_or_else(|| InceptionError::Write {
        path: path.to_owned(),
        detail: "output has no parent directory".to_owned(),
    })?;
    let backup_dir = parent.join("backups");
    fs::create_dir_all(&backup_dir).map_err(|error| InceptionError::Write {
        path: backup_dir.clone(),
        detail: format!("backup directory failed: {error}"),
    })?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("inception.json");
    let backup = backup_dir.join(format!("{filename}.{}.bak", sha256_hex(&bytes)));
    if backup.exists() {
        let existing = fs::read(&backup).map_err(|error| InceptionError::Write {
            path: backup.clone(),
            detail: format!("backup verification failed: {error}"),
        })?;
        if existing != bytes {
            return Err(InceptionError::Write {
                path: backup,
                detail: "existing backup content differs from target".to_owned(),
            });
        }
        return Ok(Some(backup));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|error| InceptionError::Write {
            path: backup.clone(),
            detail: format!("backup create failed: {error}"),
        })?;
    file.write_all(&bytes).map_err(|error| InceptionError::Write {
        path: backup.clone(),
        detail: format!("backup write failed: {error}"),
    })?;
    file.sync_all().map_err(|error| InceptionError::Write {
        path: backup.clone(),
        detail: format!("backup fsync failed: {error}"),
    })?;
    drop(file);
    File::open(&backup_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| InceptionError::Write {
            path: backup_dir,
            detail: format!("backup parent fsync failed: {error}"),
        })?;
    Ok(Some(backup))
}

/// One content-keyed backup of the inception artifact.
///
/// The name carries the SHA-256 of the CONTENT, not a timestamp
/// (`snapshot_existing` builds `{filename}.{sha256}.bak`). That is deliberate — it makes a
/// duplicate backup a no-op instead of an accumulating pile — and it has a consequence the
/// restore path must respect: **content-keyed backups have no order.** There is no
/// "latest" to resolve, so a restore that picks one when several exist would be guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub path: PathBuf,
    /// SHA-256 of the backup's bytes, as recorded in its filename.
    pub content_sha: String,
}

/// What a restore did, or would have done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// `0` when the artifact already matched the backup byte for byte.
    pub actions: usize,
    pub restored_from: Option<PathBuf>,
    pub content_sha: String,
}

/// Every backup of `output`, sorted by content hash for determinism.
///
/// # Errors
///
/// Fails only if the backup directory exists and cannot be read. A MISSING directory is
/// an empty list, not an error: never initialised and nothing-to-restore are the same
/// observable state here, and the caller is the one positioned to type that refusal.
pub fn list_backups(output: &Path) -> Result<Vec<BackupEntry>, InceptionError> {
    let Some(parent) = output.parent() else {
        return Ok(Vec::new());
    };
    let backup_dir = parent.join("backups");
    if !backup_dir.is_dir() {
        return Ok(Vec::new());
    }
    let filename = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("inception.json");
    let prefix = format!("{filename}.");
    let entries = fs::read_dir(&backup_dir).map_err(|error| InceptionError::Write {
        path: backup_dir.clone(),
        detail: format!("backup listing failed: {error}"),
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| InceptionError::Write {
            path: backup_dir.clone(),
            detail: format!("backup entry unreadable: {error}"),
        })?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        let Some(sha) = rest.strip_suffix(".bak") else {
            continue;
        };
        // Own the sha BEFORE moving `path`: `sha` borrows through `name`, which borrows
        // `path`, so constructing the struct with `path` first is E0505. Same shape as the
        // liveness.rs:86 error diagnosed for %7 tonight -- bind the derived value, then move.
        let content_sha = sha.to_owned();
        found.push(BackupEntry { path, content_sha });
    }
    found.sort_by(|left, right| left.content_sha.cmp(&right.content_sha));
    Ok(found)
}

/// Restore `output` from `entry`, through the SAME atomic write the writer uses.
///
/// Idempotent: when the artifact already matches the backup, `actions` is `0` and nothing
/// is written. That mirrors the writer's own property rather than re-implementing it.
///
/// # Errors
///
/// Refuses a backup whose bytes do not hash to the SHA in its own filename. A corrupted
/// backup restored silently would be worse than no restore at all — the operator would
/// believe the artifact had been recovered.
pub fn restore_backup(
    output: &Path,
    entry: &BackupEntry,
) -> Result<RestoreReport, InceptionError> {
    let bytes = fs::read(&entry.path).map_err(|error| InceptionError::Write {
        path: entry.path.clone(),
        detail: format!("backup read failed: {error}"),
    })?;
    let actual = sha256_hex(&bytes);
    if actual != entry.content_sha {
        return Err(InceptionError::Write {
            path: entry.path.clone(),
            detail: format!(
                "backup integrity failed: filename claims {} but content hashes {actual}",
                entry.content_sha
            ),
        });
    }
    if fs::read(output).is_ok_and(|current| current == bytes) {
        return Ok(RestoreReport {
            actions: 0,
            restored_from: None,
            content_sha: actual,
        });
    }
    write_atomic(output, &bytes)?;
    Ok(RestoreReport {
        actions: 1,
        restored_from: Some(entry.path.clone()),
        content_sha: actual,
    })
}

fn temporary_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("inception.json");
    path.with_file_name(format!(".{filename}.{}.{}.tmp", std::process::id(), stamp))
}

/// One durability effect of an atomic replace, in the order it completed.
///
/// THE ORDER IS THE LAW. The staging file's bytes must be on stable storage
/// BEFORE the rename publishes them (q7jz: renaming an unsynced file is a torn
/// write), and the parent directory entry must be fsynced AFTER it (u7qq: the
/// entry is not durable until the parent is synced).
///
/// Neither order was observable. No failure can be injected BETWEEN the write
/// and the rename through the public API -- that is a real property of the
/// seam, not a gap in the tests -- and the remote lane runs as ROOT, so every
/// permission-based injection silently succeeds there. So the seam is MADE
/// here instead of worked around: each value is CONSTRUCTED BY THE SYSCALL
/// THAT PERFORMED IT and returned only on its success. A recorded effect
/// therefore cannot exist without its syscall having happened, and deleting
/// the fsync deletes its record with it. The vector is not a log written
/// beside the work; it is the work's return value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicWriteEffect {
    /// The staging file was created exclusively, beside the destination.
    StageCreated(PathBuf),
    /// This many bytes reached the staging fd.
    BytesWritten(usize),
    /// `fsync(2)` on the STAGING FILE's fd returned success.
    FileFsynced(PathBuf),
    /// `rename(2)` published the staging file over the destination.
    Renamed { from: PathBuf, to: PathBuf },
    /// `fsync(2)` on the PARENT DIRECTORY's fd returned success.
    ParentFsynced(PathBuf),
}

fn stage_create(temporary: &Path) -> Result<(File, AtomicWriteEffect), InceptionError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|error| InceptionError::Write {
            path: temporary.to_owned(),
            detail: error.to_string(),
        })?;
    Ok((file, AtomicWriteEffect::StageCreated(temporary.to_owned())))
}

fn stage_write(
    file: &mut File,
    temporary: &Path,
    bytes: &[u8],
) -> Result<AtomicWriteEffect, InceptionError> {
    file.write_all(bytes)
        .map_err(|error| InceptionError::Write {
            path: temporary.to_owned(),
            detail: error.to_string(),
        })?;
    Ok(AtomicWriteEffect::BytesWritten(bytes.len()))
}

/// q7jz. The effect is the fsync's own success value, so it cannot be reported
/// without the fsync having returned Ok.
fn stage_fsync(file: &File, temporary: &Path) -> Result<AtomicWriteEffect, InceptionError> {
    file.sync_all().map_err(|error| InceptionError::Write {
        path: temporary.to_owned(),
        detail: error.to_string(),
    })?;
    Ok(AtomicWriteEffect::FileFsynced(temporary.to_owned()))
}

fn publish_rename(temporary: &Path, path: &Path) -> Result<AtomicWriteEffect, InceptionError> {
    fs::rename(temporary, path).map_err(|error| InceptionError::Write {
        path: path.to_owned(),
        detail: error.to_string(),
    })?;
    Ok(AtomicWriteEffect::Renamed {
        from: temporary.to_owned(),
        to: path.to_owned(),
    })
}

/// u7qq. Cites beads_rust sync/mod.rs:507-509: the directory entry is not
/// durable until the parent is fsynced, so this runs AFTER the rename, never
/// before it.
#[cfg(unix)]
fn parent_fsync(parent: &Path) -> Result<AtomicWriteEffect, InceptionError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| InceptionError::Write {
            path: parent.to_owned(),
            detail: format!("parent fsync failed: {error}"),
        })?;
    Ok(AtomicWriteEffect::ParentFsynced(parent.to_owned()))
}

/// Atomic replace, returning the ordered durability effects it performed.
///
/// This is the ONLY write path -- [`write_atomic`] is a thin discard of the
/// return value -- so an observer of the vector is observing production, not a
/// parallel test implementation.
///
/// # Errors
/// [`InceptionError::Write`] naming the path whose syscall failed. A failure
/// at any step removes the staging file rather than abandoning it.
pub fn write_atomic_observed(
    path: &Path,
    bytes: &[u8],
) -> Result<Vec<AtomicWriteEffect>, InceptionError> {
    let parent = path.parent().ok_or_else(|| InceptionError::Write {
        path: path.to_owned(),
        detail: "output has no parent directory".to_owned(),
    })?;
    fs::create_dir_all(parent).map_err(|error| InceptionError::Write {
        path: parent.to_owned(),
        detail: error.to_string(),
    })?;

    let temporary = temporary_path(path);
    let result = (|| {
        let mut effects = Vec::with_capacity(5);
        let (mut file, created) = stage_create(&temporary)?;
        effects.push(created);
        effects.push(stage_write(&mut file, &temporary, bytes)?);
        effects.push(stage_fsync(&file, &temporary)?);
        drop(file);
        effects.push(publish_rename(&temporary, path)?);
        #[cfg(unix)]
        effects.push(parent_fsync(parent)?);
        Ok(effects)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), InceptionError> {
    write_atomic_observed(path, bytes).map(|_| ())
}

#[derive(Debug)]
enum ReadbackValidationError {
    Malformed(String),
    MissingKey(String),
    ExtraKey(String),
    WrongType {
        key: String,
        expected: &'static str,
        found: &'static str,
    },
    Empty(String),
    Invalid { key: String, detail: String },
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn reject_extra_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    prefix: &str,
) -> Result<(), ReadbackValidationError> {
    let mut extras: Vec<String> = object
        .keys()
        .filter(|key| !allowed.iter().any(|allowed| *allowed == key.as_str()))
        .map(|key| format!("{prefix}{key}"))
        .collect();
    extras.sort();
    extras
        .into_iter()
        .next()
        .map_or(Ok(()), |key| Err(ReadbackValidationError::ExtraKey(key)))
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<&'a Value, ReadbackValidationError> {
    object
        .get(field)
        .ok_or_else(|| ReadbackValidationError::MissingKey(key.to_owned()))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<&'a str, ReadbackValidationError> {
    let value = required_value(object, field, key)?;
    match value {
        Value::String(text) if !text.trim().is_empty() => Ok(text),
        Value::String(_) => Err(ReadbackValidationError::Empty(key.to_owned())),
        other => Err(ReadbackValidationError::WrongType {
            key: key.to_owned(),
            expected: "string",
            found: value_type(other),
        }),
    }
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<&'a Map<String, Value>, ReadbackValidationError> {
    let value = required_value(object, field, key)?;
    value
        .as_object()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: key.to_owned(),
            expected: "object",
            found: value_type(value),
        })
}

fn required_bool(
    object: &Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<bool, ReadbackValidationError> {
    let value = required_value(object, field, key)?;
    value
        .as_bool()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: key.to_owned(),
            expected: "boolean",
            found: value_type(value),
        })
}

fn parse_epistemic_ledger(
    object: &Map<String, Value>,
) -> Result<EpistemicLedger, ReadbackValidationError> {
    let value = required_value(object, "epistemic", "epistemic")?;
    let ledger = value.as_object().ok_or_else(|| ReadbackValidationError::WrongType {
        key: "epistemic".to_owned(),
        expected: "object",
        found: value_type(value),
    })?;
    reject_extra_keys(ledger, &["known", "unknown", "gaps"], "epistemic.")?;

    for (category, fields) in [
        ("known", &["claim", "evidence_source", "evidence_command"][..]),
        ("unknown", &["question", "owner", "resolving_experiment", "cost"][..]),
        ("gaps", &["missing_capability", "owner", "cost_if_left_open"][..]),
    ] {
        let array_value = required_value(ledger, category, &format!("epistemic.{category}"))?;
        let array = array_value.as_array().ok_or_else(|| ReadbackValidationError::WrongType {
            key: format!("epistemic.{category}"),
            expected: "array",
            found: value_type(array_value),
        })?;
        for (index, item) in array.iter().enumerate() {
            let item_object = item.as_object().ok_or_else(|| ReadbackValidationError::WrongType {
                key: format!("epistemic.{category}[{index}]"),
                expected: "object",
                found: value_type(item),
            })?;
            reject_extra_keys(
                item_object,
                fields,
                &format!("epistemic.{category}[{index}]."),
            )?;
        }
    }

    let parsed: EpistemicLedger = serde_json::from_value(value.clone()).map_err(|error| {
        ReadbackValidationError::Invalid {
            key: "epistemic".to_owned(),
            detail: format!("INCEPTION_EPISTEMIC_SCHEMA detail={error}"),
        }
    })?;
    parsed.validate().map_err(|error| {
        let key = match &error {
            EpistemicValidationError::EmptyLedger => "epistemic".to_owned(),
            EpistemicValidationError::BlankField { category, index, field } => {
                format!("epistemic.{category}[{index}].{field}")
            }
        };
        ReadbackValidationError::Invalid {
            key,
            detail: error.to_string(),
        }
    })?;
    Ok(parsed)
}

fn validate_readback(contents: &str) -> Result<InceptionReadback, ReadbackValidationError> {
    let value: Value = serde_json::from_str(contents)
        .map_err(|error| ReadbackValidationError::Malformed(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: "$".to_owned(),
            expected: "object",
            found: value_type(&value),
        })?;
    let allowed: Vec<&str> = REQUIRED_KEYS
        .iter()
        .chain(OPTIONAL_KEYS.iter())
        .copied()
        .collect();
    reject_extra_keys(object, &allowed, "")?;

    let schema_version = required_string(object, "schema_version", "schema_version")?;
    if schema_version != SCHEMA_VERSION {
        return Err(ReadbackValidationError::Invalid {
            key: "schema_version".to_owned(),
            detail: format!("expected={SCHEMA_VERSION} found={schema_version}"),
        });
    }
    let project_id = required_string(object, "project_id", "project_id")?.to_owned();

    let identity = required_object(object, "repo_identity", "repo_identity")?;
    reject_extra_keys(
        identity,
        &["canonical_path", "git_marker", "source_revision", "host_identity"],
        "repo_identity.",
    )?;
    let repo_identity = RepoIdentity {
        canonical_path: required_string(
            identity,
            "canonical_path",
            "repo_identity.canonical_path",
        )?
        .to_owned(),
        git_marker: required_string(identity, "git_marker", "repo_identity.git_marker")?
            .to_owned(),
        source_revision: required_string(
            identity,
            "source_revision",
            "repo_identity.source_revision",
        )?
        .to_owned(),
        host_identity: required_string(identity, "host_identity", "repo_identity.host_identity")?
            .to_owned(),
    };

    let template_identity = match object.get("template_identity") {
        None => None,
        Some(_) => {
            let template = required_object(object, "template_identity", "template_identity")?;
            reject_extra_keys(
                template,
                &["canonical_path", "source_sha256", "source_revision"],
                "template_identity.",
            )?;
            let canonical_path = required_string(
                template,
                "canonical_path",
                "template_identity.canonical_path",
            )?
            .to_owned();
            let source_sha256 = required_string(
                template,
                "source_sha256",
                "template_identity.source_sha256",
            )?
            .to_owned();
            if source_sha256.len() != 64
                || !source_sha256
                    .chars()
                    .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'))
            {
                return Err(ReadbackValidationError::Invalid {
                    key: "template_identity.source_sha256".to_owned(),
                    detail: "expected 64 lowercase hexadecimal characters".to_owned(),
                });
            }
            let source_revision = required_string(
                template,
                "source_revision",
                "template_identity.source_revision",
            )?
            .to_owned();
            if !(7..=64).contains(&source_revision.len())
                || !source_revision.chars().all(|character| character.is_ascii_hexdigit())
            {
                return Err(ReadbackValidationError::Invalid {
                    key: "template_identity.source_revision".to_owned(),
                    detail: "expected 7..=64 hexadecimal characters".to_owned(),
                });
            }
            Some(TemplateIdentity {
                canonical_path,
                source_sha256,
                source_revision,
            })
        }
    };

    let control_files = required_object(object, "control_files", "control_files")?;
    reject_extra_keys(control_files, REQUIRED_CONTROL_FILES, "control_files.")?;
    for relative in REQUIRED_CONTROL_FILES {
        let key = format!("control_files.{relative}");
        if !required_bool(control_files, relative, &key)? {
            return Err(ReadbackValidationError::Invalid {
                key,
                detail: "expected=true".to_owned(),
            });
        }
    }

    let epistemic = parse_epistemic_ledger(object)?;

    let host = required_object(object, "host_capabilities", "host_capabilities")?;
    reject_extra_keys(host, &["os", "arch", "filesystem"], "host_capabilities.")?;
    for field in ["os", "arch", "filesystem"] {
        let key = format!("host_capabilities.{field}");
        required_string(host, field, &key)?;
    }

    let tools_value = required_value(object, "required_tools", "required_tools")?;
    let tools = tools_value
        .as_array()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: "required_tools".to_owned(),
            expected: "array",
            found: value_type(tools_value),
        })?;
    let mut seen = BTreeSet::new();
    for (index, value) in tools.iter().enumerate() {
        let key = format!("required_tools[{index}]");
        let tool = match value {
            Value::String(tool) if !tool.trim().is_empty() => tool,
            Value::String(_) => return Err(ReadbackValidationError::Empty(key)),
            other => {
                return Err(ReadbackValidationError::WrongType {
                    key,
                    expected: "string",
                    found: value_type(other),
                });
            }
        };
        if !REQUIRED_TOOLS.contains(&tool.as_str()) || !seen.insert(tool.to_owned()) {
            return Err(ReadbackValidationError::ExtraKey(key));
        }
    }
    for tool in REQUIRED_TOOLS {
        if !seen.contains(*tool) {
            return Err(ReadbackValidationError::MissingKey(format!("required_tools.{tool}")));
        }
    }

    let trust = required_object(object, "trust_status", "trust_status")?;
    reject_extra_keys(
        trust,
        &["status", "reason_code", "policy_sha256", "control_files_complete"],
        "trust_status.",
    )?;
    for field in ["status", "reason_code", "policy_sha256"] {
        let key = format!("trust_status.{field}");
        required_string(trust, field, &key)?;
    }
    if !required_bool(trust, "control_files_complete", "trust_status.control_files_complete")? {
        return Err(ReadbackValidationError::Invalid {
            key: "trust_status.control_files_complete".to_owned(),
            detail: "expected=true".to_owned(),
        });
    }

    Ok(InceptionReadback {
        project_id,
        repo_identity,
        control_files_complete: true,
        epistemic,
        template_identity,
    })
}

/// Read back the declared `required_tools` set from an on-disk inception
/// artifact. Separate from [`read_inception`] because that readback answers
/// "is this repo's identity intact"; this one answers "which tool set did the
/// artifact actually commit to", which L1's doctor report must carry rather
/// than restate from its own constant.
///
/// Refuses a partial set: a tool missing from the file is reported, never
/// silently backfilled from [`REQUIRED_TOOLS`].
pub fn read_required_tools(output: &Path) -> Result<Vec<String>, InceptionError> {
    let contents = match fs::read_to_string(output) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(InceptionError::ReadbackMissing {
                path: output.to_owned(),
            });
        }
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: error.to_string(),
            });
        }
    };
    let value: Value =
        serde_json::from_str(&contents).map_err(|error| InceptionError::ReadbackMalformed {
            path: output.to_owned(),
            detail: error.to_string(),
        })?;
    let array = value
        .get("required_tools")
        .and_then(Value::as_array)
        .ok_or_else(|| InceptionError::ReadbackMissingKey {
            path: output.to_owned(),
            key: "required_tools".to_owned(),
        })?;
    let mut tools = Vec::with_capacity(array.len());
    for (index, entry) in array.iter().enumerate() {
        match entry.as_str() {
            Some(tool) if !tool.trim().is_empty() => tools.push(tool.to_owned()),
            _ => {
                return Err(InceptionError::ReadbackWrongType {
                    path: output.to_owned(),
                    key: format!("required_tools[{index}]"),
                    expected: "string",
                    found: "non-string or empty",
                });
            }
        }
    }
    for tool in REQUIRED_TOOLS {
        if !tools.iter().any(|seen| seen == tool) {
            return Err(InceptionError::ReadbackMissingKey {
                path: output.to_owned(),
                key: format!("required_tools.{tool}"),
            });
        }
    }
    Ok(tools)
}

pub fn read_inception(output: &Path) -> Result<InceptionReadback, InceptionError> {
    let contents = match fs::read_to_string(output) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(InceptionError::ReadbackMissing {
                path: output.to_owned(),
            });
        }
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: error.to_string(),
            });
        }
    };
    validate_readback(&contents).map_err(|error| match error {
        ReadbackValidationError::Malformed(detail) => InceptionError::ReadbackMalformed {
            path: output.to_owned(),
            detail,
        },
        ReadbackValidationError::MissingKey(key) => InceptionError::ReadbackMissingKey {
            path: output.to_owned(),
            key,
        },
        ReadbackValidationError::ExtraKey(key) => InceptionError::ReadbackExtraKey {
            path: output.to_owned(),
            key,
        },
        ReadbackValidationError::WrongType { key, expected, found } => {
            InceptionError::ReadbackWrongType {
                path: output.to_owned(),
                key,
                expected,
                found,
            }
        }
        ReadbackValidationError::Empty(key) => InceptionError::ReadbackEmpty {
            path: output.to_owned(),
            key,
        },
        ReadbackValidationError::Invalid { key, detail } => InceptionError::ReadbackInvalid {
            path: output.to_owned(),
            key,
            detail,
        },
    })
}


fn emit_stage_event(
    repo_root: &Path,
    layer: Layer,
    stage_from: &str,
    stage_to: &str,
    actor: &str,
    reason: &str,
) -> Result<usize, InceptionError> {
    let journal_path = default_repo_journal(repo_root);
    let journal = DurableJournal::open(&journal_path).map_err(|error| InceptionError::Readback {
        path: journal_path.clone(),
        detail: format!("lifecycle journal open failed: {error}"),
    })?;
    let code = ReasonCode::new(reason).map_err(|error| InceptionError::Readback {
        path: journal_path.clone(),
        detail: error.to_string(),
    })?;
    let event = LifecycleEvent::new(layer, stage_from, stage_to, actor, EmitOutcome::Emitted, code);
    let readback = lifecycle_event::emit_one_host(&journal, event).map_err(|error| {
        InceptionError::Readback {
            path: journal_path,
            detail: format!("lifecycle event emit failed: {error}"),
        }
    })?;
    Ok(readback.lines)
}

fn emit_init_event(repo_root: &Path) -> Result<usize, InceptionError> {
    emit_stage_event(repo_root, Layer::L2, "S1.L1", "S1.L2", "ompo-init", "INIT_REPROBE_OK")
}

/// Ownership anchor: an AGENTS.md that does not name this repository is foreign.
/// Heuristic, stated plainly: content cannot prove ownership, so a foreign read
/// refuses loudly unless the caller supplies an explicit typed consent record.
pub const PROJECT_AGENTS_OWNERSHIP_STAMP: &str = "omp-orchestrator";

/// Trust gate (cbl7/jlna): resolve owned policy or validate a caller-supplied
/// consent record. Missing control files are refused by build_manifest first;
/// no environment variable, global flag, or mutable side file is consulted.
fn require_full_template_input(input: &InputManifest) -> Result<(), TemplateIdentityError> {
    match input {
        InputManifest::Full => Ok(()),
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => Err(TemplateIdentityError::PartialInput {
            bound_kind: bound_kind.clone(),
            bound_value: *bound_value,
            source: source.clone(),
        }),
        InputManifest::Refused { reason } => Err(TemplateIdentityError::RefusedInput {
            reason: reason.clone(),
        }),
    }
}

fn revision_and_source_at_head(
    authority_root: &Path,
    canonical_template: &Path,
) -> Result<(String, Vec<u8>), TemplateIdentityError> {
    let mut revision_command = Command::new("git");
    revision_command
        .arg("-C")
        .arg(authority_root)
        .args(["rev-parse", "HEAD"]);
    let revision_output = run_command_output_typed(&mut revision_command).map_err(|error| {
        TemplateIdentityError::RevisionUnresolvable {
            path: canonical_template.to_owned(),
            detail: error.to_string(),
        }
    })?;
    let revision = String::from_utf8(revision_output.stdout)
        .map_err(|error| TemplateIdentityError::Malformed {
            field: "source_revision",
            detail: error.to_string(),
        })?
        .trim()
        .to_owned();
    if !(7..=64).contains(&revision.len())
        || !revision.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err(TemplateIdentityError::Malformed {
            field: "source_revision",
            detail: format!("expected 7..=64 hexadecimal characters, found={revision:?}"),
        });
    }
    let relative = canonical_template
        .strip_prefix(authority_root)
        .map_err(|_| TemplateIdentityError::AuthorityRootEscape {
            authority_root: authority_root.to_owned(),
            template_path: canonical_template.to_owned(),
        })?;
    let relative = relative.to_str().ok_or_else(|| TemplateIdentityError::Malformed {
        field: "canonical_path",
        detail: "template path is not UTF-8".to_owned(),
    })?;
    let object = format!("{revision}:{relative}");
    let mut show_command = Command::new("git");
    show_command
        .arg("-C")
        .arg(authority_root)
        .args(["show", object.as_str()]);
    let output = run_command_output_typed(&mut show_command).map_err(|error| {
        TemplateIdentityError::RevisionUnresolvable {
            path: canonical_template.to_owned(),
            detail: error.to_string(),
        }
    })?;
    Ok((revision, output.stdout))
}

pub fn capture_template_identity(
    authority_root: &Path,
    template_path: &Path,
    input: &InputManifest,
) -> Result<TemplateIdentity, TemplateIdentityError> {
    require_full_template_input(input)?;
    let authority_root = authority_root
        .canonicalize()
        .map_err(|error| TemplateIdentityError::Malformed {
            field: "authority_root",
            detail: error.to_string(),
        })?;
    let canonical_template = template_path.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            TemplateIdentityError::Missing {
                path: template_path.to_owned(),
            }
        } else {
            TemplateIdentityError::Unreadable {
                path: template_path.to_owned(),
                detail: error.to_string(),
            }
        }
    })?;
    if !canonical_template.starts_with(&authority_root) {
        return Err(TemplateIdentityError::AuthorityRootEscape {
            authority_root,
            template_path: canonical_template,
        });
    }
    let source = fs::read(&canonical_template).map_err(|error| TemplateIdentityError::Unreadable {
        path: canonical_template.clone(),
        detail: error.to_string(),
    })?;
    if source.is_empty() {
        return Err(TemplateIdentityError::Empty {
            path: canonical_template,
        });
    }
    let canonical_path = canonical_template
        .to_str()
        .ok_or_else(|| TemplateIdentityError::Malformed {
            field: "canonical_path",
            detail: "template path is not UTF-8".to_owned(),
        })?
        .to_owned();
    let source_sha256 = sha256_hex(&source);
    let (source_revision, committed_source) =
        revision_and_source_at_head(&authority_root, &canonical_template)?;
    if committed_source != source {
        return Err(TemplateIdentityError::SourceChanged {
            path: canonical_template,
            expected_sha256: sha256_hex(&committed_source),
            actual_sha256: source_sha256,
            expected_revision: source_revision.clone(),
            actual_revision: source_revision,
        });
    }
    Ok(TemplateIdentity {
        canonical_path,
        source_sha256,
        source_revision,
    })
}

pub fn verify_template_identity_current(
    authority_root: &Path,
    identity: &TemplateIdentity,
) -> Result<(), TemplateIdentityError> {
    let canonical_template = PathBuf::from(&identity.canonical_path);
    let authority_root = authority_root
        .canonicalize()
        .map_err(|error| TemplateIdentityError::Malformed {
            field: "authority_root",
            detail: error.to_string(),
        })?;
    if !canonical_template.starts_with(&authority_root) {
        return Err(TemplateIdentityError::AuthorityRootEscape {
            authority_root,
            template_path: canonical_template,
        });
    }
    let source = fs::read(&canonical_template).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            TemplateIdentityError::Missing {
                path: canonical_template.clone(),
            }
        } else {
            TemplateIdentityError::Unreadable {
                path: canonical_template.clone(),
                detail: error.to_string(),
            }
        }
    })?;
    let actual_sha256 = sha256_hex(&source);
    let (actual_revision, committed_source) =
        revision_and_source_at_head(&authority_root, &canonical_template)?;
    if actual_sha256 != identity.source_sha256
        || actual_revision != identity.source_revision
        || committed_source != source
    {
        return Err(TemplateIdentityError::SourceChanged {
            path: canonical_template,
            expected_sha256: identity.source_sha256.clone(),
            actual_sha256,
            expected_revision: identity.source_revision.clone(),
            actual_revision,
        });
    }
    Ok(())
}

fn trusted_init_decision(
    repo_root: &Path,
    consent: Option<&TrustedInitConsent>,
) -> Result<TrustedInitDecision, InceptionError> {
    let canonical =
        repo_root
            .canonicalize()
            .map_err(|error| InceptionError::RepositoryUnreadable {
                path: repo_root.to_owned(),
                detail: format!("canonicalize for trusted-init consent failed: {error}"),
            })?;
    let path = canonical.join("AGENTS.md");
    let bytes = fs::read(&path).map_err(|error| InceptionError::RepositoryUnreadable {
        path: path.clone(),
        detail: format!("AGENTS.md unreadable: {error}"),
    })?;
    let text =
        std::str::from_utf8(&bytes).map_err(|error| InceptionError::RepositoryUnreadable {
            path: path.clone(),
            detail: format!("AGENTS.md is not UTF-8: {error}"),
        })?;
    if text.trim().is_empty() {
        return Err(InceptionError::EmptyAgentsMd { path });
    }
    if text.contains(PROJECT_AGENTS_OWNERSHIP_STAMP) {
        return Ok(TrustedInitDecision::OwnedPolicy);
    }

    let policy_sha256 = sha256_hex(&bytes);
    let Some(consent) = consent else {
        return Err(InceptionError::UntrustedAgentsMd { path });
    };
    let TrustedInitConsent::Explicit {
        decision_id,
        repository_scope,
        source_revision: provided_source_revision,
        policy_sha256: provided_policy_sha256,
        template_path,
        template_input,
    } = consent
    else {
        return Err(InceptionError::TrustedInitConsentMissing {
            path,
            policy_sha256,
        });
    };

    let decision_id_valid = !decision_id.is_empty()
        && decision_id.len() <= 128
        && decision_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        });
    if !decision_id_valid {
        return Err(InceptionError::TrustedInitConsentMalformed {
            field: "decision_id",
            detail: "expected 1..=128 ASCII identifier characters".to_owned(),
        });
    }
    let revision_valid = (7..=64).contains(&provided_source_revision.len())
        && provided_source_revision
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !revision_valid {
        return Err(InceptionError::TrustedInitConsentMalformed {
            field: "source_revision",
            detail: "expected a 7..=64 character hexadecimal revision".to_owned(),
        });
    }
    let digest_valid = provided_policy_sha256.len() == 64
        && provided_policy_sha256
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'));
    if !digest_valid {
        return Err(InceptionError::TrustedInitConsentMalformed {
            field: "policy_sha256",
            detail: "expected a 64 character lowercase hexadecimal SHA-256".to_owned(),
        });
    }
    if repository_scope != &canonical {
        return Err(InceptionError::TrustedInitConsentScopeMismatch {
            expected: canonical,
            provided: repository_scope.clone(),
        });
    }

    let expected_source_revision = source_revision(&canonical)?;
    if provided_source_revision != &expected_source_revision
        || provided_policy_sha256 != &policy_sha256
    {
        return Err(InceptionError::TrustedInitConsentStaleOrReplayed {
            expected_source_revision,
            provided_source_revision: provided_source_revision.clone(),
            expected_policy_sha256: policy_sha256,
            provided_policy_sha256: provided_policy_sha256.clone(),
        });
    }
    let template_identity =
        capture_template_identity(&canonical, template_path, template_input)?;
    Ok(TrustedInitDecision::ExplicitConsent {
        decision_id: decision_id.clone(),
        repository_scope: canonical,
        source_revision: provided_source_revision.clone(),
        policy_sha256,
        template_identity,
    })
}

/// L2-BUILD-AGENTS-STAMP (bead 43x7): AGENTS.md stamp identity with live
/// source revision and a typed status. Reports three independent facts:
/// whether the ownership token is present (a `contains` check against
/// [`PROJECT_AGENTS_OWNERSHIP_STAMP`] -- the token is referenced, never
/// copied, and no line count is pinned), the live HEAD revision when git
/// answers, and the status joining the two. Missing, empty, foreign, and
/// unreadable files all read as unstamped rather than erroring: absence
/// of evidence is a report field, not a refusal (refusal lives in
/// [`verify_agents_ownership`], which this never calls).
///
/// INERT BY DESIGN (rule 9): no caller yet. The L2 trust flow owns
/// adoption; until then this stays available, not invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentsStampStatus {
    Stamped,
    Unstamped,
    GitUnavailable,
}

/// The stamp report: token presence, live revision, joined status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsStampReport {
    pub stamp_present: bool,
    pub source_revision: Option<String>,
    pub status: AgentsStampStatus,
}

/// Probe a control-file stamp and the live source revision: the shared
/// core behind [`agents_stamp_report`] and [`claude_stamp_report`]. One
/// token, one join rule, two filenames -- a second copy would agree with
/// the subject by construction.
pub fn stamp_report(repo: &Path, filename: &str) -> AgentsStampReport {
    let stamped = fs::read_to_string(repo.join(filename))
        .is_ok_and(|text| text.contains(PROJECT_AGENTS_OWNERSHIP_STAMP));
    let source_revision = source_revision(repo).ok();
    let status = match (stamped, &source_revision) {
        (true, Some(_)) => AgentsStampStatus::Stamped,
        (true, None) => AgentsStampStatus::GitUnavailable,
        (false, _) => AgentsStampStatus::Unstamped,
    };
    AgentsStampReport {
        stamp_present: stamped,
        source_revision,
        status,
    }
}

/// Probe the AGENTS.md stamp and the live source revision.
pub fn agents_stamp_report(repo: &Path) -> AgentsStampReport {
    stamp_report(repo, "AGENTS.md")
}

/// Probe the CLAUDE.md stamp and the live source revision. Same token,
/// same join rule as [`agents_stamp_report`]: no second stamp
/// vocabulary, no copied file contents, no pinned line counts.
pub fn claude_stamp_report(repo: &Path) -> AgentsStampReport {
    stamp_report(repo, "CLAUDE.md")
}

/// L2-BUILD-BEADS (bead zb2p): tracker initialization state with project
/// identity. Reports four facts about the `.beads` tracker beside the
/// repo root: the project identity (the id prefix of the first issue
/// row -- the scope `br` files under), whether the required state was
/// readable, whether its permission bits allow writing, and the joined
/// status. Read-only by construction: one directory probe, one file
/// read, one metadata read -- never a second tracker client, never a
/// write. Identity comes from the first row only (see NO-CLAIM in the
/// entry leg); writability is permission-bit evidence, not an access
/// proof (a privileged uid may override bits -- fail-closed by design).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadsInitStatus {
    Ready,
    Missing,
    Unreadable,
    Uninitialized,
    Unwritable,
}

/// The tracker init report: project identity plus state flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeadsInitReport {
    pub project_identity: Option<String>,
    pub readable: bool,
    pub writable: bool,
    pub status: BeadsInitStatus,
}

/// Project identity from one issue row: the id prefix `br` scopes under
/// (`omp-orchestrator-01tzb` -> `omp-orchestrator`). A row without a
/// hyphenated id carries no usable identity.
fn beads_project_identity(first_line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(first_line).ok()?;
    let id = value.get("id")?.as_str()?;
    let (prefix, _) = id.rsplit_once('-')?;
    (!prefix.is_empty()).then(|| prefix.to_owned())
}

/// Probe the `.beads` tracker initialization state: directory presence,
/// required-state readability (the issues file reads and yields a
/// project identity), and permission-bit writability. Missing directory
/// reads as Missing; a missing or empty issues file reads as
/// Uninitialized (no identity to establish); an unreadable or
/// unparseable one reads as Unreadable; denied write bits read as
/// Unwritable. Ready only when all three answer.
pub fn beads_init_report(repo: &Path) -> BeadsInitReport {
    let unready = |status: BeadsInitStatus| BeadsInitReport {
        project_identity: None,
        readable: false,
        writable: false,
        status,
    };
    if !repo.join(".beads").is_dir() {
        return unready(BeadsInitStatus::Missing);
    }
    let file = repo.join(".beads/issues.jsonl");
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return unready(BeadsInitStatus::Uninitialized)
        }
        Err(_) => return unready(BeadsInitStatus::Unreadable),
    };
    let first = match text.lines().find(|line| !line.trim().is_empty()) {
        Some(first) => first,
        None => return unready(BeadsInitStatus::Uninitialized),
    };
    let identity = match beads_project_identity(first) {
        Some(identity) => identity,
        None => return unready(BeadsInitStatus::Unreadable),
    };
    let writable = fs::metadata(&file).is_ok_and(|meta| !meta.permissions().readonly());
    BeadsInitReport {
        project_identity: Some(identity),
        readable: true,
        writable,
        status: if writable {
            BeadsInitStatus::Ready
        } else {
            BeadsInitStatus::Unwritable
        },
    }
}

/// L2-BUILD-RUST-TOOLCHAIN (bead yhia): declared toolchain identity with
/// an active-toolchain match. Reports the `channel` declared in the
/// repo's rust-toolchain.toml plus the live `rustc --version`, joined
/// into a status. Missing file reads as Missing; an unreadable file as
/// Unreadable; a present file with no usable `channel` line as
/// Unparseable (unquoted channel) or Unpinned (absent or empty channel);
/// a declared pin the active toolchain does not satisfy as Mismatched.
/// The channel scan follows the same line rules as the doctor's
/// `read_toolchain_channel` -- trimmed lines, `#` comments skipped,
/// `[toolchain]` section tracked, quoted value -- so the two can never
/// disagree on what a pin IS. The matcher below is the same vocabulary
/// under the same name for the same reason; its canonical home is
/// `crates/ompo-doctor/src/lib.rs`, unreachable from here by dependency
/// direction (the doctor depends on this crate), so the entry carries
/// the gate-side copy rather than a second parser with its own semantics.
///
/// `ompo_doctor_ref`: `read_toolchain_channel` at
/// `crates/ompo-doctor/src/lib.rs:812`, `toolchain_matches_pin` at
/// `:849`. If those move, this comment -- not the semantics -- is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolchainPinStatus {
    Ready,
    Missing,
    Unreadable,
    Unparseable,
    Unpinned,
    Mismatched,
}

/// The toolchain pin report: declared and active identities plus status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainPinReport {
    pub declared: Option<String>,
    pub active: Option<String>,
    pub status: ToolchainPinStatus,
}

/// Whether an active `rustc --version` line satisfies a pinned channel.
/// Same vocabulary and semantics as the doctor's `toolchain_matches_pin`:
/// stable has no marker word; dated nightlies match on the date; semver
/// pins match on the core prefix at a `.` boundary. Fail-closed on
/// garbage: a core that does not start with a digit satisfies nothing.
#[must_use]
pub fn toolchain_matches_pin(channel: &str, active_version: &str) -> bool {
    let core = active_version
        .strip_prefix("rustc ")
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or("");
    if !core.starts_with(|b: char| b.is_ascii_digit()) {
        return false;
    }
    match channel {
        "stable" => !core.contains('-'),
        "beta" => core.contains("-beta"),
        "nightly" => core.contains("-nightly"),
        dated if dated.starts_with("nightly-") => {
            core.contains("-nightly") && active_version.contains(&dated["nightly-".len()..])
        }
        version => core == version || core.starts_with(&format!("{version}.")),
    }
}

/// Declared `channel` from a rust-toolchain.toml body: `Some(channel)`
/// when a usable quoted value is present, `Some("")` when the key is
/// present but empty, `None` when no channel line exists. A present but
/// unquoted channel is malformed -- reported via the boolean arm, never
/// mistaken for a pin.
fn declared_toolchain_channel(text: &str) -> (Option<String>, bool) {
    let mut in_toolchain = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_toolchain = line == "[toolchain]";
            continue;
        }
        if !in_toolchain {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "channel" {
            continue;
        }
        let value = value.trim();
        let Some(quote) = value.chars().next() else {
            return (None, true);
        };
        if quote != '"' && quote != '\'' {
            return (None, true);
        }
        let Some(end) = value[1..].find(quote) else {
            return (None, true);
        };
        return (Some(value[1..1 + end].to_owned()), false);
    }
    (None, false)
}

/// Active toolchain line: `rustc --version` run from the system temp dir
/// (never the repo, whose own pin file would select the toolchain under
/// test through the rustup shim) with any `RUSTUP_TOOLCHAIN` override
/// removed, so the answer is the default toolchain. `None` when rustc
/// cannot be run or its output is empty.
fn active_toolchain_version() -> Option<String> {
    let mut command = Command::new("rustc");
    command
        .current_dir(std::env::temp_dir())
        .env_remove("RUSTUP_TOOLCHAIN")
        .arg("--version");
    match subprocess_contract::bounded_output(&mut command, IDENTITY_COMMAND_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output)
            if output.status.success() =>
        {
            let line = String::from_utf8_lossy(&output.stdout).into_owned();
            (!line.trim().is_empty()).then(|| line.trim_end().to_owned())
        }
        _ => None,
    }
}

/// Probe the repo's declared toolchain pin and its live match: the
/// `channel` in rust-toolchain.toml checked against the active
/// `rustc --version`. The repository pin (`stable`) proceeds on any lane
/// whose default toolchain is stable -- which the pin file itself
/// enforces wherever rustup resolves it. A lane defaulting elsewhere
/// refuses here; that refusal is the gate working, and the report's
/// `active` field names the cause.
pub fn toolchain_pin_report(repo: &Path) -> ToolchainPinReport {
    let text = match fs::read_to_string(repo.join("rust-toolchain.toml")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ToolchainPinReport {
                declared: None,
                active: None,
                status: ToolchainPinStatus::Missing,
            }
        }
        Err(_) => {
            return ToolchainPinReport {
                declared: None,
                active: None,
                status: ToolchainPinStatus::Unreadable,
            }
        }
    };
    let (declared, malformed) = declared_toolchain_channel(&text);
    if malformed {
        return ToolchainPinReport {
            declared: None,
            active: None,
            status: ToolchainPinStatus::Unparseable,
        };
    }
    let Some(channel) = declared else {
        return ToolchainPinReport {
            declared: None,
            active: None,
            status: ToolchainPinStatus::Unpinned,
        };
    };
    if channel.is_empty() {
        return ToolchainPinReport {
            declared: Some(channel),
            active: None,
            status: ToolchainPinStatus::Unpinned,
        };
    }
    let Some(active) = active_toolchain_version() else {
        return ToolchainPinReport {
            declared: Some(channel),
            active: None,
            status: ToolchainPinStatus::Unreadable,
        };
    };
    ToolchainPinReport {
        declared: Some(channel.clone()),
        active: Some(active.clone()),
        status: if toolchain_matches_pin(&channel, &active) {
            ToolchainPinStatus::Ready
        } else {
            ToolchainPinStatus::Mismatched
        },
    }
}
pub fn initialize(repo_root: &Path, output: &Path) -> Result<InitReport, InceptionError> {
    initialize_inner(repo_root, output, None)
}

/// Explicit consent-taking init over a foreign AGENTS.md. The caller supplies
/// repository scope, revision, policy digest, and decision id as one typed value.
pub fn initialize_trusted(
    repo_root: &Path,
    output: &Path,
    consent: &TrustedInitConsent,
) -> Result<InitReport, InceptionError> {
    initialize_inner(repo_root, output, Some(consent))
}

/// L2 entry for operator-driven init: the repository check gates before
/// any downstream L2 state continues, the CLAUDE.md and AGENTS.md stamp
/// checks gate before trust-dependent continuation, the `.beads`
/// tracker init check gates before initialization or dispatch can
/// continue, and the rust-toolchain.toml pin check gates the compiler
/// identity. The explicit Persona A remote policy is then observed and
/// carried before `initialize` runs: no remote is an explicit local-only
/// allowance, a present remote needs no allowance, and an unobservable probe
/// refuses typed. The installed pre-commit hook's canonical source manifest
/// must then match the covered source bytes at repository HEAD; mtime is never
/// consulted. A real repository with stamped control files, a ready tracker,
/// a satisfied pin, and exact hook identity proceeds with its canonical root;
/// any other state refuses typed before `initialize` runs. This lives
/// beside -- never inside --
/// shared [`initialize`]: doctor repair flows tolerate non-git checkouts
/// by design (git identity degrades to "missing"), and gating them would
/// trade measured-green repair legs for zero new capability. The output
/// path stays caller-chosen; only the root canonicalizes.
pub fn initialize_gated(
    repo_root: &Path,
    output: &Path,
    consent: &TrustedInitConsent,
) -> Result<InitReport, InceptionError> {
    let top = git_repo_toplevel(repo_root)?;
    let _cargo_member = cargo_workspace_member_report(&top, &InputManifest::full())?;
    match claude_stamp_report(&top).status {
        AgentsStampStatus::Stamped => {}
        status => {
            return Err(InceptionError::UntrustedClaudeMd {
                path: top.join("CLAUDE.md"),
                status,
            })
        }
    }
    match agents_stamp_report(&top).status {
        AgentsStampStatus::Stamped => {}
        AgentsStampStatus::Unstamped
            if fs::read_to_string(top.join("AGENTS.md"))
                .is_ok_and(|text| !text.trim().is_empty()) => {}
        status => {
            return Err(InceptionError::AgentsStampRefused {
                path: top.join("AGENTS.md"),
                status,
            })
        }
    }
    match beads_init_report(&top).status {
        BeadsInitStatus::Ready => {}
        status => {
            return Err(InceptionError::BeadsInitRefused {
                path: top.join(".beads"),
                status,
            })
        }
    }
    let pin = toolchain_pin_report(&top);
    match pin.status {
        ToolchainPinStatus::Ready => {}
        status => {
            return Err(InceptionError::ToolchainPinRefused {
                path: top.join("rust-toolchain.toml"),
                status,
                declared: pin.declared.clone(),
            })
        }
    }
    // This entry is the local operator bootstrap, so Persona A is explicit at
    // the call site rather than inferred from ambient environment state. The
    // fleet-required policy remains owned by require_remote_for_dispatch.
    let persona_remote = persona_remote_policy(&top, true)?;
    let hook_identity = hook_source_identity_report(&top);
    if hook_identity.status != HookIdentityStatus::ExactMatch {
        return Err(InceptionError::HookIdentityRefused {
            path: hook_identity.hook_path,
            status: hook_identity.status,
            detail: hook_identity.detail,
        });
    }
    let agent_mail_registration = agent_mail_registration_report(&top, &InputManifest::full())?;
    let rch_lane = rch_lane_report(&top, &InputManifest::full())?;
    let mut report = initialize_inner(&top, output, Some(consent))?;
    report.persona_remote = Some(persona_remote);
    report.agent_mail_registration = Some(agent_mail_registration);
    report.rch_lane = Some(rch_lane);
    Ok(report)
}

/// L2-BUILD-REPROBE (2zrz): fresh post-write re-probe of the predicates the
/// pre-write path accepted.
///
/// `read_inception` proves the bytes on disk parse and match the in-memory
/// manifest; it cannot prove the repository still says the same thing. This
/// re-runs the same observations `build_manifest` made before the write --
/// repository identity via `source_revision`, required control-file presence
/// via `control_file_presence` -- against the live repository and refuses on
/// any divergence, BEFORE any success evidence is emitted. A halt therefore
/// leaves zero new `INIT_REPROBE_OK` rows: no success is ever reported from
/// stale pre-write evidence.
///
/// The required-tools predicate needs no fresh observation here: it is a
/// declaration, not a measurement, and `read_inception` already re-validates
/// its membership against the post-write bytes on every path.
///
/// Shared by `initialize_inner` and `write_inception_inner`: one mechanism,
/// no parallel re-probe API.
pub fn verify_post_write_predicates(
    repo_root: &Path,
    manifest: &InceptionManifest,
    output: &Path,
) -> Result<(), InceptionError> {
    let canonical =
        repo_root
            .canonicalize()
            .map_err(|error| InceptionError::RepositoryUnreadable {
                path: repo_root.to_owned(),
                detail: format!("post-write re-probe canonicalize failed: {error}"),
            })?;
    let fresh_canonical_path = canonical.display().to_string();
    if fresh_canonical_path != manifest.repo_identity.canonical_path {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "POST_WRITE_PREDICATE_CHANGED predicate=identity field=canonical_path expected={} provided={}",
                manifest.repo_identity.canonical_path, fresh_canonical_path
            ),
        });
    }
    let fresh_revision = source_revision(&canonical)?;
    if fresh_revision != manifest.repo_identity.source_revision {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "POST_WRITE_PREDICATE_CHANGED predicate=identity field=source_revision expected={} provided={}",
                manifest.repo_identity.source_revision, fresh_revision
            ),
        });
    }
    let fresh_presence = control_file_presence(&canonical);
    let missing: Vec<String> = fresh_presence
        .iter()
        .filter_map(|(path, present)| (!present).then_some(path.clone()))
        .collect();
    if !missing.is_empty() {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "POST_WRITE_PREDICATE_CHANGED predicate=control_files missing={}",
                missing.join(",")
            ),
        });
    }
    Ok(())
}

fn initialize_inner(
    repo_root: &Path,
    output: &Path,
    consent: Option<&TrustedInitConsent>,
) -> Result<InitReport, InceptionError> {
    let mut manifest = build_manifest(repo_root)?;
    let trusted_init = trusted_init_decision(repo_root, consent)?;
    let template_identity = match &trusted_init {
        TrustedInitDecision::OwnedPolicy => None,
        TrustedInitDecision::ExplicitConsent { template_identity, .. } => {
            verify_template_identity_current(repo_root, template_identity)?;
            Some(template_identity.clone())
        }
    };
    manifest.template_identity = template_identity;
    manifest.epistemic.validate()?;
    let bytes = render_manifest(&manifest).into_bytes();
    // ONE pre-state read answers both questions: did the artifact exist, and
    // does its content differ. Reading twice would let the two answers come
    // from two different moments.
    let (preexisting, actions, backup) = match fs::read(output) {
        Ok(existing) if existing == bytes => (true, 0, None),
        Ok(_) => (true, 1, snapshot_existing(output)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (false, 1, None),
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: format!("pre-state read failed: {error}"),
            });
        }
    };
    if actions == 1 {
        write_atomic(output, &bytes)?;
    }
    let readback = read_inception(output)?;
    if readback.project_id != manifest.project_id
        || readback.repo_identity != manifest.repo_identity
    {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "repo_identity changed during initialization: expected={} found={}",
                manifest.repo_identity.canonical_path, readback.repo_identity.canonical_path
            ),
        });
    }
    // L2-BUILD-REPROBE (2zrz): fresh re-probe before any success evidence.
    // A halt here emits nothing: zero new INIT_REPROBE_OK rows.
    verify_post_write_predicates(repo_root, &manifest, output)?;
    let journal_path = default_repo_journal(repo_root);
    let journal_rows = emit_init_event(repo_root)?;
    let monitor_rows =
        verify_artifact(&journal_path).map_err(|error| InceptionError::Readback {
            path: journal_path,
            detail: format!("monitor reread failed: {error}"),
        })?;
    // THE 1:1 LAW, stated where it can be enforced rather than left to a
    // reader of two counts. Superseding pre-existing content without a
    // snapshot is unrecoverable, so it is a typed refusal, not a low ratio.
    let files_mutated = actions;
    let backups_written = usize::from(backup.is_some());
    let backup_ratio_verdict = if preexisting && files_mutated > 0 {
        if backups_written == files_mutated {
            "BACKUP_RATIO_OK_1_TO_1".to_owned()
        } else {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: format!(
                    "BACKUP_RATIO_VIOLATION backups_written={backups_written} files_mutated={files_mutated} preexisting=true"
                ),
            });
        }
    } else if files_mutated > 0 {
        // Virgin write: nothing existed, so there is nothing to snapshot.
        "BACKUP_RATIO_OK_VIRGIN_0_TO_1".to_owned()
    } else {
        "BACKUP_RATIO_OK_NO_MUTATION".to_owned()
    };
    Ok(InitReport {
        manifest,
        persona_remote: None,
        agent_mail_registration: None,
        rch_lane: None,
        trusted_init,
        actions,
        backup,
        journal_rows,
        monitor_rows,
        files_mutated,
        backups_written,
        preexisting,
        backup_ratio_verdict,
    })
}

pub fn write_inception(
    repo_root: &Path,
    output: &Path,
) -> Result<InceptionManifest, InceptionError> {
    write_inception_inner(repo_root, output, None)
}

/// Explicit consent-taking write over a foreign AGENTS.md.
pub fn write_inception_trusted(
    repo_root: &Path,
    output: &Path,
    consent: &TrustedInitConsent,
) -> Result<InceptionManifest, InceptionError> {
    write_inception_inner(repo_root, output, Some(consent))
}

fn write_inception_inner(
    repo_root: &Path,
    output: &Path,
    consent: Option<&TrustedInitConsent>,
) -> Result<InceptionManifest, InceptionError> {
    let mut manifest = build_manifest(repo_root)?;
    let trusted_init = trusted_init_decision(repo_root, consent)?;
    let template_identity = match &trusted_init {
        TrustedInitDecision::OwnedPolicy => None,
        TrustedInitDecision::ExplicitConsent { template_identity, .. } => {
            verify_template_identity_current(repo_root, template_identity)?;
            Some(template_identity.clone())
        }
    };
    manifest.template_identity = template_identity;
    manifest.epistemic.validate()?;
    let bytes = render_manifest(&manifest).into_bytes();
    let should_write = match fs::read(output) {
        Ok(existing) if existing == bytes => false,
        Ok(_) => {
            snapshot_existing(output)?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: format!("pre-state read failed: {error}"),
            });
        }
    };
    if should_write {
        write_atomic(output, &bytes)?;
    }
    let readback = read_inception(output)?;
    if readback.project_id != manifest.project_id
        || readback.repo_identity != manifest.repo_identity
    {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "repo_identity changed during readback: expected={} found={}",
                manifest.repo_identity.canonical_path, readback.repo_identity.canonical_path
            ),
        });
    }
    // L2-BUILD-REPROBE (2zrz): fresh re-probe before any success evidence.
    // A halt here emits nothing: zero new INIT_REPROBE_OK rows.
    verify_post_write_predicates(repo_root, &manifest, output)?;
    if should_write {
        emit_init_event(repo_root)?;
        // L5 writer (5iwj): the write+fsync+readback success chokepoint records one
        // S1.L4 -> S1.L5 row. The source stage matches the supervisor-tick L5 row so
        // journal readers see one consistent S1.L4 -> S1.L5 transition.
        emit_stage_event(
            repo_root,
            Layer::L5,
            "S1.L4",
            "S1.L5",
            "ompo-init",
            "INIT_WRITE_OK",
        )?;
    }
    Ok(manifest)
}

#[cfg(test)]
use tempfile::TempDir;

#[cfg(test)]
fn run_test_git(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {}
        other => panic!("git fixture command failed: {other:?}"),
    }
}

#[cfg(test)]
fn fixture() -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("fixture directory");
    for relative in REQUIRED_CONTROL_FILES {
        let path = directory.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(path, "fixture\n").expect("fixture file");
    }
    // The fixture mimics a stamped repo: its AGENTS.md carries the ownership
    // anchor, so untrusted-path legs exercise the gate while every other leg
    // runs against stamped content. Foreign-content legs overwrite this file.
    fs::write(
        directory.path().join("AGENTS.md"),
        format!("fixture {PROJECT_AGENTS_OWNERSHIP_STAMP}\n"),
    )
    .expect("fixture stamp");
    fs::write(directory.path().join("template.md"), b"template source\n")
        .expect("template source");
    run_test_git(directory.path(), &["init", "-q"]);
    run_test_git(directory.path(), &["add", "."]);
    run_test_git(
        directory.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let output = directory.path().join(".omp-orchestrator/inception.json");
    (directory, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn explicit_consent(root: &Path, decision_id: &str) -> TrustedInitConsent {
        let repository_scope = root.canonicalize().expect("canonical consent scope");
        let source_revision = source_revision(&repository_scope).expect("consent revision");
        let policy_sha256 = sha256_hex(
            &fs::read(repository_scope.join("AGENTS.md")).expect("consent policy bytes"),
        );
        TrustedInitConsent::Explicit {
            decision_id: decision_id.to_owned(),
            repository_scope,
            source_revision,
            policy_sha256,
            template_path: root.join("template.md"),
            template_input: InputManifest::full(),
        }
    }
    #[test]
    fn writes_and_reads_all_required_fields() {
        let (directory, output) = fixture();
        let manifest = write_inception(directory.path(), &output).expect("write inception");
        let contents = fs::read_to_string(&output).expect("read inception");
        validate_readback(&contents).expect("required fields");
        for key in REQUIRED_KEYS {
            assert!(
                contents.contains(&format!("\"{key}\":")),
                "required key {key}"
            );
        }
        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
        assert!(!manifest.project_id.is_empty());
        assert!(!manifest.repo_identity.canonical_path.is_empty());
        assert!(!manifest.repo_identity.source_revision.is_empty());
        assert!(!manifest.repo_identity.host_identity.is_empty());
        let journal = fs::read_to_string(default_repo_journal(directory.path()))
            .expect("lifecycle journal");
        assert!(journal.contains("\"layer\":\"L2\""));
        assert!(journal.contains("\"reason_code\":\"INIT_REPROBE_OK\""));
        assert!(journal.contains("\"stage_to\":\"S1.L2\""));
        assert_eq!(manifest.required_tools.len(), REQUIRED_TOOLS.len());
    }

    /// Typed scan refusal: zero S1.L5 rows is an ERROR with its own reason,
    /// distinct from a content mismatch. Absence of evidence is not a pass.
    fn find_s1_l5_row(journal: &Path) -> Result<serde_json::Value, String> {
        let text = std::fs::read_to_string(journal).map_err(|error| {
            format!(
                "L5_SCAN_UNREADABLE_JOURNAL path={} detail={error}",
                journal.display()
            )
        })?;
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line)
                .map_err(|error| format!("L5_SCAN_UNPARSEABLE_ROW detail={error}"))?;
            if value.get("stage_to").and_then(|stage| stage.as_str()) == Some("S1.L5") {
                return Ok(value);
            }
        }
        Err(format!(
            "L5_SCAN_ZERO_S1_L5_ROWS journal={} lines={}",
            journal.display(),
            text.lines().count()
        ))
    }

    /// L5 writer leg (5iwj): one `write_inception` call emits one S1.L5 row
    /// beside the S1.L2 init row, through the shared stage-event core.
    #[test]
    fn write_inception_emits_one_s1_l5_row() {
        let (directory, output) = fixture();
        write_inception(directory.path(), &output).expect("write succeeds");
        let row =
            find_s1_l5_row(&default_repo_journal(directory.path())).expect("S1.L5 row exists");
        assert_eq!(row["stage_to"], "S1.L5");
        assert_eq!(row["stage_from"], "S1.L4");
        assert_eq!(row["layer"], "L5");
        assert_eq!(row["reason_code"], "INIT_WRITE_OK");
        assert_eq!(row["actor"], "ompo-init");
        assert_eq!(row["outcome"], "emitted");
    }

    #[test]
    fn readback_returns_identity_and_backups_replaced_artifact() {
        let (directory, output) = fixture();
        let first = write_inception(directory.path(), &output).expect("first write");
        fs::write(&output, "tampered\n").expect("tamper artifact");
        let second = write_inception(directory.path(), &output).expect("replacement write");
        let readback = read_inception(&output).expect("readback");
        assert_eq!(readback.repo_identity, second.repo_identity);
        assert!(readback.control_files_complete);
        assert_eq!(first.repo_identity, second.repo_identity);
        let backup_dir = output.parent().unwrap().join("backups");
        let backups: Vec<_> = fs::read_dir(&backup_dir)
            .expect("backup directory")
            .map(|entry| entry.expect("backup entry").path())
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&backups[0]).expect("backup artifact"), b"tampered\n");
    }

    #[test]
    fn writer_second_run_is_idempotent_without_events_or_backup() {
        let (directory, output) = fixture();
        write_inception(directory.path(), &output).expect("first write");
        let before = fs::read(&output).expect("first artifact");
        let journal_before =
            fs::read_to_string(default_repo_journal(directory.path())).expect("journal before");
        write_inception(directory.path(), &output).expect("second write");
        assert_eq!(fs::read(&output).expect("second artifact"), before);
        assert_eq!(
            fs::read_to_string(default_repo_journal(directory.path())).expect("journal after"),
            journal_before,
            "unchanged writer emits no lifecycle event"
        );
        assert!(
            !output.parent().unwrap().join("backups").exists(),
            "unchanged writer creates no backup"
        );
    }

    #[test]
    fn readback_refusal_classes_are_typed_and_distinct() {
        for kind in ["missing", "extra", "wrong_type", "empty"] {
            let (directory, output) = fixture();
            write_inception(directory.path(), &output).expect("write fixture");
            let mut value: Value =
                serde_json::from_str(&fs::read_to_string(&output).expect("read fixture"))
                    .expect("fixture JSON");
            let object = value.as_object_mut().expect("manifest object");
            match kind {
                "missing" => {
                    object.remove("schema_version");
                }
                "extra" => {
                    object.insert("unexpected".to_owned(), Value::Bool(true));
                }
                "wrong_type" => {
                    object.insert("project_id".to_owned(), Value::Bool(true));
                }
                "empty" => {
                    object.insert("project_id".to_owned(), Value::String(String::new()));
                }
                _ => unreachable!("case is enumerated"),
            }
            fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode mutation"))
                .expect("write mutation");
            let error = read_inception(&output).expect_err("mutation must refuse");
            match kind {
                "missing" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackMissingKey { key, .. } if key == "schema_version"
                    ));
                }
                "extra" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackExtraKey { key, .. } if key == "unexpected"
                    ));
                }
                "wrong_type" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackWrongType { key, expected, found, .. }
                            if key == "project_id" && expected == "string" && found == "boolean"
                    ));
                }
                "empty" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackEmpty { key, .. } if key == "project_id"
                    ));
                }
                _ => unreachable!("case is enumerated"),
            }
        }

        let (directory, output) = fixture();
        write_inception(directory.path(), &output).expect("write optional fixture");
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&output).expect("read optional fixture"))
                .expect("optional fixture JSON");
        let object = value.as_object_mut().expect("manifest object");
        for key in OPTIONAL_KEYS {
            let value = if *key == "template_identity" {
                serde_json::json!({
                    "canonical_path": "/template",
                    "source_sha256": "a".repeat(64),
                    "source_revision": "0123456789abcdef",
                })
            } else {
                Value::String("optional".to_owned())
            };
            object.insert((*key).to_owned(), value);
        }
        fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode optional fixture"))
            .expect("write optional fixture");
        read_inception(&output).expect("SCHEMAS optional keys remain allowed");
    }

    #[test]
    fn missing_and_malformed_artifacts_are_distinct_anti_vacuity_errors() {
        let (_directory, output) = fixture();
        let missing = read_inception(&output).expect_err("missing artifact must refuse");
        assert_eq!(missing.exit_code(), 2);
        assert!(matches!(missing, InceptionError::ReadbackMissing { .. }));

        // `fixture()` deliberately leaves the artifact DIRECTORY absent so the
        // leg above is a genuine ABSENCE rather than a parse failure. Planting a
        // corrupt artifact therefore has to create that directory first: without
        // it this `fs::write` itself fails NotFound, the malformed leg is never
        // reached, and the distinction the test name asserts goes unmeasured.
        fs::create_dir_all(output.parent().expect("artifact path has a parent"))
            .expect("create artifact directory");
        fs::write(&output, "{").expect("write malformed artifact");
        let malformed = read_inception(&output).expect_err("malformed artifact must refuse");
        assert_eq!(malformed.exit_code(), 2);
        assert!(matches!(malformed, InceptionError::ReadbackMalformed { .. }));
    }

    #[test]
    fn initialize_reprobes_and_second_run_has_zero_artifact_actions() {
        let (directory, output) = fixture();
        let first = initialize(directory.path(), &output).expect("first init");
        let second = initialize(directory.path(), &output).expect("second init");
        assert_eq!(first.actions, 1);
        assert_eq!(second.actions, 0);
        assert_eq!(second.backup, None);
        assert_eq!(second.monitor_rows, first.monitor_rows + 1);
        assert_eq!(second.manifest.repo_identity, first.manifest.repo_identity);
        assert_eq!(second.monitor_rows, second.journal_rows);
    }


    #[test]
    fn missing_control_file_refuses_before_write() {
        let (directory, output) = fixture();
        fs::remove_file(directory.path().join("AGENTS.md")).expect("remove control file");
        let error = write_inception(directory.path(), &output).expect_err("must refuse");
        assert!(error
            .to_string()
            .contains("INCEPTION_CONTROL_FILES_MISSING"));
        assert!(!output.exists(), "refusal must not create the artifact");
    }

    /// Known-bad (cbl7): init over an unstamped foreign AGENTS.md halts with a
    /// typed refusal and writes nothing -- no artifact, no journal rows.
    #[test]
    fn foreign_agents_md_halts_without_writes() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        let error = write_inception(directory.path(), &output).expect_err("must halt");
        assert!(
            matches!(error, InceptionError::UntrustedAgentsMd { .. }),
            "wrong refusal: {error:?}"
        );
        assert!(
            error.to_string().starts_with("HUMAN_HALT"),
            "halt must name itself: {error}"
        );
        assert!(!output.exists(), "halt must not write the artifact");
        assert!(
            !default_repo_journal(directory.path()).exists(),
            "halt must not write journal rows"
        );
    }

    /// Opt-in: the trusted variant writes the identical foreign content.
    #[test]
    fn trusted_opt_in_writes_foreign_agents_md() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        let consent = explicit_consent(directory.path(), "unit-write");
        write_inception_trusted(directory.path(), &output, &consent).expect("opt-in writes");
        assert!(output.exists(), "opt-in must produce the artifact");
    }

    /// Opt-in covers the `initialize` entry point too: same foreign content,
    /// trusted flag on, artifact produced.
    #[test]
    fn trusted_initialize_writes_foreign_agents_md() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        let consent = explicit_consent(directory.path(), "unit-initialize");
        let report = initialize_trusted(directory.path(), &output, &consent)
            .expect("opt-in initializes");
        assert!(matches!(
            report.trusted_init,
            TrustedInitDecision::ExplicitConsent { ref decision_id, .. }
                if decision_id == "unit-initialize"
        ));
        assert!(output.exists(), "opt-in must produce the artifact");
    }

    /// Anti-vacuity: an empty AGENTS.md is its own typed error, never a halt
    /// for a foreign repo and never a pass.
    #[test]
    fn empty_agents_md_is_typed_error() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "   \n").expect("empty agents file");
        let error = write_inception(directory.path(), &output).expect_err("must refuse");
        assert!(
            matches!(error, InceptionError::EmptyAgentsMd { .. }),
            "wrong refusal: {error:?}"
        );
        assert!(!output.exists(), "refusal must not write the artifact");
    }

    /// The `initialize` entry point shares the gate: foreign content halts there too.
    #[test]
    fn initialize_over_foreign_agents_md_halts() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        let error = initialize(directory.path(), &output).expect_err("must halt");
        assert!(
            matches!(error, InceptionError::UntrustedAgentsMd { .. }),
            "wrong refusal: {error:?}"
        );
        assert!(!output.exists(), "halt must not write the artifact");
    }
}
