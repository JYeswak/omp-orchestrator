#![forbid(unsafe_code)]

//! Writer-gated preregistration for loop-owned plan and evidence changes.
//!
//! A hypothesis is useful only when it exists in the committed parent before
//! the evidence it predicts. This crate owns the pure, deterministic rule;
//! callers supply the committed parent revision and the changed evidence rows.
//! Git access and the plan writer remain caller concerns.

use asupersync::Cx;
use asupersync::process::{Command, Stdio};
use asupersync::time::timeout;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::time::Duration;
pub const SCHEMA_VERSION: &str = "preregistration-gate/v1";
pub const HYPOTHESES_PATH: &str = "docs/plan/HYPOTHESES.jsonl";
pub const GENERATED_PLAN_PATH: &str = "docs/PLAN.md";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub prediction: String,
    pub falsifier: String,
    pub evidence_scope: String,
    pub recorded_commit: String,
    #[serde(default)]
    pub observed_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRow {
    pub path: String,
    pub line: usize,
    pub hypothesis_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateReport {
    pub schema_version: &'static str,
    pub base_revision: String,
    pub hypothesis_count: usize,
    pub changed_path_count: usize,
    pub checked_path_count: usize,
    pub checked_evidence_row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    EmptyRegistry,
    MalformedRegistry {
        line: usize,
        detail: String,
    },
    DuplicateHypothesis {
        id: String,
        line: usize,
    },
    MissingField {
        line: usize,
        field: &'static str,
    },
    InvalidBaseRevision {
        revision: String,
    },
    UnscopedChangedPath {
        path: String,
    },
    MissingEvidenceRows {
        path: String,
    },
    MissingHypothesisCitation {
        path: String,
        line: usize,
    },
    UnknownHypothesis {
        id: String,
        path: String,
        line: usize,
    },
    HypothesisOutOfParent {
        id: String,
        recorded_commit: String,
        base_revision: String,
    },
    HypothesisScopeMismatch {
        id: String,
        path: String,
        line: usize,
    },
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRegistry => f.write_str("PREREGISTRATION_ERROR empty hypothesis registry"),
            Self::MalformedRegistry { line, detail } => {
                write!(
                    f,
                    "PREREGISTRATION_ERROR malformed registry line={line} {detail}"
                )
            }
            Self::DuplicateHypothesis { id, line } => {
                write!(
                    f,
                    "PREREGISTRATION_ERROR duplicate hypothesis id={id} line={line}"
                )
            }
            Self::MissingField { line, field } => {
                write!(f, "PREREGISTRATION_ERROR missing field={field} line={line}")
            }
            Self::InvalidBaseRevision { revision } => {
                write!(f, "PREREGISTRATION_ERROR invalid base revision={revision}")
            }
            Self::UnscopedChangedPath { path } => {
                write!(
                    f,
                    "PREREGISTRATION_ERROR changed path has no hypothesis scope path={path}"
                )
            }
            Self::MissingEvidenceRows { path } => {
                write!(
                    f,
                    "PREREGISTRATION_ERROR changed evidence has no rows path={path}"
                )
            }
            Self::MissingHypothesisCitation { path, line } => write!(
                f,
                "PREREGISTRATION_ERROR evidence row lacks hypothesis_id path={path} line={line}"
            ),
            Self::UnknownHypothesis { id, path, line } => write!(
                f,
                "PREREGISTRATION_ERROR evidence cites unknown hypothesis_id={id} path={path} line={line}"
            ),
            Self::HypothesisOutOfParent {
                id,
                recorded_commit,
                base_revision,
            } => write!(
                f,
                "PREREGISTRATION_ERROR hypothesis={id} recorded_commit={recorded_commit} is not the committed parent={base_revision}"
            ),
            Self::HypothesisScopeMismatch { id, path, line } => write!(
                f,
                "PREREGISTRATION_ERROR hypothesis={id} does not scope evidence path={path} line={line}"
            ),
        }
    }
}

impl std::error::Error for GateError {}

fn required_text(value: &str) -> bool {
    !value.trim().is_empty()
}

fn is_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn scope_matches(scope: &str, path: &str) -> bool {
    let scope = scope.trim_end_matches('/');
    path == scope || path.starts_with(&format!("{scope}/"))
}

fn is_generated_or_registry(path: &str) -> bool {
    path == HYPOTHESES_PATH || path == GENERATED_PLAN_PATH
}

