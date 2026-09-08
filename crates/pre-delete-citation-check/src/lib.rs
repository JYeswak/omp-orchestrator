#![forbid(unsafe_code)]

//! pre-delete-citation-check — refuses deleting a file that any CLOSED bead cites.
//!
//! THE DEFECT (measured 2026-08-31): 45c613d deleted four bin/ scripts. Two surfaced
//! HOURS later as close-evidence RED (cp-op5uu BAD_PATH bin/omp-idle-dispatch.sh,
//! cp-3k9jq BAD_PATH bin/fleet-composite.py), with everything downstream UNRUN — a
//! gate refusing every dispatch, far from the mistake that caused it.
//!
//! KEYED ON THE STAGED DELETION, not on "path missing from tree": 160 cited bin script
//! paths existed, 9 were absent from every working tree, but only 4 were EVER PRESENT
//! and removed (git log --diff-filter=D proves it). A gate keyed on absence would be
//! 56% false-positive on day one.
//!
//! SCANS close_reason AND comments: cp-3k9jq's close_reason is 104 chars with zero
//! path citations, but its comments cite bin/fleet-composite.py in three places. A
//! gate scanning only close reasons passes this deletion and the incident recurs.
//!
//! ESCAPE HATCH THAT IS RECORDED, NOT SILENT: all four of today's deletions were
//! CORRECT (the scripts were replaced by Rust crates). The override names the
//! superseding artifact and the caller writes a comment onto each affected bead so
//! the citation gets REPAIRED, not bypassed.
//! ENFORCES: a staged deletion is refused when any readable CLOSED bead cites that path.
//! STILL PASSES: unrelated deletions and a checked, citation-free closed-bead set.
//! PROVENANCE: the staged deletion list and the tracker JSON are the two inputs; a missing or
//! empty tracker is an error, never evidence that no bead cites the deletion.

use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

pub use omp_types::named_outcomes::ChildOutcome;

/// Deadline for `git diff --cached --diff-filter=D --name-only`.
///
/// A local index read with no lock contention; 30s is two orders of magnitude
/// above any observed value and exists to terminate a wedged child, not to
/// police a slow one.
pub const GIT_DIFF_DEADLINE: Duration = Duration::from_secs(30);

/// Deadline for `br list --json --status closed`.
///
/// DELIBERATELY GENEROUS, and the number is argued rather than picked. This gate
/// runs on the COMMIT path against a ~31 MB `.beads/beads.db` with several live
/// writers, where AGENTS.md records reads at 40-250s, one `br comments add` at
/// 56.7s under contention, and a close attempt held for 290s. A ceiling below
/// that band would fire on a HEALTHY-but-contended read and refuse every commit
/// in the repo -- the over-strict-gate failure the acceptance for this fix names,
/// and the same defect as the `mail_pending` ceiling set under its subject's own
/// documented deadline, where every measured CANCELED was the caller's SIGTERM
/// landing first. 300s sits above the observed band, so a fire means WEDGED.
pub const BR_LIST_DEADLINE: Duration = Duration::from_secs(300);

/// Spawn `command` under `deadline` and return the TYPED outcome.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-62lz`, P0): both spawns on this
/// commit-path gate used a raw `.output()` with NO DEADLINE. A wedged `br` blocked
/// a `git commit` forever and the operator could not tell "br is thinking" from
/// "br is wedged".
///
/// WHICH BUG THIS IS, because conflating the two is how a fixer leaves the hole
/// open: this is a DEADLINE hole, NOT the undrained-pipe deadlock. `.output()`
/// already drains both stdout and stderr -- that is what it is for -- so the
/// ~64 KiB `try_wait()` deadlock in AGENTS.md's asupersync section never applied
/// here. Its tell is 0% CPU with no children; this one's tell is a live child that
/// never returns. Adding a pipe drain would have "fixed" a bug that was not present
/// and left this one intact.
///
/// A TIMEOUT IS NOT A VERDICT. The deadline arm returns
/// `ChildOutcome::TimedOut { group_killed: true }` and never `Completed`, so no
/// caller can read a killed child's empty stdout as "nothing found". `SpawnFailed`
/// stays distinct from `TimedOut` because the remedies differ: a PATH/env problem
/// versus a wedged subject.
///
/// `bounded_output` rather than `bounded_status`: this crate CAPTURES stdout and
/// parses it. `subprocess-contract` makes the child its own process-group leader
/// and signals the GROUP on the deadline, so grandchildren cannot survive at
/// ppid=1 -- which is why this does not hand-roll a timer.
pub fn run_bounded(command: &mut Command, deadline: Duration) -> ChildOutcome {
    match subprocess_contract::bounded_output(command, deadline) {
        subprocess_contract::BoundedOutcome::Completed(output) => ChildOutcome::Completed {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        subprocess_contract::BoundedOutcome::TimedOut => ChildOutcome::TimedOut {
            after_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
            group_killed: true,
        },
        subprocess_contract::BoundedOutcome::Unspawned(error) => ChildOutcome::SpawnFailed {
            message: error.to_string(),
        },
    }
}

/// A closed bead whose blob (close_reason or comments) cites a deleted path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationConflict {
    /// The path being deleted.
    pub deleted_path: String,
    /// The bead that cites it.
    pub bead_id: String,
    /// Which surface: "close_reason" or "comment[N]".
    pub field: String,
}

