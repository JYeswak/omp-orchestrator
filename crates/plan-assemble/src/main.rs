#![forbid(unsafe_code)]
//! PLAN-ASSEMBLE — regenerate `docs/PLAN.md` from the section files.
//!
//! # Why this crate exists
//!
//! `Lens00Brief`, the held-out operator-at-3am lens, filed:
//!
//! > The brief says the assembled document "went stale" and must be "re-assembled" …
//! > But I have no command to re-assemble it, no path to the four-way identity check,
//! > and no explicit instruction whether I should edit PLAN.md in place (the brief
//! > says no), invoke an assembly script (which path?), or wait for a gate to rebuild
//! > it (what gate?).
//!
//! Measured 2026-09-01, and worse than the lens could see from one section:
//!
//! - `docs/PLAN.md` is **651,662 bytes** — the assembled plan a reader opens.
//! - `assembly_freshness.rs` is a gate that **demands** it be current.
//! - The only thing that could produce it was **3,297 bytes of Python in `/tmp`**.
//!
//! So a gate required freshness while the tool satisfying it lived nowhere in the
//! repository. On the next reboot `PLAN.md` becomes permanently stale and
//! un-regenerable: the gate fails forever with no available remedy. That is the same
//! clearable-provenance defect already fixed for the inventory captures (§4.7) and the
//! wire-proof frame (§1.2.3) — this time aimed at the tool rather than the evidence.
//!
//! And the repository's one rule forbids the obvious fix: **no `.sh`, no `.py`**, gated
//! by `no-shell-gate` over `git ls-files`. The assembler could not simply be committed;
//! it had to be ported. So the rule did its job — it refused a shortcut that would have
//! left a Python dependency in a Rust workspace — and this binary is what the rule cost
//! and bought.
//!
//! # Acceptance
//!
//! **Byte-identical output to the Python original.** Anything else would register as a
//! real change in `assembly_freshness` and in every downstream figure, which would make
//! the port indistinguishable from an edit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // Walk up for the marker rather than trusting cwd: this runs from xtask, from a
    // hook, and by hand, and each has a different working directory.
    let mut cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if cur.join("docs/plan").is_dir() {
            return cur;
        }
        if !cur.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

/// Sections are `NN-name.md` — two leading digits then a dash. Matches the Python
/// `^\d\d-`, and deliberately excludes SURFACE-MAP, CONVERGENCE and coverage JSON.
fn is_section(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'-' && name.ends_with(".md")
}

/// `^\| (\d+) \| ".+?" \|` — a refuted-claim row: pipe, digits, pipe, quoted text, pipe.
fn is_refuted_claim_row(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("| ") else { return false };
    let mut it = rest.splitn(2, " | ");
    let Some(num) = it.next() else { return false };
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Some(tail) = it.next() else { return false };
    // `".+?" |` — a quoted span followed by a pipe. Non-greedy in the original, so the
    // FIRST closing quote that is followed by " |" ends it.
    let Some(after_open) = tail.strip_prefix('"') else { return false };
    after_open.contains("\" |")
}

/// `^\| Q\d+ \|` / `^\| K\d+ \|`
fn is_prefixed_row(line: &str, tag: char) -> bool {
    let Some(rest) = line.strip_prefix("| ") else { return false };
    let mut ch = rest.chars();
    if ch.next() != Some(tag) {
        return false;
    }
    let digits: String = ch.clone().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    rest[1 + digits.len()..].starts_with(" |")
}

/// Minimal JSONL string-field read. No serde: this binary must build in the smallest
/// possible dependency set, and the gates it feeds already parse this way.
fn field(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":");
    let i = line.find(&pat)? + pat.len();
    let rest = line[i..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        Some(r.split('"').next().unwrap_or("").to_owned())
    } else {
        Some(
            rest.split(|c: char| c == ',' || c == '}')
                .next()
                .unwrap_or("")
                .trim()
                .to_owned(),
        )
    }
}

