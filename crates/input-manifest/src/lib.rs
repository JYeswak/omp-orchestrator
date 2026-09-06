#![forbid(unsafe_code)]

//! Typed coverage of the input an instrument actually consumed.
//!
//! A result without this field is not a result from this crate. `FULL` means the
//! declared input was consumed; `PARTIAL` names the bound and its source; `REFUSED`
//! carries the reason the instrument would not claim coverage. There is deliberately
//! no `Default` implementation: callers must choose the state at construction.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The only three coverage states an emitted instrument result may carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum InputManifest {
    #[serde(rename = "FULL")]
    Full,
    #[serde(rename = "PARTIAL")]
    Partial {
        bound_kind: String,
        bound_value: u64,
        source: String,
    },
    #[serde(rename = "REFUSED")]
    Refused { reason: String },
}

impl InputManifest {
    /// Construct the complete-input state explicitly.
    pub const fn full() -> Self {
        Self::Full
    }

    /// Construct a bounded-input state. A zero bound is an empty result, not a
    /// meaningful partial observation.
    pub fn partial(
        bound_kind: impl Into<String>,
        bound_value: u64,
        source: impl Into<String>,
    ) -> Result<Self, ManifestError> {
        let bound_kind = non_empty(bound_kind.into(), "bound_kind")?;
        let source = non_empty(source.into(), "source")?;
        if bound_value == 0 {
            return Err(ManifestError::EmptyScanSet {
                source: source.clone(),
            });
        }
        Ok(Self::Partial {
            bound_kind,
            bound_value,
            source,
        })
    }

    /// Construct a restrictive state with a named reason.
    pub fn refused(reason: impl Into<String>) -> Result<Self, ManifestError> {
        Ok(Self::Refused {
            reason: non_empty(reason.into(), "reason")?,
        })
    }

    /// Acceptance evidence requires complete input coverage.
    pub fn require_full(&self) -> Result<(), ManifestError> {
        match self {
            Self::Full => Ok(()),
            Self::Partial {
                bound_kind,
                bound_value,
                source,
            } => Err(ManifestError::NonCitable {
                state: "PARTIAL".to_owned(),
                detail: format!(
                    "bound_kind={bound_kind} bound_value={bound_value} source={source}"
                ),
            }),
            Self::Refused { reason } => Err(ManifestError::NonCitable {
                state: "REFUSED".to_owned(),
                detail: reason.clone(),
            }),
        }
    }

    pub const fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}

fn non_empty(value: String, field: &'static str) -> Result<String, ManifestError> {
    if value.trim().is_empty() {
        return Err(ManifestError::InvalidField {
            field: field.to_owned(),
        });
    }
    Ok(value)
}

/// Errors are separate from the three result states. In particular, an empty
/// scan is not `FULL` with zero rows and is not `PARTIAL`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestError {
    EmptyScanSet { source: String },
    InvalidField { field: String },
    NonCitable { state: String, detail: String },
    Io { path: String, detail: String },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScanSet { source } => {
                write!(
                    formatter,
                    "EmptyScanSet source={source} -- no input rows were observed"
                )
            }
            Self::InvalidField { field } => write!(formatter, "MANIFEST_INVALID field={field}"),
            Self::NonCitable { state, detail } => {
                write!(
                    formatter,
                    "MANIFEST_NON_CITABLE state={state} detail={detail}"
                )
            }
            Self::Io { path, detail } => {
                write!(formatter, "MANIFEST_IO path={path} detail={detail}")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// A generic result wrapper for tools that do not need a domain-specific result.
/// The required field is intentional: no constructor can omit the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifested<T> {
    pub result: T,
    pub manifest: InputManifest,
}

impl<T> Manifested<T> {
    pub fn new(result: T, manifest: InputManifest) -> Self {
        Self { result, manifest }
    }

    pub fn require_full(&self) -> Result<(), ManifestError> {
        self.manifest.require_full()
    }
}

/// One `test result:` line from a cargo test run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoTargetCount {
    pub target_index: usize,
    pub source_line: usize,
    pub passed: u64,
    pub failed: u64,
    pub ignored: u64,
    pub measured: u64,
    pub filtered: u64,
}

