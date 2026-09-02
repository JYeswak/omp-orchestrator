#![forbid(unsafe_code)]
//! Legs for the round stamp. Four required: known-GOOD, fires-on-known-bad, mutation with a
//! byte-identical restore, and anti-vacuity.

use convergence_stamp::{
    census, parse_stamp, refusals, render_document, render_stamp, round_ledger_files,
    section_files, sha256_hex, RoundCensus, RoundRefusal, Stamp,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives two levels below repo root")
        .to_path_buf()
}

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omp-round-stamp-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("docs/plan")).expect("fixture");
    root
}

/// FIPS 180-4 published vectors. The digest is what makes the stamp pin content instead of an
/// mtime, so it is checked against an authority outside this repo, not against itself.
#[test]
fn sha256_matches_the_published_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    // and it agrees with the tool the rest of this repo cites
    let readme = repo_root().join("README.md");
    if let Ok(bytes) = fs::read(&readme) {
        let ours = sha256_hex(&bytes);
        let out = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(&readme)
            .output()
            .expect("shasum must run");
        let theirs = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned();
        assert_eq!(ours, theirs, "our digest must agree with shasum -a 256");
    }
}

/// ANTI-VACUITY. An empty repo must be an ERROR, never an admitted round.
#[test]
fn an_empty_census_refuses_rather_than_admitting() {
    let root = fixture("empty");
    let c = census(&root);
    assert!(c.rows.is_empty(), "fixture has no rounds");
    let refused = refusals(&c, None);
    assert_eq!(refused, vec![RoundRefusal::EmptyCensus]);
    assert!(
        refused[0].code() == "ROUND_REFUSED_EMPTY_CENSUS",
        "an empty scan must be distinguishable from a clean one"
    );
}

fn seed(root: &Path) {
    fs::write(
        root.join("docs/plan/CONVERGENCE.jsonl"),
        "{\"round\":8,\"section\":\"00-brief\",\"lens\":\"evidence\",\"graded_by\":\"A\",\
         \"new_findings\":2,\"verdict\":\"FAIL\"}\n",
    )
    .expect("seed convergence");
    fs::write(
        root.join("docs/plan/round19-Eye.jsonl"),
        "{\"round\":19,\"section\":\"00-brief\",\"lens\":\"fresh\",\"graded_by\":\"B\",\
         \"new_findings\":3,\"verdict\":\"MAJOR_OPEN\"}\n",
    )
    .expect("seed round file");
    fs::write(root.join("docs/plan/00-brief.md"), b"section one\n").expect("seed section");
}

/// KNOWN-GOOD. A freshly cut stamp admits the next round.
///
/// Without this leg the gate is attack-only, and an over-strict gate gets routed around —
/// `state-wildcard-lint` reached 89% false positives that way.
#[test]
fn a_freshly_cut_stamp_admits_a_round() {
    let root = fixture("good");
    seed(&root);
    let c = census(&root);
    fs::write(root.join("docs/plan/STAMP.toml"), render_stamp(&c, "test")).expect("write stamp");
    let stamp = parse_stamp(&fs::read_to_string(root.join("docs/plan/STAMP.toml")).expect("read"));
    assert_eq!(refusals(&c, Some(&stamp)), Vec::new(), "a current stamp admits");
    assert_eq!(stamp.rounds, BTreeSet::from([8, 19]));
}

