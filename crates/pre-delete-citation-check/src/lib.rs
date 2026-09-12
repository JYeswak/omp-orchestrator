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

/// Deadline for the `git` reads on the commit path: the staged-deletion diff and the
/// two index reads behind [`read_index_mirror`].
///
/// A local index read with no lock contention; 30s is two orders of magnitude
/// above any observed value and exists to terminate a wedged child, not to
/// police a slow one.
pub const GIT_DIFF_DEADLINE: Duration = Duration::from_secs(30);

// REMOVED WITH ITS SPAWN: `BR_LIST_DEADLINE` argued a 300s ceiling for
// `br list --json --status closed`, and nothing on the commit path spawns `br` any more.
// The tracker oracle is the INDEX mirror (`omp-orchestrator-dpa4` moved it off `br`;
// `omp-orchestrator-5lgku` moved it off the worktree), which takes no `.beads/.write.lock`
// and needs no contention band. A tuned constant whose subject no longer runs is a
// documented fiction, so it is deleted rather than kept "in case".

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

// SUPERSEDED AND DELETED: `parse_closed_beads` and `parse_closed_beads_checked` parsed
// `br list --json --status closed`. Nothing spawns `br` any more -- the oracle is the
// STAGED `.beads/issues.jsonl` blob -- and that JSON never carried a `comments` key, which
// is the `omp-orchestrator-dpa4` defect those functions embodied. Keeping a second parser
// for a surface no caller reads is how two oracles drift; `parse_closed_beads_jsonl_checked`
// is now the only closed-bead parser in the crate.

/// Read closed beads from the `.beads/issues.jsonl` MIRROR, comments included.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-dpa4`): `ClosedBead::comments` exists,
/// [`check_deletions`] scans it, and until this function landed EVERY production caller
/// passed an empty vector. The deleted `parse_closed_beads` said so in its own body --
/// *"The br JSON does not inline comments; the caller fetches them separately"* -- and no
/// caller ever did. Measured 2026-09-07: `br list --json --status closed` returns **196
/// rows and 0 of
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

/// Repo-relative path of the tracker mirror. ONE constant, so the INDEX read, the
/// worktree helper and every message name the same file and cannot drift apart.
pub const MIRROR_PATH: &str = ".beads/issues.jsonl";

/// WORKTREE path of the tracker mirror.
///
/// NOT THE GATE'S ORACLE. The commit path must read [`read_index_mirror`]; this helper
/// exists for environment discrimination (does this tree carry a mirror at all) and for
/// messages. A caller that reads these bytes on the commit path re-opens 249hz.
pub fn beads_mirror_path(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(MIRROR_PATH)
}

/// Why the staged mirror could not be read.
///
/// TYPED, because the remedies differ and the binary maps a DEADLINE to its own exit
/// code: a timeout is not a verdict, and collapsing it into "git failed" is the
/// conflation that cost this repo a six-hour false diagnosis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorReadError {
    /// `git` ran and refused: not a repository, unreadable index, corrupt object.
    GitFailed {
        code: Option<i32>,
        argv: String,
        detail: String,
    },
    /// `git` never returned within the deadline, so the staged mirror is UNKNOWN.
    Timeout { after_ms: u64, group_killed: bool },
    /// `git` could not be spawned at all: a PATH/env problem, not a tracker problem.
    Unspawnable { detail: String },
    /// The staged blob is not UTF-8. Decoding it LOSSILY would turn unreadable bytes
    /// into a clean-looking mirror with no records, which is a silent pass.
    NotUtf8 { bytes: usize },
}

