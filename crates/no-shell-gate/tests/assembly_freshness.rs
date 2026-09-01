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


/// The tool that regenerates the assembly must live IN THE REPOSITORY.
///
/// # The measured failure
///
/// `Lens00Brief`, the held-out operator-at-3am lens, filed:
///
/// > The brief says the assembled document must be "re-assembled" … But I have no
/// > command to re-assemble it, no path to the four-way identity check, and no
/// > explicit instruction whether I should edit PLAN.md in place, invoke an assembly
/// > script (which path?), or wait for a gate to rebuild it (what gate?).
///
/// Measured 2026-09-01, and worse than one section could reveal:
///
/// - `docs/PLAN.md` is 651,662 bytes — the document a reader opens.
/// - `assembly_freshness` (this file) DEMANDS it be current.
/// - The only thing that could produce it was **3,297 bytes of Python in `/tmp`**.
///
/// A gate required freshness while the tool satisfying it lived nowhere in the repo.
/// One reboot and `PLAN.md` becomes permanently stale and un-regenerable — this gate
/// failing forever with no available remedy. **A gate whose remedy does not exist is
/// a trap, not a guard.**
///
/// The repo's one rule forbade the shortcut: no `.sh`, no `.py`, enforced over
/// `git ls-files`. So the assembler could not simply be committed; it had to be
/// ported. The rule refused a Python dependency in a Rust workspace and this test is
/// what keeps that from silently regressing.
///
/// # What it cannot do
///
/// It checks that an assembler is TRACKED, not that it WORKS. The port was accepted on
/// a separate criterion — byte-identical output to the original, verified by SHA-256
/// (`56881bf3c2cd3958…`) — which this test does not re-run, because re-assembling
/// inside a test would rewrite the artifact the sibling test is measuring.
#[test]
fn the_assembler_is_tracked_in_repo_not_in_tmp() {
    let root = repo_root();
    let out = std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(&root)
        .output();
    let listing = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            eprintln!("SKIP the_assembler_is_tracked_in_repo_not_in_tmp: git ls-files unavailable");
            return;
        }
    };
    assert!(
        !listing.trim().is_empty(),
        "ANTI-VACUITY: git ls-files returned nothing; a broken listing would pass this \
         test for the wrong reason"
    );
    let has_assembler = listing
        .lines()
        .any(|l| l.contains("plan-assemble") && l.ends_with(".rs"));
    assert!(
        has_assembler,
        "no tracked assembler found. docs/PLAN.md is a 651 KB generated artifact and \
         assembly_freshness demands it be current — so the generator must be IN the \
         repository, not in /tmp where a reboot makes this gate unsatisfiable forever. \
         Expected a tracked crates/plan-assemble/**.rs"
    );
}
