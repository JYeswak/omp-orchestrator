#![forbid(unsafe_code)]

use input_manifest::InputManifest;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::process::{Command, ExitCode};
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

const GH_DEADLINE: Duration = Duration::from_secs(30);

pub const EXIT_NO_RUN: u8 = 3;
pub const EXIT_GH_UNAVAILABLE: u8 = 4;
pub const EXIT_RUN_UNAVAILABLE: u8 = 5;
pub const EXIT_LOG_LINE_MISSING: u8 = 6;
pub const EXIT_AGGREGATE_MALFORMED: u8 = 7;
pub const EXIT_RUN_METADATA_MALFORMED: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationError {
    NoRun,
    GhUnavailable(String),
    RunUnavailable { run_id: String, detail: String },
    LogLineMissing { run_id: String },
    AggregateMalformed { line: String },
    RunMetadataMalformed { detail: String },
}

impl CitationError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::NoRun => EXIT_NO_RUN,
            Self::GhUnavailable(_) => EXIT_GH_UNAVAILABLE,
            Self::RunUnavailable { .. } => EXIT_RUN_UNAVAILABLE,
            Self::LogLineMissing { .. } => EXIT_LOG_LINE_MISSING,
            Self::AggregateMalformed { .. } => EXIT_AGGREGATE_MALFORMED,
            Self::RunMetadataMalformed { .. } => EXIT_RUN_METADATA_MALFORMED,
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoRun => "CI_CITATION_NO_RUN",
            Self::GhUnavailable(_) => "CI_CITATION_GH_UNAVAILABLE",
            Self::RunUnavailable { .. } => "CI_CITATION_RUN_UNAVAILABLE",
            Self::LogLineMissing { .. } => "CI_CITATION_LOG_LINE_MISSING",
            Self::AggregateMalformed { .. } => "CI_CITATION_AGGREGATE_MALFORMED",
            Self::RunMetadataMalformed { .. } => "CI_CITATION_RUN_METADATA_MALFORMED",
        }
    }
}

impl fmt::Display for CitationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRun => {
                f.write_str("CI_CITATION_NO_RUN no completed or in-progress run was returned")
            }
            Self::GhUnavailable(detail) => {
                write!(f, "CI_CITATION_GH_UNAVAILABLE detail={detail}")
            }
            Self::RunUnavailable { run_id, detail } => {
                write!(
                    f,
                    "CI_CITATION_RUN_UNAVAILABLE run_id={run_id} detail={detail}"
                )
            }
            Self::LogLineMissing { run_id } => {
                write!(
                    f,
                    "CI_CITATION_LOG_LINE_MISSING run_id={run_id} no GATE_RUNNER aggregate line"
                )
            }
            Self::AggregateMalformed { line } => {
                write!(f, "CI_CITATION_AGGREGATE_MALFORMED line={line}")
            }
            Self::RunMetadataMalformed { detail } => {
                write!(f, "CI_CITATION_RUN_METADATA_MALFORMED detail={detail}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Citation {
    schema_version: &'static str,
    run_id: String,
    head_sha: String,
    aggregate: String,
    input_manifest: InputManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunIdentity {
    run_id: String,
    head_sha: String,
}

fn gh_output(args: &[&str]) -> Result<std::process::Output, CitationError> {
    let mut command = Command::new("gh");
    command.args(args);
    match bounded_output(&mut command, GH_DEADLINE) {
        BoundedOutcome::Completed(output) => Ok(output),
        BoundedOutcome::TimedOut => Err(CitationError::GhUnavailable(
            "gh command timed out after 30s".to_owned(),
        )),
        BoundedOutcome::Unspawned(error) => Err(CitationError::GhUnavailable(error.to_string())),
    }
}

fn is_not_found(output: &std::process::Output) -> bool {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    text.contains("404") || text.contains("not found")
}

fn resolve_identity(selector: &str) -> Result<RunIdentity, CitationError> {
    if selector != "latest" {
        let output = gh_output(&[
            "run", "view", selector, "--json", "headSha", "--jq", ".headSha",
        ])?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if is_not_found(&output) {
                return Err(CitationError::RunUnavailable {
                    run_id: selector.to_owned(),
                    detail,
                });
            }
            return Err(CitationError::GhUnavailable(detail));
        }
        let head_sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if head_sha.is_empty() {
            return Err(CitationError::RunMetadataMalformed {
                detail: format!("run_id={selector} empty headSha"),
            });
        }
        return Ok(RunIdentity {
            run_id: selector.to_owned(),
            head_sha,
        });
    }

    let output = gh_output(&[
        "run",
        "list",
        "--limit",
        "1",
        "--json",
        "databaseId,headSha,status,conclusion",
    ])?;
    if !output.status.success() {
        return Err(CitationError::GhUnavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let rows: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        CitationError::RunMetadataMalformed {
            detail: error.to_string(),
        }
    })?;
    let row = rows
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or(CitationError::NoRun)?;
    let run_id = row
        .get("databaseId")
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .ok_or_else(|| CitationError::RunMetadataMalformed {
            detail: "latest row missing databaseId".to_owned(),
        })?;
    let head_sha = row
        .get("headSha")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| CitationError::RunMetadataMalformed {
            detail: format!("run_id={run_id} missing headSha"),
        })?;
    Ok(RunIdentity { run_id, head_sha })
}

