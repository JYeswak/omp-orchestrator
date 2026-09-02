#![forbid(unsafe_code)]

//! INVARIANT SUITE for `docs/contracts/finding_contract.md` — laws `FC-L1` … `FC-L5`.
//!
//! # This suite exercises Finding::file with a test Publisher
//!
//! The production caller is the supervisor route, which uses BrPublisher and is tested in
//! the supervisor binary and br-publisher integration tests below the crate boundary.
//! The contract tests continue to prove the spool ordering, cancellation, and recovery laws.
//!
//! # Three legs assert a law is NOT enforced, on purpose
//!
//! `FC-L2`, `FC-L4` and `FC-L5` are stated in the contract and unenforced in the code. Those
//! legs are **pinned defects**: they assert today's behaviour and name the bead that will
//! change it, so the fix cannot land silently — the leg goes RED and forces the contract to
//! be updated in the same commit. A characterisation test is the honest alternative to a
//! contract that claims a guarantee its type does not carry.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use asupersync::Cx;
use asupersync::runtime::RuntimeBuilder;
use finding::{Finding, FindingError, Publisher, pending};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .expect("crate must live beneath the workspace root")
        .to_path_buf()
}

fn source() -> String {
    std::fs::read_to_string(crate_root().join("src/lib.rs")).expect("the crate source must exist")
}

/// A scratch spool directory, unique per leg so parallel test threads cannot collide.
fn spool_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "finding-contract-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|x| x.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn complete_finding() -> Finding {
    Finding::new(
        "Finding::file has no caller, so the spool-then-publish path has never run",
        "the crate was written to make an unfiled gap impossible and was then bypassed in the \
         next tool call; a mechanism that has never executed cannot be relied on",
        "run `cargo test -p finding --test finding_contract`; the suite must exercise file() \
         against a test Publisher and observe the spool row on disk before publish",
        vec!["gate".into(), "kernel".into()],
        1,
    )
    .expect("the fixture finding must satisfy the bead contract")
}

// ---------------------------------------------------------------------------------------
// A Publisher, so `file()` is reachable at all
// ---------------------------------------------------------------------------------------

/// Records what it observed AT PUBLISH TIME, which is how `FC-L3`'s ordering is proved
/// rather than asserted: the durable row must already be on disk when the publisher runs.
struct TestPublisher {
    spool_dir: PathBuf,
    returns: Result<String, ()>,
    pending_at_publish: Cell<usize>,
    ran: Cell<bool>,
}

impl TestPublisher {
    fn ok(spool_dir: &Path, id: &str) -> Self {
        Self {
            spool_dir: spool_dir.to_path_buf(),
            returns: Ok(id.to_owned()),
            pending_at_publish: Cell::new(usize::MAX),
            ran: Cell::new(false),
        }
    }
    fn failing(spool_dir: &Path) -> Self {
        Self {
            spool_dir: spool_dir.to_path_buf(),
            returns: Err(()),
            pending_at_publish: Cell::new(usize::MAX),
            ran: Cell::new(false),
        }
    }
}

impl Publisher for TestPublisher {
    async fn publish(&self, _cx: &Cx, _finding: &Finding) -> Result<String, FindingError> {
        self.ran.set(true);
        let rows = pending(&self.spool_dir).expect("the spool dir must be readable at publish");
        self.pending_at_publish.set(rows.len());
        match &self.returns {
            Ok(id) => Ok(id.clone()),
            Err(()) => Err(FindingError::PublishFailed("test publisher refused".into())),
        }
    }
}

/// Run one async body on a real asupersync runtime with a live `Cx`.
fn with_cx<T>(body: impl AsyncFnOnce(&Cx) -> T) -> T {
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .expect("asupersync runtime");
    runtime.block_on(async {
        let cx = Cx::current().expect("runtime Cx");
        body(&cx).await
    })
}

// ---------------------------------------------------------------------------------------
// FC-L1 — a gap observed is a gap filed
// ---------------------------------------------------------------------------------------