impl fmt::Display for CitationConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} cites deleted path \"{}\" in {}",
            self.bead_id, self.deleted_path, self.field
        )
    }
}

/// A closed bead's citable text surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedBead {
    pub id: String,
    pub close_reason: String,
    pub comments: Vec<String>,
}

/// Check whether any closed bead cites any of the staged-deletion paths.
///
/// Pure: takes the deletion list and the closed beads, returns the conflicts.
/// The caller wires git and br; this function only cross-references.
pub fn check_deletions(
    staged_deletions: &[String],
    closed_beads: &[ClosedBead],
) -> Vec<CitationConflict> {
    let mut conflicts = Vec::new();
    for bead in closed_beads {
        for deletion in staged_deletions {
            if bead.close_reason.contains(deletion.as_str()) {
                conflicts.push(CitationConflict {
                    deleted_path: deletion.clone(),
                    bead_id: bead.id.clone(),
                    field: "close_reason".to_owned(),
                });
            }
            for (index, comment) in bead.comments.iter().enumerate() {
                if comment.contains(deletion.as_str()) {
                    conflicts.push(CitationConflict {
                        deleted_path: deletion.clone(),
                        bead_id: bead.id.clone(),
                        field: format!("comment[{index}]"),
                    });
                }
            }
        }
    }
    conflicts
}

/// Parse `git diff --cached --diff-filter=D --name-only` output into a list of paths.
pub fn parse_staged_deletions(git_output: &str) -> Vec<String> {
    git_output
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Extract closed beads from `br list --json --status closed` output.
/// The `br list` wraps rows in `.issues`.
pub fn parse_closed_beads(br_json: &str) -> Vec<ClosedBead> {
    let parsed: Result<Value, _> = serde_json::from_str(br_json);
    let Ok(value) = parsed else { return Vec::new() };
    let issues = match value.get("issues").and_then(Value::as_array) {
        Some(issues) => issues,
        None => return Vec::new(),
    };

    let mut beads = Vec::new();
    for issue in issues {
        let status = issue.get("status").and_then(Value::as_str).unwrap_or("");
        if status != "closed" {
            continue;
        }
        let id = issue
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let close_reason = issue
            .get("close_reason")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        // The br JSON does not inline comments; the caller fetches them separately.
        beads.push(ClosedBead {
            id,
            close_reason,
            comments: Vec::new(),
        });
    }
    beads
}

/// Checked tracker parse for the gate boundary. Empty, malformed, or non-record output is
/// restrictive: the deletion cannot be certified safe when the bead store was not read.
pub fn parse_closed_beads_checked(br_json: &str) -> Result<Vec<ClosedBead>, String> {
    let value: Value = serde_json::from_str(br_json)
        .map_err(|error| format!("PRE_DELETE_BEADS_UNREADABLE reason=malformed_json detail={error}"))?;
    let issues = value
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| "PRE_DELETE_BEADS_UNREADABLE reason=missing_issues".to_owned())?;
    if issues.is_empty() {
        return Err("PRE_DELETE_BEADS_EMPTY reason=zero_bead_records_readable".to_owned());
    }
    let beads = parse_closed_beads(br_json);
    if beads.is_empty() {
        return Err("PRE_DELETE_BEADS_EMPTY reason=no_closed_records_readable".to_owned());
    }
    Ok(beads)
}

