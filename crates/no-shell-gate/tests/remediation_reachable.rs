//! REMEDIATION GATE — every remedy the plan names must be invocable, or disclosed as
//! projected within sight of where it is named.
//!
//! # The shape this exists to stop, measured four times in one session
//!
//! A refusal that names a remedy the reader cannot run is worse than a refusal that
//! names none: it spends the operator's trust and their time. Tonight, four separate
//! instances of exactly that:
//!
//! | # | where | the remedy that did not exist |
//! |---|---|---|
//! | 1 | `04-diagrams` | cited `/tmp` captures a reboot deletes |
//! | 2 | `01-idea` | cited a `/tmp` frame as the only wire-level proof |
//! | 3 | `assembly_freshness` | demanded freshness; assembler was Python in `/tmp` |
//! | 4 | `08-end-users` | `"remediation":"omp-orchestrator why TA-2"` — no `why` subcommand |
//!
//! The fourth was raised by the held-out operator-at-3am lens as BLOCKED and is a
//! **verified false positive** — measured 2026-09-01, all four remediation strings in
//! that section carry a `PROJECTED` / `not been executed` marker within 6–9 lines, and
//! the section holds 18 such markers. The disclosure was already adjacent and honest.
//!
//! So this gate does not fix a defect. It **converts authorial care into mechanical
//! enforcement**: the property holds today by the author's diligence, and nothing kept
//! it holding for the fifth remediation string somebody adds next week.
//!
//! # What it enforces, precisely
//!
//! For every `"remediation"` / `"hint"` string naming an `omp-orchestrator <sub>`
//! invocation: either `<sub>` appears in the shipped binary's own `--help`, or a
//! projection marker appears within 20 lines of the naming site.
//!
//! # What it does NOT enforce — read this before citing it
//!
//! - It does **not** check that an invocable remedy actually *remedies* anything.
//! - It does **not** cover remedies phrased as prose rather than a JSON field. The
//!   strict-extractor lesson applies: a loose pattern for this reported **13 of 15
//!   "not invocable"**, all flags, identifiers and bead ids — the extractor was wrong,
//!   not the docs, and a gate keyed on it would have manufactured its own findings.
//! - 20 lines is a **judgement**, not a measurement. It is wide enough to clear a
//!   fenced transcript and narrow enough that a section-top disclaimer 160 lines away
//!   does not count.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
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

/// Pull `"remediation": "..."` / `"hint": "..."` values with their 1-based line number.
fn remediations(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        for key in ["\"remediation\"", "\"hint\""] {
            let Some(k) = line.find(key) else { continue };
            let rest = &line[k + key.len()..];
            let Some(c) = rest.find(':') else { continue };
            let after = rest[c + 1..].trim_start();
            let Some(open) = after.strip_prefix('"') else { continue };
            let Some(end) = open.find('"') else { continue };
            let val = open[..end].trim().to_owned();
            if val.len() >= 3 {
                out.push((i + 1, val));
            }
        }
    }
    out
}

fn is_projection_marker(line: &str) -> bool {
    line.contains("PROJECTED")
        || line.contains("not been executed")
        || line.contains("Nothing below has been")
        || line.contains("PROJECTION")
}

/// The shipped binary's own help text — the authority on which subcommands exist.
fn shipped_help(root: &Path) -> Option<String> {
    for candidate in [
        PathBuf::from("/Volumes/BuildShared/cargo-targets/release/omp-orchestrator"),
        root.join("target/release/omp-orchestrator"),
        root.join("target/debug/omp-orchestrator"),
    ] {
        if candidate.is_file() {
            if let Ok(o) = std::process::Command::new(&candidate).arg("--help").output() {
                let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
                s.push_str(&String::from_utf8_lossy(&o.stderr));
                if !s.trim().is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

#[test]
fn every_named_remediation_is_invocable_or_disclosed_as_projected() {
    let root = repo_root();
    let Some(help) = shipped_help(&root) else {
        eprintln!("SKIP every_named_remediation_is_invocable_or_disclosed_as_projected: no built binary");
        return;
    };

    let dir = root.join("docs/plan");
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|e| {
            e.flatten()
                .map(|x| x.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        let markers: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| is_projection_marker(l))
            .map(|(i, _)| i + 1)
            .collect();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

        for (line_no, cmd) in remediations(&text) {
            let toks: Vec<&str> = cmd.split_whitespace().collect();
            // Only judge invocations of OUR binary; a remedy naming git or cargo is
            // outside this gate's competence and saying so beats guessing.
            if toks.len() < 2 || !toks[0].starts_with("omp-orchestrator") {
                continue;
            }
            checked += 1;
            let sub = toks[1];
            let invocable = help.split_whitespace().any(|w| {
                w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-') == sub
            });
            if invocable {
                continue;
            }
            let nearest = markers
                .iter()
                .map(|m| line_no.abs_diff(*m))
                .min()
                .unwrap_or(usize::MAX);
            if nearest > 20 {
                failures.push(format!(
                    "{name}:{line_no}: remediation \"{cmd}\" names subcommand `{sub}` that the \
                     shipped binary does not answer, and the nearest projection marker is \
                     {nearest} lines away (limit 20)"
                ));
            }
        }
    }

    // ANTI-VACUITY: a scan that examined nothing must not read as a pass. Measured
    // 2026-09-01 there are 4 such strings in 08-end-users; zero means the extractor
    // broke or the section was renamed, and either way this gate stopped guarding.
    assert!(
        checked > 0,
        "ANTI-VACUITY: examined zero omp-orchestrator remediations across {} plan files. \
         Four were measured on 2026-09-01 in 08-end-users.md; zero means the extractor \
         broke, not that the plan is clean",
        files.len()
    );

    assert!(
        failures.is_empty(),
        "remediation names an unreachable remedy with no adjacent projection marker:\n  {}",
        failures.join("\n  ")
    );
}