impl fmt::Display for MirrorReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirrorReadError::GitFailed { code, argv, detail } => write!(
                formatter,
                "PRE_DELETE_BEADS_UNREADABLE reason=git_index_read_failed path={MIRROR_PATH} \
                 argv={argv} code={code:?} detail={detail}"
            ),
            MirrorReadError::Timeout {
                after_ms,
                group_killed,
            } => write!(
                formatter,
                "PRE_DELETE_GIT_TIMEOUT reason=index_read_timeout path={MIRROR_PATH} \
                 after_ms={after_ms} group_killed={group_killed}; no exit status was observed, \
                 so the STAGED mirror is UNKNOWN and no deletion can be certified citation-free"
            ),
            MirrorReadError::Unspawnable { detail } => write!(
                formatter,
                "PRE_DELETE_BEADS_UNREADABLE reason=git_unspawnable path={MIRROR_PATH} \
                 detail={detail}"
            ),
            MirrorReadError::NotUtf8 { bytes } => write!(
                formatter,
                "PRE_DELETE_BEADS_UNREADABLE reason=staged_blob_not_utf8 path={MIRROR_PATH} \
                 bytes={bytes}"
            ),
        }
    }
}

/// What the INDEX holds for the tracker mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexMirror {
    /// The index carries the mirror: these are the exact bytes this commit lands.
    Staged(String),
    /// The mirror is not in the index at all, so this commit's tree carries no tracker
    /// and there is nothing for the citation gate to cross-reference against.
    NotInIndex,
}

/// Run one `git` read against `repo_root` under [`GIT_DIFF_DEADLINE`], returning RAW bytes.
///
/// Bytes, not a `String`: [`run_bounded`] decodes stdout with `from_utf8_lossy`, and a
/// lossy blob read is how a mirror that cannot be decoded becomes a clean-looking mirror
/// with zero records. The strict decode happens at the one call site that needs text.
fn bounded_git_bytes(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>, MirrorReadError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(args);
    match subprocess_contract::bounded_output(&mut command, GIT_DIFF_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(output.stdout)
        }
        subprocess_contract::BoundedOutcome::Completed(output) => {
            Err(MirrorReadError::GitFailed {
                code: output.status.code(),
                argv: args.join(" "),
                detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            })
        }
        subprocess_contract::BoundedOutcome::TimedOut => Err(MirrorReadError::Timeout {
            after_ms: u64::try_from(GIT_DIFF_DEADLINE.as_millis()).unwrap_or(u64::MAX),
            group_killed: true,
        }),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(MirrorReadError::Unspawnable {
                detail: error.to_string(),
            })
        }
    }
}

/// Read the tracker mirror THE COMMIT IS MADE OF: the INDEX blob, never the worktree file.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-5lgku`, fifth instance of open P0
/// `omp-orchestrator-249hz`). The index, the worktree and HEAD are three different trees
/// and they diverge constantly in a twelve-agent shared checkout -- measured 2026-09-11 on
/// ONE file: worktree 13637 B, index 11279 B, HEAD 0 B, with the index advanced by a third
/// party mid-read. A pre-commit gate's subject is the STAGED set, so a worktree read judges
/// rows the commit is not landing and misses rows it IS landing. Both directions are real:
/// a row citing a deleted path that is staged but not yet written to the worktree passes
/// (false green), and a citation present only in the unstaged worktree refuses a commit
/// that does not contain it (false red). The false green is the dangerous one and it needs
/// nothing exotic -- `git add`, keep editing, `git commit` with no pathspec.
///
/// The same fix landed for the close-reason gate at f194a01 on the same day; this is the
/// citation gate's copy of it, in the crate that owns the reader rather than at the call
/// site, so every caller gets the index by construction.
///
/// PRESENCE IS READ PROSE-FREE. `git ls-files --stage -- <path>` prints one record when the
/// path is in the index and NOTHING when it is not, so absence is read off an empty stdout
/// rather than off git's English error text, which is locale-dependent and would make this
/// gate's verdict depend on `LC_ALL`.
///
/// TWO ABSENCES ARE NOT ONE, and this is the whole reason for [`IndexMirror`]. A mirror
/// that is in the index and carries no closed records is an ERROR
/// (`PRE_DELETE_BEADS_EMPTY`, via [`parse_closed_beads_jsonl_checked`]) -- the oracle was
/// read and came back empty. A mirror that is ABSENT FROM THE INDEX is
/// `GATE_NOT_APPLICABLE`: this commit's tree carries no tracker, and refusing every such
/// commit is the unsatisfiable-gate shape this repo removed twice on 2026-09-11.
///
/// RESIDUAL, stated: a commit that removes the mirror from the index while deleting a cited
/// file gets `NotInIndex` and this gate goes quiet for that commit. It is announced on
/// stderr by [`read_closed_beads_from_mirror`], never silent, and it is strictly narrower
/// than the worktree hole it replaces.
pub fn read_index_mirror(repo_root: &Path) -> Result<IndexMirror, MirrorReadError> {
    let entry = bounded_git_bytes(repo_root, &["ls-files", "--stage", "--", MIRROR_PATH])?;
    if String::from_utf8_lossy(&entry).trim().is_empty() {
        return Ok(IndexMirror::NotInIndex);
    }
    let spec = format!(":{MIRROR_PATH}");
    let blob = bounded_git_bytes(repo_root, &["show", &spec])?;
    let bytes = blob.len();
    // STRICT, never lossy: replacement characters would parse as a mirror that simply has
    // no closed rows, which reads identically to a healthy-but-uncited tracker.
    let text = String::from_utf8(blob).map_err(|_| MirrorReadError::NotUtf8 { bytes })?;
    Ok(IndexMirror::Staged(text))
}