/// Read closed beads from the `.beads/issues.jsonl` MIRROR, comments included.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-dpa4`): `ClosedBead::comments` exists,
/// [`check_deletions`] scans it, and until this function landed EVERY production caller
/// passed an empty vector. [`parse_closed_beads`] says so in its own body -- *"The br JSON
/// does not inline comments; the caller fetches them separately"* -- and no caller ever
/// did. Measured 2026-09-07: `br list --json --status closed` returns **196 rows and 0 of
/// them carry a `comments` key at all**, so the comment-scanning half of this gate was
/// structurally vacuous, not merely untested.
///
/// WHY THAT IS THE WHOLE POINT OF THE CRATE. This module's own header records the incident:
/// `cp-3k9jq`'s close_reason is 104 chars with zero path citations while its comments cite
/// `bin/fleet-composite.py` in three places, and *"a gate scanning only close reasons passes
/// this deletion and the incident recurs."* Measured in THIS tracker: **14 closed beads cite
/// a `bin/` or `.flywheel/` path ONLY in comments and never in close_reason** -- one of them
/// `omp-orchestrator-pre-delete-citation-check-igk`, the bead that created this gate.
///
/// WHY THE MIRROR AND NOT THE TRACKER. It also removes a subprocess from the COMMIT path.
/// The `br` spawn took the `.beads/.write.lock` contended by several live writers, which is
/// why its caller needed a deadline at all, and any deadline there is wrong in one of two
/// directions: inside the measured 40-250s band it converts contention into a refused commit,
/// above the band it makes the operator wait minutes. A file read has neither failure mode.
///
/// RESTRICTIVE, per the gate boundary: an absent, unreadable, empty, or record-free mirror is
/// an `Err`, never an empty success. A deletion cannot be certified citation-free against an
/// oracle that was not read.
///
/// RESIDUAL, stated: the JSONL is a MIRROR of the database, so a bead closed since the last
/// flush is invisible here. That is strictly narrower than the hole it replaces, which missed
/// every comment on every bead regardless of freshness.
pub fn parse_closed_beads_jsonl_checked(jsonl: &str) -> Result<Vec<ClosedBead>, String> {
    let mut rows = 0usize;
    let mut beads = Vec::new();
    for (index, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        rows += 1;
        let value: Value = serde_json::from_str(line).map_err(|error| {
            format!("PRE_DELETE_BEADS_UNREADABLE reason=malformed_jsonl line={index} detail={error}")
        })?;
        if value.get("status").and_then(Value::as_str).unwrap_or("") != "closed" {
            continue;
        }
        let comments = value
            .get("comments")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|entry| entry.get("text").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        beads.push(ClosedBead {
            id: value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            close_reason: value
                .get("close_reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            comments,
        });
    }
    if rows == 0 {
        return Err("PRE_DELETE_BEADS_EMPTY reason=zero_bead_records_readable".to_owned());
    }
    if beads.is_empty() {
        return Err("PRE_DELETE_BEADS_EMPTY reason=no_closed_records_readable".to_owned());
    }
    Ok(beads)
}

/// Path of the tracker mirror relative to a repository root.
pub fn beads_mirror_path(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".beads").join("issues.jsonl")
}

/// Read and check the mirror at `repo_root`. An absent file is restrictive, NOT an empty pass.
pub fn read_closed_beads_from_mirror(repo_root: &Path) -> Result<Vec<ClosedBead>, String> {
    let path = beads_mirror_path(repo_root);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "PRE_DELETE_BEADS_UNREADABLE reason=mirror_unreadable path={} detail={error}",
            path.display()
        )
    })?;
    parse_closed_beads_jsonl_checked(&text)
}
/// One closed bead whose close reason does not carry an admitted prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseReasonViolation {
    pub bead_id: String,
    pub verdict: ack_spine::close_reason::CloseReasonVerdict,
}

/// The bounded result of checking the closed-bead mirror's close reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseReasonPolicyReport {
    pub closed_beads: usize,
    pub verified: usize,
    pub violations: Vec<CloseReasonViolation>,
}

/// Check every closed mirror row with the canonical ack-spine classifier.
///
/// This is detection, not prevention: direct br close may store an arbitrary reason,
/// and this report is consumed on a later commit when the mirror is staged. An empty
/// record set is an error rather than a clean report, so a missing read cannot pass.
pub fn check_close_reason_policy(
    closed_beads: &[ClosedBead],
) -> Result<CloseReasonPolicyReport, String> {
    if closed_beads.is_empty() {
        return Err(
            "CLOSE_REASON_MIRROR_EMPTY reason=no_closed_records_to_check detection_only=true"
                .to_owned(),
        );
    }

    let mut verified = 0;
    let mut violations = Vec::new();
    for bead in closed_beads {
        let verdict = ack_spine::close_reason::classify_close_reason(Some(&bead.close_reason));
        if verdict.is_verified() {
            verified += 1;
        } else {
            violations.push(CloseReasonViolation {
                bead_id: bead.id.clone(),
                verdict,
            });
        }
    }
    Ok(CloseReasonPolicyReport {
        closed_beads: closed_beads.len(),
        verified,
        violations,
    })
}

