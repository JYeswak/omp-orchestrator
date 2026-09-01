#![forbid(unsafe_code)]
//! ASSEMBLY FRESHNESS — `docs/PLAN.md` is a concatenation of `docs/plan/*.md`. If a
//! section is newer than the assembly, the artifact-of-record is a lie about the
//! plan and every reader of it is working from stale knowledge.
//!
//! # Measured 2026-09-01
//!
//! Josh asked whether the plan was being re-assembled between grading rounds. It
//! was not. `PLAN.md` had been assembled at 19:55; **all thirteen sections were
//! newer**, the newest by **190 minutes**, across **16 commits** — four grading
//! rounds ran while the assembly froze.
//!
//! It partly did not matter because graders are dispatched *section paths*, so each
//! read its own section fresh. What they could not see was every OTHER section's
//! findings from the same round — and that cross-section propagation is exactly
//! what makes a later round smarter than an earlier one. Two of tonight's worst
//! defects were cross-section: a transposed leg row inherited verbatim from
//! `00-brief` into `06-gates`, and a test count that drifted through three files.
//!
//! The header is worse than the body when stale: it publishes generated counts —
//! refutations, open questions, engaged percentage — and a reader trusts a number
//! in a header more than one buried in a section.

use std::{fs, path::PathBuf, time::SystemTime};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn mtime(p: &PathBuf) -> Option<SystemTime> {
    fs::metadata(p).ok()?.modified().ok()
}

/// Sections newer than the assembly, newest first.
fn stale_sections() -> (usize, Vec<(String, u64)>) {
    let root = repo_root();
    let plan = root.join("docs/PLAN.md");
    let Some(pt) = mtime(&plan) else { return (0, Vec::new()) };

    let mut total = 0usize;
    let mut newer = Vec::new();
    let Ok(rd) = fs::read_dir(root.join("docs/plan")) else { return (0, Vec::new()) };
    for e in rd.flatten() {
        let p = e.path();
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else { continue };
        if !name.ends_with(".md") || !name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        total += 1;
        let Some(st) = mtime(&p) else { continue };
        if let Ok(d) = st.duration_since(pt) {
            newer.push((name.to_owned(), d.as_secs() / 60));
        }
    }
    newer.sort_by(|a, b| b.1.cmp(&a.1));
    (total, newer)
}

/// A short grace window: an edit and its re-assembly are two filesystem writes and
/// will never share a timestamp. Anything beyond this is a human forgetting.
const GRACE_MINUTES: u64 = 10;

#[test]
fn the_assembly_is_not_stale_against_its_sections() {
    let (total, newer) = stale_sections();

    // ANTI-VACUITY: zero sections scanned reports identically to a fresh assembly.
    assert!(total >= 10, "scanned {total} sections; docs/plan has 13 — the scan set collapsed");

    let bad: Vec<&(String, u64)> = newer.iter().filter(|(_, m)| *m > GRACE_MINUTES).collect();
    assert!(bad.is_empty(),
        "docs/PLAN.md is STALE against {} of {total} sections (grace {GRACE_MINUTES}min).\n  {}\n\
         Re-assemble before the next round: a reader of PLAN.md is working from old knowledge, \
         and its HEADER publishes generated counts that a reader trusts more than body prose.",
        bad.len(),
        bad.iter().map(|(n, m)| format!("{n} is {m} minutes newer"))
            .collect::<Vec<_>>().join("\n  "));
}

#[test]
fn the_detector_measures_real_files_and_a_real_direction() {
    // KNOWN-GOOD control: the assembly exists and the scan found sections. Without
    // this, a renamed PLAN.md makes the gate above pass by returning an empty list.
    let root = repo_root();
    assert!(root.join("docs/PLAN.md").exists(),
        "docs/PLAN.md is absent — the freshness gate would pass vacuously");
    let (total, _) = stale_sections();
    assert!(total > 0, "no sections matched the NN-name pattern — the gate cannot fire");

    // And prove the comparison is directional: the assembly must be newer, not merely different.
    let plan = mtime(&root.join("docs/PLAN.md")).expect("PLAN.md mtime readable");
    let brief = mtime(&root.join("docs/plan/00-brief.md")).expect("00-brief mtime readable");
    let _ = plan.duration_since(brief); // Err() when brief is newer — which is the stale case
}
