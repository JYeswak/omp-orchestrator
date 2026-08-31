#![forbid(unsafe_code)]

//! Machine-readable inventory for the installed OMP surface and this workspace.
//!
//! The inventory is deliberately evidence-first. Workspace packages come from
//! `cargo metadata`; OMP commands come from direct `omp` process probes; type
//! roots and declarations come from a direct `find` process against the
//! installed package; and RPC/slash metadata is parsed from the installed RPC
//! startup stream/source. No repository source-text grep is used to infer
//! ownership. A missing or malformed probe is represented as `UNKNOWN` and
//! never upgraded to a healthy result.


pub mod types_inventory;
use asupersync::Cx;
use asupersync::process::{
    Command, Output, ProcessError, ProcessGroupMode, ProcessSignalTarget, Stdio,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: &str = "omp-inventory-map/v1";
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const EXPECTED_OMP_VERSION: &str = "omp/18.0.11";
pub const EXPECTED_CLI_COMMANDS: usize = 39;
pub const EXPECTED_TYPE_ROOTS: usize = 57;
pub const EXPECTED_DECLARATIONS: usize = 14;
pub const EXPECTED_RPC_HANDLERS: usize = 42;
pub const EXPECTED_SLASH_COMMANDS: usize = 136;
pub const EXPECTED_OMP_METHODS: usize = 3;
pub const MAX_PROBE_BYTES: usize = 16 * 1024 * 1024;

const INVENTORY_CRATE: &str = "omp-inventory-map";
const NO_SOURCE_GREP: &str =
    "No repository source grep was used; ownership is derived from metadata and direct probes.";

/// A fail-closed state for every probe and generated map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProbeState {
    Known,
    Unknown,
}

impl ProbeState {
    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self, Self::Known)
    }
}

/// Typed failures for required metadata and process ownership.
#[derive(Debug)]
pub enum InventoryError {
    EmptyMetadata,
    MalformedMetadata(String),
    InvalidInput(String),
    Process { command: String, detail: String },
    Cancelled,
    OutputTooLarge { command: String, bytes: usize },
}

impl fmt::Display for InventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMetadata => {
                formatter.write_str("EMPTY_METADATA cargo metadata produced no bytes")
            }
            Self::MalformedMetadata(detail) => write!(formatter, "MALFORMED_METADATA {detail}"),
            Self::InvalidInput(detail) => write!(formatter, "INVALID_INPUT {detail}"),
            Self::Process { command, detail } => {
                write!(formatter, "PROCESS_ERROR command={command} detail={detail}")
            }
            Self::Cancelled => formatter.write_str("CANCELLED inventory probe context"),
            Self::OutputTooLarge { command, bytes } => {
                write!(
                    formatter,
                    "OUTPUT_TOO_LARGE command={command} bytes={bytes}"
                )
            }
        }
    }
}

impl std::error::Error for InventoryError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoPackage {
    pub name: String,
    pub version: String,
    pub manifest_path: String,
    pub targets: Vec<String>,
    pub path_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoSnapshot {
    pub workspace_root: String,
    pub packages: Vec<CargoPackage>,
}

/// One `[crates.<package>]` declaration from `OMP-SURFACE-MAP.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceMapDeclaration {
    pub package_name: String,
    pub classification: Option<String>,
    pub omp_surface: Option<String>,
    pub line: usize,
    pub fields: BTreeMap<String, String>,
}

/// A typed finding emitted by surface-map parsing or package-set auditing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE", tag = "kind")]
pub enum SurfaceMapAuditOutcome {
    DuplicateDeclaration {
        package_name: String,
        first_line: usize,
        duplicate_line: usize,
    },
    InvalidClassification {
        package_name: String,
        classification: String,
        line: usize,
    },
    UndeclaredPackage {
        package_name: String,
    },
    GhostDeclaration {
        package_name: String,
        line: usize,
    },
    MalformedRow {
        package_name: Option<String>,
        line: usize,
        detail: String,
    },
}

/// Parsed declarations plus syntax findings retained for a fail-closed audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceMap {
    pub declarations: Vec<SurfaceMapDeclaration>,
    pub outcomes: Vec<SurfaceMapAuditOutcome>,
}

/// Deterministic audit of the declared crate set against cargo metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceMapAudit {
    pub state: ProbeState,
    pub outcomes: Vec<SurfaceMapAuditOutcome>,
    pub declarations: Vec<SurfaceMapDeclaration>,
    pub workspace_packages: Vec<String>,
}

impl SurfaceMapAudit {
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.state.is_known() && self.outcomes.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceMapSection {
    Meta,
    Crate(usize),
}
fn strip_toml_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        match (quoted, escaped, character) {
            (false, _, '#') => return &line[..index],
            (true, false, '"') => quoted = false,
            (false, false, '"') => quoted = true,
            (true, false, character) if character == 92u8 as char => escaped = true,
            (true, true, _) => escaped = false,
            _ => {}
        }
    }
    line
}

fn valid_surface_map_package_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn parse_toml_string(raw: &str) -> Result<String, String> {
    let mut characters = raw.trim().chars();
    if characters.next() != Some('"') {
        return Err("value must be a double-quoted string".to_owned());
    }
    let mut value = String::new();
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if escaped {
            let decoded = match character {
                '"' => '"',
                '\\' => '\\',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => return Err(format!("unsupported string escape \\{character}")),
            };
            value.push(decoded);
            escaped = false;
        } else {
            match character {
                '\\' => escaped = true,
                '"' => {
                    if characters.any(|trailing| !trailing.is_whitespace()) {
                        return Err("trailing characters after string value".to_owned());
                    }
                    return Ok(value);
                }
                _ => value.push(character),
            }
        }
    }
    if escaped {
        Err("unterminated string escape".to_owned())
    } else {
        Err("unterminated string value".to_owned())
    }
}

fn parse_surface_map_assignment(line: &str) -> Result<(String, String), String> {
    let Some(equal) = line.find('=') else {
        return Err("row field is missing '='".to_owned());
    };
    let key = line[..equal].trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("row field name is malformed".to_owned());
    }
    let value = parse_toml_string(&line[equal + 1..])?;
    Ok((key.to_owned(), value))
}

fn push_surface_map_outcome(
    outcomes: &mut Vec<SurfaceMapAuditOutcome>,
    outcome: SurfaceMapAuditOutcome,
) {
    if !outcomes.contains(&outcome) {
        outcomes.push(outcome);
    }
}

