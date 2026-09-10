#![forbid(unsafe_code)]

//! The CI citation: bind ONE gate verdict to ONE run id, with an `InputManifest`.
//!
//! # THE DEFECT THIS MODULE WAS REBUILT AROUND (omp-orchestrator-pxhmd)
//!
//! **CI had never passed and could not.** Measured 2026-09-09, `gh run list --limit 100`:
//! **75 failure, 15 cancelled, ZERO success.** The cause was not a compile error and not the
//! sixteen failing crates — it was this file's contract, and it was *structurally* unsatisfiable:
//!
//! ```text
//! .github/workflows/gate.yml:154   --ci-citation "${{ github.run_id }}"
//!    -> resolve_identity(<this run's own id>)
//!    -> gh run view <this run's own id> --log
//!    -> CI_CITATION_GH_UNAVAILABLE detail=run 34326877717 is still in progress; logs
//!       will be available when it is complete
//!    -> ##[error]Process completed with exit code 4.
//! ```
//!
//! **GitHub does not serve logs for an in-progress run, and the step asking for them IS part of
//! that run.** With `if: always()` and no `continue-on-error`, the step fired even when everything
//! upstream was green, so *no commit could ever produce a green run.* The verdict the gate had
//! already measured — `GATE_RUNNER crates=87 pass=67 fail=16 …` — was computed, printed, and then
//! discarded, which is why 90 consecutive reds carried **zero information**: "sixteen crates fail"
//! and "the citation is impossible" were indistinguishable from outside.
//!
//! # THE FIX: THE PROCESS THAT MEASURED THE VERDICT EMITS IT (`local`), rather than re-reading it
//!
//! `--ci-citation local` reads the aggregate line `gate-runner --run` wrote to its own artifact in
//! the same job, and takes the run identity from `GITHUB_RUN_ID` / `GITHUB_SHA`. **No network, no
//! log store, no race** — and the citation is about the run it is in, which is what the workflow
//! was asking for and could never get.
//!
//! The alternative on the table was *citing the PREVIOUS completed run*. It was rejected: it needs
//! a token and a network round-trip to obtain a **stale** aggregate, it reports the wrong commit's
//! verdict under this commit's run id, and it is `NoRun`-red on the first run of any fresh fork.
//! Reading GitHub's log store to recover a number this process just computed was the defect, not
//! the argument passed to it.
//!
//! **Deleting the step or adding `continue-on-error` was NOT an option** — that is gate
//! self-weakening. Both remain absent, and `tests/workflow_shape.rs` fails if either appears.
//!
//! # AND THE SELF-REFERENCE IS NOW A TYPED REFUSAL AT THE SOURCE, BOTH SELECTORS
//!
//! Fixing only the workflow argument leaves the trap armed: `resolve_identity("latest")` asks
//! `gh run list --limit 1`, which **from inside CI also resolves to self** — the newest run is the
//! one you are in. So [`CitationError::SelfReference`] fires for an explicit self id *before* any
//! `gh` call, and for `latest` the instant it resolves to `GITHUB_RUN_ID`. A self-citation is now
//! named as such instead of masquerading as `GhUnavailable`.
//!
//! In-progress-ness gets its own code too, for the same reason: *"gh is missing"* and *"the run
//! has not finished"* have different remedies, and conflating them is what sent readers to the
//! toolchain for nine days.
//!
//! # NO-CLAIM
//!
//! The `local` aggregate is trusted to belong to the current run because `--run` **removes the
//! file before measuring and writes it only after**, so a stale aggregate cannot survive a run
//! that failed early. Nothing cryptographically binds the file to `GITHUB_RUN_ID`; on a shared
//! developer checkout an aggregate from an earlier local `--run` could be cited under a later run
//! id. In CI the workspace is fresh per run, which is where the guarantee is load-bearing.

use input_manifest::InputManifest;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

const GH_DEADLINE: Duration = Duration::from_secs(30);

/// The selector that cites THIS run from the aggregate `--run` just wrote. No network.
pub const LOCAL_SELECTOR: &str = "local";
/// The selector that asks GitHub for the newest run. Self-referential from inside CI.
pub const LATEST_SELECTOR: &str = "latest";

