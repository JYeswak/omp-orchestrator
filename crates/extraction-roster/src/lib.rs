#![forbid(unsafe_code)]

//! Metadata-derived extraction scope and terminal-crate roster.
//!
//! Cargo metadata owns package and dependency topology. Source scanning is
//! limited to the extraction-eligibility predicates and is reported as
//! evidence, never folded into a guessed count.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The subset of Cargo metadata needed for scope and topology.
#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    /// Every package Cargo resolved for the workspace.
    pub packages: Vec<Package>,
    /// Workspace member IDs, retained for provenance.
    #[serde(default)]
    pub workspace_members: Vec<String>,
}

/// A Cargo package identity and its dependencies.
#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    /// Cargo's stable package ID.
    pub id: String,
    /// Package name used for dependency matching.
    pub name: String,
    /// Absolute manifest path emitted by Cargo.
    pub manifest_path: String,
    /// Dependencies as emitted by Cargo metadata.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

/// A Cargo dependency edge.
#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    /// Dependency package name.
    pub name: String,
    /// Present for a local path dependency.
    #[serde(default)]
    pub path: Option<String>,
}

/// A source-only package's mechanical eligibility evidence.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PackageClassification {
    /// Source package name.
    pub name: String,
    /// Mechanical disposition, not a ship verdict.
    pub verdict: String,
    /// Number of path dependencies from Cargo metadata.
    pub path_dependency_count: usize,
    /// Source files naming another repository root.
    pub foreign_repo_names: Vec<String>,
    /// Narrow `.sh`/`.py` literal count in `src`.
    pub script_literal_count: usize,
    /// Current target packages that depend on this source-only package.
    pub target_consumers: usize,
}

/// Authority and byte comparison for a package present in both workspaces.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OverlapRecord {
    /// Shared package name.
    pub name: String,
    /// Whether package directory files match byte-for-byte.
    pub byte_identical: bool,
    /// Authority for the target workspace, pending any deliberate reconciliation.
    pub recommended_authority: String,
    /// Local dependency names present only in the target package.
    pub target_only_path_dependencies: Vec<String>,
}

/// Machine-readable extraction roster.
#[derive(Debug, Clone, Serialize)]
pub struct Roster {
    /// Stable artifact schema identifier.
    pub schema_version: String,
    /// Source manifest used for the Cargo metadata read.
    pub source_manifest: String,
    /// Target manifest used for the Cargo metadata read.
    pub target_manifest: String,
    /// Source workspace package count.
    pub source_package_count: usize,
    /// Target workspace package count.
    pub target_package_count: usize,
    /// Source-only packages with no wrapper/terminal evidence.
    pub extraction_targets: Vec<String>,
    /// Source-only packages that are direct port candidates.
    pub port_candidates: Vec<String>,
    /// Source-only packages mechanically identified as terminal candidates.
    pub terminal_crates: Vec<String>,
    /// Full source-only classification rows.
    pub source_only: Vec<PackageClassification>,
    /// Packages present in both workspaces.
    pub overlaps: Vec<OverlapRecord>,
    /// Target-only package names.
    pub target_only: Vec<String>,
    /// Number of source packages with zero local path dependencies.
    pub zero_path_dependency_crates: usize,
    /// Number of source local path-dependency edges.
    pub path_dependency_edges: usize,
    /// Source packages with no local path dependencies, sorted by name.
    pub leaf_names: Vec<String>,
    /// Explicit scope status for human review.
    pub scope_state: String,
}

impl Roster {
    /// Return overlap names in deterministic order.
    #[must_use]
    pub fn overlap_names(&self) -> Vec<String> {
        self.overlaps.iter().map(|row| row.name.clone()).collect()
    }

    /// Return metadata-derived leaf names.
    #[must_use]
    pub fn leaf_names(&self) -> Vec<String> {
        self.leaf_names.clone()
    }
}

/// Roster construction failures.
#[derive(Debug)]
pub enum RosterError {
    /// No source-only package remained after terminal classification.
    EmptyExtractionTargets,
    /// A workspace contains duplicate package names, making name-based joins unsafe.
    DuplicatePackage { workspace: String, name: String },
    /// A source package could not be inspected.
    Io { path: String, detail: String },
}

impl fmt::Display for RosterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExtractionTargets => {
                formatter.write_str("NOTHING_TO_CHECK: extraction target set is empty")
            }
            Self::DuplicatePackage { workspace, name } => {
                write!(formatter, "{workspace} has duplicate package name `{name}`")
            }
            Self::Io { path, detail } => write!(formatter, "cannot inspect `{path}`: {detail}"),
        }
    }
}