/// Parse the crate declaration subset of OMP-SURFACE-MAP.toml.
///
/// This is intentionally a small, strict parser rather than a source grep:
/// only crate rows and their quoted string fields are consumed. Syntax
/// failures remain typed outcomes so an audit can report every defect in one
/// pass instead of silently dropping a row.
#[must_use]
pub fn parse_surface_map(input: &str) -> SurfaceMap {
    let mut declarations = Vec::new();
    let mut outcomes = Vec::new();
    let mut sections = BTreeMap::new();
    let mut section = None;

    if input.trim().is_empty() {
        outcomes.push(SurfaceMapAuditOutcome::MalformedRow {
            package_name: None,
            line: 1,
            detail: "surface map is empty".to_owned(),
        });
    }

    for (line_index, raw_line) in input.lines().enumerate() {
        let line = line_index + 1;
        let content = strip_toml_comment(raw_line).trim();
        if content.is_empty() {
            continue;
        }

        if content.starts_with('[') {
            let Some(header) = content.strip_suffix(']') else {
                outcomes.push(SurfaceMapAuditOutcome::MalformedRow {
                    package_name: None,
                    line,
                    detail: "table header is unterminated".to_owned(),
                });
                section = None;
                continue;
            };
            let header = &header[1..];
            if header == "meta" {
                section = Some(SurfaceMapSection::Meta);
                continue;
            }
            let Some(package_name) = header.strip_prefix("crates.") else {
                outcomes.push(SurfaceMapAuditOutcome::MalformedRow {
                    package_name: None,
                    line,
                    detail: format!("unsupported table [{header}]"),
                });
                section = None;
                continue;
            };
            if !valid_surface_map_package_name(package_name) {
                outcomes.push(SurfaceMapAuditOutcome::MalformedRow {
                    package_name: Some(package_name.to_owned()),
                    line,
                    detail: "crate table name is malformed".to_owned(),
                });
                section = None;
                continue;
            }
            if let Some(first_line) = sections.get(package_name) {
                outcomes.push(SurfaceMapAuditOutcome::DuplicateDeclaration {
                    package_name: package_name.to_owned(),
                    first_line: *first_line,
                    duplicate_line: line,
                });
            } else {
                sections.insert(package_name.to_owned(), line);
            }
            declarations.push(SurfaceMapDeclaration {
                package_name: package_name.to_owned(),
                classification: None,
                omp_surface: None,
                line,
                fields: BTreeMap::new(),
            });
            section = Some(SurfaceMapSection::Crate(declarations.len() - 1));
            continue;
        }

        let Some(SurfaceMapSection::Crate(index)) = section else {
            if !matches!(section, Some(SurfaceMapSection::Meta)) {
                outcomes.push(SurfaceMapAuditOutcome::MalformedRow {
                    package_name: None,
                    line,
                    detail: "row appears outside a supported table".to_owned(),
                });
            }
            continue;
        };
        let declaration = &mut declarations[index];
        match parse_surface_map_assignment(content) {
            Ok((key, value)) => {
                if declaration.fields.contains_key(&key) {
                    push_surface_map_outcome(
                        &mut outcomes,
                        SurfaceMapAuditOutcome::MalformedRow {
                            package_name: Some(declaration.package_name.clone()),
                            line,
                            detail: format!("duplicate field {key}"),
                        },
                    );
                    continue;
                }
                declaration.fields.insert(key.clone(), value.clone());
                match key.as_str() {
                    "classification" => declaration.classification = Some(value),
                    "omp_surface" => declaration.omp_surface = Some(value),
                    _ => {}
                }
            }
            Err(detail) => push_surface_map_outcome(
                &mut outcomes,
                SurfaceMapAuditOutcome::MalformedRow {
                    package_name: Some(declaration.package_name.clone()),
                    line,
                    detail,
                },
            ),
        }
    }
    SurfaceMap {
        declarations,
        outcomes,
    }
}

/// Audit parsed surface declarations against the package names from cargo.
#[must_use]
pub fn audit_surface_map(surface_map: &SurfaceMap, cargo: &CargoSnapshot) -> SurfaceMapAudit {
    let mut outcomes = surface_map.outcomes.clone();
    let mut declared = BTreeSet::new();
    let mut first_lines = BTreeMap::new();
    for declaration in &surface_map.declarations {
        if let Some(first_line) =
            first_lines.insert(declaration.package_name.clone(), declaration.line)
        {
            push_surface_map_outcome(
                &mut outcomes,
                SurfaceMapAuditOutcome::DuplicateDeclaration {
                    package_name: declaration.package_name.clone(),
                    first_line,
                    duplicate_line: declaration.line,
                },
            );
        }
        declared.insert(declaration.package_name.clone());
        match declaration.classification.as_deref() {
            None => push_surface_map_outcome(
                &mut outcomes,
                SurfaceMapAuditOutcome::MalformedRow {
                    package_name: Some(declaration.package_name.clone()),
                    line: declaration.line,
                    detail: "classification is missing".to_owned(),
                },
            ),
            Some("a" | "b" | "c") => {}
            Some(classification) => push_surface_map_outcome(
                &mut outcomes,
                SurfaceMapAuditOutcome::InvalidClassification {
                    package_name: declaration.package_name.clone(),
                    classification: classification.to_owned(),
                    line: declaration.line,
                },
            ),
        }
        if declaration.omp_surface.as_deref().is_none_or(str::is_empty) {
            push_surface_map_outcome(
                &mut outcomes,
                SurfaceMapAuditOutcome::MalformedRow {
                    package_name: Some(declaration.package_name.clone()),
                    line: declaration.line,
                    detail: "omp_surface is missing".to_owned(),
                },
            );
        }
    }

    let workspace_packages = cargo
        .packages
        .iter()
        .map(|package| package.name.clone())
        .collect::<BTreeSet<_>>();
    for package_name in &workspace_packages {
        if !declared.contains(package_name) {
            outcomes.push(SurfaceMapAuditOutcome::UndeclaredPackage {
                package_name: package_name.clone(),
            });
        }
    }
    for declaration in &surface_map.declarations {
        if !workspace_packages.contains(&declaration.package_name) {
            outcomes.push(SurfaceMapAuditOutcome::GhostDeclaration {
                package_name: declaration.package_name.clone(),
                line: declaration.line,
            });
        }
    }
    let state = if outcomes.is_empty() {
        ProbeState::Known
    } else {
        ProbeState::Unknown
    };
    SurfaceMapAudit {
        state,
        outcomes,
        declarations: surface_map.declarations.clone(),
        workspace_packages: workspace_packages.into_iter().collect(),
    }
}

/// Parse both direct-probe payloads and perform the pure audit.
pub fn audit_surface_map_text(
    surface_map_input: &str,
    cargo_metadata_input: &str,
) -> Result<SurfaceMapAudit, InventoryError> {
    let cargo = parse_cargo_metadata(cargo_metadata_input)?;
    Ok(audit_surface_map(
        &parse_surface_map(surface_map_input),
        &cargo,
    ))
}