/// Bound applied while selecting cargo target lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoInputBound {
    All,
    Head { lines: usize },
}

/// A cargo-test result always carries the manifest of the extraction that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoTestResult {
    pub source: String,
    pub targets: Vec<CargoTargetCount>,
    pub manifest: InputManifest,
}

impl CargoTestResult {
    /// Parse all target lines first, then apply any requested output bound and
    /// record the bound instead of silently discarding the rest.
    pub fn extract(
        output: &str,
        bound: CargoInputBound,
        source: impl Into<String>,
    ) -> Result<Self, ManifestError> {
        let source = non_empty(source.into(), "source")?;
        let all = output
            .lines()
            .enumerate()
            .filter_map(|(index, line)| parse_cargo_result_line(line, index + 1))
            .enumerate()
            .map(|(target_index, mut target)| {
                target.target_index = target_index + 1;
                target
            })
            .collect::<Vec<_>>();
        if all.is_empty() {
            return Err(ManifestError::EmptyScanSet { source });
        }

        let (targets, manifest) = match bound {
            CargoInputBound::All => (all, InputManifest::full()),
            CargoInputBound::Head { lines } if lines == 0 => {
                return Err(ManifestError::EmptyScanSet { source });
            }
            CargoInputBound::Head { lines } if lines < all.len() => {
                let total = all.len() as u64;
                let targets = all.into_iter().take(lines).collect();
                let manifest = InputManifest::partial(
                    format!("head -{lines} of N target lines"),
                    total,
                    source.clone(),
                )?;
                (targets, manifest)
            }
            CargoInputBound::Head { .. } => (all, InputManifest::full()),
        };
        Ok(Self {
            source,
            targets,
            manifest,
        })
    }

    pub fn passed(&self) -> u64 {
        self.targets.iter().map(|target| target.passed).sum()
    }
}

fn parse_cargo_result_line(line: &str, source_line: usize) -> Option<CargoTargetCount> {
    let rest = line.trim().strip_prefix("test result:")?.trim();
    Some(CargoTargetCount {
        target_index: 0,
        source_line,
        passed: number_before(rest, " passed")?,
        failed: number_before(rest, " failed").unwrap_or(0),
        ignored: number_before(rest, " ignored").unwrap_or(0),
        measured: number_before(rest, " measured").unwrap_or(0),
        filtered: number_before(rest, " filtered").unwrap_or(0),
    })
}

fn number_before(haystack: &str, needle: &str) -> Option<u64> {
    let end = haystack.find(needle)?;
    haystack[..end]
        .rsplit(|character: char| !character.is_ascii_digit())
        .next()
        .filter(|digits| !digits.is_empty())
        .and_then(|digits| digits.parse().ok())
}

/// Whether a source path belongs to the corpus that can quote the instrument
/// itself rather than the subject under measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Repository,
    SelfReferentialCorpus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSource {
    pub path: String,
    pub kind: SourceKind,
}

pub fn source_kind(path: &str) -> SourceKind {
    let lower = path.to_ascii_lowercase();
    if lower.contains("transcript")
        || lower.contains("pane-transcript")
        || lower.contains("state-file")
        || lower.contains(".flywheel")
        || lower.contains("issues.jsonl")
    {
        SourceKind::SelfReferentialCorpus
    } else {
        SourceKind::Repository
    }
}

/// One row from a source census.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CensusRow {
    pub path: String,
    pub line: usize,
    pub text: String,
}

/// Recursive or bounded source-census mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CensusMode {
    Recursive,
    NonRecursiveGlob { pattern: String },
}

/// A census artifact carries both the result and the input manifest. Empty
/// results carry a typed `EmptyScanSet` error and a restrictive manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CensusResult {
    pub source: String,
    pub input_count: usize,
    pub rows: Vec<CensusRow>,
    pub sources: Vec<ManifestSource>,
    pub manifest: InputManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ManifestError>,
}