#[test]
fn l1_the_only_disposals_are_file_and_waive_and_a_reasonless_waiver_is_refused() {
    let err = complete_finding()
        .waive("   ")
        .expect_err("a reasonless waiver is a silent drop wearing a decision");
    assert_eq!(err, FindingError::WaiverNeedsReason);

    let waived = complete_finding()
        .waive("superseded by omp-orchestrator-exit-1-overloaded-x1o; same class, already owned")
        .expect("a reasoned waiver is the sanctioned non-filing");
    assert!(waived.reason().contains("superseded"));
    assert!(
        waived.body().contains("WHAT:"),
        "the body must survive the waiver, or a waiver erases the evidence it decided about"
    );

    // The API surface IS the law here: exactly two methods take `self` by value.
    let src = source();
    let consuming: Vec<&str> = ["pub fn waive(", "pub async fn file("]
        .into_iter()
        .filter(|needle| src.contains(needle))
        .collect();
    assert_eq!(
        consuming.len(),
        2,
        "both disposals must exist; found {consuming:?}"
    );
    // Negative control: a third consuming exit must not appear.
    for forbidden in ["pub fn discard(", "pub fn ignore(", "pub fn drop_it("] {
        assert!(
            !src.contains(forbidden),
            "a third disposal path appeared: {forbidden}"
        );
    }
}