/// Compatibility spelling for callers that name the metadata relation.
pub fn audit_surface_map_against_metadata(
    surface_map_input: &str,
    cargo_metadata_input: &str,
) -> Result<SurfaceMapAudit, InventoryError> {
    audit_surface_map_text(surface_map_input, cargo_metadata_input)
}

/// A value parsed from a probe. `value=None` is intentional evidence of an
/// unavailable or malformed probe, not an empty healthy result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeValue<T> {
    pub state: ProbeState,
    pub value: Option<T>,
    pub detail: String,
}

impl<T> ProbeValue<T> {
    #[must_use]
    pub fn known(value: T, detail: impl Into<String>) -> Self {
        Self {
            state: ProbeState::Known,
            value: Some(value),
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn unknown(detail: impl Into<String>) -> Self {
        Self {
            state: ProbeState::Unknown,
            value: None,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeEvidence {
    pub name: String,
    pub command: Vec<String>,
    pub state: ProbeState,
    pub observed: Option<usize>,
    pub output: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct InventoryInputs {
    pub cargo_metadata: String,
    pub omp_version: Option<String>,
    pub cli_help: Option<String>,
    pub type_roots: Option<Vec<String>>,
    pub declarations: Option<Vec<String>>,
    pub rpc_handlers: Option<Vec<String>>,
    pub slash_commands: Option<Vec<String>>,
    pub omp_methods: Option<Vec<String>>,
    pub transport_modes: Option<Vec<String>>,
    pub probe_evidence: Vec<ProbeEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryCounts {
    pub cli_commands: usize,
    pub type_roots: usize,
    pub declarations: usize,
    pub rpc_handlers: usize,
    pub slash_commands: usize,
    pub omp_methods: usize,
    pub workspace_crates: usize,
    pub expected_cli_commands: usize,
    pub expected_type_roots: usize,
    pub expected_declarations: usize,
    pub expected_rpc_handlers: usize,
    pub expected_slash_commands: usize,
    pub expected_omp_methods: usize,
    pub expected_workspace_crates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryRow {
    pub id: String,
    pub surface: String,
    pub kind: String,
    pub what_it_provides: String,
    pub provides: String,
    pub crate_consumes_today: String,
    pub crate_should_own: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub must_be_true: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub classification: String,
    pub status: ProbeState,
    pub map_to_none_reason: Option<String>,
    pub orphan_disposition: String,
    pub orphan_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryNode {
    pub id: String,
    pub surface: String,
    pub kind: String,
    pub label: String,
    pub source: String,
    pub what_it_provides: String,
    pub provides: String,
    pub crate_consumes_today: String,
    pub crate_should_own: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub must_be_true: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub classification: String,
    pub status: ProbeState,
    pub map_to_none_reason: Option<String>,
    pub orphan_disposition: String,
    pub orphan_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryMap {
    pub schema_version: String,
    pub generated_by: String,
    pub omp_version: Option<String>,
    pub state: ProbeState,
    pub counts: InventoryCounts,
    pub probes: Vec<ProbeEvidence>,
    pub nodes: Vec<InventoryNode>,
    pub edges: Vec<InventoryEdge>,
    pub rows: Vec<InventoryRow>,
}

#[derive(Debug, Clone)]
pub struct ProbeConfig {
    pub repo_root: PathBuf,
    pub omp_program: PathBuf,
    pub cargo_program: PathBuf,
    pub find_program: PathBuf,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            repo_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            omp_program: PathBuf::from("omp"),
            cargo_program: PathBuf::from("cargo"),
            find_program: PathBuf::from("find"),
        }
    }
}

/// Parse the only required repository metadata source.
///
/// Empty metadata is a hard error. Missing package fields and malformed JSON
/// are typed errors rather than a zero-package healthy result.
pub fn parse_cargo_metadata(input: &str) -> Result<CargoSnapshot, InventoryError> {
    if input.trim().is_empty() {
        return Err(InventoryError::EmptyMetadata);
    }
    let document: Value = serde_json::from_str(input)
        .map_err(|error| InventoryError::MalformedMetadata(error.to_string()))?;
    let workspace_root = document
        .get("workspace_root")
        .and_then(Value::as_str)
        .ok_or_else(|| InventoryError::MalformedMetadata("workspace_root missing".to_owned()))?;
    let packages = document
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| InventoryError::MalformedMetadata("packages array missing".to_owned()))?;
    if packages.is_empty() {
        return Err(InventoryError::MalformedMetadata(
            "packages array is empty".to_owned(),
        ));
    }

    let mut parsed = Vec::with_capacity(packages.len());
    for (index, package) in packages.iter().enumerate() {
        let object = package.as_object().ok_or_else(|| {
            InventoryError::MalformedMetadata(format!("packages[{index}] is not an object"))
        })?;
        let required = |name: &str| {
            object
                .get(name)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    InventoryError::MalformedMetadata(format!("packages[{index}].{name} missing"))
                })
        };
        let targets = object
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                InventoryError::MalformedMetadata(format!("packages[{index}].targets missing"))
            })?
            .iter()
            .filter_map(|target| target.get("name").and_then(Value::as_str))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let path_dependencies = object
            .get("dependencies")
            .and_then(Value::as_array)
            .map(|dependencies| {
                dependencies
                    .iter()
                    .filter_map(|dependency| {
                        let object = dependency.as_object()?;
                        object.get("path")?.as_str()?;
                        object.get("name")?.as_str().map(str::to_owned)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        parsed.push(CargoPackage {
            name: required("name")?,
            version: required("version")?,
            manifest_path: required("manifest_path")?,
            targets,
            path_dependencies,
        });
    }
    Ok(CargoSnapshot {
        workspace_root: workspace_root.to_owned(),
        packages: parsed,
    })
}

/// Parse the `COMMANDS` section of `omp --help` without consulting repository
/// source text.
pub fn parse_cli_commands(help: &str) -> ProbeValue<Vec<String>> {
    if help.trim().is_empty() {
        return ProbeValue::unknown("omp --help returned no bytes");
    }
    let mut in_commands = false;
    let mut commands = BTreeSet::new();
    for line in help.lines() {
        let trimmed = line.trim();
        if trimmed == "COMMANDS" {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if trimmed.is_empty() {
            if !commands.is_empty() {
                break;
            }
            continue;
        }
        let first = trimmed.split_whitespace().next().unwrap_or_default();
        if first.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        }) && first
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        {
            commands.insert(first.to_owned());
        }
    }
    if commands.is_empty() {
        ProbeValue::unknown("COMMANDS block missing or malformed")
    } else {
        ProbeValue::known(
            commands.into_iter().collect(),
            "parsed non-empty COMMANDS block",
        )
    }
}

/// Parse the documented `--mode=<...>` transport declaration.
pub fn parse_transport_modes(help: &str) -> ProbeValue<Vec<String>> {
    let Some(start) = help.find("--mode=<") else {
        return ProbeValue::unknown("--mode declaration missing");
    };
    let rest = &help[start + "--mode=<".len()..];
    let Some(end) = rest.find('>') else {
        return ProbeValue::unknown("--mode declaration is unterminated");
    };
    let modes = rest[..end]
        .split('|')
        .filter(|mode| !mode.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if modes.is_empty() {
        ProbeValue::unknown("--mode declaration contains no modes")
    } else {
        ProbeValue::known(modes, "parsed documented mode list")
    }
}

/// Parse an `omp --version` response.
pub fn parse_omp_version(output: &str) -> ProbeValue<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("omp/") && line.len() > 4)
        .map(|line| ProbeValue::known(line.to_owned(), "parsed omp version"))
        .unwrap_or_else(|| ProbeValue::unknown("omp version response missing or malformed"))
}

/// Parse direct `find` output into final path components.
pub fn parse_path_listing(output: &str, kind: &str) -> ProbeValue<Vec<String>> {
    let mut names = BTreeSet::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let path = Path::new(line);
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return ProbeValue::unknown(format!("{kind} listing contains a malformed path"));
        };
        if name == "." || name == ".." {
            return ProbeValue::unknown(format!("{kind} listing contains a traversal component"));
        }
        names.insert(name.to_owned());
    }
    if names.is_empty() {
        ProbeValue::unknown(format!("{kind} listing is empty"))
    } else {
        ProbeValue::known(
            names.into_iter().collect(),
            format!("parsed {kind} listing"),
        )
    }
}

/// Derive inbound RPC handler names from the installed bundle's dispatch
/// region. This is intentionally pointed at the installed artifact, never at
/// this repository's own source.
pub fn parse_rpc_handlers_source(source: &str) -> ProbeValue<Vec<String>> {
    let Some(start) = source.find("let w=async(v)=>") else {
        return ProbeValue::unknown("installed RPC dispatch start marker missing");
    };
    let body = &source[start..];
    let Some(end) = body.find("},E=new KWt") else {
        return ProbeValue::unknown("installed RPC dispatch end marker missing");
    };
    let body = &body[..end];
    let mut methods = BTreeSet::new();
    let mut cursor = body;
    while let Some(case_start) = cursor.find("case\"") {
        let after = &cursor[case_start + "case\"".len()..];
        let Some(end_quote) = after.find('"') else {
            return ProbeValue::unknown("RPC dispatch contains an unterminated case");
        };
        let method = &after[..end_quote];
        if method.is_empty() || method.chars().any(char::is_whitespace) {
            return ProbeValue::unknown("RPC dispatch contains a malformed method");
        }
        methods.insert(method.to_owned());
        cursor = &after[end_quote + 1..];
    }
    if methods.is_empty() {
        ProbeValue::unknown("RPC dispatch contains no case methods")
    } else {
        ProbeValue::known(
            methods.into_iter().collect(),
            "parsed installed RPC dispatch cases",
        )
    }
}

/// Derive the three `omp/*` methods visible in the installed bundle.
pub fn parse_omp_methods_source(source: &str) -> ProbeValue<Vec<String>> {
    let mut methods = BTreeSet::new();
    let mut cursor = source;
    while let Some(start) = cursor.find("\"omp/") {
        let after = &cursor[start + 1..];
        let Some(end_quote) = after.find('"') else {
            return ProbeValue::unknown("omp/* method string is unterminated");
        };
        methods.insert(after[..end_quote].to_owned());
        cursor = &after[end_quote + 1..];
    }
    if methods.is_empty() {
        ProbeValue::unknown("installed bundle contains no omp/* methods")
    } else {
        ProbeValue::known(
            methods.into_iter().collect(),
            "parsed installed omp/* method strings",
        )
    }
}

fn collect_rpc_command_names(value: &Value, prefix: &str, names: &mut BTreeSet<String>) {
    let Some(object) = value.as_object() else {
        return;
    };
    let Some(name) = object.get("name").and_then(Value::as_str) else {
        return;
    };
    let current = if prefix.is_empty() {
        format!("/{name}")
    } else {
        format!("{prefix}/{name}")
    };
    names.insert(current.clone());
    if let Some(subcommands) = object.get("subcommands").and_then(Value::as_array) {
        for subcommand in subcommands {
            collect_rpc_command_names(subcommand, &current, names);
        }
    }
}

/// Parse the `available_commands_update` JSON line emitted by OMP RPC startup.
pub fn parse_rpc_slash_commands(output: &str) -> ProbeValue<Vec<String>> {
    let mut names = BTreeSet::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("available_commands_update") {
            continue;
        }
        let Some(commands) = value.get("commands").and_then(Value::as_array) else {
            return ProbeValue::unknown("available_commands_update.commands is malformed");
        };
        for command in commands {
            collect_rpc_command_names(command, "", &mut names);
        }
    }
    if names.is_empty() {
        ProbeValue::unknown("RPC startup did not emit available command metadata")
    } else {
        ProbeValue::known(
            names.into_iter().collect(),
            "parsed RPC available command metadata",
        )
    }
}

fn list_state<T>(value: &Option<Vec<T>>, expected: usize) -> ProbeState {
    match value {
        Some(items) if !items.is_empty() && items.len() == expected => ProbeState::Known,
        _ => ProbeState::Unknown,
    }
}

fn value_or_unknown<T: Clone>(value: &Option<Vec<T>>, expected: usize) -> (Vec<T>, ProbeState) {
    match value {
        Some(items) if !items.is_empty() => (items.clone(), list_state(value, expected)),
        _ => (vec![], ProbeState::Unknown),
    }
}

fn owner_for(kind: &str, name: &str) -> Option<&'static str> {
    if kind == "transport"
        || kind == "slash_command"
        || (kind == "type_root"
            && matches!(name, "cli" | "commands" | "jsonrpc" | "slash-commands"))
        || (kind == "rpc_handler" && name == "get_available_commands")
    {
        Some(INVENTORY_CRATE)
    } else {
        None
    }
}

fn classification_for(kind: &str, name: &str, owner: Option<&str>) -> (String, String) {
    if owner.is_some() {
        return (
            "MAPPED_BY_DIRECT_PROBE".to_owned(),
            "Direct process probe or installed metadata parser is the named owner.".to_owned(),
        );
    }
    let candidate = matches!(
        name,
        "jsonrpc"
            | "tools"
            | "slash-commands"
            | "commands"
            | "session"
            | "task"
            | "goals"
            | "plan-mode"
            | "modes"
            | "subprocess"
            | "exec"
            | "dap"
            | "debug"
            | "memory-backend"
            | "memories"
            | "mnemopi"
            | "get_state"
            | "prompt"
            | "steer"
            | "follow_up"
            | "bash"
    );
    if candidate {
        (
            "SCRAPED_OR_OBSERVED_ALTERNATIVE".to_owned(),
            format!(
                "No typed runtime adapter owns {kind}:{name}; retain as a named wire candidate."
            ),
        )
    } else {
        (
            "CAPABILITY_NOT_USED".to_owned(),
            format!("The repository has no measured runtime trigger for {kind}:{name}."),
        )
    }
}

fn row(
    id: String,
    surface: String,
    kind: String,
    what_it_provides: String,
    status: ProbeState,
    owner: Option<&str>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    classification: String,
    orphan_reason: String,
) -> InventoryRow {
    let consumes = owner.unwrap_or("NONE").to_owned();
    let should_own = owner.unwrap_or("NONE").to_owned();
    let map_to_none_reason = owner
        .map(|_| None)
        .unwrap_or_else(|| Some(orphan_reason.clone()));
    InventoryRow {
        id,
        surface,
        kind,
        what_it_provides: what_it_provides.clone(),
        provides: what_it_provides,
        crate_consumes_today: consumes,
        crate_should_own: should_own,
        inputs,
        outputs,
        must_be_true: vec![
            "The source probe is non-empty before a known verdict is emitted.".to_owned(),
            "A versioned inventory envelope carries the probe state.".to_owned(),
        ],
        negative_evidence: vec![NO_SOURCE_GREP.to_owned()],
        classification,
        status,
        map_to_none_reason,
        orphan_disposition: if owner.is_some() {
            "WIRE".to_owned()
        } else {
            "NAMED_REASON".to_owned()
        },
        orphan_reason,
    }
}

fn node_from_row(source: String, row: &InventoryRow) -> InventoryNode {
    InventoryNode {
        id: row.id.clone(),
        surface: row.surface.clone(),
        kind: row.kind.clone(),
        label: row.surface.clone(),
        source,
        what_it_provides: row.what_it_provides.clone(),
        provides: row.provides.clone(),
        crate_consumes_today: row.crate_consumes_today.clone(),
        crate_should_own: row.crate_should_own.clone(),
        inputs: row.inputs.clone(),
        outputs: row.outputs.clone(),
        must_be_true: row.must_be_true.clone(),
        negative_evidence: row.negative_evidence.clone(),
        classification: row.classification.clone(),
        status: row.status,
        map_to_none_reason: row.map_to_none_reason.clone(),
        orphan_disposition: row.orphan_disposition.clone(),
        orphan_reason: row.orphan_reason.clone(),
    }
}

fn push_surface(
    rows: &mut Vec<InventoryRow>,
    kind: &str,
    name: &str,
    status: ProbeState,
    source: &str,
) {
    let owner = owner_for(kind, name);
    let (classification, orphan_reason) = classification_for(kind, name, owner);
    let surface = format!("{kind}:{name}");
    rows.push(row(
        format!("surface:{surface}"),
        surface,
        kind.to_owned(),
        format!("Installed OMP {kind} surface {name}"),
        status,
        owner,
        vec![
            format!("direct probe={source}"),
            "installed OMP v18.0.11".to_owned(),
        ],
        vec![
            format!("enumerated {kind} name={name}"),
            "machine-readable table row".to_owned(),
        ],
        classification,
        orphan_reason,
    ));
}

fn push_unknown_surface(rows: &mut Vec<InventoryRow>, kind: &str, source: &str) {
    push_surface(rows, kind, "UNKNOWN_PROBE", ProbeState::Unknown, source);
}

/// Build the graph and table from pure fixture inputs.
///
/// This function is the deterministic generation core used by the live
/// collector and by fixture tests. It refuses empty cargo metadata and emits
/// `UNKNOWN` rows for every unavailable/malformed OMP probe.
pub fn build_inventory_map(inputs: InventoryInputs) -> Result<InventoryMap, InventoryError> {
    let cargo = parse_cargo_metadata(&inputs.cargo_metadata)?;
    let cli = value_or_unknown(
        &inputs
            .cli_help
            .as_ref()
            .map(|help| parse_cli_commands(help).value.unwrap_or_default()),
        EXPECTED_CLI_COMMANDS,
    );
    let (cli_commands, cli_state) = cli;
    let (type_roots, type_state) = value_or_unknown(&inputs.type_roots, EXPECTED_TYPE_ROOTS);
    let (declarations, declaration_state) =
        value_or_unknown(&inputs.declarations, EXPECTED_DECLARATIONS);
    let (rpc_handlers, rpc_state) = value_or_unknown(&inputs.rpc_handlers, EXPECTED_RPC_HANDLERS);
    let (slash_commands, slash_state) =
        value_or_unknown(&inputs.slash_commands, EXPECTED_SLASH_COMMANDS);
    let (omp_methods, omp_method_state) =
        value_or_unknown(&inputs.omp_methods, EXPECTED_OMP_METHODS);
    let transport_state = match &inputs.transport_modes {
        Some(modes) if !modes.is_empty() => ProbeState::Known,
        _ => ProbeState::Unknown,
    };

    let mut rows = Vec::new();
    for name in &cli_commands {
        push_surface(&mut rows, "cli_command", name, cli_state, "omp --help");
    }
    if cli_commands.is_empty() {
        push_unknown_surface(&mut rows, "cli_command", "omp --help");
    }
    for name in &type_roots {
        push_surface(
            &mut rows,
            "type_root",
            name,
            type_state,
            "find dist/types -type d",
        );
    }
    if type_roots.is_empty() {
        push_unknown_surface(&mut rows, "type_root", "find dist/types -type d");
    }
    for name in &declarations {
        push_surface(
            &mut rows,
            "declaration",
            name,
            declaration_state,
            "find dist/types -name '*.d.ts'",
        );
    }
    if declarations.is_empty() {
        push_unknown_surface(&mut rows, "declaration", "find dist/types -name '*.d.ts'");
    }
    for name in &rpc_handlers {
        push_surface(
            &mut rows,
            "rpc_handler",
            name,
            rpc_state,
            "installed cli.js dispatch cases",
        );
    }
    if rpc_handlers.is_empty() {
        push_unknown_surface(&mut rows, "rpc_handler", "source dispatch");
    }
    for name in &slash_commands {
        push_surface(
            &mut rows,
            "slash_command",
            name,
            slash_state,
            "omp --mode=rpc startup",
        );
    }
    if slash_commands.is_empty() {
        push_unknown_surface(&mut rows, "slash_command", "omp --mode=rpc startup");
    }
    for name in &omp_methods {
        push_surface(
            &mut rows,
            "omp_method",
            name,
            omp_method_state,
            "installed cli.js string census",
        );
    }
    if omp_methods.is_empty() {
        push_unknown_surface(&mut rows, "omp_method", "installed cli.js string census");
    }
    let transport_name = inputs
        .transport_modes
        .as_ref()
        .map(|modes| format!("--mode=<{}>", modes.join("|")))
        .unwrap_or_else(|| "--mode=UNKNOWN".to_owned());
    push_surface(
        &mut rows,
        "transport",
        &transport_name,
        transport_state,
        "omp --help",
    );

    let mut nodes = Vec::with_capacity(rows.len() + cargo.packages.len() + 1);
    nodes.push(InventoryNode {
        id: "omp:installed".to_owned(),
        surface: "omp:installed".to_owned(),
        kind: "omp_installation".to_owned(),
        label: "installed OMP".to_owned(),
        source: "direct omp --version probe".to_owned(),
        what_it_provides:
            "The installed OMP v18.0.11 process and its observable metadata surfaces.".to_owned(),
        provides: "The installed OMP v18.0.11 process and its observable metadata surfaces."
            .to_owned(),
        crate_consumes_today: "NONE".to_owned(),
        crate_should_own: "NONE".to_owned(),
        inputs: vec!["omp --version".to_owned(), "omp --help".to_owned()],
        outputs: vec!["versioned surface inventory".to_owned()],
        must_be_true: vec![
            "The version probe is checked before claiming installed identity.".to_owned(),
        ],
        negative_evidence: vec![NO_SOURCE_GREP.to_owned()],
        classification: "EXTERNAL_SURFACE".to_owned(),
        status: if inputs.omp_version.as_deref() == Some(EXPECTED_OMP_VERSION) {
            ProbeState::Known
        } else {
            ProbeState::Unknown
        },
        map_to_none_reason: Some("External OMP installation is not a workspace crate.".to_owned()),
        orphan_disposition: "NAMED_REASON".to_owned(),
        orphan_reason: "External installation is the source, not an adopted workspace owner."
            .to_owned(),
    });
    for current in &rows {
        nodes.push(node_from_row(
            "generated from live probe evidence".to_owned(),
            current,
        ));
    }

    let mut edges = Vec::with_capacity(rows.len() + cargo.packages.len() * 2);
    for current in &rows {
        edges.push(InventoryEdge {
            from: "omp:installed".to_owned(),
            to: current.id.clone(),
            relation: "provides".to_owned(),
            evidence: current.inputs.join("; "),
        });
        if current.crate_consumes_today != "NONE" {
            edges.push(InventoryEdge {
                from: format!("crate:{}", current.crate_consumes_today),
                to: current.id.clone(),
                relation: "consumes".to_owned(),
                evidence: "direct process probe produced this row".to_owned(),
            });
        }
    }

    for package in &cargo.packages {
        let is_inventory = package.name == INVENTORY_CRATE;
        let owner = is_inventory.then_some(INVENTORY_CRATE);
        let reason = if is_inventory {
            "This crate owns generation and direct probe orchestration.".to_owned()
        } else {
            format!(
                "No OMP runtime trigger was observed for workspace crate {} by this map.",
                package.name
            )
        };
        let (classification, _) = if is_inventory {
            ("MAPPED_BY_DIRECT_PROBE".to_owned(), reason.clone())
        } else {
            ("CAPABILITY_NOT_USED".to_owned(), reason.clone())
        };
        let current = row(
            format!("crate:{}", package.name),
            format!("crate:{}", package.name),
            "workspace_crate".to_owned(),
            format!("Workspace crate {} from cargo metadata", package.name),
            ProbeState::Known,
            owner,
            vec![
                "cargo metadata --format-version 1 --no-deps".to_owned(),
                package.manifest_path.clone(),
            ],
            vec![
                format!("targets={}", package.targets.join(",")),
                format!("path_dependencies={}", package.path_dependencies.join(",")),
            ],
            classification,
            reason,
        );
        if !is_inventory {
            edges.push(InventoryEdge {
                from: current.id.clone(),
                to: "omp:installed".to_owned(),
                relation: "map-to-none".to_owned(),
                evidence: current.orphan_reason.clone(),
            });
        }
        for dependency in &package.path_dependencies {
            edges.push(InventoryEdge {
                from: current.id.clone(),
                to: format!("crate:{dependency}"),
                relation: "path-depends-on".to_owned(),
                evidence: "cargo metadata dependency.path".to_owned(),
            });
        }
        nodes.push(node_from_row("cargo metadata".to_owned(), &current));
        rows.push(current);
    }

    let counts = InventoryCounts {
        cli_commands: cli_commands.len(),
        type_roots: type_roots.len(),
        declarations: declarations.len(),
        rpc_handlers: rpc_handlers.len(),
        slash_commands: slash_commands.len(),
        omp_methods: omp_methods.len(),
        workspace_crates: cargo.packages.len(),
        expected_cli_commands: EXPECTED_CLI_COMMANDS,
        expected_type_roots: EXPECTED_TYPE_ROOTS,
        expected_declarations: EXPECTED_DECLARATIONS,
        expected_rpc_handlers: EXPECTED_RPC_HANDLERS,
        expected_slash_commands: EXPECTED_SLASH_COMMANDS,
        expected_omp_methods: EXPECTED_OMP_METHODS,
        expected_workspace_crates: cargo.packages.len(),
    };
    let all_known = inputs.omp_version.as_deref() == Some(EXPECTED_OMP_VERSION)
        && cli_state.is_known()
        && type_state.is_known()
        && declaration_state.is_known()
        && rpc_state.is_known()
        && slash_state.is_known()
        && omp_method_state.is_known()
        && transport_state.is_known();
    let mut probes = inputs.probe_evidence;
    probes.push(ProbeEvidence {
        name: "cargo_metadata".to_owned(),
        command: vec![
            "cargo".to_owned(),
            "metadata".to_owned(),
            "--format-version".to_owned(),
            "1".to_owned(),
            "--no-deps".to_owned(),
        ],
        state: ProbeState::Known,
        observed: Some(cargo.packages.len()),
        output: format!(
            "workspace_root={} packages={}",
            cargo.workspace_root,
            cargo.packages.len()
        ),
        detail: "live cargo metadata parsed".to_owned(),
    });
    Ok(InventoryMap {
        schema_version: SCHEMA_VERSION.to_owned(),
        generated_by: format!("{INVENTORY_CRATE} {CRATE_VERSION}"),
        omp_version: inputs.omp_version,
        state: if all_known {
            ProbeState::Known
        } else {
            ProbeState::Unknown
        },
        counts,
        probes,
        nodes,
        edges,
        rows,
    })
}

#[derive(Debug)]
struct RawProbe {
    output: Output,
}

fn command_display(program: &Path, args: &[String]) -> String {
    let mut parts = vec![program.display().to_string()];
    parts.extend(args.iter().cloned());
    parts.join(" ")
}

fn configure_process(command: &mut Command) {
    command
        .process_group_mode(ProcessGroupMode::NewProcessGroup)
        .signal_target(ProcessSignalTarget::ProcessGroup)
        .kill_on_drop(true)
        .stdin(Stdio::Null)
        .stdout(Stdio::Pipe)
        .stderr(Stdio::Pipe);
}

async fn run_process(
    cx: &Cx,
    program: &Path,
    args: &[String],
    cwd: Option<&Path>,
) -> Result<RawProbe, InventoryError> {
    cx.checkpoint().map_err(|_| InventoryError::Cancelled)?;
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    configure_process(&mut command);
    let command_name = command_display(program, args);
    let output = command
        .output_async(cx)
        .await
        .map_err(|error| match error {
            ProcessError::Io(error) => InventoryError::Process {
                command: command_name.clone(),
                detail: error.to_string(),
            },
            other => InventoryError::Process {
                command: command_name.clone(),
                detail: other.to_string(),
            },
        })?;
    let total = output.stdout.len().saturating_add(output.stderr.len());
    if total > MAX_PROBE_BYTES {
        return Err(InventoryError::OutputTooLarge {
            command: command_name,
            bytes: total,
        });
    }
    cx.checkpoint().map_err(|_| InventoryError::Cancelled)?;
    Ok(RawProbe { output })
}

fn output_text(raw: &RawProbe) -> String {
    String::from_utf8_lossy(&raw.output.stdout).into_owned()
}

fn evidence(
    name: &str,
    command: Vec<String>,
    state: ProbeState,
    observed: Option<usize>,
    output: impl Into<String>,
    detail: impl Into<String>,
) -> ProbeEvidence {
    ProbeEvidence {
        name: name.to_owned(),
        command,
        state,
        observed,
        output: output.into(),
        detail: detail.into(),
    }
}

fn package_root(program: &Path) -> Option<PathBuf> {
    let resolved = resolve_program(program)?;
    let file = resolved.file_name()?.to_str()?;
    if file != "cli.js" {
        return None;
    }
    resolved.parent()?.parent().map(Path::to_path_buf)
}

fn resolve_program(program: &Path) -> Option<PathBuf> {
    if program.components().count() > 1 {
        return program.canonicalize().ok();
    }
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(program);
        if candidate.is_file() {
            if let Ok(resolved) = candidate.canonicalize() {
                return Some(resolved);
            }
        }
    }
    None
}

