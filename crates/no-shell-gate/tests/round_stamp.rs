#![forbid(unsafe_code)]
//! ROUND ADMISSION GATE — no new grading round until one stamped document covers every round.
//!
//! Josh, 2026-09-01: *"make sure we get all rounds of edits into a single doc — no more rounds
//! of convergence until we have a stamped doc with all rounds included in it. only then can we
//! start another round."*
//!
//! The predicate lives in `convergence-stamp`; this file is the WIRE. It exists in
//! `no-shell-gate/tests/` because `.github/workflows/gate.yml:32` runs
//! `cargo test -p no-shell-gate`, so the refusal reaches CI through a job that already exists
//! rather than needing a tenth one.
//!
//! # Honest wiring note
//!
//! `gate.yml` is INVALID YAML at the time of writing — duplicate `runs-on`/`steps` in
//! `kernel-bypass-gate` at lines 46/52 and 47/53, bead
//! `omp-orchestrator-strict-parser-rejects-gate-yml-m0c`. A strict parser refuses the whole
//! file, so **that CI trigger cannot fire today**. The reachable trigger on this machine is the
//! tracked pre-commit hook binary. This gate is not wired in CI until m0c lands, and saying so
//! here is the point: a gate that reports itself wired when its trigger cannot fire is worse
//! than no gate, because a reader stops looking.

use convergence_stamp::{census, parse_stamp, refusals, RoundRefusal};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives two levels below repo root")
        .to_path_buf()
}

/// THE GATE. Every round on disk must be named by the stamp, and no section may have moved.
#[test]
fn no_new_round_is_admitted_unless_the_stamp_covers_every_round() {
    let root = repo_root();
    let c = census(&root);

    // ANTI-VACUITY: an empty census reports identically to a clean repo.
    assert!(
        !c.rows.is_empty(),
        "ANTI-VACUITY: zero round rows censused — every admission check below would pass \
         vacuously. Expected rows from docs/plan/CONVERGENCE.jsonl and docs/plan/round*.jsonl"
    );
    assert!(
        !c.section_digests.is_empty(),
        "ANTI-VACUITY: zero plan sections digested"
    );

    let stamp_path = root.join("docs/plan/STAMP.toml");
    let stamp = fs::read_to_string(&stamp_path).ok().map(|t| parse_stamp(&t));
    let refused = refusals(&c, stamp.as_ref());

    assert!(
        refused.is_empty(),
        "A NEW ROUND IS REFUSED — {} reason(s). Josh's rule: one stamped doc covering every \
         round, then and only then another round.\n  {}",
        refused.len(),
        refused
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// The stamp must name EVERY round the census found — asserted independently of `refusals`, so
/// a bug in the predicate cannot hide a missing round from both checks at once.
#[test]
fn the_stamp_names_every_round_the_census_found() {
    let root = repo_root();
    let c = census(&root);
    let stamp = parse_stamp(
        &fs::read_to_string(root.join("docs/plan/STAMP.toml"))
            .expect("docs/plan/STAMP.toml must exist — cut it with `cargo run -p convergence-stamp -- --write`"),
    );
    assert!(!c.declared_by_round.is_empty(), "ANTI-VACUITY: no rounds censused");

    let missing: Vec<u64> = c
        .declared_by_round
        .keys()
        .filter(|r| !stamp.rounds.contains(r))
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "rounds on disk and absent from the stamp: {missing:?} — remedy: \
         cargo run -p convergence-stamp -- --write"
    );
}

/// The single document must exist and must name every round, so the stamp cannot be current
/// against a document that silently dropped one.
#[test]
fn the_single_document_exists_and_names_every_round() {
    let root = repo_root();
    let doc = fs::read_to_string(root.join("docs/plan/ROUNDS.md"))
        .expect("docs/plan/ROUNDS.md must exist — it is the single stamped round document");
    let c = census(&root);
    assert!(!c.declared_by_round.is_empty(), "ANTI-VACUITY: no rounds censused");
    for round in c.declared_by_round.keys() {
        assert!(
            doc.contains(&format!("| **{round}** |")),
            "ROUNDS.md does not name round {round}"
        );
    }
}

/// KNOWN-BAD, run against a fixture so the real stamp is never touched: an unstamped round is
/// refused and the refusal NAMES the round, the file, and an invocable remedy.
#[test]
fn an_unstamped_round_is_refused_with_a_named_invocable_remedy() {
    let root = std::env::temp_dir().join(format!(
        "omp-round-stamp-wire-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("docs/plan")).expect("fixture");
    fs::write(
        root.join("docs/plan/CONVERGENCE.jsonl"),
        "{\"round\":8,\"section\":\"00-brief\",\"new_findings\":1}\n",
    )
    .expect("seed");
    fs::write(root.join("docs/plan/00-brief.md"), b"x\n").expect("seed section");
    fs::write(
        root.join("docs/plan/STAMP.toml"),
        "[stamp]\nrounds = [8]\n\n[sections]\n\"00-brief.md\" = \"\
         2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881\"\n",
    )
    .expect("seed stamp");

    // a fresh round lands, unstamped — exactly how rounds 16-21 arrived
    fs::write(
        root.join("docs/plan/round99-Fresh.jsonl"),
        "{\"round\":99,\"section\":\"00-brief\",\"new_findings\":5}\n",
    )
    .expect("seed round");

    let c = census(&root);
    let stamp = parse_stamp(&fs::read_to_string(root.join("docs/plan/STAMP.toml")).expect("read"));
    let refused = refusals(&c, Some(&stamp));
    assert!(
        refused.iter().any(|r| matches!(r, RoundRefusal::RoundNotInStamp { round: 99, .. })),
        "{refused:?}"
    );
    let text = refused.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
    assert!(text.contains("round=99"), "must name the round:\n{text}");
    assert!(text.contains("round99-Fresh.jsonl"), "must name the file:\n{text}");
    assert!(
        text.contains("cargo run -p convergence-stamp -- --write"),
        "the remedy must be invocable:\n{text}"
    );
    let _ = fs::remove_dir_all(&root);
}