/// Scan the live repository for the `GateUnwired` field/variant. The bounded
/// path models `crates/omp-orchestrator/src/*.rs`; the recursive path walks the
/// complete `crates/` tree.
pub fn scan_gate_unwired(root: &Path, mode: CensusMode) -> CensusResult {
    let (base, recursive, source) = match &mode {
        CensusMode::Recursive => (
            root.join("crates"),
            true,
            "recursive crates/ tree".to_owned(),
        ),
        CensusMode::NonRecursiveGlob { pattern } => (
            root.join("crates/omp-orchestrator/src"),
            false,
            pattern.clone(),
        ),
    };
    let mut files = Vec::new();
    collect_rs_files(&base, recursive, &mut files);
    files.sort();

    let mut rows = Vec::new();
    let mut sources = Vec::new();
    for path in &files {
        let display = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let kind = source_kind(&display);
        sources.push(ManifestSource {
            path: display.clone(),
            kind,
        });
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for (line, text) in text.lines().enumerate() {
            if text.contains("WIRED_CALLERS") {
                rows.push(CensusRow {
                    path: display.clone(),
                    line: line + 1,
                    text: text.trim().to_owned(),
                });
            }
        }
    }
    census_result(source, files.len(), rows, sources, mode)
}

fn collect_rs_files(base: &Path, recursive: bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_rs_files(&path, true, files);
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn census_result(
    source: String,
    input_count: usize,
    rows: Vec<CensusRow>,
    sources: Vec<ManifestSource>,
    mode: CensusMode,
) -> CensusResult {
    let self_only = !rows.is_empty()
        && rows
            .iter()
            .all(|row| source_kind(&row.path) == SourceKind::SelfReferentialCorpus);
    let empty = input_count == 0 || rows.is_empty();
    let (manifest, error) = if empty {
        (
            InputManifest::refused(format!("EmptyScanSet source={source}"))
                .expect("non-empty refusal reason"),
            Some(ManifestError::EmptyScanSet {
                source: source.clone(),
            }),
        )
    } else if self_only {
        (
            InputManifest::refused("SELF_REFERENTIAL_CORPUS_ONLY")
                .expect("non-empty refusal reason"),
            Some(ManifestError::NonCitable {
                state: "REFUSED".to_owned(),
                detail: "all hits came from self-referential corpus".to_owned(),
            }),
        )
    } else {
        match mode {
            CensusMode::Recursive => (InputManifest::full(), None),
            CensusMode::NonRecursiveGlob { pattern } => (
                InputManifest::partial("non-recursive glob", input_count as u64, pattern)
                    .expect("non-empty bounded census"),
                None,
            ),
        }
    };
    CensusResult {
        source,
        input_count,
        rows,
        sources,
        manifest,
        error,
    }
}

/// A grade that may be used for acceptance only when its input was complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceGrade {
    pub passed: u64,
    pub manifest: InputManifest,
}

impl AcceptanceGrade {
    pub fn new(passed: u64, manifest: InputManifest) -> Self {
        Self { passed, manifest }
    }