fn output_excerpt(raw: &RawProbe) -> String {
    let text = output_text(raw);
    text.chars().take(4096).collect()
}

/// Run all live probes and generate the map. Required cargo metadata failures
/// are returned as errors; optional OMP probes become `UNKNOWN` evidence.
pub async fn collect_inventory(
    cx: &Cx,
    config: &ProbeConfig,
) -> Result<InventoryMap, InventoryError> {
    let cargo_args = vec![
        "metadata".to_owned(),
        "--format-version".to_owned(),
        "1".to_owned(),
        "--no-deps".to_owned(),
    ];
    let cargo_probe = run_process(
        cx,
        &config.cargo_program,
        &cargo_args,
        Some(&config.repo_root),
    )
    .await?;
    if !cargo_probe.output.status.success() {
        return Err(InventoryError::Process {
            command: command_display(&config.cargo_program, &cargo_args),
            detail: String::from_utf8_lossy(&cargo_probe.output.stderr).into_owned(),
        });
    }
    let cargo_metadata = output_text(&cargo_probe);
    parse_cargo_metadata(&cargo_metadata)?;

    let mut inputs = InventoryInputs {
        cargo_metadata,
        ..InventoryInputs::default()
    };
    let omp_args = vec!["--version".to_owned()];
    match run_process(cx, &config.omp_program, &omp_args, Some(&config.repo_root)).await {
        Ok(raw) => {
            let parsed = parse_omp_version(&output_text(&raw));
            inputs.omp_version = parsed.value.clone();
            inputs.probe_evidence.push(evidence(
                "omp_version",
                vec!["omp".to_owned(), "--version".to_owned()],
                parsed.state,
                parsed.value.as_ref().map(|_| 1),
                output_excerpt(&raw),
                parsed.detail,
            ));
        }
        Err(error) => inputs.probe_evidence.push(evidence(
            "omp_version",
            omp_args,
            ProbeState::Unknown,
            None,
            "",
            error.to_string(),
        )),
    }

    let help_args = vec!["--help".to_owned()];
    match run_process(cx, &config.omp_program, &help_args, Some(&config.repo_root)).await {
        Ok(raw) => {
            let help = output_text(&raw);
            let commands = parse_cli_commands(&help);
            let modes = parse_transport_modes(&help);
            inputs.cli_help = Some(help.clone());
            inputs.transport_modes = modes.value.clone();
            inputs.probe_evidence.push(evidence(
                "omp_help_cli_commands",
                vec!["omp".to_owned(), "--help".to_owned()],
                commands.state,
                commands.value.as_ref().map(Vec::len),
                output_excerpt(&raw),
                commands.detail,
            ));
            inputs.probe_evidence.push(evidence(
                "omp_help_transport_modes",
                vec!["omp".to_owned(), "--help".to_owned()],
                modes.state,
                modes.value.as_ref().map(Vec::len),
                modes
                    .value
                    .as_ref()
                    .map(|items| items.join("|"))
                    .unwrap_or_else(|| "<unknown>".to_owned()),
                modes.detail,
            ));
        }
        Err(error) => inputs.probe_evidence.push(evidence(
            "omp_help",
            help_args,
            ProbeState::Unknown,
            None,
            "",
            error.to_string(),
        )),
    }

    if let Some(root) = package_root(&config.omp_program) {
        let types = root.join("dist").join("types");
        let directories_args = vec![
            types.display().to_string(),
            "-mindepth".to_owned(),
            "1".to_owned(),
            "-maxdepth".to_owned(),
            "1".to_owned(),
            "-type".to_owned(),
            "d".to_owned(),
            "-print".to_owned(),
        ];
        match run_process(
            cx,
            &config.find_program,
            &directories_args,
            Some(&config.repo_root),
        )
        .await
        {
            Ok(raw) => {
                let parsed = parse_path_listing(&output_text(&raw), "type roots");
                inputs.type_roots = parsed.value.clone();
                inputs.probe_evidence.push(evidence(
                    "omp_type_roots",
                    std::iter::once("find".to_owned())
                        .chain(directories_args)
                        .collect(),
                    parsed.state,
                    parsed.value.as_ref().map(Vec::len),
                    output_excerpt(&raw),
                    parsed.detail,
                ));
            }
            Err(error) => inputs.probe_evidence.push(evidence(
                "omp_type_roots",
                directories_args,
                ProbeState::Unknown,
                None,
                "",
                error.to_string(),
            )),
        }
        let declarations_args = vec![
            types.display().to_string(),
            "-mindepth".to_owned(),
            "1".to_owned(),
            "-maxdepth".to_owned(),
            "1".to_owned(),
            "-type".to_owned(),
            "f".to_owned(),
            "-name".to_owned(),
            "*.d.ts".to_owned(),
            "-print".to_owned(),
        ];
        match run_process(
            cx,
            &config.find_program,
            &declarations_args,
            Some(&config.repo_root),
        )
        .await
        {
            Ok(raw) => {
                let parsed = parse_path_listing(&output_text(&raw), "declarations");
                inputs.declarations = parsed.value.clone();
                inputs.probe_evidence.push(evidence(
                    "omp_type_declarations",
                    std::iter::once("find".to_owned())
                        .chain(declarations_args)
                        .collect(),
                    parsed.state,
                    parsed.value.as_ref().map(Vec::len),
                    output_excerpt(&raw),
                    parsed.detail,
                ));
            }
            Err(error) => inputs.probe_evidence.push(evidence(
                "omp_type_declarations",
                declarations_args,
                ProbeState::Unknown,
                None,
                "",
                error.to_string(),
            )),
        }
        let bundle = root.join("dist").join("cli.js");
        match std::fs::read_to_string(&bundle) {
            Ok(source) => {
                let handlers = parse_rpc_handlers_source(&source);
                let omp_methods = parse_omp_methods_source(&source);
                inputs.rpc_handlers = handlers.value.clone();
                inputs.omp_methods = omp_methods.value.clone();
                inputs.probe_evidence.push(evidence(
                    "omp_rpc_handlers",
                    vec![
                        "installed".to_owned(),
                        bundle.display().to_string(),
                        "dispatch cases".to_owned(),
                    ],
                    handlers.state,
                    handlers.value.as_ref().map(Vec::len),
                    handlers
                        .value
                        .as_ref()
                        .map(|items| items.join(","))
                        .unwrap_or_else(|| "<unknown>".to_owned()),
                    handlers.detail,
                ));
                inputs.probe_evidence.push(evidence(
                    "omp_methods",
                    vec![
                        "installed".to_owned(),
                        bundle.display().to_string(),
                        "omp/* strings".to_owned(),
                    ],
                    omp_methods.state,
                    omp_methods.value.as_ref().map(Vec::len),
                    omp_methods
                        .value
                        .as_ref()
                        .map(|items| items.join(","))
                        .unwrap_or_else(|| "<unknown>".to_owned()),
                    omp_methods.detail,
                ));
            }
            Err(error) => inputs.probe_evidence.push(evidence(
                "omp_bundle_metadata",
                vec!["installed".to_owned(), bundle.display().to_string()],
                ProbeState::Unknown,
                None,
                "",
                error.to_string(),
            )),
        }
    } else {
        inputs.probe_evidence.push(evidence(
            "omp_install_root",
            vec![
                "resolve".to_owned(),
                config.omp_program.display().to_string(),
            ],
            ProbeState::Unknown,
            None,
            "",
            "OMP launcher did not resolve to an installed dist/cli.js path",
        ));
    }

    let rpc_args = vec![
        "--mode=rpc".to_owned(),
        "--no-session".to_owned(),
        "--no-tools".to_owned(),
        "--max-time=5".to_owned(),
    ];
    match run_process(cx, &config.omp_program, &rpc_args, Some(&config.repo_root)).await {
        Ok(raw) => {
            let parsed = parse_rpc_slash_commands(&output_text(&raw));
            inputs.slash_commands = parsed.value.clone();
            inputs.probe_evidence.push(evidence(
                "omp_rpc_slash_commands",
                std::iter::once("omp".to_owned()).chain(rpc_args).collect(),
                parsed.state,
                parsed.value.as_ref().map(Vec::len),
                output_excerpt(&raw),
                parsed.detail,
            ));
        }
        Err(error) => inputs.probe_evidence.push(evidence(
            "omp_rpc_slash_commands",
            rpc_args,
            ProbeState::Unknown,
            None,
            "",
            error.to_string(),
        )),
    }
    build_inventory_map(inputs)
}