impl std::error::Error for RosterError {}

/// Build a roster from two independently captured Cargo metadata documents.
///
/// `target_repo_name` is the repository root that is ours. `source_repo_name`
/// is retained in the signature for callers to record provenance; foreign
/// repository names are intentionally not excluded merely because the source
/// workspace owns them. A control-plane-only lane is terminal to this target.
pub fn build_roster(
    source: &Metadata,
    target: &Metadata,
    target_repo_name: &str,
    _source_repo_name: &str,
) -> Result<Roster, RosterError> {
    let source_by_name = unique_packages(&source.packages, "source")?;
    let target_by_name = unique_packages(&target.packages, "target")?;

    let mut source_names: Vec<String> = source_by_name.keys().cloned().collect();
    source_names.sort();
    let mut target_names: Vec<String> = target_by_name.keys().cloned().collect();
    target_names.sort();

    let overlap_set: BTreeSet<&str> = source_by_name
        .keys()
        .filter_map(|name| target_by_name.contains_key(name).then_some(name.as_str()))
        .collect();
    let target_only: Vec<String> = target_by_name
        .keys()
        .filter(|name| !source_by_name.contains_key(*name))
        .cloned()
        .collect();

    let mut overlaps = Vec::new();
    for name in &overlap_set {
        let source_package = source_by_name[*name];
        let target_package = target_by_name[*name];
        let byte_identical = package_files_equal(source_package, target_package)?;
        let source_path_dependencies = path_dependency_names(source_package);
        let mut target_only_path_dependencies: Vec<String> = path_dependency_names(target_package)
            .into_iter()
            .filter(|dependency| !source_path_dependencies.contains(dependency))
            .collect();
        target_only_path_dependencies.sort();
        overlaps.push(OverlapRecord {
            name: (*name).to_owned(),
            byte_identical,
            recommended_authority: "target-workspace".to_owned(),
            target_only_path_dependencies,
        });
    }

    let mut leaf_names = Vec::new();
    let mut path_dependency_edges = 0usize;
    for package in source_by_name.values() {
        let path_count = path_dependency_names(package).len();
        path_dependency_edges += path_count;
        if path_count == 0 {
            leaf_names.push(package.name.clone());
        }
    }
    leaf_names.sort();

    let mut source_only = Vec::new();
    for name in source_names {
        if target_by_name.contains_key(&name) {
            continue;
        }
        let package = source_by_name[&name];
        let (foreign_repo_names, script_literal_count) =
            source_evidence(package, target_repo_name)?;
        let target_consumers = target
            .packages
            .iter()
            .flat_map(|consumer| consumer.dependencies.iter())
            .filter(|dependency| dependency.path.is_some() && dependency.name == package.name)
            .count();
        let verdict = if !foreign_repo_names.is_empty() {
            "TERMINAL_CANDIDATE"
        } else if script_literal_count > 0 {
            "WRAPPER_CANDIDATE"
        } else if target_consumers == 0 {
            "UNNEEDED_CANDIDATE"
        } else {
            "PORT_CANDIDATE"
        };
        source_only.push(PackageClassification {
            name: package.name.clone(),
            verdict: verdict.to_owned(),
            path_dependency_count: path_dependency_names(package).len(),
            foreign_repo_names,
            script_literal_count,
            target_consumers,
        });
    }

    let terminal_crates: Vec<String> = source_only
        .iter()
        .filter(|row| row.verdict == "TERMINAL_CANDIDATE")
        .map(|row| row.name.clone())
        .collect();
    let extraction_targets: Vec<String> = source_only
        .iter()
        .filter(|row| row.verdict != "TERMINAL_CANDIDATE")
        .map(|row| row.name.clone())
        .collect();
    let port_candidates: Vec<String> = source_only
        .iter()
        .filter(|row| row.verdict == "PORT_CANDIDATE")
        .map(|row| row.name.clone())
        .collect();
    if extraction_targets.is_empty() {
        return Err(RosterError::EmptyExtractionTargets);
    }

    let scope_state = if source_only
        .iter()
        .any(|row| row.verdict != "PORT_CANDIDATE")
    {
        "HUMAN_DECISION_REQUIRED"
    } else {
        "MECHANICALLY_ENUMERATED"
    };

    Ok(Roster {
        schema_version: "omp.extraction-roster.v1".to_owned(),
        source_manifest: String::new(),
        target_manifest: String::new(),
        source_package_count: source.packages.len(),
        target_package_count: target.packages.len(),
        extraction_targets,
        port_candidates,
        terminal_crates,
        source_only,
        overlaps,
        target_only,
        zero_path_dependency_crates: leaf_names.len(),
        path_dependency_edges,
        leaf_names,
        scope_state: scope_state.to_owned(),
    })
}