/// True when the given repo-root path is inside a git repository with at least one commit.
pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bead(id: &str, reason: &str, comments: &[&str]) -> ClosedBead {
        ClosedBead {
            id: id.to_owned(),
            close_reason: reason.to_owned(),
            comments: comments.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn close_reason_citation_is_detected() {
        // KNOWN-BAD 1: cp-op5uu's close_reason cites bin/omp-idle-dispatch.sh
        let bead = bead(
            "cp-op5uu",
            "MECHANISM: bin/omp-idle-dispatch.sh, cron 1,11,21,31,41,51",
            &[],
        );
        let conflicts = check_deletions(&["bin/omp-idle-dispatch.sh".to_owned()], &[bead]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "close_reason");
        assert_eq!(conflicts[0].bead_id, "cp-op5uu");
    }

    #[test]
    fn comment_citation_is_detected() {
        // KNOWN-BAD 2: cp-3k9jq's close_reason has ZERO paths but its comments
        // cite bin/fleet-composite.py. A gate scanning only close reasons passes
        // this deletion — leg 2 decides whether the gate is worth having.
        let bead = bead(
            "cp-3k9jq",
            "DONE: commit 92a65e4 adds the packet close prefix",
            &["4. Re-run bin/fleet-composite.py and paste the JSON."],
        );
        let conflicts = check_deletions(&["bin/fleet-composite.py".to_owned()], &[bead]);
        assert_eq!(conflicts.len(), 1);
        assert!(
            conflicts[0].field.starts_with("comment"),
            "the citation is in a comment, not the close_reason"
        );
    }

    #[test]
    fn unrelated_deletion_passes() {
        // KNOWN-GOOD: deleting a file that no closed bead cites must PASS.
        let bead = bead("cp-clean", "nothing about deleted files", &[]);
        let conflicts = check_deletions(&["bin/unrelated-thing.sh".to_owned()], &[bead]);
        assert!(conflicts.is_empty(), "no citation -> no conflict");
    }

    #[test]
    fn both_surfaces_checked_independently() {
        let bead = bead(
            "cp-both",
            "cites bin/a.sh in the close_reason",
            &["also cites bin/b.sh in a comment"],
        );
        let conflicts = check_deletions(&["bin/a.sh".to_owned(), "bin/b.sh".to_owned()], &[bead]);
        assert_eq!(conflicts.len(), 2, "both surfaces must be caught");
        assert!(conflicts.iter().any(|c| c.field == "close_reason"));
        assert!(conflicts.iter().any(|c| c.field.starts_with("comment")));
    }

    #[test]
    fn multiple_beads_citing_same_path_all_reported() {
        let beads = vec![
            bead("cp-one", "cites bin/shared.sh here", &[]),
            bead("cp-two", "also cites bin/shared.sh", &[]),
        ];
        let conflicts = check_deletions(&["bin/shared.sh".to_owned()], &beads);
        assert_eq!(conflicts.len(), 2, "every citing bead must be named");
    }

    #[test]
    fn parse_staged_deletions_filters_empty_lines() {
        let git_output = "bin/a.sh\nbin/b.py\n\nbin/c.sh\n";
        let parsed = parse_staged_deletions(git_output);
        assert_eq!(parsed, vec!["bin/a.sh", "bin/b.py", "bin/c.sh"]);
    }
    #[test]
    fn close_reason_policy_accepts_extended_prefixes() {
        let beads = vec![
            bead("good-premise", "PREMISE-FALSE: the measured premise was wrong", &[]),
            bead("good-fixed", "ALREADY-FIXED: landed in 0123456", &[]),
            bead("good-not-required", "MUTATION-NOT-REQUIRED: known-good leg already exists", &[]),
            bead("good-attributed", "MUTATION-ATTRIBUTED: live proof captured", &[]),
            bead("good-done", "DONE: worker=contabo-1 cargo test passed", &[]),
        ];
        let report = check_close_reason_policy(&beads).expect("non-empty mirror report");
        assert_eq!(report.closed_beads, 5);
        assert_eq!(report.verified, 5);
        assert!(report.violations.is_empty(), "{report:?}");
    }

    #[test]
    fn close_reason_policy_names_prose_and_empty_rows() {
        let beads = vec![
            bead("bad-prose", "fixed it", &[]),
            bead("bad-empty", "", &[]),
        ];
        let report = check_close_reason_policy(&beads).expect("non-empty mirror report");
        assert_eq!(report.closed_beads, 2);
        assert_eq!(report.verified, 0);
        assert_eq!(
            report
                .violations
                .iter()
                .map(|violation| violation.bead_id.as_str())
                .collect::<Vec<_>>(),
            vec!["bad-prose", "bad-empty"]
        );
        assert_eq!(report.violations[0].verdict.label(), "CLOSE_REASON_POLICY_REFUSED");
        assert_eq!(report.violations[1].verdict.label(), "CLOSE_REASON_EMPTY");
    }

    #[test]
    fn close_reason_policy_rejects_an_empty_scan() {
        let error = check_close_reason_policy(&[]).expect_err("empty mirror must fail closed");
        assert!(error.contains("CLOSE_REASON_MIRROR_EMPTY"), "{error}");
    }
}
