//! LINE-CITE ROT GATE — bead `omp-orchestrator-d1n`.
//!
//! # What was actually wrong, which is not what the bead assumed
//!
//! The bead says "321 line-cites -> constructs" on the theory that every `path:LINE`
//! citation rots as the tree moves. Measured 2026-09-01 across `docs/plan/`:
//!
//! | class | count |
//! |---|---:|
//! | total `path:LINE` citations | 236 |
//! | resolve inside this repo and the line EXISTS | 122 |
//! | resolve inside this repo and the line is BEYOND EOF | **0** |
//! | do not resolve here at all — they name OTHER REPOSITORIES | **111** |
//!
//! **In-repo rot is zero.** The predicted failure is not the one we have. The real
//! defect is the 111: `asupersync/...` (30), `franken_lean/...` (12), `pi_agent_rust/...`
//! (11), `beads_rust/...` (10) and friends are citations into repositories that are not
//! this one. They are legitimate evidence — the plan genuinely draws on Jeffrey's corpus
//! — but they are **unverifiable from here and typographically identical to a local
//! cite**, so a reader cannot tell which claims this repo can check and which it cannot.
//!
//! A first pass at this measurement reported "191 broken cites" because the resolver
//! only tried the literal path; a second reported 3 rotted because a basename fallback
//! matched `beads_rust/.beads/issues.jsonl` onto OUR `.beads/issues.jsonl`. Both were
//! reader defects, and both would have manufactured findings about a healthy document.
//! That is why this gate resolves conservatively and reports UNRESOLVED rather than
//! guessing — an unresolved cite is a question, not an accusation.
//!
//! # What this enforces
//!
//! 1. A cite that resolves inside this repo must name a line that EXISTS. This is the
//!    rot check, and it currently passes 122/122 — it exists to keep it that way.
//! 2. A cite that does not resolve here must name a KNOWN EXTERNAL REPO. Adding a new
//!    external source is a decision someone should make on purpose, not a typo that
//!    silently reads as evidence.
//! 3. ANTI-VACUITY: parsing zero cites is an ERROR. 236 were measured; a zero means the
//!    parser broke. `citation_integrity` failed exactly this way today — its parser
//!    matched 0 of 32 real citations because it required backticks the plan never uses,
//!    and only its anti-vacuity leg caught it.
//!
//! # NO-CLAIM
//!
//! An in-range line number is not a CORRECT one: this proves the line exists, never that
//! it still says what the citing sentence claims. Converting cites to construct names is
//! the durable fix and remains open under `d1n`; this gate stops the class from getting
//! worse and makes the external-vs-local split visible, which is strictly less than the
//! bead asks for and is stated so nobody reads a green here as "citations are correct".

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Repositories the plan legitimately cites and this checkout cannot verify. A cite
/// resolving to none of these and to no local file is a typo or a new source.
const KNOWN_EXTERNAL_REPOS: &[(&str, &str)] = &[
    ("asupersync", "Jeffrey's cancellation/scheduling substrate; the adapter contract this repo binds to"),
    ("franken_lean", "Jeffrey's Lean/proof-lane repo; source of the empty UNWIRED_LANE_ALLOWANCE pattern"),
    ("pi_agent_rust", "the OMP agent runtime we orchestrate"),
    ("beads_rust", "the tracker (br) whose JSONL format the plan cites"),
    ("frankensqlite", "Jeffrey's SQLite rewrite; cited for concurrent-writer claims"),
    ("control-plane", "the repo these crates are extracted FROM"),
    ("aadc", "Jeffrey's aadc repo in the mirror; 06-gates cites its .beads/issues.jsonl for the anti-vacuity discipline"),
];

fn repo_root() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if cur.join("docs/plan").is_dir() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

fn tracked(root: &std::path::Path) -> BTreeSet<String> {
    std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Conservative: literal path, then the two prefixes this repo actually uses. NO
/// basename fallback — that is what produced the false `beads_rust -> .beads` match.
fn resolve(path: &str, tracked: &BTreeSet<String>) -> Option<String> {
    for cand in [
        path.to_owned(),
        format!("crates/{path}"),
        format!("docs/{path}"),
    ] {
        if tracked.contains(&cand) {
            return Some(cand);
        }
    }
    let suffix = format!("/{path}");
    let hits: Vec<&String> = tracked.iter().filter(|t| t.ends_with(&suffix)).collect();
    if hits.len() == 1 {
        return Some(hits[0].clone());
    }
    None
}

/// `path.ext:LINE` — the extensions the plan actually cites.
fn cites(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(':') {
        let before = &text[..i];
        let after = &text[i + 1..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            continue;
        }
        let tok: String = before
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || "._/-".contains(*c))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !tok.contains('.') {
            continue;
        }
        // A path written with an ELISION (`.../doctor_subsystems/surface.rs`) is prose
        // shorthand for "somewhere under a long prefix", not a citation a reader could
        // follow. Flagging it would be the gate manufacturing findings about a
        // deliberate authorial abbreviation - the exact false-positive class that made
        // state-wildcard-lint 89% noise and got it routed around.
        if tok.starts_with("...") || tok.contains("/...") {
            continue;
        }
        let ok_ext = ["rs", "toml", "md", "jsonl", "json"]
            .iter()
            .any(|e| tok.ends_with(&format!(".{e}")));
        if ok_ext {
            if let Ok(n) = digits.parse::<usize>() {
                out.push((tok, n));
            }
        }
    }
    out
}