/// Read the closed beads the COMMIT carries. An unreadable oracle is restrictive; a mirror
/// that is absent from the index is `GATE_NOT_APPLICABLE`, announced rather than silent.
///
/// This is the shim the pre-commit hook calls: it flattens [`MirrorReadError`] to the
/// `String` the hook renders. Callers that must distinguish a DEADLINE from a refusal --
/// this crate's own binary does, to keep its exit code 4 -- call [`read_index_mirror`].
pub fn read_closed_beads_from_mirror(repo_root: &Path) -> Result<Vec<ClosedBead>, String> {
    match read_index_mirror(repo_root).map_err(|error| error.to_string())? {
        IndexMirror::Staged(text) => parse_closed_beads_jsonl_checked(&text),
        IndexMirror::NotInIndex => {
            eprintln!(
                "pre-delete-citation-check: GATE_NOT_APPLICABLE reason=mirror_not_in_index \
                 path={MIRROR_PATH} root={} -- this commit's tree carries no tracker, so there \
                 is no closed-bead oracle to cross-reference and nothing was checked",
                repo_root.display()
            );
            Ok(Vec::new())
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedCloseReasonViolation {
    pub bead_id: String,
    pub reason: String,
}

/// SUPERSEDED: `ScopedCloseReasonPolicyReport` and `check_close_reason_policy_scoped` were
/// removed in favour of [`check_staged_close_reason_policy`], which applies ONE rule per row
/// (sanctioned prefix AND worker authority) instead of two authorities that disagreed about
/// the same input. The `MirrorBead` byte-level parser went with them: the staged scan now
/// reuses [`parse_closed_beads_jsonl_checked`], so there is a single mirror reader.

/// ONE AUTHORITY: the worker-attribution demand now lives ONLY in
/// `ack_spine::close_reason::classify_close_reason`, which OWNS the invariant and
/// already refuses a cargo figure lacking `worker=`/`local` as
/// `CloseReasonVerdict::CargoWorkerMissing` (label `CLOSE_REASON_WORKER_MISSING`).
/// The local `has_worker_attribution` copy and its unconditional arm were DELETED
/// rather than kept in sync: two copies of a rule drift, and this one drifted into
/// demanding execution authority from rows that made no execution claim — a row
/// whose evidence is `df -h` and `du` has no number to attribute to a tree.

/// One closed staged row's disposition under the staged close-reason policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedCloseReasonReport {
    /// Every closed row readable in the staged mirror -- the scan denominator.
    pub closed_beads: usize,
    /// Closed rows that were not already closed in the HEAD mirror.
    pub newly_closed: usize,
    /// Newly closed rows carrying BOTH a sanctioned prefix and worker authority.
    pub verified: usize,
    /// Rows already closed in the HEAD baseline; reported, never refused.
    pub historical_closed: usize,
    /// One entry per newly closed row that fails the policy.
    pub violations: Vec<ScopedCloseReasonViolation>,
    /// Rows ALREADY closed in the HEAD baseline whose reason cannot be verified. Reported as a
    /// visible disposition so history stays legible; never a refusal, because this commit did
    /// not close them.
    pub legacy_unrecoverable: Vec<String>,
}

/// Check the staged mirror's closed rows against the canonical close-reason policy.
///
/// DETECTION, NOT PREVENTION. `br close` stores arbitrary reasons -- ack-spine names its own
/// bypass at `crates/ack-spine/src/close_reason.rs:34-37` ("the installed br accepts and
/// stores arbitrary close-reason strings; it is not the validator") and again at `:123-124`
/// -- so a bad row is caught on the NEXT commit that stages the mirror, never at close time.
/// The caller must read that mirror from the INDEX; a worktree read is a false green (249hz).
///
/// TWO ABSENCES ARE NOT ONE. `head_mirror == None` means the mirror is absent from HEAD (a
/// first commit that ADDS it is legitimate, so the baseline is empty). A HEAD mirror that
/// exists and cannot be parsed is an `Err` -- a blind gate must not pass as an empty baseline.
///
/// ONE VIOLATION PER ROW: a prose reason that also lacks worker authority is one conflict, so
/// the conflict count stays a row count and remains comparable with `closed_beads`.
pub fn check_staged_close_reason_policy(
    head_mirror: Option<&str>,
    staged_closed: &[ClosedBead],
) -> Result<StagedCloseReasonReport, String> {
    let head_closed: std::collections::BTreeSet<String> = match head_mirror {
        None => std::collections::BTreeSet::new(),
        Some(text) => match parse_closed_beads_jsonl_checked(text) {
            Ok(rows) => rows.into_iter().map(|row| row.id).collect(),
            // A HEAD mirror with zero closed rows is a legitimate empty baseline.
            Err(error) if error.starts_with("PRE_DELETE_BEADS_EMPTY") => {
                std::collections::BTreeSet::new()
            }
            Err(error) => {
                return Err(format!(
                    "CLOSE_REASON_HEAD_MIRROR_UNREADABLE reason=head_mirror_present_but_unparsable detail={error}"
                ))
            }
        },
    };

    let mut report = StagedCloseReasonReport {
        closed_beads: staged_closed.len(),
        newly_closed: 0,
        verified: 0,
        historical_closed: 0,
        violations: Vec::new(),
        legacy_unrecoverable: Vec::new(),
    };
    for bead in staged_closed {
        // The execution authority may live in the row's COMMENTS rather than its
        // reason, and for 17 rows in this tracker it does: the reason field is
        // UNAMENDABLE without `br reopen` + `br close`, which flaps a closed bead
        // OPEN — and zero-open-S1 is the predicate the S1 done-bar is defined
        // over. Four agents briefly flapped eight S1 rows before that was ruled
        // out. The mirror this gate already reads carries the comments
        // (`parse_closed_beads_jsonl_checked` populates them), so this widens the
        // SEARCH SURFACE and not the rule.
        let external_authority = bead
            .comments
            .iter()
            .any(|comment| ack_spine::close_reason::has_worker_authority(comment));
        if head_closed.contains(&bead.id) {
            report.historical_closed += 1;
            let historical =
                ack_spine::close_reason::classify_close_reason_with_external_authority(
                    Some(&bead.close_reason),
                    external_authority,
                );
            if !historical.is_verified() {
                report.legacy_unrecoverable.push(bead.id.clone());
            }
            continue;
        }
        report.newly_closed += 1;
        let verdict = ack_spine::close_reason::classify_close_reason_with_external_authority(
            Some(&bead.close_reason),
            external_authority,
        );
        if !verdict.is_verified() {
            report.violations.push(ScopedCloseReasonViolation {
                bead_id: bead.id.clone(),
                reason: format!("bead={} {verdict}", bead.id),
            });
            continue;
        }
        // NOTE: there is deliberately no second worker-authority arm here. The
        // verdict above ALREADY refuses a cargo figure with no authority in the
        // reason AND none in any comment (CargoWorkerMissing, label
        // CLOSE_REASON_WORKER_MISSING), so a local arm could only restate it or
        // disagree with it.
        report.verified += 1;
    }
    Ok(report)
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
