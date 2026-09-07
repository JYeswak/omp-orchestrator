//! S1 criterion R8, self-referential half — the half that had no runner.
//!
//! NAMED TARGET per the mmt4 ruling: cite this as
//! `cargo test -p text-structure --test self_referential`, never a bare `-p` aggregate.
//!
//! THE CLASS, measured across one session, every instance a FALSE POSITIVE on a document that
//! was already correct: a citation-hygiene scan over a corpus containing its own defect
//! reports finds its own specimens. `CONTRACT.md`'s only `mirror:beads_rust/...` hit is the
//! sentence refusing that form; `planning_to_exhaustion.md`'s unprefixed ids are inside the
//! row reporting the dropped `j`; a contract's own doc-name census counted itself 24/9 where
//! the truth was 23/8.
//!
//! Every leg below asserts a MESSAGE or a split, never a bare bool.

use text_structure::{classify_scan, prose_specimen_stripped, ScanHit};

fn hit(path: &str, matched: &str) -> ScanHit {
    ScanHit {
        path: path.to_owned(),
        matched: matched.to_owned(),
    }
}

/// POSITIVE CONTROL, and it is mandatory: a stripper that removes everything would make every
/// scan pass. Prove it still FINDS a needle that is NOT a specimen before trusting any zero.
#[test]
fn the_stripper_still_finds_a_needle_outside_a_specimen() {
    let source = "A bare mirror:beads_rust/foo cite sits here in prose.\n";
    let stripped = prose_specimen_stripped(source);
    assert!(
        stripped.contains("mirror:beads_rust/foo"),
        "the stripper must NOT blank prose; a stripper that removes everything makes every \
         scan vacuously green: {stripped:?}"
    );
}

/// FIRES-ON-KNOWN-BAD: the real `CONTRACT.md` shape — a specimen inside the rule refusing it.
#[test]
fn a_specimen_inside_the_rule_is_blanked() {
    let source =
        "a scout labelled ack-spine as `mirror:beads_rust/...` — fabricated; refused.\n";
    let stripped = prose_specimen_stripped(source);
    assert!(
        !stripped.contains("mirror:beads_rust"),
        "a backticked specimen must be blanked, or the rule's own text scores as a violation: \
         {stripped:?}"
    );
    assert_eq!(
        stripped.lines().count(),
        source.lines().count(),
        "line structure must survive stripping — line numbers are cited against it"
    );
    assert_eq!(
        stripped.len(),
        source.len(),
        "blanking must preserve byte length so column offsets survive"
    );
}

/// A fenced block is a specimen too. This is the shape a commit message or a bead comment uses.
#[test]
fn a_fenced_block_is_blanked_and_prose_after_it_is_not() {
    let source = "before plf.7.2 here\n```\nplf.7.2 inside a fence\n```\nafter plf.7.2 here\n";
    let stripped = prose_specimen_stripped(source);
    let occurrences = stripped.matches("plf.7.2").count();
    assert_eq!(
        occurrences, 2,
        "the two prose occurrences must survive and the fenced one must not; got {occurrences} \
         in {stripped:?}"
    );
}

/// An UNCLOSED span blanks to end of line. Over-stripping is the safe direction: it can only
/// report LESS, and the failure being prevented is a false positive against a compliant file.
#[test]
fn an_unclosed_span_strips_to_end_of_line_rather_than_leaking() {
    let source = "text `unclosed specimen plf.7.2\nnext line plf.7.2\n";
    let stripped = prose_specimen_stripped(source);
    assert_eq!(
        stripped.matches("plf.7.2").count(),
        1,
        "only the SECOND line's occurrence may survive: {stripped:?}"
    );
}

/// A source with no backticks is returned borrowed — the cheap path must not allocate.
#[test]
fn a_specimen_free_source_is_returned_unchanged() {
    let source = "no specimens here at all\n";
    assert!(matches!(
        prose_specimen_stripped(source),
        std::borrow::Cow::Borrowed(_)
    ));
}

