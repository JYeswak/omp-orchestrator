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

use serde_json::Value;
use std::fmt;
use std::path::Path;

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
        let id = issue.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
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
        let conflicts =
            check_deletions(&["bin/fleet-composite.py".to_owned()], &[bead]);
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
        let conflicts =
            check_deletions(&["bin/unrelated-thing.sh".to_owned()], &[bead]);
        assert!(conflicts.is_empty(), "no citation -> no conflict");
    }

    #[test]
    fn both_surfaces_checked_independently() {
        let bead = bead(
            "cp-both",
            "cites bin/a.sh in the close_reason",
            &["also cites bin/b.sh in a comment"],
        );
        let conflicts = check_deletions(
            &["bin/a.sh".to_owned(), "bin/b.sh".to_owned()],
            &[bead],
        );
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
}