pub const EXIT_NO_RUN: u8 = 3;
pub const EXIT_GH_UNAVAILABLE: u8 = 4;
pub const EXIT_RUN_UNAVAILABLE: u8 = 5;
pub const EXIT_LOG_LINE_MISSING: u8 = 6;
pub const EXIT_AGGREGATE_MALFORMED: u8 = 7;
pub const EXIT_RUN_METADATA_MALFORMED: u8 = 8;
/// The selector names the run this process is executing inside. Unsatisfiable by construction.
pub const EXIT_SELF_REFERENCE: u8 = 9;
/// The run exists but has not finished, so its logs do not exist yet. Distinct from `gh` failing.
pub const EXIT_RUN_IN_PROGRESS: u8 = 10;
/// `local` was asked for and its inputs were absent. An absent aggregate is an ERROR, never a pass.
pub const EXIT_LOCAL_AGGREGATE_UNAVAILABLE: u8 = 11;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationError {
    NoRun,
    GhUnavailable(String),
    RunUnavailable {
        run_id: String,
        detail: String,
    },
    LogLineMissing {
        run_id: String,
    },
    AggregateMalformed {
        line: String,
    },
    RunMetadataMalformed {
        detail: String,
    },
    /// The selector resolved to the run this process is running inside.
    SelfReference {
        run_id: String,
        selector: String,
    },
    /// The run is real and not finished. Its logs cannot exist yet; waiting is the remedy.
    RunInProgress {
        run_id: String,
        detail: String,
    },
    /// `local` could not be satisfied: no run context, or no aggregate to cite.
    LocalAggregateUnavailable {
        detail: String,
    },
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
            Self::SelfReference { .. } => EXIT_SELF_REFERENCE,
            Self::RunInProgress { .. } => EXIT_RUN_IN_PROGRESS,
            Self::LocalAggregateUnavailable { .. } => EXIT_LOCAL_AGGREGATE_UNAVAILABLE,
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
            Self::SelfReference { .. } => "CI_CITATION_SELF_REFERENCE",
            Self::RunInProgress { .. } => "CI_CITATION_RUN_IN_PROGRESS",
            Self::LocalAggregateUnavailable { .. } => "CI_CITATION_LOCAL_AGGREGATE_UNAVAILABLE",
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
            Self::SelfReference { run_id, selector } => {
                write!(
                    f,
                    "CI_CITATION_SELF_REFERENCE run_id={run_id} selector={selector} \
                     detail=this process is running INSIDE run {run_id}, whose logs GitHub will \
                     not serve until it completes -- cite `local` to emit the aggregate this run \
                     measured, or name a PRIOR completed run"
                )
            }
            Self::RunInProgress { run_id, detail } => {
                write!(
                    f,
                    "CI_CITATION_RUN_IN_PROGRESS run_id={run_id} detail={detail}"
                )
            }
            Self::LocalAggregateUnavailable { detail } => {
                write!(f, "CI_CITATION_LOCAL_AGGREGATE_UNAVAILABLE {detail}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Citation {
    schema_version: &'static str,
    run_id: String,
    head_sha: String,
    /// `in_process` when the measuring process emitted it, `gh_log` when read back from GitHub.
    /// A consumer must be able to tell "the gate said so" from "GitHub's log store said so".
    aggregate_source: &'static str,
    aggregate: String,
    input_manifest: InputManifest,
}

const SCHEMA_VERSION: &str = "omp-orchestrator/gate-runner-ci-citation/v2";
const SOURCE_IN_PROCESS: &str = "in_process";
const SOURCE_GH_LOG: &str = "gh_log";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunIdentity {
    run_id: String,
    head_sha: String,
}

/// What one `gh` invocation produced, as strings.
///
/// A domain reply rather than `std::process::Output` for one reason: `ExitStatus` cannot be
/// constructed portably, so an `Output`-shaped seam is untestable — and an untestable seam is
/// exactly how `cite()` and `resolve_identity()` reached production with **zero** coverage while
/// the suite stayed green on three `parse_aggregate` fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GhReply {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl GhReply {
    fn detail(&self) -> String {
        let trimmed = self.stderr.trim();
        if trimmed.is_empty() {
            self.stdout.trim().to_owned()
        } else {
            trimmed.to_owned()
        }
    }

    fn haystack(&self) -> String {
        format!("{}{}", self.stdout, self.stderr).to_ascii_lowercase()
    }

    fn is_not_found(&self) -> bool {
        let text = self.haystack();
        text.contains("404") || text.contains("not found")
    }

    /// GitHub's verbatim refusal, from run 34326877717:
    /// `run 34326877717 is still in progress; logs will be available when it is complete`.
    fn is_in_progress(&self) -> bool {
        let text = self.haystack();
        text.contains("still in progress") || text.contains("logs will be available")
    }
}

type GhCall<'a> = &'a dyn Fn(&[&str]) -> Result<GhReply, CitationError>;

/// Everything the citation needs from outside the process, injected rather than read inline.
///
/// The environment reads and the `gh` spawn are the two things a test cannot have, and they are
/// the two things the unsatisfiable path was made of.
struct CiContext<'a> {
    /// `GITHUB_RUN_ID`: the run this process is executing INSIDE, when there is one.
    self_run_id: Option<String>,
    /// `GITHUB_SHA`: the commit that run is gating.
    head_sha: Option<String>,
    /// Where `gate-runner --run` wrote the aggregate it measured.
    aggregate_path: PathBuf,
    gh: GhCall<'a>,
}