fn unique_packages<'a>(
    packages: &'a [Package],
    workspace: &str,
) -> Result<BTreeMap<String, &'a Package>, RosterError> {
    let mut result = BTreeMap::new();
    for package in packages {
        if result.insert(package.name.clone(), package).is_some() {
            return Err(RosterError::DuplicatePackage {
                workspace: workspace.to_owned(),
                name: package.name.clone(),
            });
        }
    }
    Ok(result)
}

fn path_dependency_names(package: &Package) -> Vec<String> {
    let mut names: Vec<String> = package
        .dependencies
        .iter()
        .filter(|dependency| dependency.path.is_some())
        .map(|dependency| dependency.name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}

fn package_files_equal(source: &Package, target: &Package) -> Result<bool, RosterError> {
    let source_files = collect_files(package_root(source).join("src"))?;
    let target_files = collect_files(package_root(target).join("src"))?;
    Ok(source_files == target_files)
}

fn package_root(package: &Package) -> PathBuf {
    Path::new(&package.manifest_path)
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf)
}

fn collect_files(root: PathBuf) -> Result<BTreeMap<String, Vec<u8>>, RosterError> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return Ok(files);
    }
    collect_files_recursive(&root, &root, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), RosterError> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|error| RosterError::Io {
            path: current.display().to_string(),
            detail: error.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| RosterError::Io {
            path: current.display().to_string(),
            detail: error.to_string(),
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        if path.is_dir() {
            collect_files_recursive(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| RosterError::Io {
                    path: path.display().to_string(),
                    detail: error.to_string(),
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path).map_err(|error| RosterError::Io {
                path: path.display().to_string(),
                detail: error.to_string(),
            })?;
            files.insert(relative, bytes);
        }
    }
    Ok(())
}

fn source_evidence(
    package: &Package,
    target_repo_name: &str,
) -> Result<(Vec<String>, usize), RosterError> {
    let src = package_root(package).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files)?;
    files.sort();

    let mut foreign = BTreeSet::new();
    let mut script_literal_count = 0usize;
    for path in files {
        let bytes = fs::read(&path).map_err(|error| RosterError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        let text = String::from_utf8_lossy(&bytes);
        for literal in code_string_literals(&text) {
            script_literal_count += literal.matches(".sh").count();
            script_literal_count += literal.matches(".py").count();
            let mut remainder = literal.as_str();
            while let Some(index) = remainder.find("/Developer/") {
                let after = &remainder[index + "/Developer/".len()..];
                let root = path_component(after);
                if !root.is_empty() && root != target_repo_name {
                    foreign.insert(root.to_owned());
                }
                remainder = after;
            }
        }
    }
    Ok((foreign.into_iter().collect(), script_literal_count))
}

fn path_component(value: &str) -> &str {
    let end = value
        .char_indices()
        .find(|(_, character)| {
            !(character.is_ascii_alphanumeric()
                || *character == '-'
                || *character == '_'
                || *character == '.')
        })
        .map_or(value.len(), |(index, _)| index);
    &value[..end]
}

fn code_string_literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut literals = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            index += 2;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        index += 1;
        let start = index;
        let mut literal = String::new();
        while index < bytes.len() {
            match bytes[index] {
                b'\\' if index + 1 < bytes.len() => {
                    literal.push(bytes[index] as char);
                    literal.push(bytes[index + 1] as char);
                    index += 2;
                }
                b'"' => {
                    break;
                }
                byte => {
                    literal.push(byte as char);
                    index += 1;
                }
            }
        }
        if index < bytes.len() && bytes[index] == b'"' {
            literals.push(literal);
            index += 1;
        } else {
            let _ = start;
            break;
        }
    }
    literals
}

fn collect_rs_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), RosterError> {
    if !root.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(root)
        .map_err(|error| RosterError::Io {
            path: root.display().to_string(),
            detail: error.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| RosterError::Io {
            path: root.display().to_string(),
            detail: error.to_string(),
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}