fn parse_aggregate(log: &str) -> Result<String, CitationError> {
    let line = log
        .lines()
        .filter(|line| line.starts_with("GATE_RUNNER crates="))
        .last()
        .ok_or_else(|| CitationError::LogLineMissing {
            run_id: "unknown".to_owned(),
        })?;
    let mut values = std::collections::BTreeMap::new();
    for token in line.split_whitespace().skip(1) {
        let (key, value) =
            token
                .split_once('=')
                .ok_or_else(|| CitationError::AggregateMalformed {
                    line: line.to_owned(),
                })?;
        let number = value
            .parse::<u64>()
            .map_err(|_| CitationError::AggregateMalformed {
                line: line.to_owned(),
            })?;
        values.insert(key, number);
    }
    for key in [
        "crates",
        "pass",
        "fail",
        "unmeasurable",
        "short",
        "no_tests",
    ] {
        if !values.contains_key(key) {
            return Err(CitationError::AggregateMalformed {
                line: line.to_owned(),
            });
        }
    }
    if values["pass"] + values["fail"] + values["unmeasurable"] != values["crates"] {
        return Err(CitationError::AggregateMalformed {
            line: line.to_owned(),
        });
    }
    Ok(line.to_owned())
}

pub fn run(selector: &str) -> ExitCode {
    match cite(selector) {
        Ok(citation) => {
            println!(
                "{}",
                serde_json::to_string(&citation).expect("CI citation is serializable")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn cite(selector: &str) -> Result<Citation, CitationError> {
    let identity = resolve_identity(selector)?;
    let output = gh_output(&["run", "view", &identity.run_id, "--log"])?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if is_not_found(&output) {
            return Err(CitationError::RunUnavailable {
                run_id: identity.run_id,
                detail,
            });
        }
        return Err(CitationError::GhUnavailable(detail));
    }
    let aggregate =
        parse_aggregate(&String::from_utf8_lossy(&output.stdout)).map_err(|error| match error {
            CitationError::LogLineMissing { .. } => CitationError::LogLineMissing {
                run_id: identity.run_id.clone(),
            },
            other => other,
        })?;
    Ok(Citation {
        schema_version: "omp-orchestrator/gate-runner-ci-citation/v1",
        run_id: identity.run_id,
        head_sha: identity.head_sha,
        aggregate,
        input_manifest: InputManifest::full(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_good_aggregate_and_manifest_are_citable() {
        let line = "GATE_RUNNER crates=88 pass=70 fail=16 unmeasurable=2 short=0 no_tests=0";
        let aggregate = parse_aggregate(line).expect("known-good aggregate");
        assert_eq!(aggregate, line);
        let citation = Citation {
            schema_version: "omp-orchestrator/gate-runner-ci-citation/v1",
            run_id: "34171417882".to_owned(),
            head_sha: "666ec909f16de2967e931708b83f6e051ea977c6".to_owned(),
            aggregate,
            input_manifest: InputManifest::full(),
        };
        let json = serde_json::to_value(citation).expect("citation json");
        assert_eq!(json["input_manifest"]["state"], "FULL");
        assert_eq!(json["run_id"], "34171417882");
        assert_eq!(json["head_sha"], "666ec909f16de2967e931708b83f6e051ea977c6");
    }

    #[test]
    fn missing_and_malformed_aggregates_have_distinct_codes() {
        let missing = parse_aggregate("workflow completed without the gate summary")
            .expect_err("missing aggregate");
        assert_eq!(missing.code(), "CI_CITATION_LOG_LINE_MISSING");
        assert_eq!(missing.exit_code(), EXIT_LOG_LINE_MISSING);
        let malformed = parse_aggregate(
            "GATE_RUNNER crates=88 pass=70 fail=16 unmeasurable=x short=0 no_tests=0",
        )
        .expect_err("malformed aggregate");
        assert_eq!(malformed.code(), "CI_CITATION_AGGREGATE_MALFORMED");
        assert_eq!(malformed.exit_code(), EXIT_AGGREGATE_MALFORMED);
        assert_ne!(missing.exit_code(), malformed.exit_code());
    }

    #[test]
    fn no_run_and_gh_errors_are_distinct() {
        assert_eq!(CitationError::NoRun.code(), "CI_CITATION_NO_RUN");
        assert_eq!(CitationError::NoRun.exit_code(), EXIT_NO_RUN);
        let gh = CitationError::GhUnavailable("gh missing".to_owned());
        assert_eq!(gh.code(), "CI_CITATION_GH_UNAVAILABLE");
        assert_eq!(gh.exit_code(), EXIT_GH_UNAVAILABLE);
        let absent = CitationError::RunUnavailable {
            run_id: "999999999999999".to_owned(),
            detail: "HTTP 404".to_owned(),
        };
        assert_eq!(absent.code(), "CI_CITATION_RUN_UNAVAILABLE");
        assert_eq!(absent.exit_code(), EXIT_RUN_UNAVAILABLE);
    }
}