/// The pin this leg used to carry, and why it is gone.
///
/// It was named `l1_must_use_is_only_a_warning_and_does_not_survive_option` and it
/// asserted `denies == 0` — **a PINNED DEFECT whose failure was the signal that the fix
/// landed.** It fired on `omp-orchestrator-finding-l1-bypassable-py3` with
/// `unused_must_use is now denied in 1 manifest(s)`, and `docs/contracts/finding_contract.md`
/// §FC-L1 was updated in the same commit as required.
///
/// It is replaced rather than deleted: the same two facts are still asserted, in the
/// direction that now holds. A pin left in place after its defect is fixed becomes a
/// test that fails forever and gets `#[ignore]`d, which is how a suite goes vacuously
/// green.
#[test]
fn l1_unused_must_use_is_denied_and_the_producer_does_not_return_option() {
    let src = source();
    assert!(
        src.contains("#[must_use"),
        "the type must carry #[must_use]; without it FC-L1 has no enforcement at all"
    );

    let mut manifests = vec![repo_root().join("Cargo.toml")];
    let crates_dir = repo_root().join("crates");
    if let Ok(entries) = std::fs::read_dir(&crates_dir) {
        for e in entries.flatten() {
            let m = e.path().join("Cargo.toml");
            if m.is_file() {
                manifests.push(m);
            }
        }
    }
    // ANTI-VACUITY, unchanged: an empty or tiny scan set is an ERROR, not a pass.
    assert!(
        manifests.len() > 40,
        "manifest scan set is {} — an empty or tiny scan set is an ERROR, not a pass",
        manifests.len()
    );
    let mut denies = 0usize;
    let mut forbids_unsafe = 0usize;
    for m in &manifests {
        let text = std::fs::read_to_string(m).unwrap_or_default();
        if text.contains(r#"unused_must_use = "deny""#) {
            denies += 1;
        }
        if text.contains(r#"unsafe_code = "forbid""#) {
            forbids_unsafe += 1;
        }
    }
    // POSITIVE CONTROL, retained: if the reader cannot find forbid(unsafe_code) it
    // cannot be trusted about unused_must_use either.
    assert!(
        forbids_unsafe >= 40,
        "POSITIVE CONTROL FAILED: the reader found {forbids_unsafe} forbid(unsafe_code) \
         manifests, so its count for unused_must_use proves nothing"
    );
    assert!(
        denies >= 1,
        "unused_must_use is denied in {denies} manifest(s): FC-L1 is back to a warning a \
         build can ignore"
    );

    // AND THE CRATE WHERE DROPPING HAPPENS MUST INHERIT IT. MEASURED: reverting the
    // producer's manifest from `[lints] workspace = true` back to its own
    // `[lints.rust] unsafe_code = "forbid"` left BOTH packages green -- the deny still
    // existed at the root and applied to nobody who matters. A lint denied in a manifest
    // no code inherits is the BUILT-vs-WIRED defect at the manifest layer.
    for crate_name in ["finding", "finding-dispatch"] {
        let manifest = std::fs::read_to_string(
            repo_root().join("crates").join(crate_name).join("Cargo.toml"),
        )
        .unwrap_or_default();
        assert!(
            manifest.contains("[lints]") && manifest.contains("workspace = true"),
            "crates/{crate_name} does not inherit workspace lints, so unused_must_use is \
             not denied there"
        );
        // A local `[lints.rust]` table REPLACES the inherited set rather than adding to
        // it, so its presence silently opts the crate out.
        assert!(
            !manifest.contains("[lints.rust]"),
            "crates/{crate_name} declares its own [lints.rust] table, which REPLACES the \
             workspace set and drops the deny"
        );
    }

    // THE OPTION HOLE. `#[must_use]` does not propagate through `Option`, MEASURED with
    // rustc 1.100.0-nightly on a two-line probe:
    //
    //   direct();          -> warning: unused `Finding` that must be used
    //   returns_option();  -> NO WARNING
    //   returns_enum();    -> warning: unused `MaybeFinding` that must be used
    //
    // So the producer's signature is load-bearing, and asserting the lint alone would
    // leave the bypass in place.
    let producer = std::fs::read_to_string(repo_root().join("crates/finding-dispatch/src/lib.rs"))
        .expect("the producer must exist; it is the only function that builds a Finding");
    assert!(
        producer.contains("pub fn finding_for(decision: &SupervisorDecision, recurrence_count: u32) -> MaybeFinding"),
        "the producer must return a #[must_use] type, not Option<Finding>"
    );
    // SCOPED TO THE PRODUCER'S OWN LINE, not the whole file. The first version asserted
    // the string `-> Option<Finding>` was absent anywhere, and it failed on
    // `MaybeFinding::into_owed(self) -> Option<Finding>` -- an accessor that SHOULD
    // return an Option. Tenth instance today of a checker matching something legitimate
    // that merely mentions the pattern; the cure is the same every time, which is to key
    // on structure (this line) rather than on the file's text.
    let producer_line = producer
        .lines()
        .find(|line| line.contains("pub fn finding_for"))
        .expect("the producer's signature line");
    assert!(
        !producer_line.contains("Option<"),
        "the producer returns an Option wrapper, which reopens the bypass -- must_use \
         does not survive it: {producer_line}"
    );
    // And the outer type must actually carry the attribute, with a reason a caller reads.
    //
    // KEYED ON A LINE THAT STARTS WITH THE ATTRIBUTE, not on the first occurrence of the
    // text. `find("#[must_use")` matched the DOC COMMENT above the attribute -- prose
    // that names the attribute while explaining it. Eleventh instance today of a checker
    // reading text ABOUT the thing instead of the thing, and the third in this one file.
    let attr = producer
        .lines()
        .find(|line| line.trim_start().starts_with("#[must_use"))
        .expect("MaybeFinding must carry a #[must_use] ATTRIBUTE, not a mention of one");
    assert!(
        attr.contains("filed or waived"),
        "the #[must_use] reason must name the obligation, or a caller sees only \
         'unused value': {attr}"
    );
}

// ---------------------------------------------------------------------------------------
// FC-L2 — Filed must name the bead id  (PINNED DEFECT)
// ---------------------------------------------------------------------------------------