impl<'a> CiContext<'a> {
    fn from_env(aggregate_path: &Path, gh: GhCall<'a>) -> Self {
        Self {
            self_run_id: non_empty_env("GITHUB_RUN_ID"),
            head_sha: non_empty_env("GITHUB_SHA"),
            aggregate_path: aggregate_path.to_path_buf(),
            gh,
        }
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn gh_exec(args: &[&str]) -> Result<GhReply, CitationError> {
    let mut command = Command::new("gh");
    command.args(args);
    match bounded_output(&mut command, GH_DEADLINE) {
        BoundedOutcome::Completed(output) => Ok(GhReply {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
        BoundedOutcome::TimedOut => Err(CitationError::GhUnavailable(
            "gh command timed out after 30s".to_owned(),
        )),
        BoundedOutcome::Unspawned(error) => Err(CitationError::GhUnavailable(error.to_string())),
    }
}

/// THE GUARD. A run cannot cite itself, and this is where both selectors meet it.
fn refuse_self_reference(
    run_id: &str,
    selector: &str,
    ctx: &CiContext<'_>,
) -> Result<(), CitationError> {
    if ctx.self_run_id.as_deref() == Some(run_id) {
        return Err(CitationError::SelfReference {
            run_id: run_id.to_owned(),
            selector: selector.to_owned(),
        });
    }
    Ok(())
}

fn resolve_identity(selector: &str, ctx: &CiContext<'_>) -> Result<RunIdentity, CitationError> {
    if selector != LATEST_SELECTOR {
        // BEFORE the network, not after: asking GitHub about a run we are inside cannot
        // succeed, and spending a round-trip to be told so is how the error came back as
        // `GhUnavailable` instead of naming the real defect.
        refuse_self_reference(selector, selector, ctx)?;
        let reply = (ctx.gh)(&[
            "run", "view", selector, "--json", "headSha", "--jq", ".headSha",
        ])?;
        if !reply.ok {
            let detail = reply.detail();
            if reply.is_in_progress() {
                return Err(CitationError::RunInProgress {
                    run_id: selector.to_owned(),
                    detail,
                });
            }
            if reply.is_not_found() {
                return Err(CitationError::RunUnavailable {
                    run_id: selector.to_owned(),
                    detail,
                });
            }
            return Err(CitationError::GhUnavailable(detail));
        }
        let head_sha = reply.stdout.trim().to_owned();
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

    let reply = (ctx.gh)(&[
        "run",
        "list",
        "--limit",
        "1",
        "--json",
        "databaseId,headSha,status,conclusion",
    ])?;
    if !reply.ok {
        return Err(CitationError::GhUnavailable(reply.detail()));
    }
    let rows: Value =
        serde_json::from_str(&reply.stdout).map_err(|error| CitationError::RunMetadataMalformed {
            detail: error.to_string(),
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
    // AND HERE IS THE OTHER HALF OF THE TRAP. From inside CI the newest run IS this run, so
    // `latest` is self-referential for exactly the same reason the explicit id was. It can only
    // be caught after resolution, because nothing knows which run `latest` names until GitHub
    // says so.
    refuse_self_reference(&run_id, LATEST_SELECTOR, ctx)?;
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

pub fn run(selector: &str, aggregate_path: &Path) -> ExitCode {
    let gh: fn(&[&str]) -> Result<GhReply, CitationError> = gh_exec;
    let ctx = CiContext::from_env(aggregate_path, &gh);
    match cite(selector, &ctx) {
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

fn cite(selector: &str, ctx: &CiContext<'_>) -> Result<Citation, CitationError> {
    if selector == LOCAL_SELECTOR {
        return cite_local(ctx);
    }
    let identity = resolve_identity(selector, ctx)?;
    let reply = (ctx.gh)(&["run", "view", &identity.run_id, "--log"])?;
    if !reply.ok {
        let detail = reply.detail();
        if reply.is_in_progress() {
            return Err(CitationError::RunInProgress {
                run_id: identity.run_id,
                detail,
            });
        }
        if reply.is_not_found() {
            return Err(CitationError::RunUnavailable {
                run_id: identity.run_id,
                detail,
            });
        }
        return Err(CitationError::GhUnavailable(detail));
    }
    let aggregate = parse_aggregate(&reply.stdout).map_err(|error| match error {
        CitationError::LogLineMissing { .. } => CitationError::LogLineMissing {
            run_id: identity.run_id.clone(),
        },
        other => other,
    })?;
    Ok(Citation {
        schema_version: SCHEMA_VERSION,
        run_id: identity.run_id,
        head_sha: identity.head_sha,
        aggregate_source: SOURCE_GH_LOG,
        aggregate,
        input_manifest: InputManifest::full(),
    })
}

/// Cite THIS run from the aggregate the gate step measured. No `gh`, no network, no race.
fn cite_local(ctx: &CiContext<'_>) -> Result<Citation, CitationError> {
    let run_id = ctx.self_run_id.clone().ok_or_else(|| {
        CitationError::LocalAggregateUnavailable {
            detail: "detail=GITHUB_RUN_ID is unset or empty, so there is no run to bind the \
                     verdict to -- `local` is for a step running INSIDE a run"
                .to_owned(),
        }
    })?;
    let head_sha =
        ctx.head_sha
            .clone()
            .ok_or_else(|| CitationError::LocalAggregateUnavailable {
                detail: format!(
                    "run_id={run_id} detail=GITHUB_SHA is unset or empty -- a citation with a \
                     guessed commit is worse than a refusal"
                ),
            })?;
    let text = std::fs::read_to_string(&ctx.aggregate_path).map_err(|error| {
        CitationError::LocalAggregateUnavailable {
            detail: format!(
                "run_id={run_id} path={} detail={error} -- `gate-runner --run` removes this file \
                 before measuring and writes it only after, so its absence means no verdict was \
                 produced. An absent aggregate is an ERROR, never a pass.",
                ctx.aggregate_path.display()
            ),
        }
    })?;
    let aggregate = parse_aggregate(&text).map_err(|error| match error {
        CitationError::LogLineMissing { .. } => CitationError::LogLineMissing {
            run_id: run_id.clone(),
        },
        other => other,
    })?;
    Ok(Citation {
        schema_version: SCHEMA_VERSION,
        run_id,
        head_sha,
        aggregate_source: SOURCE_IN_PROCESS,
        aggregate,
        input_manifest: InputManifest::full(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real captured aggregate: run 34326877717, the run that proved CI unsatisfiable.
    const REAL_AGGREGATE: &str =
        "GATE_RUNNER crates=87 pass=67 fail=16 unmeasurable=4 short=0 no_tests=0";
    const SELF_RUN_ID: &str = "34326877717";
    const SELF_HEAD_SHA: &str = "6884fd481a4f1a8c3f9a2e0b7d4c651e0a9f3b21";

    /// A `gh` seam that REFUSES to be called. Any code path reaching the network with this
    /// installed fails the test by panic — which is how the anti-vacuity claim is discharged
    /// rather than asserted.
    fn forbidden_gh(args: &[&str]) -> Result<GhReply, CitationError> {
        panic!("gh must not be invoked on this path, but was called with {args:?}");
    }

    fn ok_reply(stdout: &str) -> Result<GhReply, CitationError> {
        Ok(GhReply {
            ok: true,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    fn err_reply(stderr: &str) -> Result<GhReply, CitationError> {
        Ok(GhReply {
            ok: false,
            stdout: String::new(),
            stderr: stderr.to_owned(),
        })
    }

    fn ctx<'a>(
        self_run_id: Option<&str>,
        head_sha: Option<&str>,
        aggregate_path: &Path,
        gh: GhCall<'a>,
    ) -> CiContext<'a> {
        CiContext {
            self_run_id: self_run_id.map(ToOwned::to_owned),
            head_sha: head_sha.map(ToOwned::to_owned),
            aggregate_path: aggregate_path.to_path_buf(),
            gh,
        }
    }

    /// A path this test owns. Not `mktemp`: `AGENTS.md` wants an owned, reapable location, and the
    /// process id plus the leg name gives one that never collides with a concurrent test binary.
    fn scratch_file(leg: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gate-runner-citation-{}-{leg}.line",
            std::process::id()
        ));
        std::fs::write(&path, contents).expect("scratch aggregate is writable");
        path
    }

    fn absent_file(leg: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gate-runner-citation-{}-{leg}-absent.line",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    // ── THE DEFECT, FIRED ON ────────────────────────────────────────────────────────────────

    /// FIRES-ON-KNOWN-BAD, and it is the production input verbatim.
    ///
    /// `gate.yml` passed `${{ github.run_id }}` while running inside that run. This is that call.
    /// It must refuse BEFORE the `gh` spawn — proven by `forbidden_gh`, which panics if reached.
    #[test]
    fn citing_this_runs_own_id_is_refused_before_any_gh_call() {
        let path = absent_file("self-explicit");
        let context = ctx(
            Some(SELF_RUN_ID),
            Some(SELF_HEAD_SHA),
            &path,
            &forbidden_gh,
        );
        let error = cite(SELF_RUN_ID, &context).expect_err(
            "a run citing itself is unsatisfiable and must refuse, not attempt the log fetch",
        );
        assert_eq!(error.code(), "CI_CITATION_SELF_REFERENCE");
        assert_eq!(error.exit_code(), 9, "the wire contract, pinned to a literal");
        assert_ne!(
            error.exit_code(),
            EXIT_GH_UNAVAILABLE,
            "the 90-run outage reported this as GhUnavailable, which sent readers to the \
             toolchain for nine days"
        );
        assert!(
            error.to_string().contains("running INSIDE run"),
            "the message must name the mechanism: {error}"
        );
    }

    /// THE OTHER HALF OF THE TRAP (`resolve_identity` line for `latest`).
    ///
    /// From inside CI the newest run is this run, so `latest` is self-referential too. Fixing only
    /// the workflow argument would leave this armed. The seam asserts the log fetch is never
    /// reached, and returns a real `gh run list` row shape.
    #[test]
    fn the_latest_selector_is_refused_when_it_resolves_to_this_run() {
        let path = absent_file("self-latest");
        let seen_log = std::cell::Cell::new(false);
        let gh = |args: &[&str]| {
            if args.contains(&"--log") {
                seen_log.set(true);
            }
            assert!(
                args.contains(&"list"),
                "`latest` must resolve through `gh run list`, got {args:?}"
            );
            ok_reply(&format!(
                r#"[{{"databaseId":{SELF_RUN_ID},"headSha":"{SELF_HEAD_SHA}","status":"in_progress","conclusion":null}}]"#
            ))
        };
        let context = ctx(Some(SELF_RUN_ID), Some(SELF_HEAD_SHA), &path, &gh);
        let error = cite(LATEST_SELECTOR, &context)
            .expect_err("`latest` resolving to this run is the same defect and must refuse");
        assert_eq!(error.code(), "CI_CITATION_SELF_REFERENCE");
        assert_eq!(error.exit_code(), 9);
        assert!(
            !seen_log.get(),
            "the doomed `gh run view --log` must never be reached once self-reference is known"
        );
    }

    /// An in-progress OTHER run is named as such, not as `gh` being broken.
    ///
    /// The stderr is GitHub's verbatim refusal from run 34326877717.
    #[test]
    fn an_in_progress_run_is_distinguished_from_gh_being_unavailable() {
        let path = absent_file("in-progress");
        let gh = |args: &[&str]| {
            if args.contains(&"--log") {
                return err_reply(
                    "run 99999999999 is still in progress; logs will be available when it is \
                     complete",
                );
            }
            ok_reply(SELF_HEAD_SHA)
        };
        let context = ctx(Some(SELF_RUN_ID), Some(SELF_HEAD_SHA), &path, &gh);
        let error = cite("99999999999", &context).expect_err("an unfinished run has no logs");
        assert_eq!(error.code(), "CI_CITATION_RUN_IN_PROGRESS");
        assert_eq!(error.exit_code(), 10);
        assert_ne!(error.exit_code(), EXIT_GH_UNAVAILABLE);
    }

    /// ANTI-VACUITY for `local`: nothing to cite is an ERROR, never a pass.
    ///
    /// Without this leg the fix would be indistinguishable from deleting the citation: a step that
    /// exits 0 having read nothing is the vacuous green this repo has already paid for.
    #[test]
    fn a_missing_local_aggregate_is_an_error_never_a_pass() {
        let path = absent_file("local-missing");
        let context = ctx(
            Some(SELF_RUN_ID),
            Some(SELF_HEAD_SHA),
            &path,
            &forbidden_gh,
        );
        let error = cite(LOCAL_SELECTOR, &context)
            .expect_err("an absent aggregate must not produce a citation");
        assert_eq!(error.code(), "CI_CITATION_LOCAL_AGGREGATE_UNAVAILABLE");
        assert_eq!(error.exit_code(), 11);

        // Present but with no aggregate line is the OTHER absence, and keeps its own code.
        let empty = scratch_file("local-empty", "the run died before it measured anything\n");
        let context = ctx(
            Some(SELF_RUN_ID),
            Some(SELF_HEAD_SHA),
            &empty,
            &forbidden_gh,
        );
        let error =
            cite(LOCAL_SELECTOR, &context).expect_err("a file with no aggregate line is not a pass");
        assert_eq!(error.code(), "CI_CITATION_LOG_LINE_MISSING");
        assert_eq!(error.exit_code(), 6);
        let _ = std::fs::remove_file(&empty);
    }

    /// And no run context is a refusal rather than a fabricated identity.
    #[test]
    fn local_without_a_run_context_refuses_instead_of_guessing() {
        let present = scratch_file("no-context", REAL_AGGREGATE);
        let context = ctx(None, Some(SELF_HEAD_SHA), &present, &forbidden_gh);
        let error = cite(LOCAL_SELECTOR, &context).expect_err("no GITHUB_RUN_ID, no citation");
        assert_eq!(error.code(), "CI_CITATION_LOCAL_AGGREGATE_UNAVAILABLE");
        assert!(error.to_string().contains("GITHUB_RUN_ID"), "{error}");

        let context = ctx(Some(SELF_RUN_ID), None, &present, &forbidden_gh);
        let error = cite(LOCAL_SELECTOR, &context).expect_err("no GITHUB_SHA, no citation");
        assert_eq!(error.code(), "CI_CITATION_LOCAL_AGGREGATE_UNAVAILABLE");
        assert!(error.to_string().contains("GITHUB_SHA"), "{error}");
        let _ = std::fs::remove_file(&present);
    }

    // ── KNOWN-GOOD: THE LEGITIMATE CASES STILL WORK ─────────────────────────────────────────

    /// KNOWN-GOOD (mandatory). The mode the workflow now uses cites successfully.
    ///
    /// An attack-only suite ships an over-strict gate that gets routed around — which is exactly
    /// what 90 unread reds were. This leg is the proof the fix is usable: real `cite()`, real
    /// `parse_aggregate`, real `Citation`, and the network provably untouched.
    #[test]
    fn the_local_selector_cites_the_aggregate_this_run_measured() {
        let path = scratch_file("local-good", &format!("{REAL_AGGREGATE}\n"));
        let context = ctx(
            Some(SELF_RUN_ID),
            Some(SELF_HEAD_SHA),
            &path,
            &forbidden_gh,
        );
        let citation = cite(LOCAL_SELECTOR, &context).expect("a measured aggregate is citable");
        assert_eq!(citation.run_id, SELF_RUN_ID);
        assert_eq!(citation.head_sha, SELF_HEAD_SHA);
        assert_eq!(citation.aggregate, REAL_AGGREGATE);
        assert_eq!(citation.aggregate_source, "in_process");
        let json = serde_json::to_value(&citation).expect("citation json");
        assert_eq!(json["input_manifest"]["state"], "FULL");
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        let _ = std::fs::remove_file(&path);
    }

    /// KNOWN-GOOD (mandatory). A COMPLETED run, cited by id, still works end to end.
    ///
    /// This is the path the guard must not have broken: a different, finished run is legitimate to
    /// cite, goes through `resolve_identity` and the log fetch, and yields `gh_log` provenance.
    #[test]
    fn a_completed_other_run_still_cites_correctly_through_the_gh_seam() {
        let path = absent_file("remote-good");
        let completed = "34171417882";
        let sha = "666ec909f16de2967e931708b83f6e051ea977c6";
        let gh = |args: &[&str]| {
            if args.contains(&"--log") {
                // Shaped like a real `gh run view --log`: prefixed columns and surrounding noise.
                return ok_reply(&format!(
                    "gate\tstep\t2026-09-09T08:33:40Z GATE_RUNNER_CHECKS executed=20 failed=8\n\
                     {REAL_AGGREGATE}\n\
                     gate\tstep\t2026-09-09T08:33:40Z PASS crate=ack-spine targets=9\n"
                ));
            }
            assert!(
                args.contains(&"headSha"),
                "identity resolution must ask for headSha, got {args:?}"
            );
            ok_reply(&format!("{sha}\n"))
        };
        // We are inside a DIFFERENT run, which is the normal CI condition.
        let context = ctx(Some(SELF_RUN_ID), Some(SELF_HEAD_SHA), &path, &gh);
        let citation =
            cite(completed, &context).expect("a completed run's log is citable and must stay so");
        assert_eq!(citation.run_id, completed);
        assert_eq!(citation.head_sha, sha);
        assert_eq!(citation.aggregate, REAL_AGGREGATE);
        assert_eq!(citation.aggregate_source, "gh_log");
    }

    /// The guard is keyed on the INPUT, not hardwired. `latest` naming another run still resolves.
    #[test]
    fn the_latest_selector_still_resolves_when_it_names_another_run() {
        let path = absent_file("latest-good");
        let other = "34295273236";
        let sha = "1d5af33889a1b2c3d4e5f60718293a4b5c6d7e8f";
        let gh = |args: &[&str]| {
            if args.contains(&"--log") {
                return ok_reply(&format!("{REAL_AGGREGATE}\n"));
            }
            ok_reply(&format!(
                r#"[{{"databaseId":{other},"headSha":"{sha}","status":"completed","conclusion":"success"}}]"#
            ))
        };
        let context = ctx(Some(SELF_RUN_ID), Some(SELF_HEAD_SHA), &path, &gh);
        let citation = cite(LATEST_SELECTOR, &context).expect("a non-self latest is citable");
        assert_eq!(citation.run_id, other);
        assert_eq!(citation.head_sha, sha);
    }

    /// Outside CI there is no self run id, so nothing is self-referential and the guard is inert.
    #[test]
    fn without_a_run_context_no_selector_is_self_referential() {
        let path = absent_file("no-self");
        let gh = |args: &[&str]| {
            if args.contains(&"--log") {
                return ok_reply(&format!("{REAL_AGGREGATE}\n"));
            }
            ok_reply(&format!("{SELF_HEAD_SHA}\n"))
        };
        let context = ctx(None, None, &path, &gh);
        let citation = cite(SELF_RUN_ID, &context)
            .expect("from a laptop, run 34326877717 is just a completed run");
        assert_eq!(citation.run_id, SELF_RUN_ID);
    }

    // ── THE WIRE CONTRACT, BOTH AXES ────────────────────────────────────────────────────────

    /// MUTATION TARGET: reason code AND exit code, pinned together, against LITERALS.
    ///
    /// `assert_eq!(err.exit_code(), EXIT_GH_UNAVAILABLE)` is a TAUTOLOGY against the constant and
    /// survives renumbering it — which is why the previous suite could not have caught an exit
    /// code collapse. Mutating `EXIT_GH_UNAVAILABLE` from 4 to 3 must turn this RED, both because
    /// the literal stops matching and because two reason codes stop being distinct.
    #[test]
    fn every_reason_code_is_pinned_to_its_exit_code_and_they_are_distinct() {
        let table: Vec<(&str, u8, CitationError)> = vec![
            ("CI_CITATION_NO_RUN", 3, CitationError::NoRun),
            (
                "CI_CITATION_GH_UNAVAILABLE",
                4,
                CitationError::GhUnavailable("gh missing".to_owned()),
            ),
            (
                "CI_CITATION_RUN_UNAVAILABLE",
                5,
                CitationError::RunUnavailable {
                    run_id: "999999999999999".to_owned(),
                    detail: "HTTP 404".to_owned(),
                },
            ),
            (
                "CI_CITATION_LOG_LINE_MISSING",
                6,
                CitationError::LogLineMissing {
                    run_id: "34326877717".to_owned(),
                },
            ),
            (
                "CI_CITATION_AGGREGATE_MALFORMED",
                7,
                CitationError::AggregateMalformed {
                    line: "GATE_RUNNER crates=x".to_owned(),
                },
            ),
            (
                "CI_CITATION_RUN_METADATA_MALFORMED",
                8,
                CitationError::RunMetadataMalformed {
                    detail: "empty headSha".to_owned(),
                },
            ),
            (
                "CI_CITATION_SELF_REFERENCE",
                9,
                CitationError::SelfReference {
                    run_id: "34326877717".to_owned(),
                    selector: "34326877717".to_owned(),
                },
            ),
            (
                "CI_CITATION_RUN_IN_PROGRESS",
                10,
                CitationError::RunInProgress {
                    run_id: "34326877717".to_owned(),
                    detail: "still in progress".to_owned(),
                },
            ),
            (
                "CI_CITATION_LOCAL_AGGREGATE_UNAVAILABLE",
                11,
                CitationError::LocalAggregateUnavailable {
                    detail: "path=/nowhere".to_owned(),
                },
            ),
        ];
        let mut seen_codes = std::collections::BTreeSet::new();
        let mut seen_exits = std::collections::BTreeSet::new();
        for (code, exit, error) in &table {
            assert_eq!(error.code(), *code, "reason code drifted for {error:?}");
            assert_eq!(
                error.exit_code(),
                *exit,
                "{code} must keep exit code {exit}: an operator's `rc=` is the only signal a \
                 workflow step carries"
            );
            assert!(
                error.to_string().starts_with(code),
                "the rendered line must lead with its own reason code: {error}"
            );
            assert!(seen_codes.insert(*code), "duplicate reason code {code}");
            assert!(
                seen_exits.insert(*exit),
                "duplicate exit code {exit} for {code} -- two causes sharing a code is how a \
                 remedy gets applied to the wrong defect"
            );
        }
        assert_eq!(table.len(), 9, "every variant is in the table");
    }

    #[test]
    fn known_good_aggregate_and_manifest_are_citable() {
        let line = "GATE_RUNNER crates=88 pass=70 fail=16 unmeasurable=2 short=0 no_tests=0";
        let aggregate = parse_aggregate(line).expect("known-good aggregate");
        assert_eq!(aggregate, line);
        let citation = Citation {
            schema_version: SCHEMA_VERSION,
            run_id: "34171417882".to_owned(),
            head_sha: "666ec909f16de2967e931708b83f6e051ea977c6".to_owned(),
            aggregate_source: SOURCE_GH_LOG,
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
}