/// FIRES-ON-KNOWN-BAD 1: a round exists that the stamp does not name.
///
/// This is the exact measured failure — rounds 16-21 lived only in per-agent files and never
/// reached the canonical ledger, so nothing noticed they were unrepresented.
#[test]
fn a_round_the_stamp_does_not_name_refuses_and_names_it() {
    let root = fixture("newround");
    seed(&root);
    let c0 = census(&root);
    fs::write(root.join("docs/plan/STAMP.toml"), render_stamp(&c0, "test")).expect("stamp");

    // a new per-agent round file lands, exactly as rounds 16-21 did
    fs::write(
        root.join("docs/plan/round23-Newcomer.jsonl"),
        "{\"round\":23,\"section\":\"00-brief\",\"lens\":\"x\",\"graded_by\":\"C\",\
         \"new_findings\":7,\"verdict\":\"BLOCKED\"}\n",
    )
    .expect("new round");

    let c1 = census(&root);
    let stamp = parse_stamp(&fs::read_to_string(root.join("docs/plan/STAMP.toml")).expect("read"));
    let refused = refusals(&c1, Some(&stamp));
    assert!(
        refused.iter().any(|r| matches!(r, RoundRefusal::RoundNotInStamp { round: 23, .. })),
        "{refused:?}"
    );
    let rendered = refused
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("round=23"), "the refusal must NAME the round:\n{rendered}");
    assert!(
        rendered.contains("round23-Newcomer.jsonl"),
        "and name the file that carried it:\n{rendered}"
    );
    // the remedy must be invocable, not decorative
    assert!(
        rendered.contains("cargo run -p convergence-stamp -- --write"),
        "a refusal whose remedy does not exist is a trap, not a guard:\n{rendered}"
    );
}

/// FIRES-ON-KNOWN-BAD 2: a section edited after the stamp refuses the next round.
#[test]
fn a_section_edited_after_the_stamp_refuses() {
    let root = fixture("drift");
    seed(&root);
    let c0 = census(&root);
    fs::write(root.join("docs/plan/STAMP.toml"), render_stamp(&c0, "test")).expect("stamp");

    fs::write(root.join("docs/plan/00-brief.md"), b"section one, edited\n").expect("edit");
    let c1 = census(&root);
    let stamp = parse_stamp(&fs::read_to_string(root.join("docs/plan/STAMP.toml")).expect("read"));
    let refused = refusals(&c1, Some(&stamp));
    assert!(
        refused.iter().any(|r| matches!(r, RoundRefusal::SectionDrifted { section, .. } if section == "00-brief.md")),
        "{refused:?}"
    );
}

/// FIRES-ON-KNOWN-BAD 3: no stamp at all refuses.
#[test]
fn no_stamp_refuses() {
    let root = fixture("nostamp");
    seed(&root);
    let c = census(&root);
    assert_eq!(refusals(&c, None), vec![RoundRefusal::NoStamp]);
}

/// MUTATION, with a byte-identical restore.
///
/// Break the thing the round check keys on — the stamp's `rounds` list — confirm RED, restore
/// byte-identically, confirm GREEN. A leg that stays green under mutation is not attributable.
#[test]
fn mutating_the_rounds_list_goes_red_and_a_byte_identical_restore_goes_green() {
    let root = fixture("mutation");
    seed(&root);
    let c = census(&root);
    let path = root.join("docs/plan/STAMP.toml");
    fs::write(&path, render_stamp(&c, "test")).expect("stamp");

    let before = fs::read(&path).expect("read");
    let before_sha = sha256_hex(&before);
    let stamp = parse_stamp(&String::from_utf8_lossy(&before));
    assert_eq!(refusals(&c, Some(&stamp)), Vec::new(), "baseline must be GREEN");

    // drop round 19 from the list — the single fact the round predicate reads
    let mutated = String::from_utf8_lossy(&before).replace("rounds = [8, 19]", "rounds = [8]");
    assert_ne!(
        mutated.as_bytes(),
        before.as_slice(),
        "the mutation must actually change the file, or the leg proves nothing"
    );
    fs::write(&path, mutated.as_bytes()).expect("mutate");
    let red = refusals(&c, Some(&parse_stamp(&fs::read_to_string(&path).expect("read"))));
    assert!(
        red.iter().any(|r| matches!(r, RoundRefusal::RoundNotInStamp { round: 19, .. })),
        "mutation must go RED on exactly the dropped round: {red:?}"
    );

    fs::write(&path, &before).expect("restore");
    let after_sha = sha256_hex(&fs::read(&path).expect("read"));
    assert_eq!(before_sha, after_sha, "restore must be byte-identical");
    let green = refusals(&c, Some(&parse_stamp(&fs::read_to_string(&path).expect("read"))));
    assert_eq!(green, Vec::new(), "restored stamp must be GREEN again");
}