#[test]
fn l2_filed_carries_whatever_the_publisher_returned_including_an_empty_id() {
    // PINNED DEFECT, bead omp-orchestrator-finding-l2-unvalidated-id-3j8.
    // `Filed { id }` is populated straight from `publisher.publish()` with no validation,
    // so a publisher that returns "" produces a `Filed` claiming a bead that does not
    // exist. FC-L2 says such a Finding must be unconstructible; today it is routine.
    let dir = spool_dir("l2-empty-id");
    let filed = with_cx(async |cx| {
        let publisher = TestPublisher::ok(&dir, "");
        complete_finding()
            .file(cx, &dir, &publisher)
            .await
            .expect("filing succeeds even with an empty id — that is the defect")
    });
    assert!(
        filed.id().is_empty(),
        "FC-L2 is now ENFORCED: an empty bead id was refused. Update the contract's FC-L2 \
         row and close the bead in the same commit."
    );
    assert!(
        filed.body().contains("ACCEPTANCE:"),
        "the filed body must still carry the standard shape"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn l2_a_real_bead_shaped_id_round_trips_into_filed_and_retires_the_spool_row() {
    // The KNOWN-GOOD arm. Without it this suite could be over-strict about ids and still
    // look green, and an over-strict gate gets routed around.
    let dir = spool_dir("l2-good-id");
    let filed = with_cx(async |cx| {
        let publisher = TestPublisher::ok(&dir, "omp-orchestrator-fixture-idshape-abc");
        complete_finding()
            .file(cx, &dir, &publisher)
            .await
            .expect("filing must succeed")
    });
    assert_eq!(filed.id(), "omp-orchestrator-fixture-idshape-abc");
    assert!(
        pending(&dir).expect("sweep").is_empty(),
        "a published row must leave the pending sweep, or the recovery lane re-files it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------
// FC-L3 — the spool row survives the process that saw it
// ---------------------------------------------------------------------------------------

#[test]
fn l3_the_durable_row_is_on_disk_before_the_publisher_runs() {
    // ORDERING PROVED, NOT ASSERTED. The publisher reads the spool directory at the moment
    // it is invoked; observing a pending row there is the only evidence that the durable
    // write preceded the cancellable effect.
    let dir = spool_dir("l3-ordering");
    let observed = with_cx(async |cx| {
        let publisher = TestPublisher::ok(&dir, "omp-orchestrator-fixture-ordering-xyz");
        let filed = complete_finding()
            .file(cx, &dir, &publisher)
            .await
            .expect("filing must succeed");
        assert!(publisher.ran.get(), "the publisher must actually have run");
        assert!(!filed.id().is_empty());
        publisher.pending_at_publish.get()
    });
    assert_eq!(
        observed, 1,
        "the publisher saw {observed} pending row(s); FC-L3 requires exactly 1 — the durable \
         record must exist BEFORE publish, or a cancellation loses the finding"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn l3_a_publisher_failure_leaves_the_row_recoverable_not_lost() {
    let dir = spool_dir("l3-deferred");
    let err = with_cx(async |cx| {
        let publisher = TestPublisher::failing(&dir);
        complete_finding()
            .file(cx, &dir, &publisher)
            .await
            .expect_err("a refusing publisher must not report success")
    });
    assert!(
        matches!(err, FindingError::PublishFailed(_)),
        "expected PublishFailed, got {err:?}"
    );
    let rows = pending(&dir).expect("sweep");
    assert_eq!(
        rows.len(),
        1,
        "a failed publish must leave exactly one recoverable row: a failure is a DEFERRED \
         finding, never a lost one — the same rule as 'a timeout is not a verdict'"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn l3_respooling_the_same_finding_does_not_duplicate_the_row() {
    // Recovery must be idempotent, or a retry after deferral files the bead twice.
    let dir = spool_dir("l3-idem");
    let first = complete_finding().spool(&dir).expect("first spool");
    let second = complete_finding().spool(&dir).expect("second spool");
    assert_eq!(
        first.path(),
        second.path(),
        "the row name must be content-derived"
    );
    assert_eq!(pending(&dir).expect("sweep").len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn l3_an_unreadable_spool_dir_is_an_error_not_an_empty_sweep() {
    // ANTI-VACUITY. "I could not look" and "there is nothing there" are opposite
    // conditions, and an empty sweep over an absent directory reports identically to a
    // clean one.
    let absent = std::env::temp_dir().join(format!(
        "finding-contract-absent-{}-xyzzy",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&absent);
    assert!(
        matches!(pending(&absent), Err(FindingError::SpoolUnwritable(_))),
        "an absent spool dir must ERROR, never report zero pending"
    );
    // Positive control on the same reader: a real directory sweeps successfully.
    let dir = spool_dir("l3-antivacuity-control");
    let _ = complete_finding().spool(&dir).expect("spool");
    assert_eq!(
        pending(&dir).expect("the control sweep must succeed").len(),
        1,
        "POSITIVE CONTROL FAILED: the sweep cannot see a row it just wrote, so its error \
         above proves nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------
// FC-L4 — a finding reporting something missing must name its replacement  (PINNED DEFECT)
// ---------------------------------------------------------------------------------------

#[test]
fn l4_a_finding_cannot_name_a_replacement_today() {
    // PINNED DEFECT, bead omp-orchestrator-finding-l4-no-replacement-jz2.
    // fh N040: a replacement claim needs a smoke check at BOTH ends — the new thing works
    // AND the old caller is gone. A finding that reports an absence without naming what
    // should be used instead is not actionable, and `Finding` has no field for it.
    let src = source();
    for absent in ["replacement", "instead_use", "supersedes"] {
        assert!(
            !src.contains(absent),
            "FC-L4 may now be enforced: `{absent}` appears in the source. Update the \
             contract's FC-L4 row and close the bead in the same commit."
        );
    }
    // Positive control on the same reader: the fields that DO exist are found.
    for present in ["acceptance", "labels", "priority"] {
        assert!(
            src.contains(present),
            "POSITIVE CONTROL FAILED: the reader cannot see `{present}`, so its absences \
             above prove nothing"
        );
    }
}

// ---------------------------------------------------------------------------------------
// FC-L5 — waived is not closed  (PINNED DEFECT)
// ---------------------------------------------------------------------------------------

#[test]
fn l5_a_waiver_carries_no_expiry_so_it_is_permanent_today() {
    // PINNED DEFECT, bead omp-orchestrator-finding-l5-waiver-no-expiry-jex.
    // A waiver with a reason and no expiry is a silent permanent exemption: the reason is
    // never revisited, which is the failure `UNWIRED_LANE_ALLOWANCE` already recorded —
    // "an allowance that outlives its reason is worse than no allowance, because it reads
    // as a considered exception when it is only an un-revisited one."
    let src = source();
    for absent in ["expiry", "expires", "revisit_by", "until"] {
        assert!(
            !src.contains(absent),
            "FC-L5 may now be enforced: `{absent}` appears in the source. Update the \
             contract's FC-L5 row and close the bead in the same commit."
        );
    }
    let waived = complete_finding()
        .waive("deferred to the exit-code registry work; same owner")
        .expect("reasoned waiver");
    assert!(
        !waived.reason().is_empty(),
        "the reason is the only thing a waiver carries today"
    );
    for present in ["WaiverNeedsReason", "pub fn reason"] {
        assert!(
            src.contains(present),
            "POSITIVE CONTROL FAILED: the reader cannot see `{present}`"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Construction: the bead standard as a type precondition
// ---------------------------------------------------------------------------------------

#[test]
fn a_planted_incomplete_finding_is_refused_field_by_field() {
    // FIRES-ON-KNOWN-BAD, one arm per required field. A P0 sat at the head of the ready
    // queue on 2026-08-31 with no ACCEPTANCE section; two agents triaged it and went idle,
    // because a bead you cannot write run-X-expect-Y for can only be adjudicated.
    for (what, why, acceptance, labels, missing) in [
        ("", "w", "a", vec!["gate".to_owned()], "WHAT"),
        ("x", "", "a", vec!["gate".to_owned()], "WHY"),
        ("x", "w", "", vec!["gate".to_owned()], "ACCEPTANCE"),
        ("x", "w", "a", vec!["   ".to_owned()], "LABELS"),
        ("x", "w", "a", Vec::new(), "LABELS"),
    ] {
        let err = Finding::new(what, why, acceptance, labels, 1)
            .expect_err("an incomplete finding must be unconstructible");
        assert_eq!(err, FindingError::MissingField(missing), "{missing} arm");
    }
}

#[test]
fn a_complete_finding_is_accepted_and_its_body_carries_the_standard_shape() {
    // KNOWN-GOOD leg.
    let body = complete_finding().body();
    for key in ["WHAT:", "WHY:", "ACCEPTANCE:"] {
        assert!(body.contains(key), "body must carry {key}: {body}");
    }
    assert_eq!(complete_finding().labels().len(), 2);
    assert_eq!(complete_finding().priority(), 1);
}
