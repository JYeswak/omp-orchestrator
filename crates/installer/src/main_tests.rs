use super::*;

const DIGEST: &str = "bdd2a7291457c6a5e371324772061f751ba7774d8d7057895f5d5ea8daa773f1";
const PAYLOAD: &[u8] = b"B03 fixed buffer SHA-256 payload\n";

/// Fixture attempt identity: explicit pane/incarnation/attempt threading
/// through every migrated emit call. Deterministic values so journal-row
/// asserts stay exact.
fn fixture_identity() -> installer::AttemptIdentity {
    installer::AttemptIdentity {
        pane: "fixture-pane".to_owned(),
        incarnation: "fixture-incarnation".to_owned(),
        attempt: "fixture-attempt".to_owned(),
    }
}

/// Emit-time manifest fixture: FULL state attested with no digest value
/// yet (verification happens downstream), mirroring production callers.
fn full_manifest() -> installer::InputManifest {
    installer::InputManifest::Full {
        digest: String::new(),
    }
}

fn install_args(digest: &str, bin_dir: &str, equals: bool) -> Vec<String> {
    let mut args = vec![
        "--install".to_owned(),
        "installer".to_owned(),
        "--bin-dir".to_owned(),
        bin_dir.to_owned(),
    ];
    if equals {
        args.push(format!("--sha256={digest}"));
    } else {
        args.extend(["--sha256".to_owned(), digest.to_owned()]);
    }
    args
}

fn control_args(flag: &str, digest: &str, equals: bool) -> Vec<String> {
    let mut args = vec![flag.to_owned()];
    if equals {
        args.push(format!("--sha256={digest}"));
    } else {
        args.extend(["--sha256".to_owned(), digest.to_owned()]);
    }
    args
}

#[test]
fn parser_preserves_dispatch_and_digest_precedence() {
    let value = parse_cli_args(install_args(DIGEST, "scratch-home", false)).expect("value form");
    let equals = parse_cli_args(install_args(DIGEST, "scratch-home", true)).expect("equals form");
    assert_eq!(value.positional, vec!["--install".to_owned(), "installer".to_owned()]);
    assert_eq!(value.bin_dir, PathBuf::from("scratch-home"));
    assert_eq!(value.expected_sha256.as_deref(), Some(DIGEST));
    assert_eq!(equals.expected_sha256, value.expected_sha256);

    let mut duplicate = install_args("bad", "scratch-home", false);
    duplicate.push(format!("--sha256={DIGEST}"));
    assert_eq!(parse_cli_args(duplicate).expect("duplicate flags").expected_sha256.as_deref(), Some(DIGEST));

    for (flag, equals_form) in [("--check", true), ("--version", false)] {
        let parsed = parse_cli_args(control_args(flag, DIGEST, equals_form)).expect("control flag");
        assert_eq!(parsed.positional, vec![flag]);
        assert_eq!(parsed.expected_sha256.as_deref(), Some(DIGEST));
    }
}

#[test]
fn parsed_digest_reaches_production_verification_action() {
    let source = std::env::temp_dir().join(format!("omp-installer-b03-cli-{}", std::process::id()));
    std::fs::write(&source, PAYLOAD).expect("write artifact");
    let parsed = parse_cli_args(install_args(DIGEST, source.to_str().expect("UTF-8 path"), false))
        .expect("production arguments");
    let mut action_called = false;
    installer::verify_sha256_before_install(&source, parsed.expected_sha256.as_deref(), || {
        action_called = true;
        Ok::<(), installer::InstallError>(())
    })
    .expect("parsed digest reaches action");
    assert!(action_called, "matching parsed digest did not reach action");
    std::fs::remove_file(source).expect("cleanup artifact");
}

#[test]
fn pane_and_incarnation_flags_thread_explicitly() {
    // Space form lands verbatim; nothing is derived from environment.
    let parsed = parse_cli_args(vec![
        "--check".to_owned(),
        "--pane".to_owned(),
        "pane-49".to_owned(),
        "--incarnation".to_owned(),
        "incarnation-3".to_owned(),
    ])
    .expect("pane flags parse");
    assert_eq!(parsed.pane, "pane-49", "pane threads explicitly");
    assert_eq!(
        parsed.incarnation, "incarnation-3",
        "incarnation threads explicitly"
    );
    // Equals form agrees, and the last occurrence wins like --bin-dir.
    let parsed = parse_cli_args(vec![
        "--check".to_owned(),
        "--pane=first".to_owned(),
        "--pane=second".to_owned(),
        "--incarnation=first".to_owned(),
    ])
    .expect("equals-form pane flags parse");
    assert_eq!(parsed.pane, "second", "last pane wins");
    assert_eq!(parsed.incarnation, "first", "incarnation parses");
    // Absent flags default to empty (unknown, never fabricated).
    let bare =
        parse_cli_args(vec!["--check".to_owned()]).expect("bare check parses");
    assert!(
        bare.pane.is_empty() && bare.incarnation.is_empty(),
        "absent identity stays empty, never invented"
    );
}