/// THE CLASSIFIER: a hit in the file that DECLARES the rule is never citable as a defect.
#[test]
fn a_hit_in_the_rule_file_is_self_referential_not_citable() {
    let hits = vec![
        hit("docs/plan/flow/CONTRACT.md", "mirror:beads_rust/..."),
        hit("docs/plan/flow/boxes/S3.toml", "mirror:beads_rust/x"),
    ];
    let verdict = classify_scan(&hits, &["docs/plan/flow/CONTRACT.md"]).expect("classifies");
    assert_eq!(verdict.self_referential.len(), 1);
    assert_eq!(verdict.citable.len(), 1);
    assert_eq!(verdict.citable[0].path, "docs/plan/flow/boxes/S3.toml");

    let census = verdict.census();
    assert!(census.contains("hits=2"), "{census:?}");
    assert!(census.contains("self_referential=1"), "{census:?}");
    assert!(
        census.contains("citable=1"),
        "the census must carry BOTH denominators — a bare count of either half is the defect \
         it exists to split: {census:?}"
    );
}

/// ANTI-VACUITY: an empty hit set is an ERROR with a named reason, never an empty pass.
#[test]
fn an_empty_scan_set_is_a_named_error() {
    let error = classify_scan(&[], &["x"]).expect_err("an empty scan must refuse");
    assert!(
        error.contains("SCAN_EMPTY reason=zero_hits"),
        "typed, not prose: {error:?}"
    );
    assert!(
        error.contains("never a pass"),
        "the refusal must say why: {error:?}"
    );
}

/// A declared rule file that matches NOTHING is also an error. An allowlist row matching
/// nothing is the same never-fires shape as the gate it guards, and it silently widens the
/// citable set on the next edit.
#[test]
fn a_stale_rule_file_row_is_refused_by_name() {
    let hits = vec![hit("docs/plan/flow/boxes/S3.toml", "x")];
    let error = classify_scan(&hits, &["docs/plan/flow/GONE.md"])
        .expect_err("a rule file matching no hit must refuse");
    assert!(
        error.contains("SCAN_STALE_RULE_FILE"),
        "typed: {error:?}"
    );
    assert!(
        error.contains("GONE.md"),
        "the refusal must NAME the stale row: {error:?}"
    );
}

/// A suffix match counts, so a caller may declare `CONTRACT.md` without the full path — but it
/// must still match something, which the stale-row leg above enforces.
#[test]
fn a_rule_file_may_be_declared_by_suffix() {
    let hits = vec![hit("docs/plan/flow/CONTRACT.md", "x")];
    let verdict = classify_scan(&hits, &["CONTRACT.md"]).expect("suffix must match");
    assert_eq!(verdict.self_referential.len(), 1);
    assert!(verdict.citable.is_empty());
}

/// THE REAL CORPUS, end to end: the two documents that bit us this session must come back
/// SELF-REFERENTIAL, not as defects. Declines when the corpus is absent rather than passing.
#[test]
fn the_real_documents_that_bit_us_classify_as_self_referential() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let contract = root.join("docs/plan/flow/CONTRACT.md");
    let exhaustion = root.join("docs/contracts/planning_to_exhaustion.md");
    if !contract.is_file() || !exhaustion.is_file() {
        println!(
            "UNMEASURED reason=corpus_absent root={} — rch syncs source, so this may run on a \
             worker without docs/. Not a pass for the subject.",
            root.display()
        );
        return;
    }

    // CONTRACT.md: the mirror-ellipsis specimen.
    let raw = std::fs::read_to_string(&contract).expect("readable");
    assert!(
        raw.contains("mirror:beads_rust/..."),
        "premise: CONTRACT.md must still contain the specimen, else this leg proves nothing"
    );
    let stripped = prose_specimen_stripped(&raw);
    assert!(
        !stripped.contains("mirror:beads_rust/..."),
        "the CONTRACT.md specimen must be stripped — it is quoted inside the rule refusing it"
    );

    // planning_to_exhaustion.md: the dropped-j specimens in the row reporting the defect.
    let raw2 = std::fs::read_to_string(&exhaustion).expect("readable");
    let bare_before = raw2.matches("`plf.7.2`").count();
    assert!(
        bare_before > 0,
        "premise: the PX-D5 row must still quote the specimen"
    );
    let stripped2 = prose_specimen_stripped(&raw2);
    assert!(
        !stripped2.contains("plf.7.2"),
        "the dropped-j ids are backticked specimens inside the finding that reports them and \
         must not survive stripping"
    );
}