    pub fn can_close_bead(&self) -> Result<(), ManifestError> {
        if self.passed == 0 {
            return Err(ManifestError::EmptyScanSet {
                source: "acceptance grade".to_owned(),
            });
        }
        self.manifest.require_full()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerSuspect {
    pub path: String,
    pub line: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerAuditResult {
    pub source: String,
    pub files_scanned: usize,
    pub rows_scanned: usize,
    pub suspect_rows: Vec<LedgerSuspect>,
    pub manifest: InputManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ManifestError>,
}

/// Sweep existing JSONL ledgers for verdict rows without a declared manifest
/// or with a non-citable manifest. A refusal is the honest result while legacy
/// rows remain unannotated; the census must not bless them as FULL.
pub fn audit_ledgers(root: &Path) -> LedgerAuditResult {
    audit_ledgers_at(root, &root.join(".flywheel"))
}

/// Audit an explicit ledger directory when a remote build excludes hidden
/// repository metadata; the source path remains explicit in the artifact.
pub fn audit_ledgers_at(root: &Path, base: &Path) -> LedgerAuditResult {
    let mut files = Vec::new();
    collect_jsonl_files(base, &mut files);
    files.sort();
    let source = format!("{}/**/*.jsonl", base.display());
    let mut rows_scanned = 0usize;
    let mut suspect_rows = Vec::new();
    for path in &files {
        let display = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let Ok(text) = fs::read_to_string(path) else {
            suspect_rows.push(LedgerSuspect {
                path: display,
                line: 0,
                reason: "UNREADABLE_LEDGER".to_owned(),
            });
            continue;
        };
        for (line, raw) in text.lines().enumerate() {
            if raw.trim().is_empty() {
                continue;
            }
            rows_scanned += 1;
            let value: serde_json::Value = match serde_json::from_str(raw) {
                Ok(value) => value,
                Err(_) => {
                    suspect_rows.push(LedgerSuspect {
                        path: display.clone(),
                        line: line + 1,
                        reason: "MALFORMED_LEDGER_ROW".to_owned(),
                    });
                    continue;
                }
            };
            let manifest = value.get("manifest").or_else(|| {
                value
                    .get("result")
                    .and_then(|result| result.get("manifest"))
            });
            match manifest
                .and_then(|manifest| manifest.get("state"))
                .and_then(serde_json::Value::as_str)
            {
                Some("FULL") => {}
                Some("PARTIAL") => suspect_rows.push(LedgerSuspect {
                    path: display.clone(),
                    line: line + 1,
                    reason: "PARTIAL_RESULT".to_owned(),
                }),
                Some("REFUSED") => suspect_rows.push(LedgerSuspect {
                    path: display.clone(),
                    line: line + 1,
                    reason: "REFUSED_RESULT".to_owned(),
                }),
                Some(other) => suspect_rows.push(LedgerSuspect {
                    path: display.clone(),
                    line: line + 1,
                    reason: format!("UNKNOWN_MANIFEST_STATE={other}"),
                }),
                None => suspect_rows.push(LedgerSuspect {
                    path: display.clone(),
                    line: line + 1,
                    reason: "MISSING_MANIFEST".to_owned(),
                }),
            }
        }
    }
    let empty = files.is_empty() || rows_scanned == 0;
    let (manifest, error) = if empty {
        (
            InputManifest::refused(format!("EmptyScanSet source={source}"))
                .expect("non-empty refusal reason"),
            Some(ManifestError::EmptyScanSet {
                source: source.clone(),
            }),
        )
    } else if suspect_rows.is_empty() {
        (InputManifest::full(), None)
    } else {
        (
            InputManifest::refused("LEDGER_MANIFEST_AUDIT_REFUSED")
                .expect("non-empty refusal reason"),
            Some(ManifestError::NonCitable {
                state: "REFUSED".to_owned(),
                detail: format!("{} suspect ledger rows", suspect_rows.len()),
            }),
        )
    };
    LedgerAuditResult {
        source,
        files_scanned: files.len(),
        rows_scanned,
        suspect_rows,
        manifest,
        error,
    }
}

fn collect_jsonl_files(base: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(tag: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("input-manifest-{tag}-{suffix}"));
        fs::create_dir_all(&path).expect("scratch");
        path
    }

    #[test]
    fn the_wire_has_exactly_three_states_and_partial_is_not_citable() {
        let partial =
            InputManifest::partial("head -1 of N target lines", 3, "cargo test").expect("partial");
        let wire = serde_json::to_string(&partial).expect("serialize");
        assert!(wire.contains("\"state\":\"PARTIAL\""), "{wire}");
        assert!(wire.contains("bound_kind"), "{wire}");
        assert!(partial.require_full().is_err());
        let refused = InputManifest::refused("over budget").expect("refused");
        assert!(refused.require_full().is_err());
        assert!(InputManifest::full().require_full().is_ok());
    }

    #[test]
    fn cargo_head_bound_is_declared_and_all_targets_are_retained_for_full() {
        let output = concat!(
            "test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out\n",
            "test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out\n",
        );
        let partial = CargoTestResult::extract(
            output,
            CargoInputBound::Head { lines: 1 },
            "cargo test -p omp-orchestrator",
        )
        .expect("partial cargo result");
        assert_eq!(partial.targets.len(), 1);
        assert_eq!(partial.passed(), 26);
        match partial.manifest {
            InputManifest::Partial {
                bound_kind,
                bound_value,
                ..
            } => {
                assert_eq!(bound_kind, "head -1 of N target lines");
                assert_eq!(bound_value, 2);
            }
            other => panic!("expected partial, got {other:?}"),
        }
        let full = CargoTestResult::extract(
            output,
            CargoInputBound::All,
            "cargo test -p omp-orchestrator",
        )
        .expect("full cargo result");
        assert!(full.manifest.is_full());
        assert_eq!(full.targets.len(), 2);
        assert_eq!(full.passed(), 39);
    }

    #[test]
    fn empty_cargo_input_is_typed_error_not_full_zero() {
        let error = CargoTestResult::extract("compile failed", CargoInputBound::All, "cargo test")
            .expect_err("no target lines must error");
        assert!(matches!(error, ManifestError::EmptyScanSet { .. }));
    }

    #[test]
    fn self_referential_sources_are_flagged_and_never_full() {
        let root = scratch("self");
        let source = root.join("crates/omp-orchestrator/src");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("main.rs"), "// GateUnwired\n").expect("fixture");
        let mut result = scan_gate_unwired(
            &root,
            CensusMode::NonRecursiveGlob {
                pattern: "state-file/pane-transcript/*.rs".to_owned(),
            },
        );
        result.sources = vec![
            ManifestSource {
                path: "crates/real.rs".to_owned(),
                kind: SourceKind::Repository,
            },
            ManifestSource {
                path: ".flywheel/pane-transcript.txt".to_owned(),
                kind: SourceKind::SelfReferentialCorpus,
            },
        ];
        assert!(matches!(
            source_kind(".flywheel/pane-transcript.txt"),
            SourceKind::SelfReferentialCorpus
        ));
        let only_self = census_result(
            "transcript".to_owned(),
            9,
            (1..=9)
                .map(|line| CensusRow {
                    path: ".flywheel/pane-transcript.txt".to_owned(),
                    line,
                    text: "WIRED_CALLERS".to_owned(),
                })
                .collect(),
            result.sources,
            CensusMode::Recursive,
        );
        assert!(!only_self.manifest.is_full());
        assert!(only_self.error.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn partial_grade_cannot_close_a_bead() {
        let grade = AcceptanceGrade::new(
            26,
            InputManifest::partial("head -1", 2, "cargo test").expect("partial"),
        );
        let error = grade
            .can_close_bead()
            .expect_err("partial evidence cannot close a bead");
        assert!(matches!(
            error,
            ManifestError::NonCitable { state, .. } if state == "PARTIAL"
        ));
    }
    #[test]
    fn ledger_audit_refuses_unmanifested_verdict_rows() {
        let root = scratch("ledger-missing");
        let flywheel = root.join(".flywheel");
        fs::create_dir_all(&flywheel).expect("flywheel");
        fs::write(flywheel.join("events.jsonl"), "{\"status\":\"PASS\"}\n").expect("ledger");
        let result = audit_ledgers(&root);
        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.rows_scanned, 1);
        assert!(!result.suspect_rows.is_empty());
        assert!(result.error.is_some());
        assert!(!result.manifest.is_full());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ledger_audit_full_manifest_is_known_good() {
        let root = scratch("ledger-full");
        let flywheel = root.join(".flywheel");
        fs::create_dir_all(&flywheel).expect("flywheel");
        fs::write(
            flywheel.join("events.jsonl"),
            "{\"status\":\"PASS\",\"manifest\":{\"state\":\"FULL\"}}\n",
        )
        .expect("ledger");
        let result = audit_ledgers(&root);
        assert_eq!(result.rows_scanned, 1);
        assert!(result.suspect_rows.is_empty());
        assert!(result.error.is_none());
        assert!(result.manifest.is_full());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn empty_census_is_error_not_full_zero_or_partial() {
        let root = scratch("empty-census");
        let result = scan_gate_unwired(&root, CensusMode::Recursive);
        assert_eq!(result.input_count, 0);
        assert!(result.rows.is_empty());
        assert!(matches!(
            result.error,
            Some(ManifestError::EmptyScanSet { .. })
        ));
        assert!(matches!(result.manifest, InputManifest::Refused { .. }));
        let _ = fs::remove_dir_all(root);
    }
}