// ── gj669: lifecycle-event write failure propagates before success ──
//
// KNOWN-BAD for this file: restore log-and-continue — make `emit_s1` swallow
// its `Err` (return `Ok` unconditionally) and the `emit_*_failure_is_typed`
// legs redden; make `guard_success` return `SUCCESS` on `Err` and
// `success_guard_refuses_on_emit_failure` reddens. Every leg pins message
// AND exit: `cargo` returns 101 for unrelated causes, so neither alone tells
// which band moved.
//
// The journal file is the ANTI-VACUITY half here: "empty manifest" is an
// empty journal (or an empty emit set), which must refuse -- never read as
// an emitted event. The TYPED half (Partial/Refused cannot emit) lives in
// the lib boundary and its legs in tests/l0_install.rs.

fn gj669_repo(name: &str) -> PathBuf {
    let repo =
        std::env::temp_dir().join(format!("omp-installer-gj669-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).expect("fixture repo");
    repo
}

fn gj669_journal(repo: &PathBuf) -> PathBuf {
    repo.join(".omp-orchestrator/work/s1/lifecycle.jsonl")
}

fn gj669_read_rows(journal: &PathBuf) -> Vec<String> {
    match std::fs::read_to_string(journal) {
        Ok(text) => text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// A repo whose journal parent is a FILE: `create_dir_all` fails ENOTDIR for
/// every uid. A chmod-unreadable fixture would be root-blind on workers; a
/// file component in the path is not.
fn gj669_blocked_repo(name: &str) -> PathBuf {
    let repo = gj669_repo(name);
    std::fs::write(repo.join(".omp-orchestrator"), b"not a directory").expect("blocker file");
    repo
}

fn gj669_cleanup(repo: &PathBuf) {
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn emit_s1_known_good_roundtrips_with_readback() {
    let repo = gj669_repo("known-good");
    // Fixture reason is a real allowlisted S1.L0 reason (the allowlist
    // governs membership; this leg owns plumbing, not vocabulary).
    let readback = installer::emit_s1(&repo, Layer::L0, "S1.L0", EmitOutcome::Emitted, "INSTALL_VERIFIED", &fixture_identity(), &full_manifest(), &[])
    .expect("healthy emit answers with readback");
    assert_eq!(
        readback.lines, 1,
        "one emit appends exactly one row, never zero-or-many"
    );
    let rows = gj669_read_rows(&gj669_journal(&repo));
    assert_eq!(
        rows.len(),
        1,
        "the journal holds exactly the emitted row: {rows:?}"
    );
    assert!(
        rows[0].contains("INSTALL_VERIFIED") && rows[0].contains("\"emitted\""),
        "the row carries reason and outcome: {}",
        rows[0]
    );
    gj669_cleanup(&repo);
}

#[test]
fn emit_s1_write_failure_is_typed() {
    let repo = gj669_blocked_repo("write-fail");
    // Real allowlisted reason: the failure under test comes from the
    // blocked journal path, never from the reason string.
    let error = installer::emit_s1(&repo, Layer::L0, "S1.L0", EmitOutcome::Emitted, "INSTALL_VERIFIED", &fixture_identity(), &full_manifest(), &[])
    .expect_err("a file-blocked journal parent must refuse");
    assert!(
        matches!(&error, lifecycle_event::EmitError::Io { op, .. } if *op == "create_dir_all"),
        "write failure must name its op, got: {error}"
    );
    assert!(
        gj669_read_rows(&gj669_journal(&repo)).is_empty(),
        "a refused write persists nothing"
    );
    gj669_cleanup(&repo);
}

#[test]
fn emit_readback_failure_is_typed() {
    // The crate's designated injector: pretend the write returned success,
    // then prove readback refuses on the empty journal. A true end-to-end
    // readback failure is unconstructible in-process (append+fsync+readback
    // on one file cannot disagree with itself); production `emit_one_host`
    // cannot skip readback, pinned by lifecycle-event's own
    // `refuses_when_readback_missing` leg. Measured here instead: `/dev/null`
    // refuses at `fsync_file` with EINVAL on workers, so it injects a write
    // failure, never a readback one.
    let repo = gj669_repo("readback-fail");
    let journal =
        lifecycle_event::DurableJournal::open(gj669_journal(&repo)).expect("journal opens");
    let code = lifecycle_event::ReasonCode::new("GJ669_READBACK_FAIL").expect("reason");
    let event = lifecycle_event::LifecycleEvent::new(
        Layer::L0,
        "HUMAN",
        "S1.L0",
        "installer",
        EmitOutcome::Refused,
        code,
    );
    let error = lifecycle_event::readback_after_claimed_write(&journal, &[event])
        .expect_err("a write that reads back empty must refuse");
    assert!(
        matches!(&error, lifecycle_event::EmitError::ReadbackFailed { .. }),
        "readback failure must keep its variant, got: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("LIFECYCLE_EVENT_READBACK_FAILED"),
        "readback failure must carry its exact reason, got: {error}"
    );
    assert_eq!(
        installer::guard_success(Err(error), "GJ669 MUST NOT PRINT"),
        ExitCode::from(1),
        "this readback refusal must withhold success with exit 1"
    );
    gj669_cleanup(&repo);
}

#[test]
fn success_guard_refuses_on_emit_failure() {
    let injected = lifecycle_event::EmitError::Io {
        op: "gj669-fixture",
        detail: "injected".to_owned(),
    };
    assert_eq!(
        installer::guard_success(Err(injected), "GJ669 MUST NOT PRINT"),
        ExitCode::from(1),
        "a refused emit must refuse success with exit 1, never SUCCESS"
    );
    let durable = lifecycle_event::Readback {
        path: PathBuf::from("gj669-fixture"),
        lines: 1,
    };
    assert_eq!(
        installer::guard_success(Ok(durable), "GJ669 OK"),
        ExitCode::SUCCESS,
        "a durable event advances"
    );
}

#[test]
fn check_git_refusal_emits_without_converting_failure() {
    // A tempdir is never a git repo, so `git_head` refuses deterministically;
    // the journal itself is healthy, so the refusal event must land while the
    // original exit 3 survives.
    let repo = gj669_repo("check-refusal");
    let bin = gj669_repo("check-refusal-bin");
    assert_eq!(
        run_check(&repo, &bin, &fixture_identity()),
        ExitCode::from(3),
        "the non-repo check keeps its original refusal"
    );
    let rows = gj669_read_rows(&gj669_journal(&repo));
    assert_eq!(
        rows.len(),
        1,
        "the restrictive path emits exactly one refusal row: {rows:?}"
    );
    assert!(
        rows[0].contains("CHECK_GIT_HEAD_REFUSED") && rows[0].contains("\"refused\""),
        "the row names the refusal and its outcome: {}",
        rows[0]
    );
    gj669_cleanup(&repo);
    gj669_cleanup(&bin);
}

#[test]
fn check_refusal_emit_failure_preserves_exit() {
    // Both the check AND the refusal emit fail here: the original 3 must
    // survive rather than converting to the emit's failure or to success.
    let repo = gj669_blocked_repo("check-refusal-blocked");
    let bin = gj669_repo("check-refusal-blocked-bin");
    assert_eq!(
        run_check(&repo, &bin, &fixture_identity()),
        ExitCode::from(3),
        "a refused emit must not convert the original refusal"
    );
    assert!(
        gj669_read_rows(&gj669_journal(&repo)).is_empty(),
        "nothing was persisted"
    );
    gj669_cleanup(&repo);
    gj669_cleanup(&bin);
}

#[test]
fn install_unknown_target_emits_refusal_with_exit_2() {
    // No `.build_in_flight` marker exists in a fresh fixture, so the fence
    // passes deterministically and the unknown target refuses with exit 2.
    let repo = gj669_repo("install-refusal");
    let bin = gj669_repo("install-refusal-bin");
    assert_eq!(
        run_install(&repo, &bin, "definitely-not-a-target", None, &fixture_identity()),
        ExitCode::from(2),
        "the unknown target keeps its original refusal"
    );
    let rows = gj669_read_rows(&gj669_journal(&repo));
    assert_eq!(
        rows.len(),
        1,
        "the restrictive path emits exactly one refusal row: {rows:?}"
    );
    assert!(
        rows[0].contains("INSTALL_UNKNOWN_TARGET") && rows[0].contains("\"refused\""),
        "the row names the refusal and its outcome: {}",
        rows[0]
    );
    gj669_cleanup(&repo);
    gj669_cleanup(&bin);
}

#[test]
fn empty_emit_batch_is_refused() {
    // Anti-vacuity: an empty emit set is an ERROR, never a pass. Without
    // this, a caller emitting nothing would read as durable.
    let repo = gj669_repo("empty-batch");
    let journal =
        lifecycle_event::DurableJournal::open(gj669_journal(&repo)).expect("journal opens");
    let error = lifecycle_event::emit_host(&journal, &[]).expect_err("empty batch must refuse");
    assert!(
        matches!(error, lifecycle_event::EmitError::EmptyBatch),
        "an empty emit set must refuse as EmptyBatch, got: {error}"
    );
    gj669_cleanup(&repo);
}

#[test]
fn empty_journal_certifies_nothing() {
    // Anti-vacuity: a missing journal reads as zero rows, and zero rows must
    // never match an emitted event. Absence is not evidence.
    let repo = gj669_repo("empty-journal");
    let rows = gj669_read_rows(&gj669_journal(&repo));
    assert!(rows.is_empty(), "a fresh fixture journal starts empty");
    assert!(
        !rows.iter().any(|line| line.contains("\"emitted\"")),
        "an empty journal must never read as an emitted event"
    );
    gj669_cleanup(&repo);
}

#[test]
fn delta_flag_routes_as_a_single_operator_action() {
    let parsed = parse_cli_args(vec!["--delta".to_owned()]).expect("delta parses");
    assert_eq!(parsed.positional, vec!["--delta"]);
}

#[test]
fn delta_command_refuses_without_the_production_writer() {
    let repo = gj669_repo("qod0-delta-missing-writer");
    assert_eq!(
        run_delta(&repo),
        ExitCode::from(4),
        "the reachable --delta consumer must read the canonical report"
    );
    gj669_cleanup(&repo);
}
