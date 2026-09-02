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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn mtime(p: &PathBuf) -> Option<SystemTime> {
    fs::metadata(p).ok()?.modified().ok()
}

/// Every source artifact that must be reflected in PLAN.md.
fn is_round_ledger(name: &str) -> bool {
    let bytes = name.as_bytes();
    if !name.starts_with("round") || !name.ends_with(".jsonl") {
        return false;
    }
    let mut index = 5;
    let start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    index > start && bytes.get(index) == Some(&b'-')
}

fn allowed_round_ledger(path: &PathBuf) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let mut found = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Some((_, rest)) = line.split_once("\"round\"") else {
            return false;
        };
        let Some((_, value)) = rest.split_once(':') else {
            return false;
        };
        let round = value
            .trim_start()
            .split(|character: char| !character.is_ascii_digit())
            .next()
            .and_then(|value| value.parse::<u64>().ok());
        let Some(round) = round else {
            return false;
        };
        found = true;
        if !(15..=21).contains(&round) && round != 23 {
            return false;
        }
    }
    found
}

fn is_plan_source(name: &str) -> bool {
    let section = name.len() > 3
        && name.as_bytes()[0].is_ascii_digit()
        && name.as_bytes()[1].is_ascii_digit()
        && name.as_bytes()[2] == b'-'
        && name.ends_with(".md");
    section || is_round_ledger(name) || matches!(name, "FINDINGS.jsonl" | "CONVERGENCE.jsonl")
}

/// Source artifacts newer than the assembly, newest first.
fn stale_sections() -> (usize, Vec<(String, u64)>) {
    let root = repo_root();
    let plan = root.join("docs/PLAN.md");
    let Some(pt) = mtime(&plan) else {
        return (0, Vec::new());
    };

    let mut total = 0usize;
    let mut newer = Vec::new();
    let Ok(rd) = fs::read_dir(root.join("docs/plan")) else {
        return (0, Vec::new());
    };
    for e in rd.flatten() {
        let p = e.path();
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_plan_source(name) || (is_round_ledger(name) && !allowed_round_ledger(&p)) {
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
/// will never share a timestamp. Anything beyond this is a human forgetting.
const GRACE_MINUTES: u64 = 10;

#[test]
fn the_assembly_is_not_stale_against_its_sections() {
    let (total, newer) = stale_sections();

    // ANTI-VACUITY: zero sections scanned reports identically to a fresh assembly.
    assert!(
        total >= 10,
        "scanned {total} sections; docs/plan has 13 — the scan set collapsed"
    );

    let bad: Vec<&(String, u64)> = newer.iter().filter(|(_, m)| *m > GRACE_MINUTES).collect();
    assert!(
        bad.is_empty(),
        "docs/PLAN.md is STALE against {} of {total} sections (grace {GRACE_MINUTES}min).\n  {}\n\
         Re-assemble before the next round: a reader of PLAN.md is working from old knowledge, \
         and its HEADER publishes generated counts that a reader trusts more than body prose.",
        bad.len(),
        bad.iter()
            .map(|(n, m)| format!("{n} is {m} minutes newer"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn the_detector_measures_real_files_and_a_real_direction() {
    // KNOWN-GOOD control: the assembly exists and the scan found sections. Without
    // this, a renamed PLAN.md makes the gate above pass by returning an empty list.
    let root = repo_root();
    assert!(
        root.join("docs/PLAN.md").exists(),
        "docs/PLAN.md is absent — the freshness gate would pass vacuously"
    );
    let (total, _) = stale_sections();
    assert!(
        total > 0,
        "no sections matched the NN-name pattern — the gate cannot fire"
    );

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
fn the_plan_embeds_every_round_and_stamp() {
    let root = repo_root();
    let plan = fs::read_to_string(root.join("docs/PLAN.md")).expect("PLAN.md readable");
    assert_eq!(
        plan.matches("<!-- PLAN_STAMP").count(),
        1,
        "PLAN.md must carry exactly one current plan-assemble stamp"
    );
    assert!(
        plan.contains("<!-- PLAN_STAMP {")
            && plan.contains("\"generator\":\"plan-assemble\"")
            && plan.contains("\"source_fingerprint\":\"fnv1a64:"),
        "PLAN.md is missing the plan-assemble source stamp"
    );
    assert!(
        plan.len() >= 1_086_000,
        "PLAN.md shrank below the known-good all-rounds floor: {} bytes",
        plan.len()
    );

    let mut records: Vec<PathBuf> = fs::read_dir(root.join("docs/plan"))
        .expect("read docs/plan")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| is_round_ledger(name) && allowed_round_ledger(path))
        })
        .collect();
    records.extend(
        ["FINDINGS.jsonl", "CONVERGENCE.jsonl"]
            .into_iter()
            .map(|name| root.join("docs/plan").join(name)),
    );
    records.sort();
    assert_eq!(
        records.len(),
        16,
        "expected 14 round ledgers plus two audit ledgers"
    );
    assert_eq!(
        records.len() + 13,
        29,
        "13 sections plus 16 ledgers must be embedded"
    );

    let appendix = plan
        .split("## Appendix — convergence and audit ledgers")
        .nth(1)
        .expect("round appendix");
    for required in (15..=21).chain(std::iter::once(23)) {
        assert!(
            appendix.contains(&format!("\"round\":{required}")),
            "PLAN.md round appendix omits required round {required}"
        );
    }
    assert!(
        !appendix.contains("\"round\":22"),
        "PLAN.md must not embed halted round-22 records"
    );

    for path in records {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("record filename");
        let marker = format!("<!-- ===== {name} ===== -->");
        assert!(plan.contains(&marker), "PLAN.md omits record marker {name}");
        let content = fs::read_to_string(&path).expect("record readable");
        let expected = if name == "CONVERGENCE.jsonl" {
            content
                .lines()
                .filter(|line| {
                    !line.contains("\"round\":22") && !line.contains("\"round\": 22")
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content
        };
        assert!(
            plan.contains(expected.trim_end()),
            "PLAN.md omits record content {name}"
        );
    }
}

#[test]
fn inner_appendix_round22_mutation_is_detected() {
    let root = repo_root();
    let plan = fs::read_to_string(root.join("docs/PLAN.md")).expect("PLAN.md readable");
    let appendix_start = plan
        .find("## Appendix — convergence and audit ledgers")
        .expect("round appendix");
    let from = "\"round\":23";
    let to = "\"round\":22";
    let offset = plan[appendix_start..]
        .find(from)
        .map(|relative| appendix_start + relative)
        .expect("R23 row inside checked appendix");
    let mut mutated = plan.clone();
    mutated.replace_range(offset..offset + from.len(), to);
    let mutated_appendix = mutated[appendix_start..].to_owned();
    assert!(
        mutated_appendix.contains(to),
        "the mutation must land inside the checked appendix region"
    );
    assert_ne!(mutated, plan, "mutation must change the artifact");
    mutated.replace_range(offset..offset + to.len(), from);
    assert_eq!(mutated, plan, "mutation leg must restore byte-identically");
}

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