#[test]
fn every_in_repo_line_cite_names_a_line_that_exists() {
    let Some(root) = repo_root() else {
        eprintln!("SKIP every_in_repo_line_cite_names_a_line_that_exists: no repo root");
        return;
    };
    let files = tracked(&root);
    assert!(
        !files.is_empty(),
        "ANTI-VACUITY: git ls-files returned nothing; a broken listing would resolve every \
         cite to nothing and this gate would pass by failing to look"
    );

    let mut scanned = 0usize;
    let mut rotted: Vec<String> = Vec::new();
    let mut unresolved: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut lens: BTreeMap<String, usize> = BTreeMap::new();

    let dir = root.join("docs/plan");
    let mut mds: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect()
        })
        .unwrap_or_default();
    mds.sort();

    for md in &mds {
        let Ok(text) = std::fs::read_to_string(md) else { continue };
        let name = md.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_owned();
        for (path, line) in cites(&text) {
            scanned += 1;
            match resolve(&path, &files) {
                Some(real) => {
                    let n = *lens.entry(real.clone()).or_insert_with(|| {
                        std::fs::read_to_string(root.join(&real))
                            .map(|b| b.lines().count())
                            .unwrap_or(0)
                    });
                    if n > 0 && line > n {
                        rotted.push(format!("{name}: {path}:{line} -> {real} has {n} lines"));
                    }
                }
                None => {
                    let top = path.split('/').next().unwrap_or("").to_owned();
                    if !KNOWN_EXTERNAL_REPOS.iter().any(|(r, _)| *r == top) {
                        unresolved.entry(top).or_default().push(format!("{name}: {path}:{line}"));
                    }
                }
            }
        }
    }

    // ANTI-VACUITY: 236 cites measured 2026-09-01. Zero means the parser broke, which is
    // precisely how citation_integrity failed today - 0 of 32 matched, caught only here.
    assert!(
        scanned >= 50,
        "ANTI-VACUITY: parsed only {scanned} line-cites across {} plan files; 236 were \
         measured on 2026-09-01. A collapse means this parser broke, not that the plan \
         stopped citing.",
        mds.len()
    );

    assert!(
        rotted.is_empty(),
        "line-cite rot - the cited line is beyond end-of-file:\n  {}",
        rotted.join("\n  ")
    );

    // RATCHET, not a wall. These are real citations into OTHER repositories written
    // without a repo prefix -- `doctor.rs:924`, `consumer.rs:1299`, `audit_index.jsonl:3251`.
    // A reader cannot tell which repo they name, so they are unverifiable from anywhere,
    // which is a genuine defect. But there are dozens, converting them is bead `d1n`, and a
    // gate that is red for weeks gets routed around -- the measured death of
    // `state-wildcard-lint` at 89% false positives. So the count is a CEILING that may only
    // fall. New unprefixed external cites fail immediately; the existing debt is visible,
    // counted, and cannot grow.
    //
    // Lower this number when you convert some. Never raise it.
    const UNPREFIXED_EXTERNAL_CITE_CEILING: usize = 43; // measured 2026-09-01; d1n drives it down
    let unresolved_count: usize = unresolved.values().map(Vec::len).sum();
    assert!(
        unresolved_count <= UNPREFIXED_EXTERNAL_CITE_CEILING,
        "unprefixed external line-cites rose to {unresolved_count}, above the ceiling of \
         {UNPREFIXED_EXTERNAL_CITE_CEILING}. These name files in OTHER repositories with no \
         repo prefix, so no reader can resolve them. Convert the new one to a construct-cite \
         or prefix it with a KNOWN_EXTERNAL_REPOS name; do NOT raise the ceiling:\n  {}",
        unresolved
            .iter()
            .map(|(k, v)| format!("[{k}] x{}", v.len()))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// The allowance table must not rot into a list of names nobody cites any more - a stale
/// permit is indistinguishable from a live one and quietly widens what passes.
#[test]
fn every_known_external_repo_is_still_cited_and_carries_a_reason() {
    let Some(root) = repo_root() else { return };
    let dir = root.join("docs/plan");
    let mut all = String::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "md") {
                all.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
            }
        }
    }
    assert!(!all.is_empty(), "ANTI-VACUITY: read zero plan text");
    let mut dead = Vec::new();
    for (repo, reason) in KNOWN_EXTERNAL_REPOS {
        assert!(
            !reason.trim().is_empty(),
            "external repo {repo} has no reason; an allowance without one is a placeholder"
        );
        if !all.contains(&format!("{repo}/")) {
            dead.push(*repo);
        }
    }
    assert!(
        dead.is_empty(),
        "KNOWN_EXTERNAL_REPOS entries no longer cited anywhere in the plan - remove them so \
         the table stays a record of real sources: {dead:?}"
    );
}