/// Parse the committed JSONL hypothesis registry.
///
/// Empty input is an error. Unknown JSON fields are tolerated for forward
/// compatibility, but all fields required by the writer contract must be
/// present and non-empty.
pub fn parse_registry(input: &str) -> Result<Vec<Hypothesis>, GateError> {
    if input.trim().is_empty() {
        return Err(GateError::EmptyRegistry);
    }
    let mut rows = Vec::new();
    for (index, raw) in input.lines().enumerate() {
        let line = index + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let row: Hypothesis =
            serde_json::from_str(raw).map_err(|error| GateError::MalformedRegistry {
                line,
                detail: error.to_string(),
            })?;
        for (field, value) in [
            ("id", row.id.as_str()),
            ("prediction", row.prediction.as_str()),
            ("falsifier", row.falsifier.as_str()),
            ("evidence_scope", row.evidence_scope.as_str()),
            ("recorded_commit", row.recorded_commit.as_str()),
        ] {
            if !required_text(value) {
                return Err(GateError::MissingField { line, field });
            }
        }
        if !is_revision(&row.recorded_commit) {
            return Err(GateError::MalformedRegistry {
                line,
                detail: "recorded_commit must be a 40-character hexadecimal revision".to_owned(),
            });
        }
        if row.evidence_scope.starts_with('/') || row.evidence_scope.contains("..") {
            return Err(GateError::MalformedRegistry {
                line,
                detail: "evidence_scope must be a relative path without parent traversal"
                    .to_owned(),
            });
        }
        if rows
            .iter()
            .any(|existing: &Hypothesis| existing.id == row.id)
        {
            return Err(GateError::DuplicateHypothesis { id: row.id, line });
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(GateError::EmptyRegistry);
    }
    Ok(rows)
}

/// Validate a pre-write plan/evidence transition against the committed parent.
///
/// The caller must derive base_revision, committed_revisions, changed_paths,
/// and evidence_rows from the repository using its bounded process boundary.
/// This pure function enforces that every changed non-generated path is scoped
/// by a hypothesis already present in the committed parent history, and every
/// changed evidence row cites a known, correctly scoped hypothesis.
pub fn validate_pre_write(
    registry_input: &str,
    base_revision: &str,
    committed_revisions: &[String],
    changed_paths: &[String],
    evidence_rows: &[EvidenceRow],
) -> Result<GateReport, GateError> {
    if !is_revision(base_revision) {
        return Err(GateError::InvalidBaseRevision {
            revision: base_revision.to_owned(),
        });
    }
    let hypotheses = parse_registry(registry_input)?;
    for hypothesis in &hypotheses {
        if !committed_revisions
            .iter()
            .any(|revision| revision == &hypothesis.recorded_commit)
        {
            return Err(GateError::HypothesisOutOfParent {
                id: hypothesis.id.clone(),
                recorded_commit: hypothesis.recorded_commit.clone(),
                base_revision: base_revision.to_owned(),
            });
        }
    }

    let mut checked_path_count = 0;
    for path in changed_paths {
        if is_generated_or_registry(path) {
            continue;
        }
        let scoped = hypotheses
            .iter()
            .any(|hypothesis| scope_matches(&hypothesis.evidence_scope, path));
        if !scoped {
            return Err(GateError::UnscopedChangedPath { path: path.clone() });
        }
        checked_path_count += 1;
    }

    let mut checked_evidence_row_count = 0;
    for row in evidence_rows {
        if !changed_paths.iter().any(|path| path == &row.path) {
            continue;
        }
        let id = row
            .hypothesis_id
            .as_deref()
            .filter(|value| required_text(value))
            .ok_or_else(|| GateError::MissingHypothesisCitation {
                path: row.path.clone(),
                line: row.line,
            })?;
        let hypothesis = hypotheses
            .iter()
            .find(|hypothesis| hypothesis.id == id)
            .ok_or_else(|| GateError::UnknownHypothesis {
                id: id.to_owned(),
                path: row.path.clone(),
                line: row.line,
            })?;
        if !scope_matches(&hypothesis.evidence_scope, &row.path) {
            return Err(GateError::HypothesisScopeMismatch {
                id: id.to_owned(),
                path: row.path.clone(),
                line: row.line,
            });
        }
        checked_evidence_row_count += 1;
    }

    Ok(GateReport {
        schema_version: SCHEMA_VERSION,
        base_revision: base_revision.to_owned(),
        hypothesis_count: hypotheses.len(),
        changed_path_count: changed_paths.len(),
        checked_path_count,
        checked_evidence_row_count,
    })
}

fn parse_evidence_rows_impl(
    path: String,
    input: &str,
    selected_lines: Option<&[usize]>,
) -> Result<Vec<EvidenceRow>, GateError> {
    if input.trim().is_empty() {
        return Err(GateError::MissingEvidenceRows { path });
    }
    let mut rows = Vec::new();
    for (index, raw) in input.lines().enumerate() {
        let line = index + 1;
        if raw.trim().is_empty() || selected_lines.is_some_and(|lines| !lines.contains(&line)) {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(raw).map_err(|error| GateError::MalformedRegistry {
                line,
                detail: format!("evidence JSONL: {error}"),
            })?;
        rows.push(EvidenceRow {
            path: path.clone(),
            line,
            hypothesis_id: value
                .get("hypothesis_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        });
    }
    if rows.is_empty() {
        return Err(GateError::MissingEvidenceRows { path });
    }
    Ok(rows)
}

/// Parse every non-empty JSONL evidence row into citation rows.
pub fn parse_evidence_rows(
    path: impl Into<String>,
    input: &str,
) -> Result<Vec<EvidenceRow>, GateError> {
    parse_evidence_rows_impl(path.into(), input, None)
}

/// Parse only the newly added line numbers from a changed evidence file.
pub fn parse_evidence_rows_at(
    path: impl Into<String>,
    input: &str,
    selected_lines: &[usize],
) -> Result<Vec<EvidenceRow>, GateError> {
    parse_evidence_rows_impl(path.into(), input, Some(selected_lines))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryInputs {
    pub base_revision: String,
    pub registry: String,
    pub committed_revisions: Vec<String>,
    pub changed_paths: Vec<String>,
    pub evidence_rows: Vec<EvidenceRow>,
}

const MAX_GIT_OUTPUT_BYTES: usize = 1024 * 1024;
const GIT_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

async fn git_text(cx: &Cx, root: &Path, args: &[&str]) -> Result<String, String> {
    cx.checkpoint()
        .map_err(|_| "PREREGISTRATION_ERROR git probe cancelled".to_owned())?;
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::Null)
        .stdout(Stdio::Pipe)
        .stderr(Stdio::Pipe);
    let output = match timeout(
        cx.now_for_observability(),
        GIT_PROBE_TIMEOUT,
        subprocess_contract::run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Err(format!("PREREGISTRATION_ERROR git probe failed: {error}"));
        }
        Err(_) => {
            return Err(format!(
                "PREREGISTRATION_ERROR git probe timed out after {}s",
                GIT_PROBE_TIMEOUT.as_secs()
            ));
        }
    };
    let total_bytes = output.stdout.len().saturating_add(output.stderr.len());
    if total_bytes > MAX_GIT_OUTPUT_BYTES {
        return Err(format!(
            "PREREGISTRATION_ERROR git probe output exceeds {} bytes",
            MAX_GIT_OUTPUT_BYTES
        ));
    }
    if !output.status.success() {
        return Err(format!(
            "PREREGISTRATION_ERROR git {:?} exited {:?}: {}",
            args,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("PREREGISTRATION_ERROR git output is not UTF-8: {error}"))
}

fn add_changed_path(paths: &mut Vec<String>, path: &str) {
    let path = path.trim();
    if !path.is_empty() && !paths.iter().any(|existing| existing == path) {
        paths.push(path.to_owned());
    }
}

pub fn added_line_numbers(diff: &str) -> Vec<usize> {
    let mut next_line = None;
    let mut added = Vec::new();
    for line in diff.lines() {
        if line.starts_with("@@") {
            next_line = line
                .split_whitespace()
                .find(|part| part.starts_with('+'))
                .and_then(|part| part.trim_start_matches('+').split(',').next())
                .and_then(|number| number.parse::<usize>().ok());
            continue;
        }
        let Some(line_number) = next_line.as_mut() else {
            continue;
        };
        if line.starts_with("+++") {
            continue;
        }
        if line.starts_with('+') {
            added.push(*line_number);
            *line_number += 1;
        } else if !line.starts_with('-') {
            *line_number += 1;
        }
    }
    added
}

/// Collect the committed parent and the working-tree change set for witnesses
/// and the plan writer. All git processes use the bounded subprocess contract.
pub async fn collect_repository_inputs(cx: &Cx, root: &Path) -> Result<RepositoryInputs, String> {
    let base_revision = git_text(cx, root, &["rev-parse", "HEAD"])
        .await?
        .trim()
        .to_owned();
    let registry_ref = format!("HEAD:{HYPOTHESES_PATH}");
    let registry = git_text(cx, root, &["show", &registry_ref]).await?;
    let committed_revisions = git_text(cx, root, &["rev-list", "HEAD"])
        .await?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let diff_output = git_text(
        cx,
        root,
        &["diff", "--name-only", "HEAD", "--", "docs/plan"],
    )
    .await?;
    let status_output = git_text(
        cx,
        root,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            "docs/plan",
        ],
    )
    .await?;
    let mut changed_paths = Vec::new();
    for path in diff_output.lines() {
        add_changed_path(&mut changed_paths, path);
    }
    for line in status_output.lines() {
        let path = line
            .get(3..)
            .unwrap_or(line)
            .split(" -> ")
            .last()
            .unwrap_or(line);
        add_changed_path(&mut changed_paths, path);
    }
    changed_paths.sort();
    let mut evidence_rows = Vec::new();
    for path in &changed_paths {
        if path.starts_with("docs/plan/") && path.ends_with(".jsonl") && path != HYPOTHESES_PATH {
            let text = std::fs::read_to_string(root.join(path)).map_err(|error| {
                format!("PREREGISTRATION_ERROR cannot read changed evidence {path}: {error}")
            })?;
            let diff = git_text(cx, root, &["diff", "--unified=0", "HEAD", "--", path]).await?;
            let selected = added_line_numbers(&diff);
            let selected = if selected.is_empty() {
                (1..=text.lines().count()).collect::<Vec<_>>()
            } else {
                selected
            };
            evidence_rows.extend(
                parse_evidence_rows_at(path.clone(), &text, &selected)
                    .map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(RepositoryInputs {
        base_revision,
        registry,
        committed_revisions,
        changed_paths,
        evidence_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "0123456789abcdef0123456789abcdef01234567";
    const REGISTRY: &str = r#"{"id":"h1","prediction":"plan remains materialized","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"0123456789abcdef0123456789abcdef01234567","observed_result":null}"#;

    #[test]
    fn empty_registry_is_error() {
        assert_eq!(parse_registry("\n"), Err(GateError::EmptyRegistry));
    }

    #[test]
    fn valid_parent_and_cited_evidence_pass() {
        let paths = vec!["docs/plan/01-idea.md".to_owned(), "docs/PLAN.md".to_owned()];
        let evidence = vec![EvidenceRow {
            path: "docs/plan/01-idea.md".to_owned(),
            line: 1,
            hypothesis_id: Some("h1".to_owned()),
        }];
        let report = validate_pre_write(REGISTRY, BASE, &[BASE.to_owned()], &paths, &evidence)
            .expect("valid gate");
        assert_eq!(report.hypothesis_count, 1);
        assert_eq!(report.checked_path_count, 1);
        assert_eq!(report.checked_evidence_row_count, 1);
    }

    #[test]
    fn uncited_evidence_is_refused() {
        let paths = vec!["docs/plan/FINDINGS.jsonl".to_owned()];
        let evidence = vec![EvidenceRow {
            path: paths[0].clone(),
            line: 4,
            hypothesis_id: None,
        }];
        let error = validate_pre_write(REGISTRY, BASE, &[BASE.to_owned()], &paths, &evidence)
            .expect_err("uncited evidence must refuse");
        assert!(matches!(
            error,
            GateError::MissingHypothesisCitation { line: 4, .. }
        ));
    }

    #[test]
    fn changed_path_without_scope_is_refused() {
        let paths = vec!["src/main.rs".to_owned()];
        let error = validate_pre_write(REGISTRY, BASE, &[BASE.to_owned()], &paths, &[])
            .expect_err("unscoped path must refuse");
        assert!(matches!(error, GateError::UnscopedChangedPath { .. }));
    }

    #[test]
    fn recorded_commit_must_be_the_parent() {
        let paths = Vec::new();
        let error = validate_pre_write(
            REGISTRY,
            "fedcba9876543210fedcba9876543210fedcba98",
            &[],
            &paths,
            &[],
        )
        .expect_err("future or different parent must refuse");
        assert!(matches!(error, GateError::HypothesisOutOfParent { .. }));
    }
}