fn main() -> std::process::ExitCode {
    let root = repo_root();
    let dir = root.join("docs/plan");

    let mut sections: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(is_section)
            })
            .collect(),
        Err(e) => {
            eprintln!("PLAN_ASSEMBLE_ERROR cannot read {}: {e}", dir.display());
            return std::process::ExitCode::from(2);
        }
    };
    sections.sort();
    if sections.is_empty() {
        // ANTI-VACUITY: assembling zero sections would write a header-only PLAN.md and
        // report success, which is exactly the vacuous pass this repo refuses.
        eprintln!("PLAN_ASSEMBLE_ERROR zero sections matched NN-*.md — refusing to write an empty plan");
        return std::process::ExitCode::from(2);
    }

    let brief = std::fs::read_to_string(dir.join("00-brief.md")).unwrap_or_default();
    let n = brief.lines().filter(|l| is_refuted_claim_row(l)).count();
    let qs = brief.lines().filter(|l| is_prefixed_row(l, 'Q')).count();
    let ks = brief.lines().filter(|l| is_prefixed_row(l, 'K')).count();

    let sm_text = std::fs::read_to_string(dir.join("SURFACE-MAP.jsonl")).unwrap_or_default();
    let sm: Vec<&str> = sm_text.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for l in &sm {
        let d = field(l, "disposition").unwrap_or_else(|| "null".into());
        *counts.entry(d).or_insert(0) += 1;
    }
    let get = |k: &str| counts.get(k).copied().unwrap_or(0);
    let eng = get("CONSUMED") + get("WIRE") + get("VALIDATE");

    let cv_text = std::fs::read_to_string(dir.join("CONVERGENCE.jsonl")).unwrap_or_default();
    let mut rounds: Vec<u64> = Vec::new();
    let mut tot: u64 = 0;
    for l in cv_text.lines().filter(|l| !l.trim().is_empty()) {
        if let Some(r) = field(l, "round").and_then(|v| v.parse::<u64>().ok()) {
            if !rounds.contains(&r) {
                rounds.push(r);
            }
        }
        tot += field(l, "new_findings")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
    }

    if sm.is_empty() {
        eprintln!("PLAN_ASSEMBLE_ERROR SURFACE-MAP.jsonl is empty — the engaged percentage would divide by zero");
        return std::process::ExitCode::from(2);
    }
    let engaged_pct = 100.0 * (eng as f64) / (sm.len() as f64);

    let mut head: Vec<String> = vec![
        "# PLAN.md — omp-orchestrator\n".into(),
        "**A single installable Rust binary that takes a repo's own work graph and drives it to completion".into(),
        "across a fleet of agents, refusing every step it cannot prove.**\n".into(),
        "Assembled from `docs/plan/`. **The section files are the source of truth**; this document is their".into(),
        "concatenation. Edit a section, then re-assemble — never edit here, and never re-stamp this file's".into(),
        "mtime to satisfy the freshness gate (§12.11 records the author doing exactly that).\n".into(),
        "> ## Three things a reader should know before the contents\n".into(),
        "> **1. The headline finding was refuted, and it was the first of eight.** §10 claimed a typed".into(),
        "> worker-completion signal was precedent-free across 210 repositories. It ships in the tool we wrap".into(),
        "> and crosses the wire: `AgentEndEvent`, `isTerminal:true`, captured live on `--mode=rpc`. Seven".into(),
        "> more named gaps have upstream types; an eighth root (`dist/types/plan-mode/`) surfaced later.\n".into(),
        "> **2. Convergence has been retracted once.** Rounds 8–9 banked 3 sections under a two-lens rule.".into(),
        format!("> Round 10 graded with readers who had never seen the ledger and all three fell — {tot} findings"),
        format!("> across {} rounds. Rounds 8–9 measured the graders. Fresh eyes is now a clause of the rule.\n", rounds.len()),
        "> **3. There is no external-validation loop.** Every gate suite here is internal — us checking us.".into(),
        "> `loop-engineering` names that as insufficient for \"shipped\"; §12.11 records the gap.\n".into(),
        format!("> §8 carries **{qs} open questions** and **{ks} kill criteria** — one (K1) void. {n} measured claims"),
        format!("> were refuted while this was written, kept as labelled retractions. Surface map: **{} surfaces**,", sm.len()),
        format!("> {} consumed / {} wire / {} validate /", get("CONSUMED"), get("WIRE"), get("VALIDATE")),
        format!("> {} retired / {} unknown — **{engaged_pct:.1}% engaged**.\n", get("RETIRE"), get("UNPROBEABLE-PENDING")),
        "---\n".into(),
        "## Contents\n".into(),
    ];
    for p in &sections {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        let title = text
            .lines()
            .find(|l| l.starts_with("# "))
            .map(|l| l[2..].trim().to_owned())
            .unwrap_or_else(|| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_owned()
            });
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        head.push(format!("- **`{name}`** — {title}"));
    }
    head.push(String::new());
    head.push("---".into());

    let mut body: Vec<String> = Vec::new();
    for p in &sections {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let text = std::fs::read_to_string(p).unwrap_or_default();
        body.push(format!("\n\n<!-- ===== {name} ===== -->\n"));
        body.push(text.trim_end().to_owned());
        body.push("\n\n---".into());
    }

    let out = format!("{}{}\n", head.join("\n"), body.join("\n"));
    let target = root.join("docs/PLAN.md");
    if let Err(e) = std::fs::write(&target, &out) {
        eprintln!("PLAN_ASSEMBLE_ERROR cannot write {}: {e}", target.display());
        return std::process::ExitCode::from(2);
    }

    println!(
        "  assembled: {} lines / {}KB / {} sections",
        out.lines().count(),
        out.len() / 1024,
        sections.len()
    );
    let dupes = out.matches("<!-- ===== 00-brief.md ===== -->").count();
    println!("  duplicate-section check: 00-brief appears {dupes} time(s) — must be 1");
    if dupes != 1 {
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::SUCCESS
}