fn read_bounded_surface_map(path: &Path) -> Result<String, InventoryError> {
    let reported_bytes = std::fs::metadata(path)
        .map_err(|error| InventoryError::Process {
            command: format!("read {}", path.display()),
            detail: error.to_string(),
        })
        .and_then(|metadata| {
            usize::try_from(metadata.len()).map_err(|_| InventoryError::OutputTooLarge {
                command: format!("read {}", path.display()),
                bytes: usize::MAX,
            })
        })?;
    if reported_bytes > MAX_PROBE_BYTES {
        return Err(InventoryError::OutputTooLarge {
            command: format!("read {}", path.display()),
            bytes: reported_bytes,
        });
    }
    let bytes = std::fs::read(path).map_err(|error| InventoryError::Process {
        command: format!("read {}", path.display()),
        detail: error.to_string(),
    })?;
    if bytes.len() > MAX_PROBE_BYTES {
        return Err(InventoryError::OutputTooLarge {
            command: format!("read {}", path.display()),
            bytes: bytes.len(),
        });
    }
    String::from_utf8(bytes).map_err(|error| InventoryError::Process {
        command: format!("read {}", path.display()),
        detail: format!("surface map is not UTF-8: {error}"),
    })
}

/// Read the repository surface declaration and cargo metadata through the
/// same bounded direct process probe used by the inventory collector.
pub async fn collect_surface_map_audit(
    cx: &Cx,
    config: &ProbeConfig,
) -> Result<SurfaceMapAudit, InventoryError> {
    let cargo_args = vec![
        "metadata".to_owned(),
        "--format-version".to_owned(),
        "1".to_owned(),
        "--no-deps".to_owned(),
    ];
    let cargo_probe = run_process(
        cx,
        &config.cargo_program,
        &cargo_args,
        Some(&config.repo_root),
    )
    .await?;
    if !cargo_probe.output.status.success() {
        return Err(InventoryError::Process {
            command: command_display(&config.cargo_program, &cargo_args),
            detail: String::from_utf8_lossy(&cargo_probe.output.stderr).into_owned(),
        });
    }
    let cargo_metadata = output_text(&cargo_probe);
    let cargo = parse_cargo_metadata(&cargo_metadata)?;
    cx.checkpoint().map_err(|_| InventoryError::Cancelled)?;
    let surface_map_path = config.repo_root.join("OMP-SURFACE-MAP.toml");
    let surface_map_input = read_bounded_surface_map(&surface_map_path)?;
    Ok(audit_surface_map(
        &parse_surface_map(&surface_map_input),
        &cargo,
    ))
}

/// Small, pure trigger classifier used by fixture tests and consumers that
/// need to distinguish an absent trigger from a measured one.
pub fn classify_trigger_data(trigger: Option<&str>) -> ProbeState {
    match trigger.map(str::trim) {
        Some(value) if !value.is_empty() => ProbeState::Known,
        _ => ProbeState::Unknown,
    }
}
