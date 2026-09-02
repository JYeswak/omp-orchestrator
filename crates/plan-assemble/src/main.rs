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
    b.len() > 3
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b'-'
        && name.ends_with(".md")
}

/// Round ledgers are the durable record of every convergence edit.
fn is_round(name: &str) -> bool {
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

fn mix_fingerprint(hash: &mut u64, byte: u8) {
    *hash ^= byte as u64;
    *hash = hash.wrapping_mul(0x1000_0000_01b3);
}

/// Deterministic source identity without a dependency or a timestamp.
fn source_fingerprint(inputs: &[PathBuf], root: &Path) -> Result<String, String> {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for path in inputs {
        let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        for byte in relative.as_bytes() {
            mix_fingerprint(&mut hash, *byte);
        }
        mix_fingerprint(&mut hash, 0);
        let bytes = std::fs::read(path)
            .map_err(|error| format!("cannot read stamp input {}: {error}", path.display()))?;
        for byte in bytes {
            mix_fingerprint(&mut hash, byte);
        }
        mix_fingerprint(&mut hash, 0xff);
    }
    Ok(format!("fnv1a64:{hash:016x}"))
}

/// `^\| (\d+) \| ".+?" \|` — a refuted-claim row: pipe, digits, pipe, quoted text, pipe.
fn is_refuted_claim_row(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("| ") else {
        return false;
    };
    let mut it = rest.splitn(2, " | ");
    let Some(num) = it.next() else { return false };
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Some(tail) = it.next() else { return false };
    // `".+?" |` — a quoted span followed by a pipe. Non-greedy in the original, so the
    // FIRST closing quote that is followed by " |" ends it.
    let Some(after_open) = tail.strip_prefix('"') else {
        return false;
    };
    after_open.contains("\" |")
}

/// `^\| Q\d+ \|` / `^\| K\d+ \|`
fn is_prefixed_row(line: &str, tag: char) -> bool {
    let Some(rest) = line.strip_prefix("| ") else {
        return false;
    };
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

fn round_numbers(path: &Path) -> Result<Vec<u64>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read round ledger {}: {error}", path.display()))?;
    let mut rounds = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let round = field(line, "round")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| {
                format!(
                    "round ledger {} has a row without numeric round",
                    path.display()
                )
            })?;
        if !rounds.contains(&round) {
            rounds.push(round);
        }
    }
    Ok(rounds)
}

/// Return an audit ledger's embedded content, omitting halted round-22 rows from the
/// canonical CONVERGENCE record while retaining the record marker itself.
fn embedded_ledger_text(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read audit ledger {}: {error}", path.display()))?;
    let is_convergence =
        path.file_name().and_then(|name| name.to_str()) == Some("CONVERGENCE.jsonl");
    if !is_convergence {
        return Ok(text);
    }
    let mut filtered = String::new();
    for line in text.lines() {
        if field(line, "round").as_deref() == Some("22") {
            continue;
        }
        filtered.push_str(line);
        filtered.push('\n');
    }
    Ok(filtered)
}

fn guard_ledger_set(round_ledgers: usize, audit_ledgers: usize) -> Result<(), String> {
    if round_ledgers == 0 || audit_ledgers == 0 {
        Err("ledger set is empty; refusing a sections-only assembly".to_owned())
    } else {
        Ok(())
    }
}