/// Inputs are DISCOVERED from disk, never hand-listed — a hand-listed set is how rounds 16-21
/// went unnoticed, so the discovery itself gets a test.
#[test]
fn every_round_bearing_file_on_disk_is_discovered() {
    let root = repo_root();
    let found: BTreeSet<String> = round_ledger_files(&root)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        found.contains("CONVERGENCE.jsonl"),
        "the canonical ledger must be discovered: {found:?}"
    );
    let mut per_agent = 0usize;
    for entry in fs::read_dir(root.join("docs/plan")).expect("plan dir").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("round") && name.ends_with(".jsonl") {
            per_agent += 1;
            assert!(found.contains(&name), "{name} was on disk and not discovered");
        }
    }
    assert!(
        per_agent > 0,
        "ANTI-VACUITY: zero per-agent round files found, so this test proved nothing"
    );
    assert!(
        !section_files(&root).is_empty(),
        "ANTI-VACUITY: zero numbered plan sections found"
    );
}

/// The real repo: the census must see BOTH halves of the split record.
#[test]
fn the_real_census_covers_both_halves_of_the_split_record() {
    let c = census(&repo_root());
    assert!(!c.rows.is_empty(), "ANTI-VACUITY: real census is empty");

    let from_canonical: BTreeSet<u64> = c
        .rows
        .iter()
        .filter(|r| r.source_file.ends_with("CONVERGENCE.jsonl"))
        .map(|r| r.round)
        .collect();
    let from_per_agent: BTreeSet<u64> = c
        .rows
        .iter()
        .filter(|r| !r.source_file.ends_with("CONVERGENCE.jsonl"))
        .map(|r| r.round)
        .collect();

    assert!(!from_canonical.is_empty(), "canonical half missing: {from_canonical:?}");
    assert!(!from_per_agent.is_empty(), "per-agent half missing: {from_per_agent:?}");
    // the measured defect: rounds present ONLY in per-agent files
    let only_per_agent: Vec<u64> = from_per_agent.difference(&from_canonical).copied().collect();
    assert!(
        !only_per_agent.is_empty(),
        "if this ever becomes empty the split is healed and this assertion should be inverted; \
         today the rounds living only in per-agent files are the reason this crate exists"
    );
}

/// The document must contain every round it censused — a summary that silently drops a round is
/// the same defect one layer up.
#[test]
fn the_document_names_every_round_in_the_census() {
    let c = census(&repo_root());
    let doc = render_document(&c, "test");
    for round in c.declared_by_round.keys() {
        assert!(
            doc.contains(&format!("| **{round}** |")),
            "round {round} missing from the rendered document"
        );
    }
    assert!(doc.contains("NO-CLAIM"), "the document must carry its own limits");
}

/// A stamp with zero sections must never admit: it would match everything vacuously.
#[test]
fn a_stamp_with_no_sections_cannot_admit() {
    let mut c = RoundCensus::default();
    c.rows.push(convergence_stamp::RoundRow {
        round: 1,
        section: "00".to_owned(),
        lens: "l".to_owned(),
        graded_by: "g".to_owned(),
        declared: 1,
        verdict: "v".to_owned(),
        source_file: "f".to_owned(),
    });
    c.declared_by_round.insert(1, 1);
    c.sources_by_round.insert(1, BTreeSet::from(["f".to_owned()]));
    // no section digests at all
    assert_eq!(refusals(&c, None), vec![RoundRefusal::EmptyCensus]);

    // and with sections present, an empty stamp still refuses rather than passing
    c.section_digests = BTreeMap::from([("00-a.md".to_owned(), "deadbeef".to_owned())]);
    let empty = Stamp::default();
    let refused = refusals(&c, Some(&empty));
    assert!(!refused.is_empty(), "an empty stamp must refuse: {refused:?}");
}
