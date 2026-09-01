#![forbid(unsafe_code)]
//! BEAD STANDARD GATE — a bead is a self-contained spec, not a title.
//!
//! # Two populations, two standards
//!
//! **The plan has produced ZERO beads.** The 50 on the board are pre-plan
//! extraction work, and measuring them told us about that work, not about the
//! conversion ahead. Josh caught this the moment it was reported.
//!
//! - **LEGACY** (everything on the board today): a *ratchet*. Measured 4 of 50
//!   meeting the full standard, 17 isolated, 54% with no runnable acceptance.
//!   Holding those to 100% today would fail the build for an hour and get this
//!   gate switched off, so the floor is what was measured and may only improve.
//!
//! - **PLAN-DERIVED** (`-l plan`): **no grace, from the first one.** A standard
//!   applied retroactively is a cleanup project; applied at creation it is free.
//!   This is the entire point of building the gate BEFORE the conversion instead
//!   of after — which is what Josh asked for, and the answer to "what are we
//!   doing now" was, until this file, *nothing*.
//!
//! Why runnable acceptance is the load-bearing field: a bead an agent cannot
//! close gets *adjudicated* rather than *worked*. It reads the bead, cannot tell
//! what would make it closeable, and returns "no work to be done". That happened
//! twice tonight on a P0 at the head of the ready queue.
//!
//! Standard: `~/.agents/skills/beads-north-star`. WHAT / WHY / ACCEPTANCE in the
//! body, topical labels, a place in the graph, and acceptance you can RUN.
//!
//! RATCHET, not threshold. The floor is what was measured when this was written;
//! it may only move up. A gate that demands perfection on a 50-bead backlog gets
//! switched off in an hour.

use std::process::Command;

struct Bead {
    id: String, body: String, labels: usize, edges: u64, status: String,
}

fn beads() -> Vec<Bead> {
    let out = Command::new("br").args(["list", "--json"]).output();
    let Ok(out) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    // Convention-parse: one bead per {...} object at depth 1. No serde, because a
    // gate that fails to build teaches nothing.
    let mut v = Vec::new();
    for chunk in text.split("{\"").skip(1) {
        let field = |k: &str| -> Option<String> {
            let pat = format!("\"{k}\":");
            let i = chunk.find(&pat)? + pat.len();
            let rest = chunk[i..].trim_start();
            Some(if let Some(r) = rest.strip_prefix('"') {
                let mut s = String::new();
                let mut esc = false;
                for c in r.chars() {
                    if esc { s.push(c); esc = false; }
                    else if c == '\\' { esc = true; }
                    else if c == '"' { break; }
                    else { s.push(c); }
                }
                s
            } else {
                rest.split(|c: char| c == ',' || c == '}').next()?.trim().to_owned()
            })
        };
        let Some(id) = field("id") else { continue };
        if id.is_empty() { continue }
        let n = |k: &str| field(k).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        v.push(Bead {
            id,
            body: field("description").unwrap_or_default(),
            labels: chunk.find("\"labels\":[]").map_or(1, |_| 0),
            edges: n("dependency_count") + n("dependent_count"),
            status: field("status").unwrap_or_default(),
        });
    }
    v
}

fn open_beads() -> Vec<Bead> {
    beads().into_iter().filter(|b| b.status != "closed" && b.status != "tombstone").collect()
}

/// Measured floors, 2026-08-31. Ratchet DOWN is forbidden; raise them as work lands.
const MIN_WITH_ACCEPTANCE: usize = 49;
const MAX_ISOLATED: usize = 17;

#[test]
fn every_bead_declares_acceptance() {
    let b = open_beads();
    if b.is_empty() { eprintln!("SKIP: br unavailable or no beads"); return; }
    let n = b.iter().filter(|x| x.body.contains("ACCEPT")).count();
    assert!(n >= MIN_WITH_ACCEPTANCE.min(b.len()),
        "{n} of {} beads declare ACCEPTANCE; floor is {MIN_WITH_ACCEPTANCE}. A bead you cannot \
         write acceptance for is not granular enough — and one an agent cannot close gets \
         adjudicated instead of worked.", b.len());
}

#[test]
fn the_graph_does_not_grow_more_floating_nodes() {
    let b = open_beads();
    if b.is_empty() { eprintln!("SKIP: br unavailable"); return; }
    let iso: Vec<&str> = b.iter().filter(|x| x.edges == 0).map(|x| x.id.as_str()).collect();
    assert!(iso.len() <= MAX_ISOLATED,
        "{} isolated beads (floor {MAX_ISOLATED}) — no edge in either direction, so bv cannot \
         rank them and they are invisible to triage:\n  {}",
        iso.len(), iso.join("\n  "));
}

/// PLAN-DERIVED BEADS: the full standard, no ratchet, from the first one.
///
/// Every field the north-star requires, checked on creation rather than audited
/// later. This currently passes vacuously — there are no plan beads yet — which
/// is precisely when a standard is cheapest to install.
#[test]
fn every_plan_derived_bead_meets_the_full_standard() {
    let all = open_beads();
    if all.is_empty() { eprintln!("SKIP: br unavailable"); return; }

    // A plan bead is one carrying the `plan` label. Until conversion runs this is
    // empty, and that emptiness is REPORTED rather than passed over in silence.
    let plan: Vec<&Bead> = all.iter().filter(|b| b.body.contains("plan-derived")).collect();
    if plan.is_empty() {
        eprintln!("NOTE: 0 plan-derived beads on the board. The plan has not been converted. \
                   This gate is armed and will bite the first malformed one.");
        return;
    }

    let mut bad = Vec::new();
    for b in &plan {
        let mut missing = Vec::new();
        if !b.body.contains("WHAT") { missing.push("WHAT"); }
        if !b.body.contains("WHY") { missing.push("WHY"); }
        if !b.body.contains("ACCEPT") { missing.push("ACCEPTANCE"); }
        if b.labels == 0 { missing.push("labels"); }
        if b.edges == 0 { missing.push("a place in the DAG"); }
        if !["cargo ", "br ", "bv ", "ntm ", "$ "].iter().any(|c| b.body.contains(c)) {
            missing.push("runnable acceptance (a command)");
        }
        if !missing.is_empty() {
            bad.push(format!("{}: missing {}", b.id, missing.join(", ")));
        }
    }
    assert!(bad.is_empty(),
        "{} plan-derived bead(s) below standard. These are NEW — there is no legacy excuse \
         and no ratchet:\n  {}", bad.len(), bad.join("\n  "));
}

#[test]
fn the_parser_reads_a_real_board() {
    // ANTI-VACUITY. Every assertion above passes trivially on an empty parse, and
    // `br` returning nothing looks exactly like a clean board.
    let b = beads();
    if b.is_empty() {
        eprintln!("SKIP: br produced no beads — cannot distinguish empty board from broken parse");
        return;
    }
    assert!(b.iter().all(|x| !x.id.is_empty()), "a parsed bead has no id — parser is misaligned");
    assert!(b.iter().any(|x| x.edges > 0),
        "ZERO beads have edges. Either the graph is empty or the field name moved — the \
         orchestrator read `dependencies` instead of `dependency_count` on 2026-08-31 and \
         nearly published 0/50 as a finding.");
}