fn guard_output_size(previous_len: usize, output_len: usize) -> Result<(), String> {
    if output_len < previous_len {
        Err(format!(
            "assembled body shrank from {previous_len} to {output_len} bytes; refusing truncation"
        ))
    } else {
        Ok(())
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
        eprintln!(
            "PLAN_ASSEMBLE_ERROR zero sections matched NN-*.md — refusing to write an empty plan"
        );
        return std::process::ExitCode::from(2);
    }

    let mut round_files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(is_round)
            })
            .collect(),
        Err(error) => {
            eprintln!(
                "PLAN_ASSEMBLE_ERROR cannot read round ledgers in {}: {error}",
                dir.display()
            );
            return std::process::ExitCode::from(2);
        }
    };
    round_files.sort();
    let mut excluded_round_files = Vec::new();
    let mut allowed_round_files = Vec::new();
    for path in round_files {
        let rounds_in_file = match round_numbers(&path) {
            Ok(rounds) => rounds,
            Err(error) => {
                eprintln!("PLAN_ASSEMBLE_ERROR {error}");
                return std::process::ExitCode::from(2);
            }
        };
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("?");
        if !rounds_in_file.is_empty()
            && rounds_in_file
                .iter()
                .all(|round| (15..=21).contains(round) || *round == 23)
        {
            allowed_round_files.push(path);
        } else {
            excluded_round_files.push(name.to_owned());
        }
    }
    round_files = allowed_round_files;
    if round_files.is_empty() {
        eprintln!("PLAN_ASSEMBLE_ERROR no round ledgers for the required 15-21 range");
        return std::process::ExitCode::from(2);
    }
    if !excluded_round_files.is_empty() {
        eprintln!(
            "PLAN_ASSEMBLE_NOTICE excluded out-of-range round ledgers: {}",
            excluded_round_files.join(",")
        );
    }

    let mut audit_ledgers = Vec::new();
    for name in ["FINDINGS.jsonl", "CONVERGENCE.jsonl"] {
        let path = dir.join(name);
        if !path.is_file() {
            eprintln!(
                "PLAN_ASSEMBLE_ERROR required audit ledger is absent: {}",
                path.display()
            );
            return std::process::ExitCode::from(2);
        }
        audit_ledgers.push(path);
    }
    if let Err(error) = guard_ledger_set(round_files.len(), audit_ledgers.len()) {
        eprintln!("PLAN_ASSEMBLE_ERROR {error}");
        return std::process::ExitCode::from(2);
    }

    let mut covered_rounds = Vec::new();
    for path in round_files.iter().chain(audit_ledgers.iter()) {
        for round in match round_numbers(path) {
            Ok(rounds) => rounds,
            Err(error) => {
                eprintln!("PLAN_ASSEMBLE_ERROR {error}");
                return std::process::ExitCode::from(2);
            }
        } {
            if !covered_rounds.contains(&round) {
                covered_rounds.push(round);
            }
        }
    }
    for required in 15..=21 {
        if !covered_rounds.contains(&required) {
            eprintln!("PLAN_ASSEMBLE_ERROR required convergence round {required} is absent from the embedded records");
            return std::process::ExitCode::from(2);
        }
    }
    let round23_files = round_files
        .iter()
        .filter_map(|path| round_numbers(path).ok())
        .filter(|rounds| rounds.contains(&23))
        .count();
    if round23_files != 4 {
        eprintln!("PLAN_ASSEMBLE_ERROR expected four round-23 ledgers, found {round23_files}");
        return std::process::ExitCode::from(2);
    }
    if !covered_rounds.contains(&23) {
        eprintln!(
            "PLAN_ASSEMBLE_ERROR required convergence round 23 is absent from the embedded records"
        );
        return std::process::ExitCode::from(2);
    }

    let mut stamp_inputs = sections.clone();
    stamp_inputs.extend(round_files.iter().cloned());
    stamp_inputs.extend(audit_ledgers.iter().cloned());
    let fingerprint = match source_fingerprint(&stamp_inputs, &root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("PLAN_ASSEMBLE_ERROR {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let round_manifest = round_files
        .iter()
        .map(|path| {
            format!(
                "\"{}\"",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let excluded_manifest = excluded_round_files
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    let stamp = format!(
        "<!-- PLAN_STAMP {{\"schema\":\"plan-stamp/v1\",\"generator\":\"plan-assemble\",\"round_range\":\"15-23 (22 void)\",\"required_rounds\":[15,16,17,18,19,20,21,23],\"sections\":{},\"round_files\":[{}],\"excluded_round_files\":[{}],\"ledgers\":[\"FINDINGS.jsonl\",\"CONVERGENCE.jsonl\"],\"source_fingerprint\":\"{}\"}} -->",
        sections.len(), round_manifest, excluded_manifest, fingerprint
    );
    if std::env::args().any(|arg| arg == "--check") {
        let target = root.join("docs/PLAN.md");
        let plan = match std::fs::read_to_string(&target) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "PLAN_STAMP_REFUSED cannot read {}: {error}",
                    target.display()
                );
                return std::process::ExitCode::from(1);
            }
        };
        if plan.matches("<!-- PLAN_STAMP").count() != 1 {
            eprintln!("PLAN_STAMP_REFUSED expected exactly one current PLAN_STAMP marker");
            return std::process::ExitCode::from(1);
        }
        if !plan.contains(&stamp) {
            eprintln!("PLAN_STAMP_REFUSED source fingerprint or manifest differs; run cargo run -p plan-assemble");
            return std::process::ExitCode::from(1);
        }
        let appendix = plan
            .split("## Appendix — convergence and audit ledgers")
            .nth(1)
            .unwrap_or("");
        for required in (15..=21).chain(std::iter::once(23)) {
            let needle = format!("\"round\":{required}");
            if !appendix.contains(&needle) {
                eprintln!("PLAN_STAMP_REFUSED embedded records do not cover round {required}");
                return std::process::ExitCode::from(1);
            }
        }
        if appendix.contains("\"round\":22") {
            eprintln!("PLAN_STAMP_REFUSED halted round-22 records must not be embedded");
            return std::process::ExitCode::from(1);
        }
        for name in &excluded_round_files {
            let marker = format!("<!-- ===== {name} ===== -->");
            if plan.contains(&marker) {
                eprintln!("PLAN_STAMP_REFUSED excluded record was embedded {name}");
                return std::process::ExitCode::from(1);
            }
        }

        for path in round_files.iter().chain(audit_ledgers.iter()) {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("?");
            let marker = format!("<!-- ===== {name} ===== -->");
            if !plan.contains(&marker) {
                eprintln!("PLAN_STAMP_REFUSED missing embedded record {name}");
                return std::process::ExitCode::from(1);
            }
        }
        println!(
            "PLAN_STAMP PASS sections={} round_files={} fingerprint={fingerprint}",
            sections.len(),
            round_files.len()
        );
        return std::process::ExitCode::SUCCESS;
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
        "Assembled from docs/plan/. The section files and round ledgers are the source of truth; this document is their generated union.".into(),
        "Edit a section or round ledger, then re-assemble — never edit here, and never re-stamp this file's".into(),
        "mtime to satisfy the freshness gate (§12.11 records the author doing exactly that).\n".into(),
        stamp.clone(),
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

    body.push(
        "\n\n## Appendix — convergence and audit ledgers\n\nEvery tracked round record, plus the findings and convergence ledgers, is embedded below. The PLAN_STAMP above identifies this exact source set.\n".into(),
    );
    for path in round_files.iter().chain(audit_ledgers.iter()) {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("?");
        let text = match embedded_ledger_text(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("PLAN_ASSEMBLE_ERROR {error}");
                return std::process::ExitCode::from(2);
            }
        };
        body.push(format!(
            "\n\n<!-- ===== {name} ===== -->\n\n### {name}\n\n~~~jsonl\n{}\n~~~\n",
            text.trim_end()
        ));
    }

    let body_text = format!("{}{}", head.join("\n"), body.join("\n"));
    let ledgers = round_files.len() + audit_ledgers.len();
    if ledgers == 0 {
        eprintln!("PLAN_ASSEMBLE_ERROR ledger set is empty; refusing a sections-only assembly");
        return std::process::ExitCode::from(2);
    }
    let embedded_records = sections.len() + ledgers;
    let target = root.join("docs/PLAN.md");
    if let Ok(previous) = std::fs::read(&target) {
        if let Err(error) = guard_output_size(previous.len(), body_text.len()) {
            eprintln!("PLAN_ASSEMBLE_ERROR {error}");
            return std::process::ExitCode::from(2);
        }
    }
    let out = body_text;
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
    println!(
        "  embedded records: {} (sections={} ledgers={})",
        embedded_records,
        sections.len(),
        ledgers
    );
    let dupes = out.matches("<!-- ===== 00-brief.md ===== -->").count();
    println!("  duplicate-section check: 00-brief appears {dupes} time(s) — must be 1");
    if dupes != 1 {
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{guard_ledger_set, guard_output_size};

    #[test]
    fn empty_ledger_sets_refuse() {
        let error = guard_ledger_set(0, 2).expect_err("round ledger absence must refuse");
        assert!(error.contains("ledger set is empty"), "{error}");
        let error = guard_ledger_set(2, 0).expect_err("audit ledger absence must refuse");
        assert!(error.contains("ledger set is empty"), "{error}");
    }

    #[test]
    fn output_shrink_refuses_and_growth_admits() {
        let error = guard_output_size(100, 99).expect_err("smaller output must refuse");
        assert!(error.contains("shrank"), "{error}");
        guard_output_size(100, 100).expect("equal output is not truncation");
        guard_output_size(100, 101).expect("larger output is admissible");
    }
}
